//! Puzzle analysis (`POST /analyze-puzzle`, `docs/PROTOCOL.md`): from a
//! candidate position, play out the solver's best line and report the
//! rule-level facts and scores a platform needs to judge it. This module only
//! produces facts; every acceptance threshold belongs to the caller.

use crate::board::{Board, Color};
use crate::engine::constants::{MAX_LEVEL, MIN_LEVEL};
use crate::engine::search::root_search;
use crate::engine::tactics::{hanging_after_last_move, material_balance, move_themes};
use crate::engine::types::SearchConfig;
use crate::game::Game;
use crate::moves::Move;

/// Response format version (byte 0).
pub const FORMAT_VERSION: u8 = 1;
/// Trigger flag: the solver can win material from a piece that was not
/// capturable that way before the opponent's last move.
pub const FLAG_HANGING_AFTER_LAST_MOVE: u8 = 1 << 0;
/// Wire size of the response header.
pub const HEADER_BYTES: usize = 5;
/// Wire size of one solver step.
pub const STEP_BYTES: usize = 19;
/// The `reply` field of a step without a defender reply.
pub const NO_REPLY: u16 = 0xFFFF;

/// Plies analysed when the caller does not say.
pub const DEFAULT_MAX_PLIES: u8 = 9;
/// Smallest and largest accepted number of plies.
pub const MIN_MAX_PLIES: u8 = 1;
pub const MAX_MAX_PLIES: u8 = 15;

/// Clamp a caller-supplied ply budget to `MIN_MAX_PLIES..=MAX_MAX_PLIES`.
pub fn clamp_max_plies(requested: i64) -> u8 {
    requested.clamp(MIN_MAX_PLIES as i64, MAX_MAX_PLIES as i64) as u8
}

/// Why a position cannot be analysed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PuzzleError {
    /// The game is over, or the side to move has no legal move.
    NoLegalMove,
}

/// One solver step of the line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PuzzleStep {
    /// The solver's best move.
    pub solver_move: Move,
    /// Score of the best move, White's point of view.
    pub best_score: i32,
    /// Score of the second-best move (the best one's when it is the only
    /// legal move, or when another move ties it), White's point of view.
    pub second_score: i32,
    /// The defender's best reply, if the line goes on.
    pub reply: Option<Move>,
    /// Lowest engine level whose search picks `solver_move`.
    pub found_at_level: u8,
    /// Material balance after the solver's move.
    pub material_after_move: i16,
    /// Material balance after the reply (the previous one without reply).
    pub material_after_reply: i16,
    /// Theme bitmask of the solver's move (`tactics::theme`).
    pub themes: u16,
}

/// The whole analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PuzzleAnalysis {
    /// Trigger flags (`FLAG_*`).
    pub flags: u8,
    /// Material balance of the candidate position.
    pub material: i16,
    pub steps: Vec<PuzzleStep>,
}

impl PuzzleAnalysis {
    /// The wire format of `POST /analyze-puzzle` (little-endian).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_BYTES + self.steps.len() * STEP_BYTES);
        out.push(FORMAT_VERSION);
        out.push(self.flags);
        out.extend_from_slice(&self.material.to_le_bytes());
        out.push(self.steps.len() as u8);
        for step in &self.steps {
            out.extend_from_slice(&step.solver_move.to_u16().to_le_bytes());
            out.extend_from_slice(&step.best_score.to_le_bytes());
            out.extend_from_slice(&step.second_score.to_le_bytes());
            out.extend_from_slice(&step.reply.map_or(NO_REPLY, Move::to_u16).to_le_bytes());
            out.push(step.found_at_level);
            out.extend_from_slice(&step.material_after_move.to_le_bytes());
            out.extend_from_slice(&step.material_after_reply.to_le_bytes());
            out.extend_from_slice(&step.themes.to_le_bytes());
        }
        out
    }
}

/// The move that led to the candidate position and the board it was played
/// from, if the caller knows them.
pub struct LastMove<'a> {
    pub board_before: &'a Board,
    pub mv: &'a Move,
}

/// Material in `i16` (the wire width); a board cannot approach the limit.
fn material_i16(board: &Board) -> i16 {
    material_balance(board).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// Level `level`'s search with every source of randomness switched off.
fn deterministic_config(level: u8) -> SearchConfig {
    SearchConfig {
        noise_temperature: 0.0,
        blunder_chance: 0.0,
        decided_slip_chance: 0.0,
        ..SearchConfig::for_level(level)
    }
}

/// The lowest engine level `MIN_LEVEL..=MAX_LEVEL` whose search picks
/// `target` in `game`.
///
/// Levels are noise-free here, so a level's result depends only on its search
/// dials (depth, quiescence, killers): levels sharing them give the same
/// answer, and each distinct setting is searched once, at its lowest level.
/// A level that searches exactly like `MAX_LEVEL` is the reference itself, so
/// a move only the reference's setting finds is reported as `MAX_LEVEL`.
fn found_at_level(game: &Game, target: &Move) -> u8 {
    let dials = |c: &SearchConfig| (c.max_depth, c.use_quiescence, c.use_killers);
    let reference = dials(&deterministic_config(MAX_LEVEL));
    let mut tried = vec![reference];
    for level in MIN_LEVEL..MAX_LEVEL {
        let config = deterministic_config(level);
        let key = dials(&config);
        if tried.contains(&key) {
            continue;
        }
        tried.push(key);
        if root_search(game, &config, None).best_move == Some(*target) {
            return level;
        }
    }
    MAX_LEVEL
}

/// Analyse the candidate position `game` for at most `max_plies` plies
/// (clamped to `MIN_MAX_PLIES..=MAX_MAX_PLIES`): the side to move is the
/// solver; every step searches the position at level `MAX_LEVEL`, plays the
/// solver's best move, then the defender's best reply, until the game ends
/// or the ply budget is spent. `last_move` is the opponent's move that
/// produced `game`, if known (it feeds `FLAG_HANGING_AFTER_LAST_MOVE`).
///
/// Cost: one level-`MAX_LEVEL` search per ply plus the cheaper
/// `found_at_level` searches per solver step. Deterministic.
pub fn analyze_puzzle(
    game: &Game,
    last_move: Option<LastMove<'_>>,
    max_plies: u8,
) -> Result<PuzzleAnalysis, PuzzleError> {
    if game.is_game_over() {
        return Err(PuzzleError::NoLegalMove);
    }
    let max_plies = max_plies.clamp(MIN_MAX_PLIES, MAX_MAX_PLIES) as usize;
    let solver = game.color_to_move();
    let reference = SearchConfig::for_level(MAX_LEVEL);

    let flags = match last_move {
        Some(last) if hanging_after_last_move(last.board_before, &game.board, last.mv, solver) => {
            FLAG_HANGING_AFTER_LAST_MOVE
        }
        _ => 0,
    };
    let mut analysis = PuzzleAnalysis {
        flags,
        material: material_i16(&game.board),
        steps: Vec::new(),
    };

    let mut game = game.clone();
    let mut plies = 0;
    while plies < max_plies && !game.is_game_over() {
        let search = root_search(&game, &reference, None);
        let Some(best) = search.best_move else {
            if analysis.steps.is_empty() {
                return Err(PuzzleError::NoLegalMove);
            }
            break;
        };
        let sign = if solver == Color::White { 1 } else { -1 };
        let second_score = search
            .root_scores
            .iter()
            .filter(|(mv, _)| *mv != best)
            .map(|&(_, score)| score)
            .max()
            .unwrap_or(search.best_score);

        let board_before = game.board;
        let found_at_level = found_at_level(&game, &best);
        let themes = move_themes(&board_before, &best);
        game.make(&best);
        plies += 1;
        let material_after_move = material_i16(&game.board);

        let mut step = PuzzleStep {
            solver_move: best,
            best_score: sign * search.best_score,
            second_score: sign * second_score,
            reply: None,
            found_at_level,
            material_after_move,
            material_after_reply: material_after_move,
            themes,
        };

        if plies < max_plies && !game.is_game_over() {
            if let Some(reply) = root_search(&game, &reference, None).best_move {
                game.make(&reply);
                plies += 1;
                step.reply = Some(reply);
                step.material_after_reply = material_i16(&game.board);
            }
        }
        let line_goes_on = step.reply.is_some();
        analysis.steps.push(step);
        if !line_goes_on {
            break;
        }
    }
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Piece, PieceType, Position};
    use crate::engine::tactics::theme;

    fn p(x: usize, y: usize) -> Position {
        Position::new(x, y)
    }

    fn put(board: &mut Board, x: usize, y: usize, color: Color, bottom: PieceType) {
        board.set_piece(&p(x, y), Some(Piece::new(color, bottom, None)));
    }

    /// White to move: a rook takes an unguarded knight on its file.
    fn hanging_knight_game() -> Game {
        let mut board = Board::empty();
        put(&mut board, 4, 8, Color::White, PieceType::King);
        put(&mut board, 4, 0, Color::Black, PieceType::King);
        put(&mut board, 0, 5, Color::White, PieceType::Rook);
        put(&mut board, 0, 1, Color::Black, PieceType::Knight);
        Game::from_board(board)
    }

    fn mv(from: (usize, usize), to: (usize, usize)) -> Move {
        Move {
            from: p(from.0, from.1),
            to: p(to.0, to.1),
            unstack: false,
        }
    }

    #[test]
    fn the_ply_budget_is_clamped_to_one_through_fifteen() {
        assert_eq!(clamp_max_plies(i64::MIN), 1);
        assert_eq!(clamp_max_plies(-3), 1);
        assert_eq!(clamp_max_plies(0), 1);
        assert_eq!(clamp_max_plies(1), 1);
        assert_eq!(clamp_max_plies(9), 9);
        assert_eq!(clamp_max_plies(15), 15);
        assert_eq!(clamp_max_plies(16), 15);
        assert_eq!(clamp_max_plies(i64::MAX), 15);
    }

    #[test]
    fn a_trivially_hanging_piece_is_found_by_the_weakest_levels() {
        let game = hanging_knight_game();
        let capture = mv((0, 5), (0, 1));
        let level = found_at_level(&game, &capture);
        assert!((1..=2).contains(&level), "found at level {level}");
    }

    #[test]
    fn the_analysis_plays_the_capture_and_reports_its_facts() {
        let game = hanging_knight_game();
        let analysis = analyze_puzzle(&game, None, 1).expect("analysable");

        assert_eq!(analysis.flags, 0);
        assert_eq!(analysis.material, 60 - 40);
        assert_eq!(analysis.steps.len(), 1);
        let step = &analysis.steps[0];
        assert_eq!(step.solver_move, mv((0, 5), (0, 1)));
        assert_eq!(step.reply, None, "a one-ply budget leaves no reply");
        assert!((1..=2).contains(&step.found_at_level));
        assert_eq!(step.material_after_move, 60);
        assert_eq!(step.material_after_reply, 60);
        assert_eq!(step.themes, theme::CAPTURE | theme::HANGING);
        assert!(step.best_score > 0, "White wins a knight");
        assert!(step.second_score <= step.best_score);
    }

    #[test]
    fn a_longer_budget_adds_the_defenders_reply_and_further_steps() {
        let game = hanging_knight_game();
        let analysis = analyze_puzzle(&game, None, 4).expect("analysable");
        assert!(analysis.steps.len() >= 2, "{:?}", analysis.steps);
        let first = &analysis.steps[0];
        let reply = first.reply.expect("the defender answers inside the budget");
        // The reply was played: the next step starts from the position after it.
        let mut replayed = game.clone();
        replayed.make(&first.solver_move);
        replayed.make(&reply);
        let second_best = root_search(&replayed, &SearchConfig::for_level(MAX_LEVEL), None)
            .best_move
            .expect("a move");
        assert_eq!(analysis.steps[1].solver_move, second_best);
        // Never more plies than asked for.
        let plies: usize = analysis
            .steps
            .iter()
            .map(|s| 1 + usize::from(s.reply.is_some()))
            .sum();
        assert!(plies <= 4);
    }

    #[test]
    fn a_black_solver_reports_white_relative_scores() {
        let mut game = hanging_knight_game();
        game.set_white_to_move(false);
        // Black to move, with its knight attacked by the rook.
        let analysis = analyze_puzzle(&game, None, 1).expect("analysable");
        let step = &analysis.steps[0];
        assert!(
            step.best_score <= step.second_score,
            "Black's best move has the lowest White-relative score: {step:?}"
        );
    }

    #[test]
    fn the_wire_layout_is_little_endian_and_fixed_width() {
        let analysis = PuzzleAnalysis {
            flags: FLAG_HANGING_AFTER_LAST_MOVE,
            material: -300,
            steps: vec![
                PuzzleStep {
                    solver_move: mv((0, 5), (0, 1)),
                    best_score: -1_000_000,
                    second_score: 0x0102_0304,
                    reply: Some(mv((4, 0), (3, 0))),
                    found_at_level: 2,
                    material_after_move: -258,
                    material_after_reply: 257,
                    themes: 0x0A05,
                },
                PuzzleStep {
                    solver_move: mv((1, 1), (2, 2)),
                    best_score: 5,
                    second_score: 5,
                    reply: None,
                    found_at_level: 10,
                    material_after_move: 0,
                    material_after_reply: 0,
                    themes: 0,
                },
            ],
        };
        let bytes = analysis.to_bytes();
        assert_eq!(bytes.len(), HEADER_BYTES + 2 * STEP_BYTES);
        assert_eq!(bytes[0], 1);
        assert_eq!(bytes[1], 1);
        assert_eq!(&bytes[2..4], &(-300i16).to_le_bytes());
        assert_eq!(bytes[4], 2);

        let first = &bytes[5..5 + STEP_BYTES];
        assert_eq!(&first[0..2], &mv((0, 5), (0, 1)).to_u16().to_le_bytes());
        assert_eq!(&first[2..6], &(-1_000_000i32).to_le_bytes());
        assert_eq!(&first[6..10], &[0x04, 0x03, 0x02, 0x01]);
        assert_eq!(&first[10..12], &mv((4, 0), (3, 0)).to_u16().to_le_bytes());
        assert_eq!(first[12], 2);
        assert_eq!(&first[13..15], &[0xFE, 0xFE]);
        assert_eq!(&first[15..17], &[0x01, 0x01]);
        assert_eq!(&first[17..19], &[0x05, 0x0A]);

        let second = &bytes[5 + STEP_BYTES..];
        assert_eq!(&second[10..12], &[0xFF, 0xFF], "no reply is 0xFFFF");
        assert_eq!(second[12], 10);
    }

    #[test]
    fn a_blunder_into_a_capture_sets_the_hanging_after_last_move_flag() {
        let mut before = Board::empty();
        put(&mut before, 4, 8, Color::White, PieceType::King);
        put(&mut before, 4, 0, Color::Black, PieceType::King);
        put(&mut before, 0, 5, Color::White, PieceType::Rook);
        put(&mut before, 1, 3, Color::Black, PieceType::Knight);
        let last = mv((1, 3), (0, 1));
        let mut game = Game::from_board(before);
        game.set_white_to_move(false);
        game.make_unchecked(&last);

        let analysis = analyze_puzzle(
            &game,
            Some(LastMove {
                board_before: &before,
                mv: &last,
            }),
            1,
        )
        .expect("analysable");
        assert_eq!(analysis.flags & FLAG_HANGING_AFTER_LAST_MOVE, 1);
        assert_eq!(analysis.steps[0].solver_move, mv((0, 5), (0, 1)));
        assert_eq!(analysis.to_bytes()[1], 1);
    }

    #[test]
    fn a_finished_game_cannot_be_analysed() {
        let mut game = hanging_knight_game();
        game.set_game_over(true, true, false);
        assert_eq!(
            analyze_puzzle(&game, None, 3).unwrap_err(),
            PuzzleError::NoLegalMove
        );
    }

    #[test]
    fn the_analysis_is_deterministic() {
        let game = hanging_knight_game();
        assert_eq!(
            analyze_puzzle(&game, None, 3).unwrap(),
            analyze_puzzle(&game, None, 3).unwrap()
        );
    }
}
