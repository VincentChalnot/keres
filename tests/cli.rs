//! Integration tests for the `keres` CLI binary (`src/main.rs`).
//!
//! These tests drive the **real binary** as a child process — Cargo hands us
//! its path through `CARGO_BIN_EXE_keres` — and assert on exit status plus
//! stdout/stderr. No test harness crate is involved, only `std::process`.
//!
//! Covered: `--help`/`--version`, the no-subcommand guidance, unknown
//! subcommands, `show-moves` (whole board, single square, coordinate parsing
//! and board orientation), `engine-move` (legality of the returned move on the
//! starting position and on a hand-built position), the `--board` base64
//! import and every way it can be malformed, and `debug-tree` (stderr stats,
//! JSONL tree output, move-list replay and its error paths, search toggles).
//!
//! Expectations are re-derived from the `keres_engine` library rather than
//! hardcoded wherever the game rules would otherwise be duplicated here.
//!
//! Anything that runs the search uses `--max-depth 1`/`2` or a tiny position,
//! except the two tests that must exercise the default depth.

use base64::{engine::general_purpose, Engine as _};
use keres_engine::{
    Board, Color, Game, Move, Piece, PieceType, Position, BOARD_DIMENSION, BOARD_SIZE,
};
use std::process::Command;

// ── Harness ─────────────────────────────────────────────────────────────────

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Run {
    /// The binary must never die on a panic: every bad input has to come out as
    /// a printed error.
    fn assert_no_panic(&self) {
        assert!(
            !self.stderr.contains("panicked"),
            "the binary panicked instead of reporting an error:\n{}",
            self.stderr
        );
    }

    fn stdout_lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }
}

fn keres(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_keres"))
        .args(args)
        .output()
        .expect("failed to run the keres binary");
    Run {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Run the CLI and require a successful exit.
fn keres_ok(args: &[&str]) -> Run {
    let run = keres(args);
    run.assert_no_panic();
    assert_eq!(
        run.code,
        Some(0),
        "expected `keres {}` to succeed\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        run.stdout,
        run.stderr
    );
    run
}

/// Run the CLI and require the documented failure exit code 1 plus an
/// explanatory message on stderr.
fn keres_err(args: &[&str], expected_stderr: &str) -> Run {
    let run = keres(args);
    run.assert_no_panic();
    assert_eq!(
        run.code,
        Some(1),
        "expected `keres {}` to exit 1\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        run.stdout,
        run.stderr
    );
    assert!(
        run.stderr.contains(expected_stderr),
        "expected stderr of `keres {}` to mention {:?}, got:\n{}",
        args.join(" "),
        expected_stderr,
        run.stderr
    );
    run
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// `create_game` wants exactly `BOARD_SIZE + 1` = 82 bytes: the 81 board
/// squares plus the flags byte. `Game::to_binary` produces 83 (it appends the
/// no-capture counter), so the wire argument is its first 82 bytes.
fn board_arg(game: &Game) -> String {
    general_purpose::STANDARD.encode(&game.to_binary()[..BOARD_SIZE + 1])
}

fn base64_of_len(len: usize) -> String {
    let mut bytes = Game::new().to_binary().to_vec();
    bytes.resize(len, 0);
    general_purpose::STANDARD.encode(&bytes)
}

/// Every square that has at least one move for the side to move, as
/// `"<piece letter>@<square>"` — i.e. exactly the headers `show-moves` prints.
fn movable_piece_headers(game: &Game) -> Vec<String> {
    let mut headers = Vec::new();
    for y in 0..BOARD_DIMENSION {
        for x in 0..BOARD_DIMENSION {
            let position = Position::new(x, y);
            if game.get_moves(&position).is_empty() {
                continue;
            }
            let piece = game
                .board
                .get_piece(&position)
                .expect("a square with moves is occupied");
            headers.push(format!(
                "{}@{}",
                keres_engine::cli_rendering::display_stack(piece),
                position
            ));
        }
    }
    headers
}

/// Every legal move of the side to move, rendered the way `Move`'s `Display`
/// impl (and therefore the CLI) renders it.
fn legal_moves(game: &Game) -> Vec<String> {
    game.get_all_moves()
        .into_iter()
        .flat_map(|pm| pm.to_moves())
        .map(|mv| mv.to_string())
        .collect()
}

/// Split `Engine move: R@E1-E5` into `("R", "E1-E5")`, without a regex.
fn parse_engine_move(run: &Run) -> (String, String) {
    let line = run
        .stdout
        .lines()
        .find(|line| line.starts_with("Engine move: "))
        .unwrap_or_else(|| {
            panic!(
                "expected an `Engine move: ` line on stdout, got:\n{}",
                run.stdout
            )
        });
    let body = line.trim_start_matches("Engine move: ").trim_end();
    let (piece, notation) = body
        .split_once('@')
        .unwrap_or_else(|| panic!("expected `<piece>@<from>-<to>`, got {body:?}"));
    assert!(
        !piece.is_empty() && piece.chars().all(|c| "SBRPGNLK+".contains(c)),
        "unexpected piece rendering {piece:?} in {line:?}"
    );
    (piece.to_string(), notation.to_string())
}

/// The `from` square of a printed `<from>-<to>[-]` notation.
fn from_square(notation: &str) -> &str {
    notation
        .split('-')
        .next()
        .expect("split always yields one element")
}

fn square(name: &str) -> Position {
    let mut chars = name.chars();
    let col = chars.next().expect("a column letter");
    let row = chars.next().expect("a row digit");
    Position::new(
        col as usize - 'A' as usize,
        8 - (row as usize - '1' as usize),
    )
}

/// A hand-built position where White has exactly one good move: the rook on
/// `E1` takes the free black soldier on `E5`.
fn free_capture_game() -> Game {
    let mut board = Board::empty();
    board.set_piece(
        &square("A1"),
        Some(Piece::new(Color::White, PieceType::King, None)),
    );
    board.set_piece(
        &square("E1"),
        Some(Piece::new(Color::White, PieceType::Rook, None)),
    );
    board.set_piece(
        &square("I9"),
        Some(Piece::new(Color::Black, PieceType::King, None)),
    );
    board.set_piece(
        &square("E5"),
        Some(Piece::new(Color::Black, PieceType::Soldier, None)),
    );
    Game::from_board(board)
}

/// The first `count` legal moves of the game, picked deterministically from
/// the generator, encoded the way `--moves` expects (u16 little-endian each).
fn opening_move_bytes(count: usize) -> Vec<u8> {
    let mut game = Game::new();
    let mut bytes = Vec::with_capacity(count * 2);
    for ply in 0..count {
        let potential = game
            .get_all_moves()
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("ply {ply} has no legal move"));
        let mv = potential
            .to_moves()
            .into_iter()
            .next()
            .expect("a potential move yields at least one move");
        game.try_make(&mv)
            .expect("a generated move must be legal in its own position");
        bytes.extend_from_slice(&mv.to_u16().to_le_bytes());
    }
    bytes
}

fn moves_arg(bytes: &[u8]) -> String {
    general_purpose::STANDARD.encode(bytes)
}

// ── 1. --help / --version ───────────────────────────────────────────────────

#[test]
fn help_succeeds_and_lists_the_three_subcommands() {
    let run = keres_ok(&["--help"]);
    for subcommand in ["show-moves", "engine-move", "debug-tree"] {
        assert!(
            run.stdout.contains(subcommand),
            "`--help` should list `{subcommand}`, got:\n{}",
            run.stdout
        );
    }
    assert!(
        run.stderr.is_empty(),
        "`--help` should not write to stderr, got:\n{}",
        run.stderr
    );
}

#[test]
fn version_succeeds_and_prints_the_binary_version() {
    let run = keres_ok(&["--version"]);
    let line = run.stdout.trim();
    assert!(
        line.starts_with("keres "),
        "`--version` should print `keres <version>`, got {line:?}"
    );
    let version = line.trim_start_matches("keres ").trim();
    assert!(
        version.split('.').count() >= 2 && version.starts_with(|c: char| c.is_ascii_digit()),
        "`--version` should print a semver-looking version, got {version:?}"
    );
}

// ── 2. No arguments ─────────────────────────────────────────────────────────

#[test]
fn no_arguments_exits_zero_and_prints_guidance_to_stderr() {
    let run = keres_ok(&[]);
    assert!(
        run.stdout.is_empty(),
        "the guidance belongs on stderr, but stdout was:\n{}",
        run.stdout
    );
    assert!(
        run.stderr.contains("No command given."),
        "expected the `No command given.` guidance, got:\n{}",
        run.stderr
    );
    for subcommand in ["show-moves", "engine-move", "debug-tree"] {
        assert!(
            run.stderr.contains(subcommand),
            "the guidance should name `{subcommand}`, got:\n{}",
            run.stderr
        );
    }
}

// ── 3. Unknown subcommand ───────────────────────────────────────────────────

#[test]
fn unknown_subcommand_fails_with_a_clap_error() {
    let run = keres(&["definitely-not-a-subcommand"]);
    run.assert_no_panic();
    assert_ne!(
        run.code,
        Some(0),
        "an unknown subcommand must not succeed\nstdout:\n{}\nstderr:\n{}",
        run.stdout,
        run.stderr
    );
    assert!(
        run.stderr.contains("unrecognized subcommand"),
        "expected clap's own error on stderr, got:\n{}",
        run.stderr
    );
}

// ── 4. show-moves for the whole starting position ───────────────────────────

#[test]
fn show_moves_lists_every_movable_white_piece_at_the_start() {
    let run = keres_ok(&["show-moves"]);
    let headers: Vec<String> = run
        .stdout_lines()
        .iter()
        .filter(|line| line.starts_with("Available moves for "))
        .map(|line| {
            line.trim_start_matches("Available moves for ")
                .trim_end()
                .trim_end_matches(':')
                .to_string()
        })
        .collect();

    // Re-derive the expectation from the library instead of hardcoding a count.
    let expected = movable_piece_headers(&Game::new());
    assert_eq!(
        headers, expected,
        "the listed pieces should match the move generator's, got:\n{}",
        run.stdout
    );
    assert!(
        !expected.is_empty(),
        "the starting position must have movable white pieces"
    );
}

#[test]
fn show_moves_prints_the_destinations_of_a_starting_soldier() {
    let a3 = square("A3");
    let piece = Game::new()
        .board
        .get_piece(&a3)
        .copied()
        .expect("A3 is occupied at the start");
    assert_eq!(piece.color, Color::White);
    assert_eq!(piece.bottom, PieceType::Soldier);

    let run = keres_ok(&["show-moves"]);
    assert!(
        run.stdout.contains("Available moves for S@A3:"),
        "expected a header for the white soldier on A3, got:\n{}",
        run.stdout
    );

    let game = Game::new();
    for pm in game.get_moves(&a3) {
        assert!(
            run.stdout.contains(&format!(" - {}", pm.to)),
            "expected destination {} of the A3 soldier in:\n{}",
            pm.to,
            run.stdout
        );
    }
}

// ── 5. show-moves for a single square ───────────────────────────────────────

#[test]
fn show_moves_for_one_square_prints_only_that_piece() {
    let run = keres_ok(&["show-moves", "G2"]);
    assert!(
        run.stdout.starts_with("Available moves for R@G2:"),
        "expected the white rook on G2, got:\n{}",
        run.stdout
    );

    let game = Game::new();
    let expected: Vec<String> = game
        .get_moves(&square("G2"))
        .iter()
        .map(|pm| pm.to.to_string())
        .collect();
    let listed: Vec<String> = run
        .stdout_lines()
        .iter()
        .filter(|line| line.starts_with(" - "))
        .map(|line| {
            line.trim_start_matches(" - ")
                .split(' ')
                .next()
                .expect("a destination square")
                .to_string()
        })
        .collect();
    assert_eq!(
        listed, expected,
        "the printed destinations should match the generator's, got:\n{}",
        run.stdout
    );
}

#[test]
fn show_moves_for_an_empty_square_says_so() {
    let e5 = square("E5");
    assert!(
        Game::new().board.get_piece(&e5).is_none(),
        "E5 is empty in the starting position"
    );
    let run = keres_ok(&["show-moves", "E5"]);
    assert_eq!(
        run.stdout.trim(),
        "No moves available for position E5.",
        "unexpected stdout:\n{}",
        run.stdout
    );
}

#[test]
fn show_moves_maps_a1_to_the_bottom_left_square() {
    // `parse_position` maps A1 to (0, 8); the starting board has White's
    // left-hand ballista there.
    let piece = Game::new()
        .board
        .get_piece(&Position::new(0, 8))
        .copied()
        .expect("(0, 8) is occupied at the start");
    assert_eq!(piece.color, Color::White);
    assert_eq!(piece.bottom, PieceType::Ballista);

    let run = keres_ok(&["show-moves", "A1"]);
    assert!(
        run.stdout.starts_with("Available moves for L@A1:"),
        "A1 must resolve to the white ballista on (0, 8), got:\n{}",
        run.stdout
    );
}

#[test]
fn show_moves_maps_i9_to_the_top_right_square() {
    // Drive the other end of the mapping through a hand-built board: a lone
    // white knight on (8, 0) must be the piece `show-moves I9` reports.
    let mut board = Board::empty();
    board.set_piece(
        &Position::new(8, 0),
        Some(Piece::new(Color::White, PieceType::Knight, None)),
    );
    board.set_piece(
        &Position::new(0, 8),
        Some(Piece::new(Color::White, PieceType::King, None)),
    );
    let game = Game::from_board(board);

    let run = keres_ok(&["show-moves", "--board", &board_arg(&game), "I9"]);
    assert!(
        run.stdout.starts_with("Available moves for N@I9:"),
        "I9 must resolve to (8, 0), got:\n{}",
        run.stdout
    );
    // A lowercase square name resolves identically.
    let lower = keres_ok(&["show-moves", "--board", &board_arg(&game), "i9"]);
    assert_eq!(lower.stdout, run.stdout);
}

#[test]
fn show_moves_rejects_malformed_squares() {
    let cases = [
        ("Z9", "Invalid column"),
        ("A0", "Invalid row"),
        ("A", "Invalid position format"),
        ("A12", "Invalid position format"),
        ("", "Invalid position format"),
    ];
    for (input, expected) in cases {
        let run = keres_err(&["show-moves", input], "Error parsing position");
        assert!(
            run.stderr.contains(expected),
            "expected {expected:?} for input {input:?}, got:\n{}",
            run.stderr
        );
        assert!(
            run.stdout.is_empty(),
            "a rejected square should print nothing on stdout, got:\n{}",
            run.stdout
        );
    }
}

// ── 6. engine-move on the starting position ─────────────────────────────────

#[test]
fn engine_move_returns_a_legal_move_for_the_starting_position() {
    // The only test that pays for a full default-depth search.
    let run = keres_ok(&["engine-move"]);
    let (piece, notation) = parse_engine_move(&run);

    let game = Game::new();
    let from = from_square(&notation);
    let origin = square(from);
    let origin_piece = game
        .board
        .get_piece(&origin)
        .copied()
        .unwrap_or_else(|| panic!("the engine moved from the empty square {from}"));
    assert_eq!(
        origin_piece.color,
        Color::White,
        "the engine moved a black piece ({notation}) while White is to move"
    );
    assert_eq!(
        piece,
        keres_engine::cli_rendering::display_stack(&origin_piece),
        "the printed piece should be the one standing on {from}"
    );

    let legal = legal_moves(&game);
    assert!(
        legal.contains(&notation),
        "the engine returned {notation}, which is not one of White's {} legal opening moves",
        legal.len()
    );
}

// ── 7. engine-move on a custom board ────────────────────────────────────────

#[test]
fn engine_move_takes_a_free_piece_on_a_custom_board() {
    let game = free_capture_game();
    let capture = Move {
        from: square("E1"),
        to: square("E5"),
        unstack: false,
    };
    assert!(
        game.check_legal(&capture).is_ok(),
        "the fixture must allow the rook capture"
    );
    assert!(game.is_capture(&capture), "E1-E5 must be a capture");

    let run = keres_ok(&["engine-move", "--board", &board_arg(&game)]);
    let (piece, notation) = parse_engine_move(&run);
    assert_eq!(piece, "R");
    assert_eq!(
        notation,
        capture.to_string(),
        "the engine should take the free soldier, got:\n{}",
        run.stdout
    );
}

// ── 8. Malformed --board ────────────────────────────────────────────────────

#[test]
fn board_argument_requires_exactly_eighty_two_bytes() {
    // The right length is accepted…
    let ok = keres_ok(&["show-moves", "--board", &board_arg(&Game::new()), "A3"]);
    assert!(ok.stdout.contains("S@A3"));

    // …and both off-by-one neighbours are named in the error.
    for len in [BOARD_SIZE, BOARD_SIZE + 2] {
        let run = keres_err(
            &["show-moves", "--board", &base64_of_len(len)],
            "Error creating game",
        );
        assert!(
            run.stderr
                .contains(&format!("expected {} bytes, got {len}", BOARD_SIZE + 1)),
            "expected the length error for {len} bytes, got:\n{}",
            run.stderr
        );
    }
}

#[test]
fn board_argument_rejects_data_that_is_not_base64() {
    let run = keres_err(&["show-moves", "--board", "!!! not base64 !!!"], "Error");
    assert!(
        run.stderr.contains("Failed to decode base64 string"),
        "expected a base64 decoding error, got:\n{}",
        run.stderr
    );
}

#[test]
fn board_argument_rejects_an_invalid_piece_encoding() {
    // Bit 7 is reserved in the square encoding, so 0x80 can never be a piece.
    let mut bytes = Game::new().to_binary()[..BOARD_SIZE + 1].to_vec();
    bytes[0] = 0b1000_0000;
    let run = keres_err(
        &[
            "show-moves",
            "--board",
            &general_purpose::STANDARD.encode(&bytes),
        ],
        "Error creating game",
    );
    assert!(
        run.stderr.contains("square 0"),
        "the error should name the offending square, got:\n{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("bit 7"),
        "the error should explain the invalid encoding, got:\n{}",
        run.stderr
    );
}

#[test]
fn engine_move_rejects_a_malformed_board_without_searching() {
    let run = keres_err(&["engine-move", "--board", "????"], "Error creating game");
    assert!(
        run.stdout.is_empty(),
        "a rejected board should produce no engine move, got:\n{}",
        run.stdout
    );
}

// ── 9. debug-tree stats ─────────────────────────────────────────────────────

#[test]
fn debug_tree_prints_stats_to_stderr_and_nothing_to_stdout() {
    let run = keres_ok(&["debug-tree", "--max-depth", "2"]);
    assert!(
        run.stdout.is_empty(),
        "without --full-tree, stdout must stay empty, got:\n{}",
        run.stdout
    );
    for label in ["Search complete in", "Best move:", "Best score:", "PV:"] {
        assert!(
            run.stderr.contains(label),
            "expected {label:?} on stderr, got:\n{}",
            run.stderr
        );
    }

    let best = run
        .stderr
        .lines()
        .find_map(|line| line.strip_prefix("Best move: "))
        .expect("a `Best move:` line")
        .trim()
        .to_string();
    assert!(
        legal_moves(&Game::new()).contains(&best),
        "the reported best move {best} is not legal in the starting position"
    );
}

// ── 10. debug-tree --full-tree ──────────────────────────────────────────────

#[test]
fn debug_tree_full_tree_emits_one_json_node_per_line() {
    let run = keres_ok(&["debug-tree", "--full-tree", "--max-depth", "2"]);
    let lines = run.stdout_lines();
    assert!(
        !lines.is_empty(),
        "--full-tree should record nodes on stdout, stderr was:\n{}",
        run.stderr
    );

    // Nodes stream out as their subtrees finish (children before parents,
    // root searches interleaved), so the structure is checked over the
    // whole set rather than line by line.
    let mut nodes = std::collections::HashMap::new();
    for (index, line) in lines.iter().enumerate() {
        let node: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("stdout line {index} is not JSON ({e}): {line}"));
        let object = node
            .as_object()
            .unwrap_or_else(|| panic!("stdout line {index} is not a JSON object: {line}"));
        for field in ["id", "parent_id", "depth", "move", "unstack", "score"] {
            assert!(
                object.contains_key(field),
                "recorded node {index} is missing `{field}`: {line}"
            );
        }
        let id = object["id"].as_u64().expect("`id` is a number");
        let parent_id = object["parent_id"]
            .as_u64()
            .expect("`parent_id` is a number");
        let depth = object["depth"].as_u64().expect("`depth` is a number");
        object["score"].as_i64().expect("`score` is a number");
        let mv = object["move"].as_str().expect("`move` is a string");
        assert!(
            mv.contains('-') && mv.len() >= 5,
            "recorded node {index} has an implausible move {mv:?}"
        );
        let unstack = object["unstack"].as_bool().expect("`unstack` is a bool");
        assert_eq!(unstack, mv.ends_with('-'), "node {index}: {line}");
        let previous = nodes.insert(id, (parent_id, depth, mv.to_owned()));
        assert!(previous.is_none(), "node id {id} recorded twice");
    }

    // The tree hangs off the root position (id 0): each root move is
    // recorded once at depth 0, every other node one ply below its parent.
    let mut root_moves = std::collections::HashSet::new();
    for (id, (parent_id, depth, mv)) in &nodes {
        if *parent_id == 0 {
            assert_eq!(*depth, 0, "only root moves attach to the root: node {id}");
            assert!(root_moves.insert(mv), "root move {mv} recorded twice");
        } else {
            let (_, parent_depth, _) = nodes
                .get(parent_id)
                .unwrap_or_else(|| panic!("node {id} has an unknown parent {parent_id}"));
            assert_eq!(*depth, parent_depth + 1, "node {id}");
        }
    }
    assert!(!root_moves.is_empty(), "no root move was recorded");

    assert!(
        run.stderr.contains("Best move:"),
        "--full-tree should still print the stats on stderr, got:\n{}",
        run.stderr
    );
}

// ── 10b. openings ───────────────────────────────────────────────────────────

#[test]
fn openings_expands_close_moves_up_to_the_ply_limit() {
    let run = keres_ok(&[
        "openings",
        "--max-depth",
        "1",
        "--plies",
        "2",
        "--margin",
        "1000000",
        "--max-branch",
        "2",
    ]);
    let lines = run.stdout_lines();
    // An unbounded margin keeps exactly `max_branch` moves everywhere:
    // 2 first moves, each followed by 2 replies, and no third ply.
    let indents: Vec<usize> = lines
        .iter()
        .map(|l| l.len() - l.trim_start().len())
        .collect();
    assert_eq!(indents, [0, 2, 2, 0, 2, 2], "stdout:\n{}", run.stdout);

    let first_moves: Vec<String> = legal_moves(&Game::new());
    for (line, indent) in lines.iter().zip(&indents) {
        let (mv, score) = line
            .trim()
            .split_once(' ')
            .unwrap_or_else(|| panic!("`<move> <score>` expected: {line}"));
        score
            .parse::<i32>()
            .unwrap_or_else(|_| panic!("score is not a number: {line}"));
        if *indent == 0 {
            assert!(first_moves.contains(&mv.to_string()), "illegal: {line}");
        }
    }
    assert!(run.stderr.contains("3 searches"), "{}", run.stderr);
}

// ── 11. debug-tree --moves ──────────────────────────────────────────────────

#[test]
fn debug_tree_replays_a_valid_move_list() {
    let bytes = opening_move_bytes(3);
    assert_eq!(bytes.len(), 6);
    let replayed = Game::from_moves(&bytes).expect("the fixture move list must replay");

    let run = keres_ok(&[
        "debug-tree",
        "--moves",
        &moves_arg(&bytes),
        "--max-depth",
        "1",
    ]);
    let best = run
        .stderr
        .lines()
        .find_map(|line| line.strip_prefix("Best move: "))
        .expect("a `Best move:` line")
        .trim()
        .to_string();
    assert!(
        legal_moves(&replayed).contains(&best),
        "the best move {best} after replaying 3 plies is not legal in that position"
    );
}

#[test]
fn debug_tree_rejects_an_odd_length_move_list() {
    let run = keres_err(
        &["debug-tree", "--moves", &moves_arg(&[0x01])],
        "Failed to replay moves",
    );
    assert!(
        run.stderr.contains("multiple of 2"),
        "the error should explain the 2-byte framing, got:\n{}",
        run.stderr
    );
    assert!(run.stdout.is_empty());
}

#[test]
fn debug_tree_rejects_an_off_board_move() {
    // `from` is a 7-bit field, so 0x7F (= 127) is representable but off-board.
    let bytes = 0x007Fu16.to_le_bytes();
    let run = keres_err(
        &["debug-tree", "--moves", &moves_arg(&bytes)],
        "Failed to replay moves",
    );
    assert!(
        run.stderr.contains("move 0") && run.stderr.contains("not a valid move"),
        "the error should name the offending move index, got:\n{}",
        run.stderr
    );
}

#[test]
fn debug_tree_rejects_a_legal_looking_but_illegal_move() {
    // Well-formed encoding, empty origin square in the starting position.
    let mv = Move {
        from: square("E5"),
        to: square("E6"),
        unstack: false,
    };
    assert!(
        Move::try_from_u16(mv.to_u16()).is_some(),
        "the fixture move must decode cleanly"
    );
    let run = keres_err(
        &[
            "debug-tree",
            "--moves",
            &moves_arg(&mv.to_u16().to_le_bytes()),
        ],
        "Failed to replay moves",
    );
    assert!(
        run.stderr.contains("move 0") && run.stderr.contains("E5"),
        "the error should explain which move failed and why, got:\n{}",
        run.stderr
    );
}

#[test]
fn debug_tree_rejects_a_move_list_that_is_not_base64() {
    let run = keres_err(
        &["debug-tree", "--moves", "*not*base64*"],
        "Failed to decode base64 move sequence",
    );
    assert!(run.stdout.is_empty());
}

// ── 12. Search toggles ──────────────────────────────────────────────────────

#[test]
fn debug_tree_accepts_every_search_toggle() {
    let run = keres_ok(&[
        "debug-tree",
        "--max-depth",
        "1",
        "--no-tt",
        "--no-ab",
        "--no-quiescence",
        "--no-killers",
    ]);
    let best = run
        .stderr
        .lines()
        .find_map(|line| line.strip_prefix("Best move: "))
        .expect("a `Best move:` line")
        .trim()
        .to_string();
    assert_ne!(
        best, "(none)",
        "the search should still find a move with every heuristic disabled, stderr:\n{}",
        run.stderr
    );
    assert!(
        legal_moves(&Game::new()).contains(&best),
        "the best move {best} is not legal in the starting position"
    );
}
