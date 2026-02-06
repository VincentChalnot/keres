# MCTS GPU Implementation Specification for Keres

## 1. Executive Summary

This document specifies a complete GPU-accelerated Monte Carlo Tree Search (MCTS) implementation for the Keres board game using Rust + wgpu. The system uses:

- **Batch MCTS with Virtual Loss**: Multiple parallel leaf selections
- **CPU-GPU asynchronous pipeline**: CPU handles tree traversal, GPU handles rollouts
- **Configurable heuristic evaluation**: Parameterizable scoring function for optimization
- **Target performance**: 10,000+ MCTS iterations per move with 256-512 position batches

***

## 2. Architecture Overview

### 2.1 High-Level Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│                      CPU (Multi-threaded)                    │
│                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │
│  │  Selection   │ -> │ Virtual Loss │ -> │ Backprop     │ │
│  │  (8 threads) │    │   Applied    │    │ (removes VL) │ │
│  └──────┬───────┘    └──────┬───────┘    └───────▲──────┘ │
│         │                    │                     │        │
│         └────────────────────┼─────────────────────┘        │
└──────────────────────────────┼──────────────────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │   Selection Queue   │
                    │  (MPSC Channel)     │
                    └──────────┬──────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│                      GPU (wgpu Compute)                      │
│                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │
│  │ Batch Collect│ -> │   Rollouts   │ -> │  Evaluation  │ │
│  │ (wait 50ms   │    │ (256-512 pos)│    │  (heuristic) │ │
│  │  or full)    │    │ 10-15 moves  │    │   scoring    │ │
│  └──────────────┘    └──────────────┘    └───────┬──────┘ │
│                                                    │        │
└────────────────────────────────────────────────────┼────────┘
                                                     │
                                          ┌──────────▼──────────┐
                                          │   Results Queue     │
                                          │  (MPSC Channel)     │
                                          └─────────────────────┘
```


### 2.2 Core Components

1. **MCTSTree** (CPU): Tree structure with per-node locking
2. **SelectionWorker** (CPU threads): Parallel tree traversal with virtual loss
3. **GPUBatchProcessor** (GPU): Batched rollout execution
4. **HeuristicConfig** (CPU/GPU): Parameterizable evaluation function
5. **Coordinator** (CPU): Orchestrates CPU-GPU pipeline

***

## 3. Data Structures

### 3.1 MCTSNode (CPU-side)

```rust
use parking_lot::RwLock;

/// Single node in the MCTS tree
/// Uses RwLock for fine-grained parallelism
pub struct MCTSNode {
    /// Number of visits (including virtual loss)
    pub visits: u32,
    
    /// Sum of evaluation scores [0.0, 1.0]
    pub value_sum: f32,
    
    /// Parent node ID (None for root)
    pub parent: Option<NodeId>,
    
    /// Child nodes mapped by move
    pub children: HashMap<Move, NodeId>,
    
    /// Game position (83 bytes: 81 board + 2 flags + 1 counter)
    pub position: Position,
    
    /// Move that led to this position
    pub incoming_move: Option<Move>,
    
    /// Expansion state
    pub is_expanded: bool,
    
    /// Legal moves cache (computed lazily)
    pub legal_moves: Option<Vec<Move>>,
}

impl MCTSNode {
    /// UCB1 formula: Q + c * sqrt(ln(N_parent) / N)
    pub fn ucb1_score(&self, parent_visits: u32, exploration_constant: f32) -> f32 {
        if self.visits == 0 {
            return f32::INFINITY;
        }
        
        let q = self.value_sum / self.visits as f32;
        let exploration = exploration_constant * 
            ((parent_visits as f32).ln() / self.visits as f32).sqrt();
        
        q + exploration
    }
}

pub type NodeId = usize;
```


### 3.2 Move Encoding

```rust
/// Move encoded as u16:
/// - Bits 0-6: From position (0-80)
/// - Bits 7-13: To position (0-80)
/// - Bit 14: Unstack flag (1 = unstack top piece only)
/// - Bit 15: Reserved
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Move(u16);

impl Move {
    pub fn new(from: u8, to: u8, unstack: bool) -> Self {
        debug_assert!(from < 81 && to < 81);
        let mut bits = (from as u16) | ((to as u16) << 7);
        if unstack {
            bits |= 1 << 14;
        }
        Self(bits)
    }
    
    pub fn from(&self) -> u8 { (self.0 & 0x7F) as u8 }
    pub fn to(&self) -> u8 { ((self.0 >> 7) & 0x7F) as u8 }
    pub fn unstack(&self) -> bool { (self.0 & (1 << 14)) != 0 }
}
```


### 3.3 Position Encoding

```rust
/// Game position: 83 bytes total
/// - Bytes 0-80: Board state (9x9 grid)
/// - Byte 81: Victory/draw flags
/// - Byte 82: Fifty-move counter (moves without capture)
#[derive(Clone, Debug)]
pub struct Position {
    pub data: [u8; 83],
}

impl Position {
    /// Board cell encoding (per byte):
    /// - Bits 0-3: Piece type (0=empty, 1-8=pieces)
    /// - Bit 4: Owner (0=player1, 1=player2)
    /// - Bits 5-7: Stack info (if stacked)
    pub fn get_cell(&self, row: u8, col: u8) -> u8 {
        debug_assert!(row < 9 && col < 9);
        self.data[(row * 9 + col) as usize]
    }
    
    pub fn is_terminal(&self) -> bool {
        self.data[^81] != 0  // Victory/draw flags set
    }
    
    pub fn current_player(&self) -> Player {
        // Derive from move count or explicit flag
        unimplemented!()
    }
}
```


### 3.4 HeuristicConfig (CPU/GPU shared)

```rust
/// Parameterizable heuristic weights
/// Shared between CPU (config) and GPU (shader constants)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HeuristicConfig {
    // Piece values (relative to Soldier=100)
    pub soldier_value: u32,      // 100
    pub jester_value: u32,       // 300
    pub commander_value: u32,    // 500
    pub paladin_value: u32,      // 300
    pub guard_value: u32,        // 300
    pub dragon_value: u32,       // 300
    pub balista_value: u32,      // 500
    pub king_value: u32,         // 10000 (game-ending)
    
    // Positional weights (0-100 scale)
    pub control_weight: u32,     // Weight for square control
    pub mobility_weight: u32,    // Weight for piece mobility
    pub king_safety_weight: u32, // Weight for king safety
    pub threat_weight: u32,      // Weight for pieces under attack
    pub promotion_weight: u32,   // Weight for promotion potential
    
    // Stack evaluation (future: can be +/- bonus)
    pub stack_bonus: i32,        // Bonus per stacked piece (default: 0)
    
    // Rollout behavior
    pub tactical_depth: u32,     // How many moves use smart heuristics (0-10)
    pub capture_priority: u32,   // Boost for capture moves (0-100)
    pub threat_priority: u32,    // Boost for threatening moves (0-100)
    
    // Reserved for future use
    pub reserved: [u32; 8],
}

impl Default for HeuristicConfig {
    fn default() -> Self {
        Self {
            soldier_value: 100,
            jester_value: 300,
            commander_value: 500,
            paladin_value: 300,
            guard_value: 300,
            dragon_value: 300,
            balista_value: 500,
            king_value: 10000,
            
            control_weight: 10,
            mobility_weight: 15,
            king_safety_weight: 50,
            threat_weight: 40,
            promotion_weight: 30,
            
            stack_bonus: 0,  // Start neutral
            
            tactical_depth: 3,
            capture_priority: 70,
            threat_priority: 60,
            
            reserved: [0; 8],
        }
    }
}
```


***

## 4. GPU Shader Specification (WGSL)

### 4.1 Entry Point

```wgsl
@group(0) @binding(0) var<storage, read> positions: array<Position>;
@group(0) @binding(1) var<storage, read_write> scores: array<f32>;
@group(0) @binding(2) var<uniform> config: HeuristicConfig;
@group(0) @binding(3) var<storage, read_write> rng_states: array<u32>;

@compute @workgroup_size(64)
fn rollout_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= arrayLength(&positions)) {
        return;
    }
    
    // Initialize RNG from per-thread seed
    var rng_state = rng_states[idx];
    
    // Copy position to local state
    var state = positions[idx];
    
    // Perform rollout (10-15 moves)
    for (var depth = 0u; depth < 15u; depth++) {
        if (is_terminal(&state)) {
            break;
        }
        
        // Generate legal moves (reuse existing Keres implementation)
        var moves = generate_legal_moves(&state);
        if (moves.count == 0u) {
            break;
        }
        
        // Select move based on depth and heuristics
        let chosen_move = select_move(&moves, &state, depth, &config, &rng_state);
        
        // Apply move
        apply_move(&mut state, chosen_move);
    }
    
    // Evaluate final position
    scores[idx] = evaluate_position(&state, &config);
    
    // Save RNG state
    rng_states[idx] = rng_state;
}
```


### 4.2 Move Selection Logic

```wgsl
fn select_move(
    moves: ptr<function, MoveList>,
    state: ptr<function, Position>,
    depth: u32,
    config: ptr<uniform, HeuristicConfig>,
    rng: ptr<function, u32>
) -> Move {
    // Phase 1: Tactical depth (smart heuristics)
    if (depth < (*config).tactical_depth) {
        // Look for high-value captures
        let capture = find_best_capture(moves, state, config);
        if (capture.is_valid && random_chance(rng, (*config).capture_priority)) {
            return capture;
        }
        
        // Look for king threats
        let threat = find_king_threat(moves, state);
        if (threat.is_valid && random_chance(rng, (*config).threat_priority)) {
            return threat;
        }
    }
    
    // Phase 2: Random selection (fast terminal)
    return random_move(moves, rng);
}

fn find_best_capture(
    moves: ptr<function, MoveList>,
    state: ptr<function, Position>,
    config: ptr<uniform, HeuristicConfig>
) -> Move {
    var best_move = Move::invalid();
    var best_value = 0u;
    
    for (var i = 0u; i < (*moves).count; i++) {
        let mv = (*moves).items[i];
        let to_cell = get_cell(state, mv.to_pos);
        
        if (is_enemy_piece(to_cell, state)) {
            let captured_value = piece_value(to_cell, config);
            if (captured_value > best_value) {
                best_value = captured_value;
                best_move = mv;
            }
        }
    }
    
    return best_move;
}
```


### 4.3 Position Evaluation

```wgsl
fn evaluate_position(state: ptr<function, Position>, config: ptr<uniform, HeuristicConfig>) -> f32 {
    var my_score = 0.0;
    var opp_score = 0.0;
    
    let current_player = get_current_player(state);
    
    // Iterate over all board squares
    for (var row = 0u; row < 9u; row++) {
        for (var col = 0u; col < 9u; col++) {
            let cell = get_cell_rc(state, row, col);
            if (is_empty(cell)) {
                continue;
            }
            
            let owner = get_owner(cell);
            let piece_type = get_piece_type(cell);
            
            // Material value (sum of stacked pieces)
            var material = piece_value(cell, config);
            
            // Mobility bonus (number of legal moves from this square)
            let mobility = count_legal_moves_from(state, row, col);
            let mobility_score = f32(mobility) * f32((*config).mobility_weight);
            
            // Square control (can this piece reach center/important squares?)
            let control_score = evaluate_control(state, row, col, config);
            
            // Threat assessment (is this piece under attack?)
            let is_threatened = is_under_attack(state, row, col, owner);
            let threat_penalty = select(0.0, f32(material) * 0.5, is_threatened);
            
            // Promotion potential (Soldier/Balista near promotion)
            var promotion_bonus = 0.0;
            if ((piece_type == SOLDIER || piece_type == BALISTA) && is_near_promotion(row, owner)) {
                promotion_bonus = f32((*config).promotion_weight);
            }
            
            // King safety (for king pieces)
            var king_safety = 0.0;
            if (piece_type == KING) {
                king_safety = evaluate_king_safety(state, row, col, config);
            }
            
            // Aggregate score for this piece
            let total_piece_score = f32(material) + mobility_score + control_score 
                                   - threat_penalty + promotion_bonus + king_safety;
            
            // Add to appropriate player's score
            if (owner == current_player) {
                my_score += total_piece_score;
            } else {
                opp_score += total_piece_score;
            }
        }
    }
    
    // Terminal position overrides
    if (is_terminal(state)) {
        if (is_winner(state, current_player)) {
            return 1.0;  // Victory
        } else {
            return 0.0;  // Defeat
        }
    }
    
    // Normalize to [0.0, 1.0] using sigmoid
    // score_diff ranges roughly [-10000, +10000] based on king values
    let score_diff = my_score - opp_score;
    return sigmoid(score_diff / 2000.0);  // Tune divisor for sensitivity
}

fn sigmoid(x: f32) -> f32 {
    return 1.0 / (1.0 + exp(-x));
}

fn evaluate_control(state: ptr<function, Position>, row: u32, col: u32, config: ptr<uniform, HeuristicConfig>) -> f32 {
    // Bonus for controlling central squares (4,4 = center)
    let center_dist = abs(f32(row) - 4.0) + abs(f32(col) - 4.0);
    let centrality_bonus = (10.0 - center_dist) * 2.0;  // Higher = more central
    
    return centrality_bonus * f32((*config).control_weight) * 0.1;
}

fn evaluate_king_safety(state: ptr<function, Position>, row: u32, col: u32, config: ptr<uniform, HeuristicConfig>) -> f32 {
    // Count friendly pieces adjacent to king
    var defenders = 0u;
    for (var dr = -1; dr <= 1; dr++) {
        for (var dc = -1; dc <= 1; dc++) {
            if (dr == 0 && dc == 0) { continue; }
            let nr = i32(row) + dr;
            let nc = i32(col) + dc;
            if (nr >= 0 && nr < 9 && nc >= 0 && nc < 9) {
                let cell = get_cell_rc(state, u32(nr), u32(nc));
                if (is_friendly(cell, get_current_player(state))) {
                    defenders++;
                }
            }
        }
    }
    
    // More defenders = safer king
    return f32(defenders) * f32((*config).king_safety_weight) * 5.0;
}

fn is_near_promotion(row: u32, owner: Player) -> bool {
    // Soldiers/Balistas promote on reaching row 0 or 8
    if (owner == PLAYER1) {
        return row <= 1u;  // Close to row 0
    } else {
        return row >= 7u;  // Close to row 8
    }
}
```


***

## 5. CPU Implementation

### 5.1 MCTSTree

```rust
use parking_lot::RwLock;
use std::sync::Arc;

pub struct MCTSTree {
    nodes: Vec<RwLock<MCTSNode>>,
    root_id: NodeId,
    config: MCTSConfig,
}

pub struct MCTSConfig {
    pub virtual_loss: u32,           // Default: 10
    pub exploration_constant: f32,   // UCB1 constant, default: sqrt(2)
    pub max_tree_size: usize,        // Memory limit
}

impl MCTSTree {
    pub fn new(root_position: Position, config: MCTSConfig) -> Self {
        let root = MCTSNode {
            visits: 0,
            value_sum: 0.0,
            parent: None,
            children: HashMap::new(),
            position: root_position,
            incoming_move: None,
            is_expanded: false,
            legal_moves: None,
        };
        
        Self {
            nodes: vec![RwLock::new(root)],
            root_id: 0,
            config,
        }
    }
    
    /// Select a leaf node with virtual loss applied
    pub fn select_leaf_with_virtual_loss(&self) -> (NodeId, Vec<NodeId>) {
        let mut path = vec![self.root_id];
        let mut node_id = self.root_id;
        
        loop {
            let node = self.nodes[node_id].read();
            
            // If not expanded, this is our leaf
            if !node.is_expanded {
                drop(node);
                self.apply_virtual_loss(&path);
                return (node_id, path);
            }
            
            // If terminal, this is our leaf
            if node.position.is_terminal() {
                drop(node);
                self.apply_virtual_loss(&path);
                return (node_id, path);
            }
            
            // If no children, need to expand
            if node.children.is_empty() {
                drop(node);
                self.apply_virtual_loss(&path);
                return (node_id, path);
            }
            
            // Select best child via UCB1
            let parent_visits = node.visits;
            let best_child = node.children.values()
                .max_by(|a, b| {
                    let score_a = self.nodes[**a].read()
                        .ucb1_score(parent_visits, self.config.exploration_constant);
                    let score_b = self.nodes[**b].read()
                        .ucb1_score(parent_visits, self.config.exploration_constant);
                    score_a.partial_cmp(&score_b).unwrap()
                })
                .copied()
                .unwrap();
            
            drop(node);
            node_id = best_child;
            path.push(node_id);
        }
    }
    
    fn apply_virtual_loss(&self, path: &[NodeId]) {
        for &node_id in path {
            let mut node = self.nodes[node_id].write();
            node.visits += self.config.virtual_loss;
            // Optional: also add pessimistic value
            // node.value_sum += 0.0; (assumes loss)
        }
    }
    
    pub fn revert_virtual_loss(&self, path: &[NodeId]) {
        for &node_id in path {
            let mut node = self.nodes[node_id].write();
            node.visits -= self.config.virtual_loss;
        }
    }
    
    pub fn backpropagate(&self, path: &[NodeId], mut score: f32) {
        for &node_id in path.iter().rev() {
            let mut node = self.nodes[node_id].write();
            node.visits += 1;
            node.value_sum += score;
            
            // Flip score for opponent
            score = 1.0 - score;
        }
    }
    
    /// Expand a node by adding all legal moves as children
    pub fn expand(&mut self, node_id: NodeId) {
        let position = {
            let node = self.nodes[node_id].read();
            if node.is_expanded {
                return;  // Already expanded by another thread
            }
            node.position.clone()
        };
        
        // Generate legal moves (using existing Keres implementation)
        let legal_moves = generate_legal_moves(&position);
        
        let mut child_ids = HashMap::new();
        for mv in legal_moves {
            let child_pos = apply_move(&position, mv);
            let child_node = MCTSNode {
                visits: 0,
                value_sum: 0.0,
                parent: Some(node_id),
                children: HashMap::new(),
                position: child_pos,
                incoming_move: Some(mv),
                is_expanded: false,
                legal_moves: None,
            };
            
            let child_id = self.nodes.len();
            self.nodes.push(RwLock::new(child_node));
            child_ids.insert(mv, child_id);
        }
        
        // Mark as expanded
        let mut node = self.nodes[node_id].write();
        node.is_expanded = true;
        node.children = child_ids;
    }
    
    /// Get best move from root based on visit counts
    pub fn best_move(&self) -> Option<Move> {
        let root = self.nodes[self.root_id].read();
        root.children.iter()
            .max_by_key(|(_, &child_id)| {
                self.nodes[child_id].read().visits
            })
            .map(|(mv, _)| *mv)
    }
}
```


### 5.2 Selection Worker (CPU Thread)

```rust
use std::sync::mpsc::{Sender, Receiver};

pub struct SelectionWorker {
    tree: Arc<MCTSTree>,
    selection_tx: Sender<(NodeId, Vec<NodeId>)>,
    result_rx: Receiver<(Vec<NodeId>, f32)>,
}

impl SelectionWorker {
    pub fn run(&self) {
        loop {
            // Phase 1: Select a leaf
            let (leaf_id, path) = self.tree.select_leaf_with_virtual_loss();
            
            // Send to GPU queue
            if self.selection_tx.send((leaf_id, path)).is_err() {
                break;  // Shutdown signal
            }
            
            // Phase 2: Process any available results
            while let Ok((path, score)) = self.result_rx.try_recv() {
                self.tree.revert_virtual_loss(&path);
                self.tree.backpropagate(&path, score);
            }
        }
    }
}
```


### 5.3 GPU Batch Processor

```rust
use wgpu;
use std::time::{Duration, Instant};

pub struct GPUBatchProcessor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    
    config: BatchConfig,
    heuristic_config: HeuristicConfig,
}

pub struct BatchConfig {
    pub batch_size: usize,        // Default: 256
    pub timeout_ms: u64,          // Default: 50
    pub max_rollout_depth: u32,   // Default: 15
}

impl GPUBatchProcessor {
    pub async fn process_batch(
        &self,
        selections: Vec<(NodeId, Vec<NodeId>)>,
    ) -> Vec<(Vec<NodeId>, f32)> {
        let batch_size = selections.len();
        
        // Extract positions
        let positions: Vec<Position> = selections.iter()
            .map(|(node_id, _)| /* get position from node_id */)
            .collect();
        
        // Create GPU buffers
        let position_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Position Buffer"),
            contents: bytemuck::cast_slice(&positions),
            usage: wgpu::BufferUsages::STORAGE,
        });
        
        let score_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Score Buffer"),
            size: (batch_size * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        
        let config_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Config Buffer"),
            contents: bytemuck::bytes_of(&self.heuristic_config),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        
        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rollout Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: position_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: score_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: config_buffer.as_entire_binding(),
                },
            ],
        });
        
        // Dispatch compute shader
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut cpass = encoder.begin_compute_pass(&Default::default());
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups((batch_size as u32 + 63) / 64, 1, 1);
        }
        
        // Read back results
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: score_buffer.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        encoder.copy_buffer_to_buffer(&score_buffer, 0, &staging_buffer, 0, score_buffer.size());
        self.queue.submit(Some(encoder.finish()));
        
        // Wait for GPU
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.await.unwrap().unwrap();
        
        let data = buffer_slice.get_mapped_range();
        let scores: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();
        
        // Package results with paths
        selections.into_iter()
            .zip(scores)
            .map(|((_, path), score)| (path, score))
            .collect()
    }
    
    pub fn run_batching_loop(
        &self,
        selection_rx: Receiver<(NodeId, Vec<NodeId>)>,
        result_tx: Sender<(Vec<NodeId>, f32)>,
    ) {
        loop {
            let mut batch = Vec::with_capacity(self.config.batch_size);
            let deadline = Instant::now() + Duration::from_millis(self.config.timeout_ms);
            
            // Accumulate batch
            while batch.len() < self.config.batch_size && Instant::now() < deadline {
                match selection_rx.try_recv() {
                    Ok(item) => batch.push(item),
                    Err(_) => std::thread::sleep(Duration::from_millis(1)),
                }
            }
            
            if batch.is_empty() {
                continue;
            }
            
            // Process on GPU
            let results = pollster::block_on(self.process_batch(batch));
            
            // Send results back
            for result in results {
                if result_tx.send(result).is_err() {
                    return;  // Shutdown
                }
            }
        }
    }
}
```


### 5.4 Main Coordinator

```rust
use std::sync::mpsc;

pub struct MCTSCoordinator {
    tree: Arc<MCTSTree>,
    gpu_processor: Arc<GPUBatchProcessor>,
    
    num_cpu_threads: usize,
    total_iterations: usize,
}

impl MCTSCoordinator {
    pub fn search(&mut self, root_position: Position) -> Move {
        // Create communication channels
        let (selection_tx, selection_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        
        // Spawn GPU thread
        let gpu_proc = self.gpu_processor.clone();
        let gpu_thread = std::thread::spawn(move || {
            gpu_proc.run_batching_loop(selection_rx, result_tx);
        });
        
        // Spawn CPU worker threads
        let mut cpu_threads = vec![];
        for _ in 0..self.num_cpu_threads {
            let worker = SelectionWorker {
                tree: self.tree.clone(),
                selection_tx: selection_tx.clone(),
                result_rx: result_rx.clone(),
            };
            cpu_threads.push(std::thread::spawn(move || worker.run()));
        }
        
        // Wait for iterations to complete
        // (In practice, use atomic counter or timeout)
        std::thread::sleep(Duration::from_secs(5));  // Example: 5 second search
        
        // Shutdown workers
        drop(selection_tx);
        for thread in cpu_threads {
            thread.join().ok();
        }
        gpu_thread.join().ok();
        
        // Return best move
        self.tree.best_move().expect("No moves available")
    }
}
```


***

## 6. Testing Strategy

### 6.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_move_encoding() {
        let mv = Move::new(0, 80, true);
        assert_eq!(mv.from(), 0);
        assert_eq!(mv.to(), 80);
        assert_eq!(mv.unstack(), true);
    }
    
    #[test]
    fn test_ucb1_calculation() {
        let node = MCTSNode {
            visits: 10,
            value_sum: 7.0,
            ..Default::default()
        };
        let score = node.ucb1_score(100, std::f32::consts::SQRT_2);
        assert!(score > 0.7 && score < 1.5);
    }
    
    #[test]
    fn test_virtual_loss_revert() {
        let tree = MCTSTree::new(Position::initial(), MCTSConfig::default());
        let path = vec![^0];
        let initial_visits = tree.nodes[^0].read().visits;
        
        tree.apply_virtual_loss(&path);
        assert_eq!(tree.nodes[^0].read().visits, initial_visits + 10);
        
        tree.revert_virtual_loss(&path);
        assert_eq!(tree.nodes[^0].read().visits, initial_visits);
    }
    
    #[test]
    fn test_backpropagation_score_flip() {
        let tree = MCTSTree::new(Position::initial(), MCTSConfig::default());
        // Add child to root
        tree.expand(0);
        let child_id = *tree.nodes[^0].read().children.values().next().unwrap();
        
        tree.backpropagate(&vec![0, child_id], 0.8);
        
        // Child should have score 0.8
        assert!((tree.nodes[child_id].read().value_sum - 0.8).abs() < 0.01);
        
        // Root should have flipped score (0.2)
        assert!((tree.nodes[^0].read().value_sum - 0.2).abs() < 0.01);
    }
}
```


### 6.2 Integration Tests

```rust
#[test]
fn test_full_search_cycle() {
    let config = HeuristicConfig::default();
    let position = Position::initial();
    let mut coordinator = MCTSCoordinator::new(
        position.clone(),
        config,
        8,      // CPU threads
        1000,   // iterations
    );
    
    let best_move = coordinator.search(position);
    assert!(best_move.from() < 81 && best_move.to() < 81);
}

#[test]
fn test_gpu_batch_processing() {
    let processor = GPUBatchProcessor::new(
        HeuristicConfig::default(),
        BatchConfig::default(),
    );
    
    let positions = vec![Position::initial(); 256];
    let selections: Vec<_> = positions.into_iter()
        .enumerate()
        .map(|(i, pos)| (i, vec![i]))
        .collect();
    
    let results = pollster::block_on(processor.process_batch(selections));
    
    assert_eq!(results.len(), 256);
    for (_, score) in &results {
        assert!(*score >= 0.0 && *score <= 1.0);
    }
}
```


### 6.3 Benchmark Tests

```rust
#[bench]
fn bench_selection_throughput(b: &mut Bencher) {
    let tree = Arc::new(MCTSTree::new(Position::initial(), MCTSConfig::default()));
    
    b.iter(|| {
        tree.select_leaf_with_virtual_loss()
    });
}

#[bench]
fn bench_gpu_rollout_256(b: &mut Bencher) {
    let processor = GPUBatchProcessor::new(
        HeuristicConfig::default(),
        BatchConfig { batch_size: 256, ..Default::default() },
    );
    
    let batch = create_test_batch(256);
    
    b.iter(|| {
        pollster::block_on(processor.process_batch(batch.clone()))
    });
}
```


### 6.4 Correctness Tests

```rust
#[test]
fn test_terminal_position_immediate_return() {
    // Position where king is captured
    let terminal_pos = Position::king_captured();
    
    let tree = MCTSTree::new(terminal_pos.clone(), MCTSConfig::default());
    let (leaf_id, _) = tree.select_leaf_with_virtual_loss();
    
    // Should immediately return root (terminal)
    assert_eq!(leaf_id, tree.root_id);
}

#[test]
fn test_heuristic_symmetry() {
    // Same position from both player perspectives should sum to ~1.0
    let position = Position::initial();
    
    let score_p1 = evaluate_position_cpu(&position, Player::Player1, &HeuristicConfig::default());
    let score_p2 = evaluate_position_cpu(&position, Player::Player2, &HeuristicConfig::default());
    
    assert!((score_p1 + score_p2 - 1.0).abs() < 0.1);
}
```


***

## 7. Project Structure

```
keres-mcts/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # Public API
│   ├── main.rs                 # CLI entry point
│   │
│   ├── core/
│   │   ├── mod.rs
│   │   ├── position.rs         # Position encoding (83 bytes)
│   │   ├── moves.rs            # Move generation & application (reuse existing)
│   │   └── rules.rs            # Game rules (reuse existing)
│   │
│   ├── mcts/
│   │   ├── mod.rs
│   │   ├── tree.rs             # MCTSTree, MCTSNode
│   │   ├── config.rs           # MCTSConfig, HeuristicConfig
│   │   ├── selection.rs        # Selection logic, UCB1
│   │   └── coordinator.rs      # Main orchestration
│   │
│   ├── gpu/
│   │   ├── mod.rs
│   │   ├── batch_processor.rs  # GPU batch handling
│   │   ├── shaders/
│   │   │   ├── rollout.wgsl    # Main compute shader
│   │   │   └── utils.wgsl      # Shared WGSL functions
│   │   └── buffers.rs          # Buffer management
│   │
│   ├── workers/
│   │   ├── mod.rs
│   │   ├── selection_worker.rs # CPU thread workers
│   │   └── channels.rs         # MPSC queue management
│   │
│   └── utils/
│       ├── mod.rs
│       ├── rng.rs              # PRNG for GPU (xorshift)
│       └── metrics.rs          # Performance tracking
│
├── tests/
│   ├── integration_tests.rs
│   ├── correctness_tests.rs
│   └── benchmark_tests.rs
│
├── benches/
│   └── mcts_bench.rs
│
└── examples/
    ├── simple_search.rs        # Basic usage example
    └── parameter_test.rs       # Heuristic config testing
```


***

## 8. Implementation Checklist

### Phase 1: Core Infrastructure (Week 1)

- [ ] Implement `Move` encoding/decoding
- [ ] Implement `Position` structure (reuse existing if available)
- [ ] Implement `MCTSNode` with RwLock
- [ ] Implement `MCTSTree` with basic selection
- [ ] Write unit tests for above


### Phase 2: CPU MCTS (Week 2)

- [ ] Implement UCB1 selection algorithm
- [ ] Implement virtual loss mechanism
- [ ] Implement backpropagation with score flipping
- [ ] Implement tree expansion
- [ ] Write integration tests for CPU-only MCTS


### Phase 3: GPU Rollouts (Week 3)

- [ ] Set up wgpu pipeline
- [ ] Implement basic rollout shader (random moves)
- [ ] Implement position evaluation shader
- [ ] Add heuristic configuration
- [ ] Test GPU batch processing in isolation


### Phase 4: CPU-GPU Integration (Week 4)

- [ ] Implement selection worker threads
- [ ] Implement GPU batch processor with timeout
- [ ] Set up MPSC channels between CPU/GPU
- [ ] Implement coordinator
- [ ] Test full pipeline with benchmarks


### Phase 5: Heuristic Refinement (Week 5)

- [ ] Implement smart move selection (captures, threats)
- [ ] Implement full position evaluation (mobility, control, etc.)
- [ ] Add parameterization for all heuristic weights
- [ ] Test against baseline (random rollouts)
- [ ] Tune default parameters


### Phase 6: Optimization \& Testing (Week 6)

- [ ] Profile CPU bottlenecks
- [ ] Profile GPU bottlenecks
- [ ] Optimize memory allocations
- [ ] Write comprehensive test suite
- [ ] Add performance benchmarks
- [ ] Document API

***

## 9. Performance Targets

| Metric | Target | Measurement |
| :-- | :-- | :-- |
| **MCTS iterations/second** | 2000-5000 | Total iterations ÷ search time |
| **GPU utilization** | >85% | wgpu profiler |
| **CPU thread efficiency** | >80% | Active time ÷ total time |
| **Batch throughput** | 100-200 batches/sec | Batches completed ÷ time |
| **Memory usage** | <2GB | Peak heap allocation |
| **Search quality** | Win vs random >90% | Self-play test games |


***

## 10. Key Implementation Notes

### 10.1 Thread Safety

- Use `parking_lot::RwLock` for per-node locking (faster than std)
- Minimize lock holding time (read position, drop lock, compute, reacquire)
- Use `Arc` for shared tree access across threads


### 10.2 GPU Optimization

- Use workgroup size of 64 (good for most GPUs)
- Pack multiple positions per batch to saturate compute units
- Use push constants for small config data if possible
- Consider double buffering for zero GPU idle time


### 10.3 Numerical Stability

- Use f32 for all scores (f64 unnecessary, slower on GPU)
- Normalize scores to [0.0, 1.0] early to prevent overflow
- Use sigmoid/tanh for bounded outputs


### 10.4 Extensibility

- Keep `HeuristicConfig` with reserved fields for future params
- Version shader code for compatibility
- Abstract move generation to support rule variants


### 10.5 Debugging

- Add optional logging for selection paths
- Track virtual loss collisions (should be <5%)
- Monitor batch fill rates (should be >80%)
- Add visualization tool for tree structure

***

## 11. References \& Further Reading

1. **Batch MCTS**: Cazenave (2017) - Batch Monte Carlo Tree Search
2. **Virtual Loss**: Chaslot et al. (2008) - Parallel Monte-Carlo Tree Search
3. **GPU Acceleration**: Rocki \& Suda (2011) - GPU Monte Carlo Tree Search
4. **AlphaZero**: Silver et al. (2017) - Mastering Chess and Shogi by Self-Play
5. **MCTS Survey**: Browne et al. (2012) - A Survey of Monte Carlo Tree Search Methods

***

## Appendix A: Default Configuration Values

```rust
pub const DEFAULT_BATCH_SIZE: usize = 256;
pub const DEFAULT_CPU_THREADS: usize = 8;
pub const DEFAULT_VIRTUAL_LOSS: u32 = 10;
pub const DEFAULT_EXPLORATION_CONSTANT: f32 = 1.414;  // sqrt(2)
pub const DEFAULT_BATCH_TIMEOUT_MS: u64 = 50;
pub const DEFAULT_ROLLOUT_DEPTH: u32 = 15;
pub const DEFAULT_MCTS_ITERATIONS: usize = 10000;

pub const PIECE_VALUES: [u32; 8] = [
    0,      // Empty
    100,    // Soldier
    300,    // Jester
    500,    // Commander
    300,    // Paladin
    300,    // Guard
    300,    // Dragon
    500,    // Balista
    10000,  // King
];
```


***

## Appendix B: Heuristic Tuning Guide

Start with these parameter ranges for grid search:


| Parameter | Min | Default | Max | Step |
| :-- | :-- | :-- | :-- | :-- |
| `capture_priority` | 50 | 70 | 90 | 10 |
| `threat_priority` | 40 | 60 | 80 | 10 |
| `tactical_depth` | 1 | 3 | 5 | 1 |
| `mobility_weight` | 5 | 15 | 30 | 5 |
| `control_weight` | 5 | 10 | 25 | 5 |
| `king_safety_weight` | 30 | 50 | 80 | 10 |

Test each configuration with 200-500 games to establish statistical significance.

***

**END OF SPECIFICATION**

This specification provides a complete roadmap for implementing GPU-accelerated MCTS for Keres. The modular design allows incremental development and testing, while the configurable heuristic system enables future optimization through self-play tournaments.
