//! GPU-accelerated batch simulation engine for MCTS
//!
//! This module provides GPU-based move application and board evaluation,
//! allowing multiple simulations to be processed in parallel on the GPU.

use super::gpu_context::GpuContext;
use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;
use wgpu::util::DeviceExt;

const BOARD_SIZE: usize = 81;
const MAX_BATCH_SIZE: usize = 1024;

/// Board state for GPU
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GpuBoardState {
    pub squares: [u32; BOARD_SIZE],
    pub white_to_move: u32,
    _padding: [u32; 3], // Padding for alignment
}

unsafe impl Pod for GpuBoardState {}
unsafe impl Zeroable for GpuBoardState {}

/// Move application structure for GPU
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GpuMoveApplication {
    pub board: GpuBoardState,
    pub move_encoding: u32,
    pub result_score: i32,
    pub valid: u32,
    _padding: [u32; 3], // Padding for alignment
}

unsafe impl Pod for GpuMoveApplication {}
unsafe impl Zeroable for GpuMoveApplication {}

/// Result of a batch simulation
#[derive(Clone, Debug)]
pub struct BatchSimulationResult {
    pub score: i32,
    pub valid: bool,
    pub board: [u8; 82],
}

/// GPU-accelerated batch simulation engine
pub struct BatchSimulationEngine {
    gpu_context: GpuContext,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl BatchSimulationEngine {
    /// Create a new batch simulation engine
    pub async fn new() -> Result<Self, String> {
        // Use shared GPU context
        let gpu_context = super::get_shared_context()?;

        // Load shader
        let shader_source = include_str!("shaders/batch_simulation.wgsl");
        let shader = gpu_context
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Batch Simulation Shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_source)),
            });

        // Create bind group layout
        let bind_group_layout =
            gpu_context
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Batch Simulation Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        // Create pipeline layout
        let pipeline_layout =
            gpu_context
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Batch Simulation Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        // Create compute pipeline
        let pipeline =
            gpu_context
                .device()
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Batch Simulation Pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });

        Ok(Self {
            gpu_context,
            pipeline,
            bind_group_layout,
        })
    }

    /// Convert board binary to GPU format
    fn board_to_gpu(&self, board_binary: &[u8; 82]) -> GpuBoardState {
        let mut gpu_board = GpuBoardState {
            squares: [0; BOARD_SIZE],
            white_to_move: board_binary[81] as u32,
            _padding: [0; 3],
        };

        for i in 0..BOARD_SIZE {
            gpu_board.squares[i] = board_binary[i] as u32;
        }

        gpu_board
    }

    /// Convert GPU board back to binary format
    fn gpu_to_board(&self, gpu_board: &GpuBoardState) -> [u8; 82] {
        let mut board = [0u8; 82];
        for i in 0..BOARD_SIZE {
            board[i] = gpu_board.squares[i] as u8;
        }
        board[81] = gpu_board.white_to_move as u8;
        board
    }

    /// Process a batch of move applications and evaluations on GPU
    pub fn process_batch(
        &self,
        boards: &[[u8; 82]],
        moves: &[u16],
    ) -> Result<Vec<BatchSimulationResult>, String> {
        if boards.len() != moves.len() {
            return Err("boards and moves must have the same length".to_string());
        }

        if boards.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = boards.len().min(MAX_BATCH_SIZE);

        // Prepare input data
        let mut applications: Vec<GpuMoveApplication> = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let gpu_board = self.board_to_gpu(&boards[i]);
            applications.push(GpuMoveApplication {
                board: gpu_board,
                move_encoding: moves[i] as u32,
                result_score: 0,
                valid: 0,
                _padding: [0; 3],
            });
        }

        // Create buffer
        let buffer =
            self.gpu_context
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Move Application Buffer"),
                    contents: bytemuck::cast_slice(&applications),
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                });

        // Create staging buffer for reading back results
        let staging_buffer = self
            .gpu_context
            .device()
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("Staging Buffer"),
                size: (std::mem::size_of::<GpuMoveApplication>() * batch_size) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

        // Create bind group
        let bind_group = self
            .gpu_context
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Batch Simulation Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

        // Create command encoder
        let mut encoder =
            self.gpu_context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Batch Simulation Encoder"),
                });

        // Dispatch compute shader
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Batch Simulation Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Calculate workgroups needed (workgroup size is 64)
            let workgroups = ((batch_size + 63) / 64) as u32;
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Copy results to staging buffer
        encoder.copy_buffer_to_buffer(
            &buffer,
            0,
            &staging_buffer,
            0,
            (std::mem::size_of::<GpuMoveApplication>() * batch_size) as u64,
        );

        // Submit commands
        self.gpu_context.queue().submit(Some(encoder.finish()));

        // Read back results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        self.gpu_context.device().poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|e| format!("Failed to receive buffer mapping result: {}", e))?
            .map_err(|e| format!("Failed to map buffer: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();
        let result_applications: &[GpuMoveApplication] = bytemuck::cast_slice(&data);

        let mut results = Vec::with_capacity(batch_size);
        for app in result_applications.iter().take(batch_size) {
            results.push(BatchSimulationResult {
                score: app.result_score,
                valid: app.valid != 0,
                board: self.gpu_to_board(&app.board),
            });
        }

        drop(data);
        staging_buffer.unmap();

        Ok(results)
    }

    /// Create a synchronized instance (blocking)
    pub fn new_sync() -> Result<Self, String> {
        pollster::block_on(Self::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_engine_creation() {
        let engine = BatchSimulationEngine::new_sync();
        if let Err(e) = &engine {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        assert!(engine.is_ok());
    }

    #[test]
    fn test_batch_processing() {
        let engine = BatchSimulationEngine::new_sync();
        if let Err(e) = &engine {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        let engine = engine.unwrap();

        // Create a simple test board with a white soldier
        let mut board = [0u8; 82];
        board[40] = 0b1000001; // White Soldier at center
        board[81] = 1; // White to move

        // Test with empty batch
        let result = engine.process_batch(&[], &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);

        // Test with a simple move (just moving forward, simplified encoding)
        let boards = vec![board];
        let moves = vec![0x0000]; // Dummy move for testing
        let result = engine.process_batch(&boards, &moves);
        assert!(result.is_ok());
    }
}
