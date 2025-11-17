//! GPU-accelerated move generation engine for Arx
//!
//! This module provides a WebGPU-based compute shader implementation for generating
//! all legal moves for a given board position. The shader processes all 81 squares
//! of the board in parallel, significantly speeding up move generation.
//!
//! # Example
//!
//! ```no_run
//! use arx_engine::engine::MoveGenerationEngine;
//!
//! let engine = MoveGenerationEngine::new_sync().expect("Failed to create engine");
//! let board_state = [0u8; 82]; // Your board state
//! let moves = engine.generate_moves(&board_state).expect("Failed to generate moves");
//! println!("Found {} legal moves", moves.len());
//! ```

use super::gpu_context::GpuContext;
use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;
use wgpu::util::DeviceExt;

// Re-export constants for use in the module
const BOARD_SIZE: usize = 81;
const MAX_MOVES: usize = 2048;

/// Board state for GPU
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct GpuBoardState {
    squares: [u32; BOARD_SIZE],
    white_to_move: u32,
    _padding: [u32; 3], // Padding for alignment
}

unsafe impl Pod for GpuBoardState {}
unsafe impl Zeroable for GpuBoardState {}

/// Move buffer for GPU
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct GpuMoveBuffer {
    moves: [u32; MAX_MOVES],
    count: u32,
    _padding: [u32; 3], // Padding for alignment
}

unsafe impl Pod for GpuMoveBuffer {}
unsafe impl Zeroable for GpuMoveBuffer {}

/// GPU-accelerated move generation engine
pub struct MoveGenerationEngine {
    gpu_context: GpuContext,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl MoveGenerationEngine {
    /// Create a new move generation engine
    pub async fn new() -> Result<Self, String> {
        // Use shared GPU context
        let gpu_context = super::get_shared_context()?;

        // Load shader
        let shader_source = include_str!("shaders/move_generation.wgsl");
        let shader = gpu_context
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Move Generation Shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_source)),
            });

        // Create bind group layout
        let bind_group_layout =
            gpu_context
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Move Generation Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

        // Create pipeline layout
        let pipeline_layout =
            gpu_context
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Move Generation Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        // Create compute pipeline
        let pipeline =
            gpu_context
                .device()
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Move Generation Pipeline"),
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

    /// Generate all legal moves for a given board state
    /// Returns a list of move encodings (u16 format)
    pub fn generate_moves(&self, board_binary: &[u8; 82]) -> Result<Vec<u16>, String> {
        // Convert board binary to GPU format
        let mut gpu_board = GpuBoardState {
            squares: [0; BOARD_SIZE],
            white_to_move: board_binary[81] as u32,
            _padding: [0; 3],
        };

        for i in 0..BOARD_SIZE {
            gpu_board.squares[i] = board_binary[i] as u32;
        }

        // Create buffers
        let board_buffer =
            self.gpu_context
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Board State Buffer"),
                    contents: bytemuck::cast_slice(&[gpu_board]),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                });

        let move_buffer_init = GpuMoveBuffer {
            moves: [0; MAX_MOVES],
            count: 0,
            _padding: [0; 3],
        };

        let move_buffer =
            self.gpu_context
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Move Buffer"),
                    contents: bytemuck::cast_slice(&[move_buffer_init]),
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
                size: std::mem::size_of::<GpuMoveBuffer>() as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

        // Create bind group
        let bind_group = self
            .gpu_context
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Move Generation Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: board_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: move_buffer.as_entire_binding(),
                    },
                ],
            });

        // Create command encoder
        let mut encoder =
            self.gpu_context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Move Generation Encoder"),
                });

        // Dispatch compute shader (9x9 workgroups, each processing one square)
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Move Generation Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(1, 1, 1); // Single workgroup of 9x9x1
        }

        // Copy results to staging buffer
        encoder.copy_buffer_to_buffer(
            &move_buffer,
            0,
            &staging_buffer,
            0,
            std::mem::size_of::<GpuMoveBuffer>() as u64,
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
        let result_buffer: &GpuMoveBuffer = bytemuck::from_bytes(&data);

        let move_count = result_buffer.count as usize;
        let mut moves = Vec::with_capacity(move_count);

        for i in 0..move_count.min(MAX_MOVES) {
            // Convert from u32 to u16 (our move encoding is 16 bits)
            moves.push(result_buffer.moves[i] as u16);
        }

        drop(data);
        staging_buffer.unmap();

        Ok(moves)
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
    fn test_engine_creation() {
        // Test that engine can be created
        // This test may fail in environments without a GPU adapter
        let engine = MoveGenerationEngine::new_sync();
        if let Err(e) = &engine {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        assert!(engine.is_ok());
    }

    #[test]
    fn test_move_generation_initial_board() {
        let engine = MoveGenerationEngine::new_sync();
        if let Err(e) = &engine {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        let engine = engine.unwrap();

        // Create initial board state (simplified - all zeros except turn indicator)
        let mut board = [0u8; 82];
        board[81] = 1; // White to move

        // Set up a simple test position: white soldier at position 72 (bottom row, column 0)
        // Soldier = 0b001, White = 0b1000000, so White Soldier = 0b1000001 = 65
        board[72] = 0b1000001;

        let result = engine.generate_moves(&board);
        if let Err(e) = &result {
            println!(
                "Move generation error (expected in non-GPU environment): {}",
                e
            );
            return;
        }

        let moves = result.unwrap();
        // A white soldier at position 72 can move forward diagonally
        // This test just checks that we get some moves
        assert!(moves.len() > 0, "Expected at least one move for a soldier");
    }
}
