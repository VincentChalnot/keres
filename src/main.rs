use base64::{engine::general_purpose, Engine as _};
use clap::{Args, Parser, Subcommand};
use keres_engine::cli_rendering::get_board_hash;
use keres_engine::{
    cli_rendering::display_stack, run_tui, Game, Position, BOARD_DIMENSION, BOARD_SIZE,
};
use keres_engine::game::Move;
use keres_engine::engine::StageConfig;

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
    /// Stage 1 search depth (default: 4)
    #[arg(long, default_value = "4")]
    stage_1_depth: i32,
    /// Number of top moves to find (MultiPV passes, default: 3)
    #[arg(long, default_value = "3")]
    top_moves: usize,
    /// Target number of distinct PV lines to collect across all passes (default: 5)
    #[arg(long, default_value = "5")]
    expected_leaves: usize,
    /// Hard cap on the number of MultiPV passes regardless of expected-leaves (default: 3)
    #[arg(long, default_value = "3")]
    max_passes: usize,
    /// Disable transposition table
    #[arg(long, default_value = "false")]
    no_tt: bool,
    /// Disable alpha-beta pruning (pure minimax)
    #[arg(long, default_value = "false")]
    no_alpha_beta: bool,
    /// Disable MVV-LVA + history move ordering
    #[arg(long, default_value = "false")]
    no_move_ordering: bool,
    /// Disable killer move heuristic
    #[arg(long, default_value = "false")]
    no_killers: bool,
    /// Override number of threads (default: num_cpus)
    #[arg(long)]
    threads: Option<usize>,

    // ── Stage 2 overrides ────────────────────────────────────────────────────
    /// Override Stage 2 search depth (default: 6)
    #[arg(long)]
    s2_depth: Option<u8>,
    /// Disable null move pruning in Stage 2
    #[arg(long, default_value = "false")]
    s2_no_null_move: bool,
    /// Disable LMR in Stage 2
    #[arg(long, default_value = "false")]
    s2_no_lmr: bool,
    /// Disable selective extensions in Stage 2
    #[arg(long, default_value = "false")]
    s2_no_extensions: bool,
    /// Disable TT in Stage 2
    #[arg(long, default_value = "false")]
    s2_no_tt: bool,
    /// Enable TreeRecorder for Stage 2, output JSONL to stderr
    #[arg(long, default_value = "false")]
    s2_debug_tree: bool,
    /// Print the final resolved StageConfig for both stages as JSON before running
    #[arg(long, default_value = "false")]
    config_dump: bool,
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
                    println!("Game hash: {}", get_board_hash(&g.board));
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
        use keres_engine::engine::{EngineConfig, Engine};
        let config = EngineConfig::default();
        let engine = Engine::new(config);
        let (mv, _stats) = engine
            .find_move(&game.board)
            .map_err(|e| format!("Engine failed to find move: {}", e))?;
        Ok(mv)
    }

    fn run_debug_tree(args: &DebugTreeArgs) {
        use keres_engine::engine::SearchConfig;
        use keres_engine::engine::stage1;
        use std::time::Instant;

        // Build the game state by replaying the encoded moves
        let mut game = Game::new();
        if let Some(encoded) = &args.moves {
            let bytes = match general_purpose::STANDARD.decode(encoded) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Failed to decode base64 moves: {}", e);
                    std::process::exit(1);
                }
            };
            if bytes.len() % 2 != 0 {
                eprintln!("Move data must be an even number of bytes");
                std::process::exit(1);
            }
            for chunk in bytes.chunks_exact(2) {
                let mv = Move::from_u16(u16::from_le_bytes([chunk[0], chunk[1]]));
                if let Err(e) = game.apply_move(mv) {
                    eprintln!("Failed to apply move {}: {}", mv.to_string(), e);
                    std::process::exit(1);
                }
            }
        }

        eprintln!("Board after replay ({} to move):",
            if game.board.is_white_to_move() { "white" } else { "black" });

        if game.board.is_game_over() {
            eprintln!("Position is terminal — no search to run");
            std::process::exit(1);
        }

        // Build Stage 1 StageConfig from CLI args
        let mut s1_config = StageConfig::stage1();
        s1_config.depth = args.stage_1_depth as u8;
        s1_config.max_passes = args.max_passes as u8;
        s1_config.expected_leaves = args.expected_leaves;
        if args.no_tt { s1_config.transposition_table = false; }
        if args.no_alpha_beta { s1_config.alpha_beta = false; }
        if args.no_move_ordering { s1_config.move_ordering = false; }
        if args.no_killers { s1_config.killer_moves = false; }
        s1_config.tree_recorder = true;

        // Build Stage 2 StageConfig from CLI args
        let mut s2_config = StageConfig::stage2();
        if let Some(d) = args.s2_depth { s2_config.depth = d; }
        if args.s2_no_null_move { s2_config.null_move_pruning = false; }
        if args.s2_no_lmr { s2_config.lmr = false; }
        if args.s2_no_extensions { s2_config.selective_extensions = false; }
        if args.s2_no_tt { s2_config.transposition_table = false; }
        if args.s2_debug_tree { s2_config.tree_recorder = true; }

        // Config dump
        if args.config_dump {
            eprintln!("Stage 1 config: {}", serde_json::to_string_pretty(&s1_config).unwrap());
            eprintln!("Stage 2 config: {}", serde_json::to_string_pretty(&s2_config).unwrap());
        }

        let threads = args.threads.unwrap_or(num_cpus::get().max(1));

        // Also keep the legacy SearchConfig path for debug tree output
        let cfg = SearchConfig {
            depth: args.stage_1_depth,
            top_moves: args.top_moves,
            expected_leaves: args.expected_leaves,
            max_passes: args.max_passes,
            no_tt: args.no_tt,
            no_alpha_beta: args.no_alpha_beta,
            no_move_ordering: args.no_move_ordering,
            no_killers: args.no_killers,
            debug_tree: true,
            threads,
        };

        let timer = Instant::now();
        let (result, stats) = stage1::stage1_search(&game.board, &cfg);
        let s1_elapsed = timer.elapsed();

        // Stage 2
        let (final_result, s2_nodes) = if result.top_moves.len() <= 1
            || stage1::all_same_root_move(&result.top_moves)
        {
            eprintln!("Stage 2 skipped (all candidates share the same root move)");
            (result.clone(), 0u64)
        } else {
            let (_, _, tt) = stage1::stage1_search_with_config(&game.board, &s1_config, threads);
            let s2_engine = stage1::Stage2Engine::new(s2_config, tt);
            let s2_result = s2_engine.search(&game.board, &result.top_moves);
            let s2_n = s2_result.nodes_visited;
            eprintln!("Stage 2 refined {} candidates, best: {} (score={})",
                result.top_moves.len(), s2_result.best_move.to_string(), s2_result.score);
            (s2_result, s2_n)
        };

        let elapsed = timer.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();

        let nodes = result.nodes_visited + s2_nodes;
        let nps = if elapsed_secs > 0.0 { nodes as f64 / elapsed_secs } else { 0.0 };

        eprintln!("Best move: {}", final_result.best_move.to_string());
        eprintln!("Score: {}", final_result.score);
        eprintln!("Depth: {}", result.depth);
        eprintln!("Nodes visited: {} (S1: {}, S2: {})", nodes, result.nodes_visited, s2_nodes);
        eprintln!("TT hit rate: {:.1}% ({} hits / {} probes)",
            stats.tt_hit_rate(), stats.tt_hits, stats.tt_probes);
        eprintln!("Time: {:.3}s (S1: {:.3}s)", elapsed_secs, s1_elapsed.as_secs_f64());
        eprintln!("Speed: {:.0} nodes/sec", nps);
        eprintln!("Top moves ({}):", result.top_moves.len());

        for (i, pv) in result.top_moves.iter().enumerate() {
            let pv_str: Vec<String> = pv.moves.iter().map(|m| m.to_string()).collect();
            eprintln!("  {}. {} (score={}) PV: {}",
                i + 1, pv.root_move.to_string(), pv.score, pv_str.join(" → "));
        }

        // Output JSONL debug tree to stdout
        let debug_tree = keres_engine::engine::search::build_debug_tree(
            &game.board, &result.top_moves,
        );
        dump_debug_tree_jsonl(&debug_tree);
    }
}

/// Recursively dump the debug tree as JSONL, each node with parent_id
use keres_engine::engine::DebugTree;
fn dump_debug_tree_jsonl(tree: &DebugTree) {
    fn walk(node: &DebugTree, parent_id: Option<usize>) {
        let mut obj = serde_json::json!({
            "node_id": node.node_id,
            "parent_id": parent_id,
            "score": node.score,
            "stage1_score": node.stage1_score,
            "white_to_move": node.white_to_move,
            "is_terminal": node.is_terminal,
        });
        if let Some(action) = &node.action {
            obj["action"] = serde_json::json!(action);
        }
        if let Some(hash) = node.hash {
            obj["hash"] = serde_json::json!(format!("{:#018x}", hash));
        }
        println!("{}", obj);
        for child in node.children.iter() {
            walk(child, Some(node.node_id));
        }
    }
    walk(tree, None);
}
