use crate::board::{Board, Color, Piece, PieceType, Position, BOARD_DIMENSION};

pub struct GameOverResult {
    pub game_over: bool,
    pub white_wins: bool,
    pub draw: bool,
}

impl GameOverResult {
    pub fn ongoing() -> Self {
        GameOverResult {
            game_over: false,
            white_wins: false,
            draw: false,
        }
    }
}

pub fn check_promotion(piece: &Piece, position: &Position) -> Option<Piece> {
    let reached_opposite_side = match piece.color {
        Color::White => position.y == 0,
        Color::Black => position.y == 8,
    };
    if !reached_opposite_side {
        return None;
    }

    let promote_piece_type = |piece_type: PieceType| -> PieceType {
        match piece_type {
            PieceType::Soldier => PieceType::Paladin,
            PieceType::Ballista => PieceType::Rook,
            _ => piece_type,
        }
    };

    let bottom_needs_promotion =
        piece.bottom == PieceType::Soldier || piece.bottom == PieceType::Ballista;
    let top_needs_promotion = piece.top.is_some()
        && (piece.top == Some(PieceType::Soldier) || piece.top == Some(PieceType::Ballista));

    if !bottom_needs_promotion && !top_needs_promotion {
        return None;
    }

    let promoted_bottom = promote_piece_type(piece.bottom);
    let promoted_top = piece.top.map(promote_piece_type);
    Some(Piece::new(piece.color, promoted_bottom, promoted_top))
}

pub fn check_game_over(board: &Board, moves_without_capture: u8) -> GameOverResult {
    let mut white_king_exists = false;
    let mut black_king_exists = false;
    let mut white_pieces = Vec::new();
    let mut black_pieces = Vec::new();

    for y in 0..BOARD_DIMENSION {
        for x in 0..BOARD_DIMENSION {
            let pos = Position::new(x, y);
            if let Some(piece) = board.get_piece(&pos) {
                if piece.is_king() {
                    if piece.color == Color::White {
                        white_king_exists = true;
                    } else {
                        black_king_exists = true;
                    }
                }
                if piece.color == Color::White {
                    white_pieces.push((*piece, pos));
                } else {
                    black_pieces.push((*piece, pos));
                }
            }
        }
    }

    if !white_king_exists {
        return GameOverResult {
            game_over: true,
            white_wins: false,
            draw: false,
        };
    }
    if !black_king_exists {
        return GameOverResult {
            game_over: true,
            white_wins: true,
            draw: false,
        };
    }

    if moves_without_capture >= 40 {
        return GameOverResult {
            game_over: true,
            white_wins: false,
            draw: true,
        };
    }

    let white_draw = check_draw_condition_for_side(&white_pieces);
    let black_draw = check_draw_condition_for_side(&black_pieces);

    if white_draw && black_draw {
        return GameOverResult {
            game_over: true,
            white_wins: false,
            draw: true,
        };
    }

    GameOverResult::ongoing()
}

fn check_draw_condition_for_side(pieces: &[(Piece, Position)]) -> bool {
    let mut non_king_pieces = Vec::new();
    for (piece, pos) in pieces {
        if !piece.is_king() {
            non_king_pieces.push((*piece, *pos));
        }
    }

    if non_king_pieces.is_empty() {
        return true;
    }

    if non_king_pieces.len() == 1 {
        let (piece, _) = &non_king_pieces[0];
        if piece.bottom == PieceType::Knight && piece.top.is_none() {
            return true;
        }
    }

    let mut all_bishops_or_guards = true;
    let mut first_square_color: Option<bool> = None;

    for (piece, pos) in &non_king_pieces {
        let is_bishop_or_guard = piece.bottom == PieceType::Bishop
            || piece.bottom == PieceType::Guard
            || piece.top == Some(PieceType::Bishop)
            || piece.top == Some(PieceType::Guard);

        if !is_bishop_or_guard {
            all_bishops_or_guards = false;
            break;
        }

        let square_is_white = (pos.x + pos.y) % 2 == 0;
        match first_square_color {
            None => first_square_color = Some(square_is_white),
            Some(color) => {
                if color != square_is_white {
                    all_bishops_or_guards = false;
                    break;
                }
            }
        }
    }

    if all_bishops_or_guards && first_square_color.is_some() {
        return true;
    }

    false
}
