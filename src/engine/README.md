# Arx Engine - MCTS GPU Engine

This module provides a GPU-accelerated Monte Carlo Tree Search (MCTS) engine for evaluating Arx board positions and
finding the best moves.

## Architecture

The engine consists of four main components:

### 0. Shared GPU Context (`gpu_context.rs`)

The shared GPU context manages GPU adapter and device selection, ensuring that all GPU-accelerated components use the
same GPU device. This prevents resource conflicts and improves efficiency.

Key features:

- Centralized GPU adapter selection with high-performance preference
- Shared device and queue for all GPU operations
- Comprehensive GPU debugging and logging
- Environment variable support for backend selection (`WGPU_BACKEND`)
- Detailed error messages for troubleshooting GPU issues
- Container-aware diagnostics

### 1. GPU Move Generation (`gpu_move_gen.rs`)

The move generation engine uses WebGPU compute shaders to efficiently generate all legal moves for a given board
position in parallel. Each square of the 9x9 board is processed by a separate thread in the shader.

Key features:

- Parallel processing of all board squares
- Full implementation of Arx movement rules in WGSL shader
- Returns encoded moves (16-bit format) that can be used by the MCTS engine

### 2. GPU Batch Simulation (`gpu_batch_sim.rs`)

The batch simulation engine processes multiple move applications and board evaluations in parallel on the GPU. This
significantly reduces CPU-GPU transfer overhead by batching operations.

Key features:

- Batch processing of up to 1024 simulations in parallel
- GPU-accelerated move application
- GPU-accelerated board evaluation
- Configurable batch sizes for optimal performance

### 3. MCTS Engine (`mod.rs` and `mcts.rs`)

The MCTS engine implements Monte Carlo Tree Search with proper tree structure, selection, expansion, simulation, and backpropagation:

**Tree Structure (`mcts.rs`)**:
- `MctsNode` represents board states with statistics (visit count, total score, win/loss/draw counts)
- Tree nodes store move that led to state, child nodes, and unexplored moves
- UCT (Upper Confidence Bound for Trees) for node selection

**Search Algorithm**:
- **Selection**: Traverse tree using UCT policy to find most promising leaf
- **Expansion**: Add new child node for an unexplored move
- **Simulation**: Random playout from expanded node to terminal state or max depth
- **Backpropagation**: Update statistics for all nodes along selection path

**Configuration**:
- `max_depth`: Maximum depth for random simulations (default: 50)
- `simulations_per_move`: Number of MCTS iterations (default: 1000)
- `exploration_constant`: UCT exploration parameter (default: 1.414 = sqrt(2))
- `gpu_batch_size`: GPU batch size for future optimization (default: 256)

**Features**:
- Statistics tracking (moves evaluated, simulations run)
- Best move selection via visit count (robust child)
- Handles force_unstack moves correctly
- CPU fallback for environments without GPU support
- Independent simulation using Game API for move generation and application

**Future GPU Integration**:
The current implementation uses CPU-based simulation for correctness. GPU batch simulation can be integrated by batching multiple MCTS iterations and using the GPU batch simulation engine for parallel rollouts.

## Performance Optimizations

The engine includes several optimizations to maximize GPU utilization and minimize latency:

1. **Batch Processing**: Multiple simulations are processed together on the GPU, reducing transfer overhead
2. **Multi-threading**: CPU work is parallelized using Rayon for better CPU utilization
3. **GPU Shaders**: Move application and board evaluation run entirely on GPU
4. **Configurable Batch Sizes**: Adjust batch size based on GPU capabilities

## Piece Values

The engine uses the following piece values for evaluation:

- Soldier: 1 point
- Jester: 3 points (like Bishop in chess)
- Commander: 5 points (like Rook in chess)
- Paladin: 3 points
- Guard: 3 points
- Dragon: 3 points (like Knight in chess)
- Ballista: 5 points
- King: 1000 points (invaluable)

## Usage

### Basic Usage with MCTS

```rust
use arx_engine::engine::{MctsEngine, EngineConfig};
use arx_engine::Game;

// Create engine with default configuration
let mut engine = MctsEngine::new()?;

// Get a board state
let game = Game::new();

// Find the best move using MCTS
let best_move = engine.find_best_move(&game.board)?;

// Get statistics
let stats = engine.get_statistics();
println!("Simulations run: {}", stats.simulations_run);

// Apply the move to the game
game.apply_move(best_move)?;
```

### Custom Configuration for MCTS

```rust
use arx_engine::engine::{MctsEngine, EngineConfig};

// Configure MCTS strength
let config = EngineConfig {
    max_depth: 50,                   // Maximum simulation depth (plies)
    simulations_per_move: 1000,      // Number of MCTS iterations
    exploration_constant: 1.414,     // UCT exploration constant (sqrt(2))
    gpu_batch_size: 512,             // GPU batch size (for future optimization)
    use_gpu_simulation: true,        // Enable GPU features when available
};

let mut engine = MctsEngine::with_config(config)?;
```

### Adjusting MCTS Strength

You can control the engine's strength by adjusting:

1. **`simulations_per_move`**: Number of MCTS iterations
    - Lower values (100-500): Faster but weaker play
    - Medium values (500-2000): Good balance
    - Higher values (2000+): Stronger but slower

2. **`max_depth`**: Maximum depth for random simulations
    - Lower values (20-30): Faster rollouts, less deep exploration
    - Medium values (30-60): Good balance
    - Higher values (60+): Deeper exploration, better terminal detection

3. **`exploration_constant`**: UCT exploration parameter
    - Lower values (0.5-1.0): More exploitative (favor best moves)
    - Standard value (1.414 = sqrt(2)): Balanced exploration/exploitation
    - Higher values (2.0+): More exploratory (try diverse moves)

4. **`gpu_batch_size`**: GPU batch size (reserved for future GPU optimization)
    - Current implementation uses CPU-based simulation
    - This parameter prepares for future GPU integration

### Statistics Tracking

The engine tracks various statistics during search:

```rust
// Get statistics after a search
let stats = engine.get_statistics();
println!("Total moves evaluated: {}", stats.total_moves_evaluated);
println!("Simulations run: {}", stats.simulations_run);
println!("GPU batches processed: {}", stats.gpu_batches_processed);
println!("CPU simulations: {}", stats.cpu_simulations);
println!("Avg moves/simulation: {:.2}", stats.avg_moves_per_simulation());

// Reset statistics
engine.reset_statistics();
```

## Board Encoding

The engine uses the same 7-bit piece encoding as the rest of the codebase:

```
Bit 6: Color (0=Black, 1=White)
Bits 5-3: Top piece code (000 if no top piece)
Bits 2-0: Bottom piece code
```

Special encoding for King: `0b_111000` (payload)

## Move Encoding

Moves are encoded in 16 bits:

```
Bit 15: force_unstack flag
Bit 14: unstackable flag
Bits 13-7: to position (0-80)
Bits 6-0: from position (0-80)
```

## Shader Implementation

### Move Generation Shader (`shaders/move_generation.wgsl`)

Implements:

- All piece movement patterns (Soldier, Jester, Commander, Paladin, Guard, Dragon, Ballista, King)
- Stacking rules
- Capture mechanics
- Move validation

Each invocation of the shader processes one square of the board, generating moves for the piece at that square if it
belongs to the current player.

### Batch Simulation Shader (`shaders/batch_simulation.wgsl`)

Implements:

- Move application logic (with unstacking support)
- Board evaluation based on piece values
- Batch processing of up to 1024 positions in parallel
- Validation of move legality

This shader processes multiple board positions simultaneously, applying moves and evaluating the resulting positions.

## Requirements

- WebGPU-compatible GPU (for GPU acceleration)
- Rust with async support
- Dependencies: `wgpu`, `bytemuck`, `pollster`, `rand`, `rayon`

## GPU Setup and Troubleshooting

### Environment Variables

- **`WGPU_BACKEND`**: Force a specific graphics backend
    - `VULKAN`: Force Vulkan backend (Linux, Windows)
    - `DX12`: Force DirectX 12 backend (Windows)
    - `METAL`: Force Metal backend (macOS)
    - `GL`: Force OpenGL backend (fallback)
    - Not set: Try all available backends (default)

Example:

```bash
WGPU_BACKEND=VULKAN cargo run --bin server
```

### Container GPU Access

When running in Docker containers, GPU access requires additional configuration:

#### Docker GPU Setup

1. **Install NVIDIA Container Toolkit** (for NVIDIA GPUs):
   ```bash
   # Ubuntu/Debian
   distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
   curl -s -L https://nvidia.github.io/nvidia-docker/gpgkey | sudo apt-key add -
   curl -s -L https://nvidia.github.io/nvidia-docker/$distribution/nvidia-docker.list | sudo tee /etc/apt/sources.list.d/nvidia-docker.list
   sudo apt-get update && sudo apt-get install -y nvidia-container-toolkit
   sudo systemctl restart docker
   ```

2. **Run container with GPU access**:
   ```bash
   # NVIDIA GPUs
   docker run --gpus all -it your-image

   # Specific GPU
   docker run --gpus device=0 -it your-image

   # AMD GPUs (with ROCm)
   docker run --device=/dev/kfd --device=/dev/dri -it your-image
   ```

3. **Verify GPU is accessible inside container**:
   ```bash
   docker run --gpus all -it your-image vulkaninfo
   ```

#### Common Container GPU Issues

**Issue: "No GPU adapters found" but `vulkaninfo` works**

This typically means WGPU cannot detect the GPU even though Vulkan is properly configured. Possible causes:

1. **Missing Vulkan ICD (Installable Client Driver)**:
    - In Alpine-based images, ensure `vulkan-loader` is installed
    - The Dockerfile already includes this, but verify it's not removed

2. **WGPU Backend Mismatch**:
    - Try forcing Vulkan backend: `WGPU_BACKEND=VULKAN`
    - Some backends may not work in all container configurations

3. **Permissions Issues**:
    - Ensure the container has access to GPU device files
    - Check `/dev/dri/` permissions inside container

4. **Runtime vs Build-time**:
    - GPU must be accessible at runtime, not just build-time
    - Ensure production deployment has GPU access configured

**Issue: Tests pass locally but fail in CI**

CI environments typically don't have GPU access. Tests are designed to gracefully skip GPU functionality:

```
Skipping test: GPU not available - Failed to find an appropriate GPU adapter
```

This is expected and normal for CI environments.

**Issue: Different GPU selected than expected**

The shared GPU context uses high-performance preference and logs which GPU is selected:

```
✓ Selected GPU: NVIDIA GeForce RTX 3080 (Vulkan)
```

Check the logs to see which GPU was selected.

### Debugging GPU Issues

Enable verbose GPU logging by running tests with output:

```bash
cargo test -- --nocapture
```

This will show:

- All available GPU adapters
- Which backend is being used
- Detailed error messages if GPU initialization fails
- Container-specific troubleshooting hints

Example output:

```
🔄 Initializing shared GPU context...
📊 Found 2 GPU adapter(s):
   [0] NVIDIA GeForce RTX 3080 - DiscreteGpu (Vulkan)
   [1] Intel(R) UHD Graphics 630 - IntegratedGpu (Vulkan)
✓ Selected GPU: NVIDIA GeForce RTX 3080 (Vulkan)
```

Or if GPU is not found:

```
❌ No GPU adapters found!
   Backends attempted: Backends(VULKAN | GL | METAL | DX12 | BROWSER_WEBGPU)
   This may indicate:
   - No GPU drivers installed
   - GPU not exposed to container (missing --device or --gpus flag)
   - Vulkan ICD not properly configured
   Suggestion: Check 'vulkaninfo' output and Docker GPU configuration
```

## Requirements

- WebGPU-compatible GPU (for GPU acceleration)
- Rust with async support
- Dependencies: `wgpu`, `bytemuck`, `pollster`, `rand`, `rayon`

## Testing

The engine includes tests that gracefully handle environments without GPU support:

```bash
cargo test --lib
```

Tests will skip GPU-dependent functionality if no adapter is available, making them CI-friendly.

## Performance

The GPU-accelerated engine provides significant performance benefits:

### Move Generation

- All squares are processed in parallel
- Typical move generation completes in microseconds

### Batch Simulation

- Process hundreds of simulations in parallel on GPU
- Dramatically reduces CPU-GPU transfer overhead
- Multi-threaded CPU processing for maximum utilization

### Expected Performance (with GPU)

- **Beginner level** (depth: 2, sims: 50): ~0.1-0.5s per move
- **Easy level** (depth: 3, sims: 100): ~0.5-1s per move
- **Medium level** (depth: 4, sims: 200): ~1-3s per move
- **Hard level** (depth: 5, sims: 300): ~3-5s per move
- **Expert level** (depth: 6, sims: 500): ~5-10s per move

*Performance varies based on GPU capability and board complexity.*

## Future Improvements

- Rust with async support
- Dependencies: `wgpu`, `bytemuck`, `pollster`, `rand`

## Testing

The engine includes tests that gracefully handle environments without GPU support:

```bash
cargo test --lib
```

Tests will skip GPU-dependent functionality if no adapter is available, making them CI-friendly.

## Performance

The GPU-accelerated move generation provides significant performance benefits:

- All squares are processed in parallel
- Typical move generation completes in microseconds
- Enables deeper search within reasonable time constraints

## Future Improvements

Potential enhancements:

- Full UCB1 tree search implementation
- Transposition tables for position caching
- Alpha-beta pruning integration
- Neural network evaluation
- Opening book support
