use crate::{Game, Piece, PieceType};
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

pub fn get_game_hash(game: &Game) -> String {
    let all_bytes = game.to_binary();
    let byte_vec = all_bytes.to_vec();
    general_purpose::STANDARD.encode(&byte_vec)
}

pub fn piece_to_char(piece_type: &PieceType) -> String {
    match piece_type {
        PieceType::Soldier => "S".to_string(),
        PieceType::Bishop => "B".to_string(),
        PieceType::Rook => "R".to_string(),
        PieceType::Paladin => "P".to_string(),
        PieceType::Guard => "G".to_string(),
        PieceType::Knight => "N".to_string(),
        PieceType::Ballista => "L".to_string(),
        PieceType::King => "K".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Color, Position};
    use crate::Move;

    /// Every piece type with the letter the CLI renders it as.
    const LETTERS: [(PieceType, &str); 8] = [
        (PieceType::Soldier, "S"),
        (PieceType::Bishop, "B"),
        (PieceType::Rook, "R"),
        (PieceType::Paladin, "P"),
        (PieceType::Guard, "G"),
        (PieceType::Knight, "N"),
        (PieceType::Ballista, "L"),
        (PieceType::King, "K"),
    ];

    const COLORS: [Color; 2] = [Color::White, Color::Black];

    #[test]
    fn piece_to_char_renders_one_fixed_letter_per_piece_type() {
        for (piece_type, expected) in LETTERS {
            assert_eq!(
                piece_to_char(&piece_type),
                expected,
                "{piece_type:?} should render as {expected}"
            );
        }
    }

    #[test]
    fn display_stack_renders_a_single_piece_as_its_own_letter() {
        for color in COLORS {
            for (piece_type, expected) in LETTERS {
                let piece = Piece::new(color, piece_type, None);
                assert_eq!(
                    display_stack(&piece),
                    expected,
                    "{color:?} {piece_type:?} alone"
                );
            }
        }
    }

    #[test]
    fn display_stack_renders_a_stack_as_top_plus_bottom() {
        let cases = [
            (PieceType::Rook, PieceType::Soldier, "S+R"),
            (PieceType::Guard, PieceType::Ballista, "L+G"),
            (PieceType::Bishop, PieceType::Knight, "N+B"),
            (PieceType::Paladin, PieceType::Paladin, "P+P"),
        ];

        for color in COLORS {
            for (bottom, top, expected) in cases {
                let piece = Piece::new(color, bottom, Some(top));
                assert_eq!(
                    display_stack(&piece),
                    expected,
                    "{color:?} {top:?} on {bottom:?}"
                );
            }
        }
    }

    #[test]
    fn display_stack_output_carries_no_colour_information() {
        // Worth pinning: the CLI renders both colours identically, so a
        // caller that needs the owner has to track it separately.
        for (bottom, top) in [
            (PieceType::Soldier, None),
            (PieceType::King, None),
            (PieceType::Rook, Some(PieceType::Soldier)),
        ] {
            let white = display_stack(&Piece::new(Color::White, bottom, top));
            let black = display_stack(&Piece::new(Color::Black, bottom, top));
            assert_eq!(white, black, "{top:?} on {bottom:?} must render the same");
        }
    }

    #[test]
    fn get_game_hash_is_stable_for_the_same_game() {
        let game = Game::new();
        assert_eq!(get_game_hash(&game), get_game_hash(&game));
        assert_eq!(get_game_hash(&game), get_game_hash(&Game::new()));
    }

    #[test]
    fn get_game_hash_changes_after_a_move() {
        let mut game = Game::new();
        let before = get_game_hash(&game);

        // A white soldier steps diagonally forward, its only kind of move.
        let mv = Move {
            from: Position::new(0, 6),
            to: Position::new(1, 5),
            unstack: false,
        };
        game.try_make(&mv).expect("soldier push should be legal");

        assert_ne!(
            before,
            get_game_hash(&game),
            "the hash must follow the game state"
        );
    }

    #[test]
    fn get_game_hash_is_standard_base64_of_the_83_byte_game_binary() {
        let game = Game::new();
        let decoded = general_purpose::STANDARD
            .decode(get_game_hash(&game))
            .expect("the hash should be valid standard base64");

        assert_eq!(decoded.len(), 83, "81 board bytes + flags + move counter");
        assert_eq!(decoded, game.to_binary().to_vec());
    }
}
