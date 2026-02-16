//! Board position evaluators for the Keres MCTS engine.
//!
//! Two implementations are provided:
//! - `CpuEvaluator`: pure-Rust heuristic evaluation
//! - `GpuEvaluator`: WGSL compute-shader evaluation via wgpu

use std::borrow::Cow;
use crate::board::{Board, Color, PieceType, Position, BOARD_SIZE};
use super::config::{ScoringWeights, DispatchParams};
use super::gpu_context::GpuContext;
use wgpu::util::DeviceExt;

/// Trait implemented by anything that can assign a [0,1] score to
/// one or more board positions.  `Send` is required so evaluators
/// can be stored inside `MctsEngine` which may be sent between threads.
pub trait Evaluator: Send {
    fn score_positions(&self, boards: &[Board]) -> Vec<f32>;
}

// ══════════  CpuEvaluator  ══════════

pub struct CpuEvaluator {
    pub weights: ScoringWeights,
}

impl CpuEvaluator {
    fn piece_disc(pt: &PieceType) -> u32 {
        match pt {
            PieceType::Soldier  => 1,
            PieceType::Bishop   => 2,
            PieceType::Rook     => 3,
            PieceType::Paladin  => 4,
            PieceType::Guard    => 5,
            PieceType::Knight   => 6,
            PieceType::Ballista => 7,
            PieceType::King     => 8,
        }
    }

    fn evaluate_single(&self, board: &Board) -> f32 {
        // All evaluations are from WHITE's perspective:
        // 1.0 = white is winning, 0.0 = black is winning, 0.5 = even.
        if board.is_game_over() {
            if board.is_draw() { return 0.5; }
            return if board.white_wins() { 1.0 } else { 0.0 };
        }

        let mut white_score: f32 = 0.0;
        let mut black_score: f32 = 0.0;

        for sq in 0..BOARD_SIZE {
            let pos = Position::from_u8(sq as u8);
            if let Some(piece) = board.get_piece(&pos) {
                let accumulator = if piece.color == Color::White {
                    &mut white_score
                } else {
                    &mut black_score
                };

                // Material for bottom piece
                let bottom_val = self.weights.material_value(Self::piece_disc(&piece.bottom));
                *accumulator += bottom_val as f32;

                // Material for top piece (stacked)
                if let Some(ref top_type) = piece.top {
                    let top_val = self.weights.material_value(Self::piece_disc(top_type));
                    *accumulator += top_val as f32;
                }

                // Centrality bonus
                let dx = if pos.x > 4 { pos.x - 4 } else { 4 - pos.x };
                let dy = if pos.y > 4 { pos.y - 4 } else { 4 - pos.y };
                let manhattan = dx + dy;
                let centrality_bonus = ((8 - manhattan) as f32) * self.weights.centrality_wt as f32;
                *accumulator += centrality_bonus;

                // Advance bonus for soldiers and ballistas near promotion
                let dominated_type = piece.top.as_ref().unwrap_or(&piece.bottom);
                let is_advanceable = matches!(dominated_type,
                    PieceType::Soldier | PieceType::Ballista);
                if is_advanceable {
                    let advance_rank = match piece.color {
                        Color::White => if pos.y > 0 { 8 - pos.y } else { 8 },
                        Color::Black => pos.y,
                    };
                    let advance_bonus = (advance_rank as f32) * self.weights.advance_wt as f32;
                    *accumulator += advance_bonus;
                }
            }
        }

        let diff = white_score - black_score;
        // Sigmoid squash into [0,1]: high = good for white
        let exponent = -diff / 2000.0;
        1.0 / (1.0 + exponent.exp())
    }
}

impl Evaluator for CpuEvaluator {
    fn score_positions(&self, boards: &[Board]) -> Vec<f32> {
        boards.iter().map(|b| self.evaluate_single(b)).collect()
    }
}

// ══════════  Board serialisation helper  ══════════

/// Pack boards into a contiguous byte array.  Each board occupies
/// 84 bytes: 83 from `to_binary()` plus 1 byte of padding.
pub fn serialize_boards(boards: &[Board]) -> Vec<u8> {
    let stride = 84usize;
    let mut blob = vec![0u8; boards.len() * stride];
    for (bi, board) in boards.iter().enumerate() {
        let bin = board.to_binary();
        let offset = bi * stride;
        blob[offset..offset + 83].copy_from_slice(&bin[..83]);
        // blob[offset + 83] stays 0 (padding)
    }
    blob
}

// ══════════  GpuEvaluator  ══════════

pub struct GpuEvaluator {
    gpu_ctx: GpuContext,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    weight_data: ScoringWeights,
    #[allow(dead_code)]
    dispatch_cfg: DispatchParams,
}

impl GpuEvaluator {
    pub fn try_build(ctx: GpuContext,
                     weights: ScoringWeights,
                     dispatch: DispatchParams) -> Result<Self, String> {
        let shader_src = include_str!("shaders/rollout.wgsl");
        let shader_module = ctx.device().create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("keres_rollout_shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_src)),
        });

        let bgl = ctx.device().create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("keres_rollout_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false, min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false, min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false, min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipe_layout = ctx.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("keres_rollout_pipe_layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = ctx.device().create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("keres_rollout_pipeline"),
            layout: Some(&pipe_layout),
            module: &shader_module,
            entry_point: Some("rollout_entry"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(GpuEvaluator {
            gpu_ctx: ctx,
            pipeline,
            bind_group_layout: bgl,
            weight_data: weights,
            dispatch_cfg: dispatch,
        })
    }
}

impl Evaluator for GpuEvaluator {
    fn score_positions(&self, boards: &[Board]) -> Vec<f32> {
        let board_count = boards.len();
        if board_count == 0 { return Vec::new(); }

        let packed = serialize_boards(boards);
        let dev = self.gpu_ctx.device();
        let q   = self.gpu_ctx.queue();

        let pos_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("keres_positions_buf"),
            contents: &packed,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let scores_size = (board_count * 4) as u64;
        let scores_buf = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("keres_scores_buf"),
            size: scores_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let weight_bytes: &[u8] = bytemuck::bytes_of(&self.weight_data);
        let weight_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("keres_weights_buf"),
            contents: weight_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let rng_data: Vec<u32> = (0..board_count as u32).map(|i| i.wrapping_mul(2654435761) | 1).collect();
        let rng_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("keres_rng_buf"),
            contents: bytemuck::cast_slice(&rng_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("keres_rollout_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: pos_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: scores_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: weight_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: rng_buf.as_entire_binding() },
            ],
        });

        let workgroups = ((board_count + 63) / 64) as u32;
        let mut encoder = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("keres_rollout_enc"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("keres_rollout_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        let staging = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("keres_staging"),
            size: scores_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(&scores_buf, 0, &staging, 0, scores_size);
        q.submit(Some(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        staging.slice(..).map_async(wgpu::MapMode::Read, move |res| { let _ = tx.send(res); });
        dev.poll(wgpu::Maintain::Wait);
        if rx.recv().unwrap().is_err() {
            return vec![0.5; board_count];
        }

        let mapped = staging.slice(..).get_mapped_range();
        let float_slice: &[f32] = bytemuck::cast_slice(&mapped);
        let out = float_slice[..board_count].to_vec();
        drop(mapped);
        staging.unmap();
        out
    }
}

// ══════════  Tests  ══════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    fn cpu_eval() -> CpuEvaluator {
        CpuEvaluator { weights: ScoringWeights::default() }
    }

    #[test]
    fn starting_position_near_half() {
        let ev = cpu_eval();
        let sc = ev.score_positions(&[Board::new()]);
        assert!((sc[0] - 0.5).abs() < 0.05,
            "symmetric start should score ~0.5, got {}", sc[0]);
    }

    #[test]
    fn terminal_win_scores_one() {
        let mut b = Board::new();
        b.set_game_over(true, true, false); // white wins
        let sc = cpu_eval().score_positions(&[b]);
        // From white's perspective, white winning = 1.0 regardless of whose turn
        assert!((sc[0] - 1.0).abs() < 1e-5, "white wins should be 1.0, got {}", sc[0]);
    }

    #[test]
    fn terminal_loss_scores_zero() {
        let mut b = Board::new();
        b.set_game_over(true, false, false); // black wins
        let sc = cpu_eval().score_positions(&[b]);
        // From white's perspective, black winning = 0.0
        assert!(sc[0].abs() < 1e-5, "black wins should be 0.0, got {}", sc[0]);
    }

    #[test]
    fn evaluation_is_symmetric_regardless_of_turn() {
        // Same board, different turn flag — material shouldn't change
        let b_white = Board::new();
        let mut b_black = Board::new();
        b_black.set_white_to_move(false);
        let ev = cpu_eval();
        let sw = ev.score_positions(&[b_white])[0];
        let sb = ev.score_positions(&[b_black])[0];
        assert!((sw - sb).abs() < 1e-5,
            "evaluation should be same regardless of turn: white={sw}, black={sb}");
    }

    #[test]
    fn terminal_draw_scores_half() {
        let mut b = Board::new();
        b.set_game_over(true, false, true);
        let sc = cpu_eval().score_positions(&[b]);
        assert!((sc[0] - 0.5).abs() < 1e-5, "draw should be 0.5, got {}", sc[0]);
    }

    #[test]
    fn batch_returns_correct_count() {
        let batch = vec![Board::new(); 7];
        let sc = cpu_eval().score_positions(&batch);
        assert_eq!(sc.len(), 7);
    }
}
