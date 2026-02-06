use base64::{engine::general_purpose, Engine as _};
use clap::{Args, Parser, Subcommand};
use keres_engine::cli_rendering::get_board_hash;
use keres_engine::{
    cli_rendering::display_stack, run_tui, Game, Position, BOARD_DIMENSION, BOARD_SIZE,
};
use keres_engine::game::Move;

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

fn main() {
    let cli = Cli::parse();

    let board_data = match &cli.command {
        Some(Commands::Play(args)) => args.board.as_deref(),
        Some(Commands::ShowMoves(args)) => args.board.as_deref(),
        Some(Commands::EngineMove(args)) => args.board.as_deref(),
        None => None,
    };

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

    // Add this function to run the engine move logic
    fn run_engine_move(game: &Game) -> Result<Move, String> {
        use keres_engine::engine::{EngineConfig, MctsEngine};
        let mcts_config = EngineConfig::default();
        let mcts_engine = MctsEngine::with_config(mcts_config)
            .map_err(|e| format!("Failed to initialize MCTS engine: {}", e))?;
        let (mv, _stats) = mcts_engine
            .find_move(&game.board)
            .map_err(|e| format!("Engine failed to find move: {}", e))?;
        Ok(mv)
    }
}
