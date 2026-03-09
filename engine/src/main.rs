use base64::{engine::general_purpose, Engine as _};
use clap::{Args, Parser, Subcommand};
use keres_engine::cli_rendering::get_game_hash;
use keres_engine::engine::search::root_search;
use keres_engine::engine::tree_recorder::TreeRecorder;
use keres_engine::engine::types::SearchConfig;
use keres_engine::{
    cli_rendering::display_stack, run_tui, Game, Position, BOARD_DIMENSION, BOARD_SIZE,
};
use keres_engine::moves::Move;
use std::time::Instant;

// musl's default allocator has severe lock contention under multi-threading
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Play(PlayArgs),
    ShowMoves(ShowMovesArgs),
    /// Request an engine move for a given board
    EngineMove(EngineMoveArgs),
    /// Run the search engine on a board loaded from a move list, output results as JSON
    DebugTree(DebugTreeArgs),
}

#[derive(Args)]
struct PlayArgs {
    /// Base64 encoded board data to import
    #[arg(long)]
    board: Option<String>,
}

#[derive(Args)]
struct ShowMovesArgs {
    /// Base64 encoded board data to import
    #[arg(long)]
    board: Option<String>,
    /// Position to show moves for
    coordinates: Option<String>,
}

#[derive(Args)]
struct EngineMoveArgs {
    /// Base64 encoded board data to import
    #[arg(long)]
    board: Option<String>,
}

#[derive(Args)]
struct DebugTreeArgs {
    /// Base64 encoded binary moves to replay before running the engine
    #[arg(long)]
    moves: Option<String>,
    /// Record and output the complete search tree as JSONL (default: PV-only)
    #[arg(long, default_value = "false")]
    full_tree: bool,
    /// Override maximum search depth (default: 4)
    #[arg(long)]
    max_depth: Option<usize>,
    /// Disable transposition table
    #[arg(long, default_value = "false")]
    no_tt: bool,
    /// Disable alpha-beta pruning (pure minimax)
    #[arg(long, default_value = "false")]
    no_ab: bool,
    /// Disable quiescence search
    #[arg(long, default_value = "false")]
    no_quiescence: bool,
    /// Disable killer move heuristic
    #[arg(long, default_value = "false")]
    no_killers: bool,
}

fn main() {
    let cli = Cli::parse();

    let board_data = match &cli.command {
        Some(Commands::Play(args)) => args.board.as_deref(),
        Some(Commands::ShowMoves(args)) => args.board.as_deref(),
        Some(Commands::EngineMove(args)) => args.board.as_deref(),
        Some(Commands::DebugTree(_)) => None, // DebugTree builds its own game from moves
        None => None,
    };

    // DebugTree has its own flow — handle it before building the default game
    if let Some(Commands::DebugTree(args)) = &cli.command {
        run_debug_tree(args);
        return;
    }

    let game = match create_game(board_data) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error creating game: {}", e);
            std::process::exit(1);
        }
    };

    match &cli.command {
        Some(Commands::ShowMoves(args)) => {
            if let Some(coordinates) = &args.coordinates {
                let position = parse_position(coordinates).unwrap_or_else(|err| {
                    eprintln!("Error parsing position: {}", err);
                    std::process::exit(1);
                });
                show_moves_for_position(&game, &position, true);
            } else {
                show_all_moves(&game);
            }
        }
        Some(Commands::EngineMove(_args)) => {
            match run_engine_move(&game) {
                Ok(mv) => {
                    let piece = game.board.get_piece(&mv.from);
                    let piece_string = if let Some(piece) = piece {
                        display_stack(piece)
                    } else {
                        "?".to_string()
                    };
                    let unstack_info = if mv.unstack {
                        "-"
                    } else {
                        ""
                    };
                    println!(
                        "Engine move: {}@{}-{}{}",
                        piece_string,
                        mv.from.to_string(),
                        mv.to.to_string(),
                        unstack_info,
                    );
                }
                Err(e) => {
                    eprintln!("Engine error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        _ => {
            match run_tui(Some(game)) {
                Ok(g) => {
                    println!("Game hash: {}", get_game_hash(&g));
                    println!("(use this to resume the game later on with the --board option)");
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            };
        }
    }

    fn show_all_moves(game: &Game) {
        for y in 0..BOARD_DIMENSION {
            for x in 0..BOARD_DIMENSION {
                let position = Position { x, y };
                show_moves_for_position(game, &position, false);
            }
        }
    }

    fn show_moves_for_position(game: &Game, position: &Position, display_empty_message: bool) {
        let moves = game.get_moves(position);
        if moves.is_empty() {
            if display_empty_message {
                println!("No moves available for position {}.", position.to_string());
            }
            return;
        }
        let piece = game.board.get_piece(position);
        let piece_string = if let Some(piece) = piece {
            display_stack(piece)
        } else {
            "?".to_string()
        };
        println!(
            "Available moves for {}@{}: ",
            piece_string,
            position.to_string()
        );
        for m in moves.iter() {
            print!(" - {}", m.to.to_string());
            if m.unstackable {
                if m.force_unstack {
                    print!(" (forced unstack)");
                } else {
                    print!(" (unstackable)");
                }
            }
            println!();
        }
    }

    fn parse_position(position: &str) -> Result<Position, String> {
        if position.len() != 2 {
            return Err("Invalid position format. Use e.g. 'B4'.".to_string());
        }
        // A1 is (0,8), I9 is (8,0)
        let x = match position.chars().nth(0).unwrap().to_ascii_uppercase() {
            'A'..='I' => position.chars().nth(0).unwrap() as usize - 'A' as usize,
            _ => return Err("Invalid column. Use letters A-I.".to_string()),
        };
        let y = match position.chars().nth(1).unwrap() {
            '1'..='9' => 8 - (position.chars().nth(1).unwrap() as usize - '1' as usize),
            _ => return Err("Invalid row. Use numbers 1-9.".to_string()),
        };

        Ok(Position { x, y })
    }

    fn create_game(board_str: Option<&str>) -> Result<Game, String> {
        match board_str {
            None => return Ok(Game::new()),
            Some("") => return Ok(Game::new()),
            Some(s) => {
                match general_purpose::STANDARD.decode(s) {
                    Ok(bytes) => {
                        // Convert bytes back to [u8; 81]
                        if bytes.len() != BOARD_SIZE + 1 {
                            return Err(format!(
                                "Invalid data length: expected {} bytes, got {}",
                                BOARD_SIZE + 1,
                                bytes.len()
                            ));
                        }

                        let mut board_data = [0; BOARD_SIZE + 2];
                        for (i, &byte) in bytes.iter().enumerate() {
                            board_data[i] = byte;
                        }

                        Game::from_binary(board_data)
                    }
                    Err(e) => Err(format!("Failed to decode base64 string: {}", e)),
                }
            }
        }
    }

    // Run the engine move logic
    fn run_engine_move(game: &Game) -> Result<Move, String> {
        use keres_engine::engine::constants::MAX_DEPTH;
        let config = SearchConfig {
            max_depth: MAX_DEPTH,
            ..Default::default()
        };
        let result = root_search(game, &config, &[], None);
        result.best_move.ok_or_else(|| "No moves available".to_string())
    }

    fn run_debug_tree(args: &DebugTreeArgs) {
        // ── Decode moves and reconstruct game ────────────────────────────────
        let game = match &args.moves {
            Some(b64) => {
                let move_bytes = match general_purpose::STANDARD.decode(b64) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("Failed to decode base64 move sequence: {}", e);
                        std::process::exit(1);
                    }
                };
                match Game::from_moves(&move_bytes) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("Failed to replay moves: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            None => Game::new(),
        };

        // ── Build search config ──────────────────────────────────────────────
        let config = SearchConfig {
            use_tt: !args.no_tt,
            use_alpha_beta: !args.no_ab,
            use_quiescence: !args.no_quiescence,
            use_killers: !args.no_killers,
            max_depth: args.max_depth.unwrap_or(keres_engine::engine::constants::MAX_DEPTH),
        };

        // ── Tree recorder (writes JSONL to stdout) ───────────────────────────
        let recorder: Option<TreeRecorder> = if args.full_tree {
            Some(TreeRecorder::stdout())
        } else {
            None
        };

        // ── Run search ───────────────────────────────────────────────────────
        let start = Instant::now();
        let result = root_search(&game, &config, &[], recorder.as_ref());
        let elapsed = start.elapsed();

        if let Some(r) = &recorder {
            r.flush();
        }

        // ── Print human-readable stats to stderr ─────────────────────────────
        eprintln!("Search complete in {:.2?}", elapsed);
        eprintln!(
            "Best move: {}",
            result
                .best_move
                .map(|m| m.to_string())
                .unwrap_or_else(|| "(none)".to_string())
        );
        eprintln!("Best score: {}", result.best_score);
        eprint!("PV: ");
        for mv in &result.pv {
            eprint!("{} ", mv.to_string());
        }
        eprintln!();
    }
}
