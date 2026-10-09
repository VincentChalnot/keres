use base64::{engine::general_purpose, Engine as _};
use clap::{Args, Parser, Subcommand, ValueEnum};
use keres_engine::engine::search::root_search;
use keres_engine::engine::tree_recorder::{TreeFormat, TreeRecorder};
use keres_engine::engine::types::SearchConfig;
use keres_engine::moves::Move;
use keres_engine::{cli_rendering::display_stack, Game, Position, BOARD_DIMENSION, BOARD_SIZE};
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
    ShowMoves(ShowMovesArgs),
    /// Request an engine move for a given board
    EngineMove(EngineMoveArgs),
    /// Run the search engine on a board loaded from a move list, output results as JSON
    DebugTree(DebugTreeArgs),
    /// Explore the engine's best openings: search a position, keep every move
    /// scoring close to the best one, search again after each of those, and
    /// repeat for a number of plies. Prints the resulting tree on stdout.
    Openings(OpeningsArgs),
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
    /// Record and output the complete search tree on stdout (default: PV-only)
    #[arg(long, default_value = "false")]
    full_tree: bool,
    /// Encoding of the --full-tree output: JSONL, or packed 13-byte binary
    /// records (see `engine::tree_recorder`)
    #[arg(long, value_enum, default_value_t = TreeFormatArg::Jsonl)]
    tree_format: TreeFormatArg,
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

#[derive(Clone, Copy, ValueEnum)]
enum TreeFormatArg {
    Jsonl,
    Binary,
}

#[derive(Args)]
struct OpeningsArgs {
    /// Base64 encoded binary moves to replay before exploring (default: the
    /// initial position)
    #[arg(long)]
    moves: Option<String>,
    /// Search depth used for every explored position (default: 4)
    #[arg(long)]
    max_depth: Option<usize>,
    /// Number of plies to expand: 1 lists the candidates of the start
    /// position, each extra ply searches again after every candidate
    #[arg(long, default_value_t = 2)]
    plies: usize,
    /// Keep moves scoring within this many engine units of the best move
    #[arg(long, default_value_t = 10)]
    margin: i32,
    /// Keep at most this many moves per position (the best ones)
    #[arg(long, default_value_t = 4)]
    max_branch: usize,
}

fn main() {
    let cli = Cli::parse();

    let board_data = match &cli.command {
        Some(Commands::ShowMoves(args)) => args.board.as_deref(),
        Some(Commands::EngineMove(args)) => args.board.as_deref(),
        Some(Commands::DebugTree(_) | Commands::Openings(_)) => None, // built from moves
        None => None,
    };

    // DebugTree and Openings have their own flow — handle them before
    // building the default game
    match &cli.command {
        Some(Commands::DebugTree(args)) => return run_debug_tree(args),
        Some(Commands::Openings(args)) => return run_openings(args),
        _ => {}
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
        Some(Commands::EngineMove(_args)) => match run_engine_move(&game) {
            Ok(mv) => {
                let piece = game.board.get_piece(&mv.from);
                let piece_string = if let Some(piece) = piece {
                    display_stack(piece)
                } else {
                    "?".to_string()
                };
                let unstack_info = if mv.unstack { "-" } else { "" };
                println!(
                    "Engine move: {}@{}-{}{}",
                    piece_string, mv.from, mv.to, unstack_info,
                );
            }
            Err(e) => {
                eprintln!("Engine error: {}", e);
                std::process::exit(1);
            }
        },
        _ => {
            eprintln!("No command given.");
            eprintln!(
                "Available: show-moves, engine-move, debug-tree, openings (run `keres --help`)"
            );
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
                println!("No moves available for position {}.", position);
            }
            return;
        }
        let piece = game.board.get_piece(position);
        let piece_string = if let Some(piece) = piece {
            display_stack(piece)
        } else {
            "?".to_string()
        };
        println!("Available moves for {}@{}: ", piece_string, position);
        for m in moves.iter() {
            print!(" - {}", m.to);
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
        let mut chars = position.chars();
        let (Some(col), Some(row), None) = (chars.next(), chars.next(), chars.next()) else {
            return Err("Invalid position format. Use e.g. 'B4'.".to_string());
        };
        // A1 is (0,8), I9 is (8,0)
        let x = match col.to_ascii_uppercase() {
            c @ 'A'..='I' => c as usize - 'A' as usize,
            _ => return Err("Invalid column. Use letters A-I.".to_string()),
        };
        let y = match row {
            '1'..='9' => 8 - (row as usize - '1' as usize),
            _ => return Err("Invalid row. Use numbers 1-9.".to_string()),
        };

        Ok(Position { x, y })
    }

    fn create_game(board_str: Option<&str>) -> Result<Game, String> {
        match board_str {
            None | Some("") => Ok(Game::new()),
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
                        board_data[..bytes.len()].copy_from_slice(&bytes);

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
        result
            .best_move
            .ok_or_else(|| "No moves available".to_string())
    }

    /// Replay an optional base64 move list from the initial position,
    /// returning the game and its position history. Exits on bad input.
    fn replay_moves_arg(moves: Option<&str>) -> (Game, Vec<u64>) {
        let Some(b64) = moves else {
            return (Game::new(), Vec::new());
        };
        let move_bytes = general_purpose::STANDARD.decode(b64).unwrap_or_else(|e| {
            eprintln!("Failed to decode base64 move sequence: {}", e);
            std::process::exit(1);
        });
        Game::replay_moves(&move_bytes).unwrap_or_else(|e| {
            eprintln!("Failed to replay moves: {}", e);
            std::process::exit(1);
        })
    }

    fn run_openings(args: &OpeningsArgs) {
        let (game, mut history) = replay_moves_arg(args.moves.as_deref());
        let config = SearchConfig {
            max_depth: args
                .max_depth
                .unwrap_or(keres_engine::engine::constants::MAX_DEPTH),
            ..Default::default()
        };
        let start = Instant::now();
        let searches = expand_openings(&game, &mut history, &config, args, 0);
        eprintln!("{searches} searches in {:.2?}", start.elapsed());
    }

    /// Search `game`, print its candidate moves (indented by `ply`, scores
    /// from White's point of view), and recurse into each one until
    /// `args.plies` is reached. Returns the number of searches run.
    fn expand_openings(
        game: &Game,
        history: &mut Vec<u64>,
        config: &SearchConfig,
        args: &OpeningsArgs,
        ply: usize,
    ) -> usize {
        let result = root_search(game, config, history, None);
        let mut searches = 1;
        let Some(&(_, best)) = result.root_scores.first() else {
            return searches;
        };
        let sign = if game.is_white_to_move() { 1 } else { -1 };
        let candidates = result
            .root_scores
            .iter()
            .take_while(|&&(_, score)| best - score <= args.margin)
            .take(args.max_branch);
        for &(mv, score) in candidates {
            println!("{:indent$}{mv} {:+}", "", sign * score, indent = ply * 2);
            if ply + 1 >= args.plies {
                continue;
            }
            let mut child = game.clone();
            child.make(&mv);
            if child.is_game_over() {
                continue;
            }
            history.push(game.board_hash());
            searches += expand_openings(&child, history, config, args, ply + 1);
            history.pop();
        }
        searches
    }

    fn run_debug_tree(args: &DebugTreeArgs) {
        // ── Decode moves and reconstruct game ────────────────────────────────
        let (game, _) = replay_moves_arg(args.moves.as_deref());

        // ── Build search config ──────────────────────────────────────────────
        let config = SearchConfig {
            use_tt: !args.no_tt,
            use_alpha_beta: !args.no_ab,
            use_quiescence: !args.no_quiescence,
            use_killers: !args.no_killers,
            max_depth: args
                .max_depth
                .unwrap_or(keres_engine::engine::constants::MAX_DEPTH),
            ..Default::default()
        };

        // ── Tree recorder (streams to stdout) ────────────────────────────────
        let recorder: Option<TreeRecorder> = if args.full_tree {
            Some(TreeRecorder::stdout(match args.tree_format {
                TreeFormatArg::Jsonl => TreeFormat::Jsonl,
                TreeFormatArg::Binary => TreeFormat::Binary,
            }))
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
            eprint!("{} ", mv);
        }
        eprintln!();
        eprintln!("Root moves (best first):");
        for (mv, score) in &result.root_scores {
            eprintln!("  {mv} {score:+}");
        }
    }
}
