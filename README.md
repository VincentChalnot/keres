# Keres Engine

Keres Engine is a Rust implementation of the abstract strategy board game **Keres**, inspired by chess but featuring unique stacking mechanics. This project provides a command-line interface and terminal UI for playing, analyzing, and exporting/importing game states.

## Game Overview
Keres is played on a 9x9 board. Players control unique pieces, each with specific movement rules. Unlike chess, friendly pieces can be stacked to combine their movement abilities, creating new tactical possibilities. For a full description of the rules and piece movements, see [rules.md](./rules.md).

## Features
- Play Keres in the terminal
- Export and import board states using base64 encoding
- Display possible moves for any position
- Visualize the board with colored pieces and stacks

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

**Docker:**
```sh
# Run with GPU access
docker compose up
```

For detailed GPU setup and troubleshooting, see the [Engine Documentation](src/engine/README.md#gpu-setup-and-troubleshooting).

## Command Line Options
The CLI supports several subcommands:

- `play` : Launches the interactive terminal UI for playing Keres. (default command)
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

## Documentation
- [Game Rules](./rules.md): Full rules and piece movements
- [Piece Encoding](.github/instructions/binary_encoding.instructions.md): Details on board and piece encoding

## License
This project is licensed under the MIT License.

