//! Rule-level tactical facts about a position or a move: which squares a
//! piece reaches, static exchange evaluation, hanging pieces, and the
//! tactical themes of a move (`POST /analyze-puzzle`, `docs/PROTOCOL.md`).
//!
//! Everything here is derived from the real move generator
//! ([`MoveGenerator`]) and the real move application ([`Game::make_unchecked`]),
//! so stacks, unstacking, the ballista's one-way slide and promotion behave
//! exactly as in play. Nothing here is on the search's hot path: it favours
//! being short and obviously equal to the rules over avoiding allocation.
//!
//! "Attack" always means *a piece of that colour could capture on that square
//! next move*, whoever is to move: the functions take the colour from the
//! piece (or an explicit `side`) and never look at the turn.

use crate::board::{Board, Color, Piece, Position};
use crate::engine::constants::KING_VALUE;
use crate::engine::eval::material::{base_value, stack_base_value};
use crate::game::Game;
use crate::game_over::check_promotion;
use crate::moves::{Move, MoveGenerator};

/// Bit positions of the move themes (the `themes` field of an
/// `/analyze-puzzle` step).
pub mod theme {
    /// The move captures an enemy piece.
    pub const CAPTURE: u16 = 1 << 0;
    /// The captured piece was undefended or losing on static exchange.
    pub const HANGING: u16 = 1 << 1;
    /// The moved piece attacks two or more worthwhile targets.
    pub const FORK: u16 = 1 << 2;
    /// After the move the enemy king can be captured.
    pub const KING_THREAT: u16 = 1 << 3;
    /// Moving the piece uncovers a worthwhile attack by another piece.
    pub const DISCOVERED: u16 = 1 << 4;
    /// The move creates a pin.
    pub const PIN: u16 = 1 << 5;
    /// The move creates a skewer.
    pub const SKEWER: u16 = 1 << 6;
    /// The move promotes.
    pub const PROMOTION: u16 = 1 << 7;
    /// The moved piece lands where it loses material on static exchange.
    pub const SACRIFICE: u16 = 1 << 8;
    /// No capture and no king threat.
    pub const QUIET: u16 = 1 << 9;
    /// The move stacks onto a friendly piece.
    pub const STACK: u16 = 1 << 10;
    /// The move splits a stack.
    pub const UNSTACK: u16 = 1 << 11;
}

/// A static exchange never runs longer than this many captures; far above
/// anything a 9×9 board produces, it only bounds the recursion.
const MAX_EXCHANGE_DEPTH: usize = 12;

fn opponent(color: Color) -> Color {
    match color {
        Color::White => Color::Black,
        Color::Black => Color::White,
    }
}

/// Value of a piece or whole stack in engine units; the king is worth
/// `KING_VALUE`.
pub fn piece_value(piece: &Piece) -> i32 {
    stack_base_value(piece)
}

/// Material balance of `board` from White's point of view: the sum of the
/// base values of every piece (both halves of a stack), White positive. The
/// kings count 0 — only what can be won or lost on the board matters here —
/// and no positional term is included.
pub fn material_balance(board: &Board) -> i32 {
    board
        .pieces()
        .filter(|(_, piece)| !piece.is_king())
        .map(|(_, piece)| {
            let value = piece_value(piece);
            match piece.color {
                Color::White => value,
                Color::Black => -value,
            }
        })
        .sum()
}

/// Every square the piece at `pos` can move to — empty squares, captures and
/// stacking onto a friendly piece alike — deduplicated, whoever is to move.
/// Empty when `pos` is empty. This is what the GUI highlights when
/// hovering a piece.
pub fn reachable_squares(board: &Board, pos: Position) -> Vec<Position> {
    let Some(piece) = board.get_piece(&pos) else {
        return Vec::new();
    };
    let generator = MoveGenerator::new(board, piece.color == Color::White);
    let mut out: Vec<Position> = Vec::new();
    for pm in generator.get_moves(&pos) {
        if !out.contains(&pm.to) {
            out.push(pm.to);
        }
    }
    out
}

/// The squares holding an enemy piece that the piece at `pos` could capture
/// next move.
pub fn attacked_squares(board: &Board, pos: Position) -> Vec<Position> {
    let Some(piece) = board.get_piece(&pos) else {
        return Vec::new();
    };
    let color = piece.color;
    reachable_squares(board, pos)
        .into_iter()
        .filter(|sq| board.get_piece(sq).is_some_and(|p| p.color != color))
        .collect()
}

/// Every move of `side` capturing the enemy piece on `sq` (a whole stack and
/// its top piece alone count as separate moves). Empty if `sq` holds no
/// enemy of `side`.
fn capture_moves_to(board: &Board, side: Color, sq: Position) -> Vec<Move> {
    if !board.get_piece(&sq).is_some_and(|p| p.color != side) {
        return Vec::new();
    }
    let generator = MoveGenerator::new(board, side == Color::White);
    board
        .pieces()
        .filter(|(_, piece)| piece.color == side)
        .flat_map(|(pos, _)| generator.get_moves(&pos))
        .filter(|pm| pm.to == sq)
        .flat_map(|pm| pm.to_moves())
        .collect()
}

/// Value of what actually travels when `mv` is played: the top piece alone
/// when unstacking, the whole piece or stack otherwise.
fn mover_value(board: &Board, mv: &Move) -> i32 {
    match board.get_piece(&mv.from) {
        None => 0,
        Some(piece) if mv.unstack => piece.top.map_or(0, base_value),
        Some(piece) => piece_value(piece),
    }
}

/// The board after `side` plays `mv`, applied with the real move rules
/// (stacking, unstacking, promotion). `mv` must be legal for `side`.
fn after_move(board: &Board, side: Color, mv: &Move) -> Board {
    let mut game = Game::from_board(*board);
    game.set_white_to_move(side == Color::White);
    game.make_unchecked(mv);
    game.board
}

/// Net material `side` wins by playing the capture `mv` (whose target is
/// worth `target_value`) and then letting the exchange on that square play
/// out: the target, minus whatever the opponent wins back.
fn capture_net(board: &Board, side: Color, mv: &Move, target_value: i32, depth: usize) -> i32 {
    if target_value >= KING_VALUE {
        // Capturing the king ends the game: there is nothing to recapture.
        return target_value;
    }
    let after = after_move(board, side, mv);
    target_value - exchange_gain_at(&after, mv.to, opponent(side), depth + 1)
}

fn exchange_gain_at(board: &Board, sq: Position, side: Color, depth: usize) -> i32 {
    if depth >= MAX_EXCHANGE_DEPTH {
        return 0;
    }
    let Some(target) = board.get_piece(&sq) else {
        return 0;
    };
    if target.color == side {
        return 0;
    }
    // The classic swap-off: always capture with the least valuable piece
    // available, and stop as soon as continuing would lose material.
    let Some(mv) = capture_moves_to(board, side, sq)
        .into_iter()
        .min_by_key(|mv| mover_value(board, mv))
    else {
        return 0;
    };
    capture_net(board, side, &mv, piece_value(target), depth).max(0)
}

/// Static exchange evaluation: the net material (engine units, `>= 0`) that
/// `side` wins by capturing the enemy piece on `sq` and then trading on that
/// square — always with the least valuable capturer, stopping when going on
/// would lose material. `0` when `sq` holds no enemy piece or capturing it
/// does not pay; the king counts `KING_VALUE`.
pub fn exchange_gain(board: &Board, sq: Position, side: Color) -> i32 {
    exchange_gain_at(board, sq, side, 0)
}

/// Net material gained when the piece on `from` captures the enemy piece on
/// `sq` and the exchange on `sq` plays out (negative when the capture loses
/// material). Of a stack's capture variants the best one counts. `None` if
/// that piece cannot capture there.
fn capture_gain_from(board: &Board, from: Position, sq: Position) -> Option<i32> {
    let piece = board.get_piece(&from)?;
    let target = board.get_piece(&sq).filter(|t| t.color != piece.color)?;
    let side = piece.color;
    let target_value = piece_value(target);
    MoveGenerator::new(board, side == Color::White)
        .get_moves(&from)
        .into_iter()
        .filter(|pm| pm.to == sq)
        .flat_map(|pm| pm.to_moves())
        .map(|mv| capture_net(board, side, &mv, target_value, 0))
        .max()
}

/// The squares of enemy pieces (kings excluded) that `side` wins material
/// from by capturing: an undefended piece, or one whose exchange on its
/// square is positive for `side`.
fn winnable_targets(board: &Board, side: Color) -> Vec<Position> {
    board
        .pieces()
        .filter(|(_, piece)| piece.color != side && !piece.is_king())
        .map(|(pos, _)| pos)
        .filter(|&pos| exchange_gain(board, pos, side) > 0)
        .collect()
}

/// `HANGING_AFTER_LAST_MOVE`: whether `solver`, to move on `after`, can win
/// material by capturing an enemy piece that was not winnable before the
/// opponent's `last_move`.
///
/// `before` is the board the opponent moved from. An enemy piece (kings
/// excluded) is *winnable* for the solver when capturing it nets material on
/// static exchange (undefended, or defended insufficiently: see
/// [`exchange_gain`]), whoever is to move. A winnable piece counts as new when
/// it stands on the square the opponent just moved to (a different piece
/// than before there), or when its square was not winnable on `before`.
pub fn hanging_after_last_move(
    before: &Board,
    after: &Board,
    last_move: &Move,
    solver: Color,
) -> bool {
    let was_winnable = winnable_targets(before, solver);
    winnable_targets(after, solver)
        .into_iter()
        .any(|sq| sq == last_move.to || !was_winnable.contains(&sq))
}

/// True when `mid` lies strictly between `origin` and `far` on one straight
/// or diagonal line.
fn strictly_between(origin: Position, mid: Position, far: Position) -> bool {
    let delta =
        |a: Position, b: Position| (b.x as isize - a.x as isize, b.y as isize - a.y as isize);
    let (ax, ay) = delta(origin, far);
    let (bx, by) = delta(origin, mid);
    let straight = ax == 0 || ay == 0 || ax.abs() == ay.abs();
    straight
        && ax * by - ay * bx == 0
        && ax * bx + ay * by > 0
        && bx.abs().max(by.abs()) < ax.abs().max(ay.abs())
}

/// An x-ray line `(attacker, front, behind)`, see `xray_lines`.
type XrayLine = (Position, Position, Position);

/// Every x-ray line of `side`: `(attacker, front, behind)` where the attacker
/// captures the enemy piece on `front`, and would capture the enemy piece on
/// `behind` — on the same line, straight behind `front` — if `front` were
/// gone. Found by lifting `front` off the board, so it covers every piece
/// that slides (or reaches two squares) exactly as it moves.
fn xray_lines(board: &Board, side: Color) -> Vec<XrayLine> {
    let mut lines = Vec::new();
    for (attacker, piece) in board.pieces() {
        if piece.color != side {
            continue;
        }
        let attacked = attacked_squares(board, attacker);
        for &front in &attacked {
            let mut cleared = *board;
            cleared.set_piece(&front, None);
            for behind in attacked_squares(&cleared, attacker) {
                if !attacked.contains(&behind) && strictly_between(attacker, front, behind) {
                    lines.push((attacker, front, behind));
                }
            }
        }
    }
    lines
}

/// The x-ray lines of `side` that are pins (front piece lighter than the
/// piece behind it, or anything in front of the king) and those that are
/// skewers (front piece worth more than the piece behind it).
fn pins_and_skewers(board: &Board, side: Color) -> (Vec<XrayLine>, Vec<XrayLine>) {
    let mut pins = Vec::new();
    let mut skewers = Vec::new();
    for line @ (_, front, behind) in xray_lines(board, side) {
        let (Some(front_piece), Some(behind_piece)) =
            (board.get_piece(&front), board.get_piece(&behind))
        else {
            continue;
        };
        let (front_value, behind_value) = (piece_value(front_piece), piece_value(behind_piece));
        if !front_piece.is_king() && behind_value > front_value {
            pins.push(line);
        } else if !behind_piece.is_king() && front_value > behind_value {
            skewers.push(line);
        }
    }
    (pins, skewers)
}

fn enemy_king_square(board: &Board, solver: Color) -> Option<Position> {
    board
        .pieces()
        .find(|(_, piece)| piece.is_king() && piece.color != solver)
        .map(|(pos, _)| pos)
}

/// The themes of `mv`, a legal move on `before` for the side that owns the
/// piece on `mv.from`, as a bitmask of [`theme`] flags. `0` if the move is not
/// legal there.
///
/// Definitions (all computed on `before` and on the board after the move;
/// "worthwhile" = capturing the piece nets material on static exchange, see
/// [`exchange_gain`]; the king always counts as worthwhile):
///
/// - `CAPTURE`: the destination held an enemy piece or stack.
/// - `HANGING`: a capture whose exchange on the destination nets material:
///   the captured piece was undefended, or worth more than what is lost back.
/// - `FORK`: the piece now on the destination attacks at least two
///   worthwhile targets (the king plus any worthwhile piece qualifies).
/// - `KING_THREAT`: after the move some piece of the mover can capture the
///   enemy king.
/// - `DISCOVERED`: a friendly piece other than the mover newly attacks a
///   worthwhile target through the square the mover left.
/// - `PIN`: the move creates a line "attacker, enemy piece, more valuable
///   enemy piece or the king" (see `xray_lines`) that did not exist before.
/// - `SKEWER`: likewise, "attacker, enemy piece, less valuable enemy piece".
/// - `PROMOTION`: the moved piece (or top piece, when unstacking) is
///   promoted on arrival.
/// - `SACRIFICE`: a non-king mover lands where the enemy's exchange wins more
///   than the move captured, so the move loses material on static exchange.
/// - `QUIET`: neither `CAPTURE` nor `KING_THREAT`.
/// - `STACK`: the destination held a friendly piece.
/// - `UNSTACK`: the top piece of a stack moves alone.
pub fn move_themes(before: &Board, mv: &Move) -> u16 {
    let Some(&source) = before.get_piece(&mv.from) else {
        return 0;
    };
    let solver = source.color;
    let enemy = opponent(solver);
    let mut game = Game::from_board(*before);
    game.set_white_to_move(solver == Color::White);
    if game.check_legal(mv).is_err() {
        return 0;
    }
    game.make_unchecked(mv);
    let after = game.board;

    let mut themes = 0u16;
    let destination = before.get_piece(&mv.to).copied();
    let captured = destination.filter(|p| p.color != solver);

    if let Some(victim) = captured {
        themes |= theme::CAPTURE;
        let net = piece_value(&victim) - exchange_gain(&after, mv.to, enemy);
        if victim.is_king() || net > 0 {
            themes |= theme::HANGING;
        }
    }
    if destination.is_some_and(|p| p.color == solver) {
        themes |= theme::STACK;
    }
    if mv.unstack {
        themes |= theme::UNSTACK;
    }

    let moved = if mv.unstack {
        Piece {
            color: solver,
            bottom: source.top.unwrap_or(source.bottom),
            top: None,
        }
    } else {
        source
    };
    if check_promotion(&moved, &mv.to).is_some() {
        themes |= theme::PROMOTION;
    }

    let king_square = enemy_king_square(&after, solver);
    if king_square.is_some_and(|k| !capture_moves_to(&after, solver, k).is_empty()) {
        themes |= theme::KING_THREAT;
    }
    if themes & (theme::CAPTURE | theme::KING_THREAT) == 0 {
        themes |= theme::QUIET;
    }

    let worthwhile = |board: &Board, attacker: Position, target: Position| {
        board.get_piece(&target).is_some_and(|p| p.is_king())
            || capture_gain_from(board, attacker, target).is_some_and(|gain| gain > 0)
    };

    if after.get_piece(&mv.to).is_some() {
        let targets = attacked_squares(&after, mv.to)
            .into_iter()
            .filter(|&t| worthwhile(&after, mv.to, t))
            .count();
        if targets >= 2 {
            themes |= theme::FORK;
        }
    }

    let discovers = after
        .pieces()
        .filter(|(pos, piece)| piece.color == solver && *pos != mv.to && *pos != mv.from)
        .any(|(attacker, _)| {
            let already = attacked_squares(before, attacker);
            attacked_squares(&after, attacker)
                .into_iter()
                .any(|target| {
                    !already.contains(&target)
                        && strictly_between(attacker, mv.from, target)
                        && worthwhile(&after, attacker, target)
                })
        });
    if discovers {
        themes |= theme::DISCOVERED;
    }

    let (pins_before, skewers_before) = pins_and_skewers(before, solver);
    let (pins_after, skewers_after) = pins_and_skewers(&after, solver);
    if pins_after.iter().any(|l| !pins_before.contains(l)) {
        themes |= theme::PIN;
    }
    if skewers_after.iter().any(|l| !skewers_before.contains(l)) {
        themes |= theme::SKEWER;
    }

    if !source.is_king() && after.get_piece(&mv.to).is_some() {
        let won_back = exchange_gain(&after, mv.to, enemy);
        if won_back > captured.as_ref().map_or(0, piece_value) {
            themes |= theme::SACRIFICE;
        }
    }

    themes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::PieceType;

    const W: Color = Color::White;
    const B: Color = Color::Black;

    fn p(x: usize, y: usize) -> Position {
        Position::new(x, y)
    }

    fn put(board: &mut Board, x: usize, y: usize, color: Color, bottom: PieceType) {
        board.set_piece(&p(x, y), Some(Piece::new(color, bottom, None)));
    }

    fn put_stack(
        board: &mut Board,
        x: usize,
        y: usize,
        color: Color,
        bottom: PieceType,
        top: PieceType,
    ) {
        board.set_piece(&p(x, y), Some(Piece::new(color, bottom, Some(top))));
    }

    /// Two kings far from everything: White's on E1, Black's on E9.
    fn kings() -> Board {
        let mut board = Board::empty();
        put(&mut board, 4, 8, W, PieceType::King);
        put(&mut board, 4, 0, B, PieceType::King);
        board
    }

    fn mv(from: (usize, usize), to: (usize, usize)) -> Move {
        Move {
            from: p(from.0, from.1),
            to: p(to.0, to.1),
            unstack: false,
        }
    }

    fn themes(board: &Board, mv: &Move) -> u16 {
        move_themes(board, mv)
    }

    #[test]
    fn material_balance_counts_both_halves_of_a_stack_and_skips_kings() {
        let mut board = kings();
        put(&mut board, 0, 6, W, PieceType::Rook);
        put_stack(&mut board, 0, 2, B, PieceType::Bishop, PieceType::Soldier);
        assert_eq!(material_balance(&board), 60 - (40 + 10));
        assert_eq!(material_balance(&kings()), 0);
    }

    #[test]
    fn reachable_squares_lists_each_destination_once_stacks_included() {
        let mut board = kings();
        // A rook carrying a paladin: both reach the same squares.
        put_stack(&mut board, 0, 4, W, PieceType::Rook, PieceType::Paladin);
        let squares = reachable_squares(&board, p(0, 4));
        let mut unique = squares.clone();
        unique.dedup();
        assert_eq!(squares.len(), unique.len());
        assert!(squares.contains(&p(0, 0)) && squares.contains(&p(8, 4)));
        assert!(reachable_squares(&board, p(5, 5)).is_empty());
    }

    #[test]
    fn exchange_gain_prices_defended_and_undefended_pieces() {
        let mut board = kings();
        put(&mut board, 0, 4, W, PieceType::Rook);
        put(&mut board, 0, 1, B, PieceType::Knight);
        assert_eq!(exchange_gain(&board, p(0, 1), W), 40);

        // Defended by a rook: rook takes knight, rook retakes — a loss.
        put(&mut board, 0, 0, B, PieceType::Rook);
        assert_eq!(exchange_gain(&board, p(0, 1), W), 0);
        // Nothing to capture on an empty square or on one's own piece.
        assert_eq!(exchange_gain(&board, p(3, 3), W), 0);
        assert_eq!(exchange_gain(&board, p(0, 4), W), 0);
    }

    #[test]
    fn capture_of_an_undefended_piece_is_capture_and_hanging() {
        let mut board = kings();
        put(&mut board, 0, 4, W, PieceType::Rook);
        put(&mut board, 0, 1, B, PieceType::Knight);
        let t = themes(&board, &mv((0, 4), (0, 1)));
        assert_eq!(t, theme::CAPTURE | theme::HANGING);
    }

    #[test]
    fn capture_of_a_defended_piece_with_a_costlier_one_is_a_sacrifice() {
        let mut board = kings();
        put(&mut board, 0, 4, W, PieceType::Rook);
        put(&mut board, 0, 1, B, PieceType::Knight);
        put(&mut board, 0, 0, B, PieceType::Rook);
        let t = themes(&board, &mv((0, 4), (0, 1)));
        assert_eq!(t, theme::CAPTURE | theme::SACRIFICE);
    }

    #[test]
    fn a_cheap_piece_taking_a_defended_rook_is_hanging_not_a_sacrifice() {
        let mut board = kings();
        put(&mut board, 1, 2, W, PieceType::Soldier);
        put(&mut board, 0, 1, B, PieceType::Rook);
        put(&mut board, 1, 0, B, PieceType::Soldier);
        let t = themes(&board, &mv((1, 2), (0, 1)));
        assert_eq!(t, theme::CAPTURE | theme::HANGING);
    }

    #[test]
    fn a_knight_landing_between_two_rooks_forks_them() {
        let mut board = kings();
        put(&mut board, 2, 5, W, PieceType::Knight);
        put(&mut board, 5, 2, B, PieceType::Rook);
        put(&mut board, 1, 4, B, PieceType::Rook);
        let t = themes(&board, &mv((2, 5), (3, 3)));
        assert_eq!(t, theme::FORK | theme::QUIET);
    }

    #[test]
    fn one_worthwhile_target_is_not_a_fork() {
        let mut board = kings();
        put(&mut board, 2, 5, W, PieceType::Knight);
        put(&mut board, 5, 2, B, PieceType::Rook);
        let t = themes(&board, &mv((2, 5), (3, 3)));
        assert_eq!(t, theme::QUIET);
    }

    #[test]
    fn attacking_the_king_is_a_king_threat_and_not_quiet() {
        let mut board = kings();
        put(&mut board, 0, 5, W, PieceType::Rook);
        let t = themes(&board, &mv((0, 5), (4, 5)));
        assert_eq!(t, theme::KING_THREAT);
    }

    #[test]
    fn a_plain_move_is_quiet() {
        let mut board = kings();
        put(&mut board, 4, 6, W, PieceType::Soldier);
        assert_eq!(themes(&board, &mv((4, 6), (3, 5))), theme::QUIET);
    }

    #[test]
    fn moving_onto_a_friendly_piece_is_a_stack() {
        let mut board = kings();
        put(&mut board, 3, 6, W, PieceType::Soldier);
        put(&mut board, 2, 5, W, PieceType::Rook);
        let t = themes(&board, &mv((3, 6), (2, 5)));
        assert_eq!(t, theme::STACK | theme::QUIET);
    }

    #[test]
    fn the_top_piece_leaving_a_stack_is_an_unstack() {
        let mut board = kings();
        put_stack(&mut board, 3, 6, W, PieceType::Rook, PieceType::Soldier);
        let mut unstack = mv((3, 6), (2, 5));
        unstack.unstack = true;
        assert_eq!(themes(&board, &unstack), theme::UNSTACK | theme::QUIET);
        // The whole stack moving is not.
        let whole = Move {
            unstack: false,
            ..unstack
        };
        assert_eq!(themes(&board, &whole), theme::QUIET);
    }

    #[test]
    fn reaching_the_last_rank_promotes() {
        let mut board = Board::empty();
        put(&mut board, 4, 8, W, PieceType::King);
        put(&mut board, 6, 0, B, PieceType::King);
        put(&mut board, 3, 1, W, PieceType::Soldier);
        let t = themes(&board, &mv((3, 1), (2, 0)));
        assert_eq!(t, theme::PROMOTION | theme::QUIET);
    }

    #[test]
    fn uncovering_a_rook_behind_a_knight_is_a_discovery() {
        let mut board = kings();
        put(&mut board, 0, 8, W, PieceType::Rook);
        put(&mut board, 0, 5, W, PieceType::Knight);
        put(&mut board, 0, 1, B, PieceType::Rook);
        let t = themes(&board, &mv((0, 5), (2, 4)));
        assert_eq!(t, theme::DISCOVERED | theme::QUIET);
    }

    #[test]
    fn an_existing_attack_is_not_a_discovery() {
        let mut board = kings();
        put(&mut board, 0, 8, W, PieceType::Rook);
        put(&mut board, 0, 1, B, PieceType::Rook);
        put(&mut board, 5, 6, W, PieceType::Knight);
        let t = themes(&board, &mv((5, 6), (6, 4)));
        assert_eq!(t & theme::DISCOVERED, 0);
    }

    #[test]
    fn attacking_a_piece_in_front_of_the_king_is_a_pin() {
        let mut board = kings();
        put(&mut board, 0, 6, W, PieceType::Rook);
        put(&mut board, 4, 2, B, PieceType::Knight);
        let t = themes(&board, &mv((0, 6), (4, 6)));
        assert_eq!(t, theme::PIN | theme::QUIET);
    }

    #[test]
    fn attacking_the_king_with_a_rook_behind_it_is_a_skewer() {
        let mut board = Board::empty();
        put(&mut board, 4, 8, W, PieceType::King);
        put(&mut board, 4, 3, B, PieceType::King);
        put(&mut board, 4, 1, B, PieceType::Rook);
        put(&mut board, 0, 6, W, PieceType::Rook);
        let t = themes(&board, &mv((0, 6), (4, 6)));
        assert_eq!(t, theme::SKEWER | theme::KING_THREAT);
    }

    #[test]
    fn landing_on_a_defended_square_for_nothing_is_a_sacrifice() {
        let mut board = kings();
        put(&mut board, 0, 3, W, PieceType::Rook);
        put(&mut board, 3, 2, B, PieceType::Soldier);
        let t = themes(&board, &mv((0, 3), (2, 3)));
        assert_eq!(t, theme::SACRIFICE | theme::QUIET);
    }

    #[test]
    fn an_illegal_move_has_no_themes() {
        let board = kings();
        assert_eq!(themes(&board, &mv((0, 0), (1, 1))), 0);
        // Not this side's piece... the king cannot jump three squares.
        assert_eq!(themes(&board, &mv((4, 8), (4, 5))), 0);
    }

    /// The black knight lands on the white rook's file, where nothing guards it.
    fn knight_blunder() -> (Board, Board, Move) {
        let mut before = kings();
        put(&mut before, 0, 5, W, PieceType::Rook);
        put(&mut before, 1, 3, B, PieceType::Knight);
        let last = mv((1, 3), (0, 1));
        let after = after_move(&before, B, &last);
        (before, after, last)
    }

    #[test]
    fn a_piece_that_just_moved_into_a_capture_is_hanging_after_the_last_move() {
        let (before, after, last) = knight_blunder();
        assert!(hanging_after_last_move(&before, &after, &last, W));
    }

    #[test]
    fn a_defended_piece_is_not_hanging_after_the_last_move() {
        let mut before = kings();
        put(&mut before, 0, 5, W, PieceType::Rook);
        put(&mut before, 1, 3, B, PieceType::Knight);
        put(&mut before, 0, 0, B, PieceType::Rook);
        let last = mv((1, 3), (0, 1));
        let after = after_move(&before, B, &last);
        assert!(!hanging_after_last_move(&before, &after, &last, W));
    }

    #[test]
    fn a_piece_that_was_already_hanging_is_not_new() {
        let mut before = kings();
        put(&mut before, 0, 5, W, PieceType::Rook);
        put(&mut before, 0, 2, B, PieceType::Knight);
        let last = mv((4, 0), (3, 0));
        let after = after_move(&before, B, &last);
        assert!(!hanging_after_last_move(&before, &after, &last, W));
    }

    #[test]
    fn moving_a_defender_away_leaves_a_piece_hanging() {
        let mut before = kings();
        put(&mut before, 0, 5, W, PieceType::Rook);
        put(&mut before, 0, 1, B, PieceType::Knight);
        put(&mut before, 0, 0, B, PieceType::Rook);
        // The rook defending the knight steps aside.
        let last = mv((0, 0), (1, 0));
        let after = after_move(&before, B, &last);
        assert!(hanging_after_last_move(&before, &after, &last, W));
    }
}
