use crate::{Board, Piece, PieceType};
use base64::engine::general_purpose;
use base64::Engine;

pub fn display_stack(piece: &Piece) -> String {
    let mut output: String = String::new();

    if let Some(ref top_piece) = piece.top {
        output.push_str(&piece_to_char(top_piece));
        output.push('+');
    }

    output.push_str(&piece_to_char(&piece.bottom));

    output
}

pub fn get_board_hash(board: &Board) -> String {
    let all_bytes = board.to_binary();
    let byte_vec = all_bytes.to_vec();
    general_purpose::STANDARD.encode(&byte_vec)
}

pub fn piece_to_char(piece_type: &PieceType) -> String {
    match piece_type {
        PieceType::Soldier => "S".to_string(),
        PieceType::Bishop => "J".to_string(),
        PieceType::Rook => "C".to_string(),
        PieceType::Paladin => "P".to_string(),
        PieceType::Guard => "G".to_string(),
        PieceType::Knight => "D".to_string(),
        PieceType::Ballista => "B".to_string(),
        PieceType::King => "K".to_string(),
    }
}
