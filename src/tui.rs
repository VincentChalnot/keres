use crate::{cli_rendering::piece_to_char, Color, Game, Piece, Position, BOARD_DIMENSION};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color as RatatuiColor, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameState {
    SelectingPiece,
    SelectingTarget {
        from: Position,
    },
    ConfirmUnstack {
        from: Position,
        to: Position,
        unstack: bool,
    },
    GameOver {
        winner: Color,
    },
}

pub struct App {
    game: Game,
    cursor_position: Position,
    game_state: GameState,
    highlighted_moves: Vec<Position>,
}

impl App {
    pub fn new() -> Self {
        let game = Game::new();
        App::from_game(game)
    }

    pub fn from_game(game: Game) -> Self {
        let game_state = if game.board.is_game_over() {
            // Determine winner: if white to move but game is over, black won (and vice versa)
            let winner = if game.board.is_white_to_move() {
                Color::Black
            } else {
                Color::White
            };
            GameState::GameOver { winner }
        } else {
            GameState::SelectingPiece
        };

        App {
            game,
            cursor_position: Position::new(0, 0),
            game_state,
            highlighted_moves: Vec::new(),
        }
    }

    pub fn move_cursor(&mut self, dx: isize, dy: isize) {
        // Don't allow cursor movement when game is over
        if matches!(self.game_state, GameState::GameOver { .. }) {
            return;
        }

        if let Some(new_pos) = self.cursor_position.get_new(dx, dy) {
            self.cursor_position = new_pos;
            self.update_highlights();
        }
    }

    /// Applies a move, updates game state and highlights, handling game over.
    fn apply_move_and_update_state(&mut self, game_move: crate::Move) -> Result<(), String> {
        self.game.apply_move(game_move)?;
        if self.game.board.is_game_over() {
            let winner = if self.game.board.is_white_to_move() {
                Color::Black
            } else {
                Color::White
            };
            self.game_state = GameState::GameOver { winner };
            self.highlighted_moves.clear();
        } else {
            self.game_state = GameState::SelectingPiece;
            self.highlighted_moves.clear();
        }
        Ok(())
    }

    pub fn handle_enter(&mut self) -> Result<(), String> {
        match self.game_state {
            GameState::SelectingPiece => {
                let moves = self.game.get_moves(&self.cursor_position);
                if !moves.is_empty() {
                    self.game_state = GameState::SelectingTarget {
                        from: self.cursor_position,
                    };
                    self.highlighted_moves = moves.iter().map(|m| m.to).collect();
                }
            }
            GameState::SelectingTarget { from } => {
                let moves = self.game.get_moves(&from);
                if let Some(potential_move) = moves.iter().find(|m| m.to == self.cursor_position) {
                    // If both stack and unstack are possible, show dialog
                    if potential_move.unstackable && !potential_move.force_unstack {
                        // Default: full stack
                        self.game_state = GameState::ConfirmUnstack {
                            from,
                            to: self.cursor_position,
                            unstack: false,
                        };
                        // Highlight both options (for UI)
                        self.highlighted_moves = vec![self.cursor_position];
                        return Ok(());
                    }
                    // If only unstack is allowed, or only stack is allowed, apply directly
                    let unstack = potential_move.force_unstack;
                    let game_move = potential_move.to_move(unstack);
                    return self.apply_move_and_update_state(game_move);
                } else {
                    self.game_state = GameState::SelectingPiece;
                    self.highlighted_moves.clear();
                }
            }
            GameState::ConfirmUnstack { from, to, unstack } => {
                // Enter = full stack, 'u' = unstack
                let moves = self.game.get_moves(&from);
                if let Some(potential_move) = moves.iter().find(|m| m.to == to) {
                    let game_move = potential_move.to_move(unstack);
                    return self.apply_move_and_update_state(game_move);
                } else {
                    self.game_state = GameState::SelectingPiece;
                    self.highlighted_moves.clear();
                }
            }
            GameState::GameOver { .. } => {}
        }
        Ok(())
    }

    pub fn handle_unstack_confirm(&mut self) -> Result<(), String> {
        // Only valid in ConfirmUnstack state
        if let GameState::ConfirmUnstack { from, to, .. } = self.game_state {
            let moves = self.game.get_moves(&from);
            if let Some(potential_move) = moves.iter().find(|m| m.to == to) {
                let game_move = potential_move.to_move(true); // unstack
                return self.apply_move_and_update_state(game_move);
            } else {
                self.game_state = GameState::SelectingPiece;
                self.highlighted_moves.clear();
            }
        }
        Ok(())
    }

    pub fn handle_escape(&mut self) {
        match self.game_state {
            GameState::SelectingPiece => {
                // Nothing to cancel
            }
            GameState::SelectingTarget { .. } => {
                self.game_state = GameState::SelectingPiece;
                self.highlighted_moves.clear();
            }
            GameState::ConfirmUnstack { from, .. } => {
                self.game_state = GameState::SelectingTarget { from };
                self.highlighted_moves.clear();
            }
            GameState::GameOver { .. } => {
                // Game is over, do nothing
            }
        }
    }

    fn update_highlights(&mut self) {
        match self.game_state {
            GameState::SelectingPiece => {
                let moves = self.game.get_moves(&self.cursor_position);
                self.highlighted_moves = moves.iter().map(|m| m.to).collect();
            }
            GameState::SelectingTarget { .. } => {
                // Keep existing highlights
            }
            GameState::ConfirmUnstack { to, .. } => {
                // Highlight the "confirm" and cancel positions
                self.highlighted_moves = vec![to];
            }
            GameState::GameOver { .. } => {
                // Game is over, clear highlights
                self.highlighted_moves.clear();
            }
        }
    }

    fn get_piece_display(&self, piece: &Piece) -> String {
        let mut output = String::new();

        if let Some(ref top_piece) = piece.top {
            output.push_str(&piece_to_char(top_piece));
            output.push('+');
            output.push_str(&piece_to_char(&piece.bottom));
        } else {
            output.push_str(" ");
            output.push_str(&piece_to_char(&piece.bottom));
            output.push_str(" ");
        }

        output
    }
}

pub fn run_tui(game: Option<Game>) -> Result<Game, Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = if let Some(game) = game {
        App::from_game(game)
    } else {
        App::new()
    };

    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(app.game)
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Esc => app.handle_escape(),
                    KeyCode::Enter => {
                        if let Err(_e) = app.handle_enter() {
                            // For now, just ignore move errors
                        }
                    }
                    KeyCode::Char('u') => {
                        if let GameState::ConfirmUnstack { .. } = app.game_state {
                            if let Err(_e) = app.handle_unstack_confirm() {
                                // For now, just ignore unstack errors
                            }
                        }
                    }
                    KeyCode::Up => app.move_cursor(0, -1),
                    KeyCode::Down => app.move_cursor(0, 1),
                    KeyCode::Left => app.move_cursor(-1, 0),
                    KeyCode::Right => app.move_cursor(1, 0),
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(20),   // Board
            Constraint::Length(5), // Instructions
        ])
        .split(f.area());

    // Title
    let title = match app.game_state {
        GameState::SelectingPiece => {
            format!(
                "{} to move - Select a piece",
                if app.game.board.is_white_to_move() {
                    "White"
                } else {
                    "Black"
                }
            )
        }
        GameState::SelectingTarget { .. } => "Select target position".to_string(),
        GameState::ConfirmUnstack { .. } => "Confirm Unstack/Stack".to_string(),
        GameState::GameOver { winner } => {
            format!(
                "🎉 GAME OVER - {} WINS! 🎉",
                match winner {
                    Color::White => "WHITE",
                    Color::Black => "BLACK",
                }
            )
        }
    };

    let title_paragraph = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL).title("Arx Game"))
        .alignment(Alignment::Center);
    f.render_widget(title_paragraph, chunks[0]);

    // Board
    render_board(f, app, chunks[1]);

    // Instructions
    let instructions = match app.game_state {
        GameState::GameOver { .. } => {
            vec![
                Line::from(vec![
                    Span::styled("Game Over!", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Press "),
                    Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to quit"),
                ]),
                Line::from(""),
            ]
        }
        GameState::ConfirmUnstack { .. } => {
            vec![
                Line::from(vec![Span::styled(
                    "Stack/Unstack Choice",
                    Style::default().add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![
                    Span::raw("Press "),
                    Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to move the full stack (default)"),
                ]),
                Line::from(vec![
                    Span::raw("Press "),
                    Span::styled("u", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to move only the top piece (unstack)"),
                ]),
                Line::from(vec![
                    Span::raw("Press "),
                    Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to cancel"),
                ]),
            ]
        }
        _ => {
            vec![
                Line::from(vec![
                    Span::raw("Use "),
                    Span::styled("Arrow Keys", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to move cursor, "),
                    Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to select"),
                ]),
                Line::from(vec![
                    Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to cancel selection, "),
                    Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" to quit"),
                ]),
            ]
        }
    };

    let instructions_paragraph = Paragraph::new(instructions)
        .block(Block::default().borders(Borders::ALL).title("Controls"))
        .alignment(Alignment::Center);
    f.render_widget(instructions_paragraph, chunks[2]);
}

fn render_board(f: &mut Frame, app: &App, area: Rect) {
    let board = app.game.board;

    // Calculate the area for the actual board (leaving space for borders and labels)
    let board_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width - 2,
        height: area.height - 2,
    };

    // Create the outer border
    let block = Block::default().borders(Borders::ALL).title("Board");
    f.render_widget(block, area);

    // Draw the board content manually using text with box drawing characters
    let mut board_lines = Vec::new();

    // Header line with column labels
    let header = String::from("     A   B   C   D   E   F   G   H   I");
    board_lines.push(Line::from(Span::styled(
        header,
        Style::default().add_modifier(Modifier::BOLD),
    )));

    // Top border
    board_lines.push(Line::from("   ┏━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┳━━━┓"));

    // Board rows
    for y in 0..BOARD_DIMENSION {
        if y > 0 {
            // Middle border between rows
            board_lines.push(Line::from("   ┣━━━╋━━━╋━━━╋━━━╋━━━╋━━━╋━━━╋━━━╋━━━┫"));
        }

        let row_num = 9 - y;
        let mut row_spans = vec![
            Span::styled(
                format!(" {} ", row_num),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("┃"),
        ];

        for x in 0..BOARD_DIMENSION {
            let position = Position::new(x, y);
            let mut cell_content = "   ".to_string();
            let mut cell_style = Style::default();

            // Check if this position has a piece
            if let Some(piece) = board.get_piece(&position) {
                cell_content = format!("{}", app.get_piece_display(piece));

                // Color the piece based on its color
                cell_style = match piece.color {
                    Color::White => Style::default().fg(RatatuiColor::White),
                    Color::Black => Style::default().fg(RatatuiColor::Red),
                };
            }

            // Highlight cursor position (only if game is not over)
            if position == app.cursor_position
                && !matches!(app.game_state, GameState::GameOver { .. })
            {
                cell_style = cell_style.bg(RatatuiColor::Blue);
            }
            // Highlight possible moves (only if game is not over)
            else if app.highlighted_moves.contains(&position)
                && !matches!(app.game_state, GameState::GameOver { .. })
            {
                cell_style = cell_style.bg(RatatuiColor::Green);
            }

            row_spans.push(Span::styled(cell_content, cell_style));
            row_spans.push(Span::raw("┃"));
        }

        row_spans.push(Span::styled(
            format!(" {}", row_num),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        board_lines.push(Line::from(row_spans));
    }

    // Bottom border
    board_lines.push(Line::from("   ┗━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┻━━━┛"));

    // Footer with column labels
    let footer = String::from("     A   B   C   D   E   F   G   H   I");
    board_lines.push(Line::from(Span::styled(
        footer,
        Style::default().add_modifier(Modifier::BOLD),
    )));

    // Current player indicator or winner message
    let status_message = match app.game_state {
        GameState::GameOver { winner } => {
            format!(
                "            {} WINS THE GAME!",
                match winner {
                    Color::White => "WHITE",
                    Color::Black => "BLACK",
                }
            )
        }
        _ => {
            let current_player = if app.game.board.is_white_to_move() {
                "WHITE"
            } else {
                "BLACK"
            };
            format!("              {} TO MOVE", current_player)
        }
    };
    board_lines.push(Line::from(Span::styled(
        status_message,
        Style::default().add_modifier(Modifier::BOLD),
    )));

    let board_paragraph = Paragraph::new(board_lines).alignment(Alignment::Left);

    f.render_widget(board_paragraph, board_area);
}
