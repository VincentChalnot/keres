# Arx Engine

Arx Engine is a Rust implementation of the abstract strategy board game **Arx**, inspired by chess but featuring unique stacking mechanics. This project provides a command-line interface and terminal UI for playing, analyzing, and exporting/importing game states.

## Game Overview
Arx is played on a 9x9 board. Players control unique pieces, each with specific movement rules. Unlike chess, friendly pieces can be stacked to combine their movement abilities, creating new tactical possibilities. For a full description of the rules and piece movements, see [rules.md](./rules.md).

## Features
- Play Arx in the terminal
- Export and import board states using base64 encoding
- Display possible moves for any position
- Visualize the board with colored pieces and stacks
- **GPU-accelerated MCTS engine** for computer opponent (see [Engine Documentation](src/engine/README.md))

## Building and Running

Ensure you have [Rust](https://www.rust-lang.org/tools/install) installed.

```sh
# Build the project
cargo build --release

# Run the game (default: interactive TUI)
cargo run --release
```

### GPU Support

The engine uses GPU acceleration for move generation and MCTS simulations. For optimal performance:

**Local Development:**
```sh
# Check GPU availability
./check_gpu.sh

# Force specific GPU backend if needed
WGPU_BACKEND=VULKAN cargo run --release
```

**Docker (AMD/Intel GPU):**
```sh
# Run with GPU access
docker compose up
```

**Docker (NVIDIA GPU):**
```sh
# Requires nvidia-container-toolkit
docker compose -f compose.yaml -f compose.gpu-nvidia.yaml up
```

For detailed GPU setup and troubleshooting, see the [Engine Documentation](src/engine/README.md#gpu-setup-and-troubleshooting).

## Command Line Options
The CLI supports several subcommands:

- `play` : Launches the interactive terminal UI for playing Arx. (default command)
- `export` : Prints the current board state as a base64 string.
- `import <data>` : Loads a board state from a base64 string.
- `show-moves [coordinates]` : Displays possible moves for a given position (e.g., `E2`).

Example usage:
```sh
# Start the interactive game
cargo run --release

# Import a board state
cargo run --release -- import "<base64_data>"

# Show possible moves for position E2
cargo run --release -- show-moves E2
```

## AI Engines

The project includes two different AI engines for computer play:

### Minimax Engine

A classical minimax search with alpha-beta pruning, featuring:
- Multiple pre-configured variants (Aggressive, Defensive, Balanced, Tactical, Positional)
- Piece-square tables for positional evaluation
- Transposition tables with Zobrist hashing
- Quiescence search for tactical stability
- Configurable search depth and evaluation weights

```sh
# Test different engine variants
cargo run --example quick_variant_test
```

### MCTS Engine

A GPU-accelerated Monte Carlo Tree Search engine:
- WebGPU compute shader for parallel move generation
- Configurable search depth and simulation count
- Piece value-based evaluation
- Adjustable difficulty levels
- Independent implementation (doesn't depend on board.rs/game.rs)

```sh
# Run the MCTS engine demo
cargo run --example engine_demo
```

### Engine Tournament System

Compare different engine variants and configurations:

```sh
# Run quick variant test
cargo run --example quick_variant_test

# Run full tournament
cargo run --example engine_tournament
```

For details on the tournament system, see [Tournament Documentation](./TOURNAMENT.md).
For engine implementation details, see [Engine Documentation](src/engine/README.md).

## Documentation
- [Game Rules](./rules.md): Full rules and piece movements
- [Piece Encoding](.github/instructions/piece_encoding.instructions.md): Details on board and piece encoding
- [Engine Documentation](src/engine/README.md): AI engine implementation details
- [Tournament System](./TOURNAMENT.md): Running matches between engine variants

## License
This project is licensed under the MIT License.

