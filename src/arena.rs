//! `arena`: engine-vs-engine harness for tuning the AI strength levels
//! (`SearchConfig::for_level`).
//!
//! Two subcommands:
//!
//! - `match A B` plays two engine configurations against each other with
//!   alternating colours and reports the score, an Elo-difference estimate,
//!   game lengths, how games ended, time per move, how often each side left
//!   its king en prise (split into avoidable blunders, positions its own
//!   noise-free search already saw as lost, and losses it could not see),
//!   and how often it declined an available king capture.
//! - `quality SPEC` self-plays one configuration and measures, for every
//!   move, how much worse it scored than the noise-free best move at the same
//!   depth — i.e. how much the noise/blunder dials actually cost per move.
//!
//! A side is a level preset with optional overrides, e.g. `7`, `L7`, or
//! `L7:depth=4,temp=5,blunder=0.02,bdepth=3` (keys map onto `SearchConfig`:
//! `depth` = `max_depth`, `temp` = `noise_temperature`, `blunder` =
//! `blunder_chance`, `bdepth` = `blunder_depth`, `slip` =
//! `decided_slip_chance`, `qs`/`killers` take `on`/`off`). Overrides let a
//! candidate retune be measured before it is written into `for_level`.
//!
//! `match --record FILE` appends every game to a JSONL file (see
//! `record_game`): a corpus for later analysis, kept apart from human games.
//!
//! Rules mirror the platform as far as the engine can see them: the game
//! ends on `Game::is_game_over` (king capture, 40-move rule,
//! insufficient material) or on the first recurrence of a position (the
//! same repetition rule the engine's `LoopDetector` assumes).
//!
//! With deterministic configurations (no noise, no blunders) every game from
//! the start position is a replay of one of two games; `--random-plies`
//! starts each colour-swapped pair of games from a shared random opening so
//! the sample means something.

use base64::{engine::general_purpose, Engine as _};
use clap::{Args, Parser, Subcommand};
use keres_engine::engine::constants::{
    BISHOP_VALUE, GUARD_VALUE, KING_VALUE, MAX_LEVEL, MIN_LEVEL, SOLDIER_VALUE,
};
use keres_engine::engine::search::outcome::DECIDED_SCORE;
use keres_engine::engine::search::rng::Rng;
use keres_engine::engine::{root_search, SearchConfig};
use keres_engine::game_over::GameOverReason;
use keres_engine::{Game, Move};
use serde_json::json;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::io::Write;
use std::str::FromStr;
use std::time::{Duration, Instant};

// musl's default allocator has severe lock contention under multi-threading
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser)]
#[command(about = "Engine-vs-engine arena for tuning AI strength levels")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Play side A against side B with alternating colours
    Match(MatchArgs),
    /// Self-play one side and measure per-move score loss vs. its noise-free search
    Quality(QualityArgs),
}

#[derive(Args)]
struct MatchArgs {
    /// Side A, e.g. `7` or `L7:temp=5,blunder=0.02,bdepth=3`
    a: Side,
    /// Side B
    b: Side,
    /// Number of games (A plays White in the even-numbered ones)
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..))]
    games: u32,
    /// Random legal plies played before the engines take over; each
    /// colour-swapped pair of games shares the same opening
    #[arg(long, default_value_t = 0)]
    random_plies: u32,
    /// Adjudicate a draw after this many plies
    #[arg(long, default_value_t = 400)]
    max_plies: u32,
    /// Append every game to this JSONL file (one object per line; see
    /// `record_game` for the format)
    #[arg(long)]
    record: Option<std::path::PathBuf>,
}

#[derive(Args)]
struct QualityArgs {
    /// Side to measure, e.g. `5` or `L5:temp=8`
    side: Side,
    /// Number of self-play games
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u32).range(1..))]
    games: u32,
    /// Stop each game after this many plies
    #[arg(long, default_value_t = 200)]
    max_plies: u32,
}

/// One engine configuration: a level preset plus any overrides.
#[derive(Clone)]
struct Side {
    label: String,
    config: SearchConfig,
}

impl FromStr for Side {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let (level, overrides) = s.split_once(':').unwrap_or((s, ""));
        let level: u8 = level
            .trim_start_matches(['L', 'l'])
            .parse()
            .map_err(|_| format!("`{s}`: expected a level number, e.g. `7` or `L7`"))?;
        if !(MIN_LEVEL..=MAX_LEVEL).contains(&level) {
            return Err(format!("level {level} outside {MIN_LEVEL}..={MAX_LEVEL}"));
        }
        let mut config = SearchConfig::for_level(level);
        for kv in overrides.split(',').filter(|kv| !kv.is_empty()) {
            let (key, value) = kv
                .split_once('=')
                .ok_or_else(|| format!("`{kv}`: expected key=value"))?;
            let bad = || format!("`{kv}`: invalid value");
            let flag = |v: &str| match v {
                "on" => Ok(true),
                "off" => Ok(false),
                _ => Err(format!("`{kv}`: expected on/off")),
            };
            match key {
                "depth" => config.max_depth = value.parse().ok().ok_or_else(bad)?,
                "temp" => config.noise_temperature = value.parse().ok().ok_or_else(bad)?,
                "blunder" => config.blunder_chance = value.parse().ok().ok_or_else(bad)?,
                "bdepth" => config.blunder_depth = value.parse().ok().ok_or_else(bad)?,
                "slip" => config.decided_slip_chance = value.parse().ok().ok_or_else(bad)?,
                "qs" => config.use_quiescence = flag(value)?,
                "killers" => config.use_killers = flag(value)?,
                _ => {
                    return Err(format!(
                        "unknown key `{key}` (depth, temp, blunder, bdepth, slip, qs, killers)"
                    ))
                }
            }
        }
        if config.max_depth == 0 || config.blunder_depth == 0 {
            return Err("depth and bdepth must be at least 1".into());
        }
        if config.noise_temperature.is_nan() || config.noise_temperature < 0.0 {
            return Err("temp must be >= 0".into());
        }
        if !(0.0..=1.0).contains(&config.blunder_chance)
            || !(0.0..=1.0).contains(&config.decided_slip_chance)
        {
            return Err("blunder and slip must be within 0..=1".into());
        }
        Ok(Side {
            label: s.to_string(),
            config,
        })
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let c = &self.config;
        write!(
            f,
            "{} (depth {}, temp {}, blunder {} at depth {}, slip {}, qs {}, killers {})",
            self.label,
            c.max_depth,
            c.noise_temperature,
            c.blunder_chance,
            c.blunder_depth,
            c.decided_slip_chance,
            on_off(c.use_quiescence),
            on_off(c.use_killers)
        )
    }
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

fn main() {
    match Cli::parse().command {
        Command::Match(args) => run_match(&args),
        Command::Quality(args) => run_quality(&args),
    }
}

/// Whether the side to move can capture the opponent's king right now.
fn king_capturable(game: &Game) -> bool {
    game.get_all_moves()
        .iter()
        .flat_map(|pm| pm.to_moves())
        .any(|mv| game.board.get_piece(&mv.to).is_some_and(|p| p.is_king()))
}

fn noise_free(config: &SearchConfig) -> SearchConfig {
    SearchConfig {
        noise_temperature: 0.0,
        blunder_chance: 0.0,
        decided_slip_chance: 0.0,
        ..config.clone()
    }
}

/// A position plus the board-hash history the engine and repetition rule need.
struct Playout {
    game: Game,
    history: Vec<u64>,
    seen: HashSet<u64>,
}

impl Playout {
    fn start() -> Self {
        let game = Game::new();
        let hash = game.board_hash();
        Playout {
            game,
            history: vec![hash],
            seen: HashSet::from([hash]),
        }
    }

    /// Play `mv`; returns `true` if it repeated an earlier position.
    fn play(&mut self, mv: &Move) -> bool {
        self.game.make(mv);
        let hash = self.game.board_hash();
        self.history.push(hash);
        !self.seen.insert(hash)
    }
}

/// `plies` uniformly random legal moves from the start position that neither
/// end the game nor repeat a position. Retries from scratch on a dead end.
fn random_opening(plies: u32, rng: &mut Rng) -> Vec<Move> {
    'retry: loop {
        let mut pos = Playout::start();
        let mut line = Vec::with_capacity(plies as usize);
        for _ in 0..plies {
            let moves: Vec<Move> = pos
                .game
                .get_all_moves()
                .iter()
                .flat_map(|pm| pm.to_moves())
                .collect();
            if moves.is_empty() {
                continue 'retry;
            }
            let mv = moves[((rng.next_f32() * moves.len() as f32) as usize).min(moves.len() - 1)];
            if pos.play(&mv) || pos.game.is_game_over() {
                continue 'retry;
            }
            line.push(mv);
        }
        return line;
    }
}

#[derive(Default)]
struct SideStats {
    moves: u32,
    time: Duration,
    /// Left the king capturable although the noise-free search at the same
    /// depth would not have, from a position it did not already see as lost:
    /// a blunder caused purely by the noise/blunder dials.
    hung_king_avoidable: u32,
    /// Left the king capturable from a position the noise-free search already
    /// scores as a forced king loss — resignation-grade, not a blunder.
    hung_king_already_lost: u32,
    /// Left the king capturable and the noise-free search would have too,
    /// without seeing the loss (only possible below depth 2).
    hung_king_unseen: u32,
    /// Could capture the enemy king and played something else.
    missed_king_capture: u32,
}

impl SideStats {
    fn add(&mut self, other: &SideStats) {
        self.moves += other.moves;
        self.time += other.time;
        self.hung_king_avoidable += other.hung_king_avoidable;
        self.hung_king_already_lost += other.hung_king_already_lost;
        self.hung_king_unseen += other.hung_king_unseen;
        self.missed_king_capture += other.missed_king_capture;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Ending {
    KingCaptured,
    FortyMoves,
    InsufficientMaterial,
    Repetition,
    NoMoves,
    PlyCap,
}

impl fmt::Display for Ending {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Ending::KingCaptured => "king captured",
            Ending::FortyMoves => "40-move rule",
            Ending::InsufficientMaterial => "insufficient material",
            Ending::Repetition => "repetition",
            Ending::NoMoves => "no legal moves",
            Ending::PlyCap => "ply cap",
        })
    }
}

struct GameRecord {
    /// +1 White won, 0 draw, -1 Black won.
    result: i32,
    plies: u32,
    ending: Ending,
    /// Every move played, opening included.
    moves: Vec<Move>,
    /// Per move: the mover's search score for the move it played, from
    /// White's point of view, at the depth it actually searched (a blunder's
    /// shallower depth included). `None` for the random opening.
    evals: Vec<Option<i32>>,
}

fn ending_of(game: &Game) -> Ending {
    match game
        .game_over_reason()
        .expect("a finished arena game always ended on a rule")
    {
        GameOverReason::KingCaptured => Ending::KingCaptured,
        GameOverReason::FortyMoveRule => Ending::FortyMoves,
        GameOverReason::InsufficientMaterial => Ending::InsufficientMaterial,
    }
}

/// Play one game from `opening`; `stats[0]` is White's, `stats[1]` Black's.
fn play_game(
    white: &Side,
    black: &Side,
    opening: &[Move],
    max_plies: u32,
    stats: &mut [SideStats; 2],
) -> GameRecord {
    let mut pos = Playout::start();
    let mut moves = opening.to_vec();
    let mut evals = vec![None; opening.len()];
    for mv in opening {
        pos.play(mv);
    }
    let (result, ending) = loop {
        if pos.game.is_game_over() {
            let result = if pos.game.is_draw() {
                0
            } else if pos.game.white_wins() {
                1
            } else {
                -1
            };
            break (result, ending_of(&pos.game));
        }
        if moves.len() as u32 >= max_plies {
            break (0, Ending::PlyCap);
        }
        let white_to_move = pos.game.is_white_to_move();
        let (side, s) = if white_to_move {
            (white, &mut stats[0])
        } else {
            (black, &mut stats[1])
        };
        let start = Instant::now();
        let searched = root_search(&pos.game, &side.config, &pos.history, None);
        s.time += start.elapsed();
        s.moves += 1;
        let Some(mv) = searched.best_move else {
            break (if white_to_move { -1 } else { 1 }, Ending::NoMoves);
        };
        let could_take_king = king_capturable(&pos.game);
        let before = pos.game.clone();
        let repeated = pos.play(&mv);
        // Any immediate win (a king capture) is equally fast.
        let won_now = pos.game.is_game_over()
            && !pos.game.is_draw()
            && pos.game.white_wins() == white_to_move;
        if could_take_king && !won_now {
            s.missed_king_capture += 1;
        }
        moves.push(mv);
        evals.push(Some(if white_to_move {
            searched.best_score
        } else {
            -searched.best_score
        }));
        if !pos.game.is_game_over() && king_capturable(&pos.game) {
            // What would the same search without noise/blunders have done?
            // (`history` without the move just played is the root's history.)
            let root_history = &pos.history[..pos.history.len() - 1];
            let clean = root_search(&before, &noise_free(&side.config), root_history, None);
            let clean_hangs = clean.best_move.is_some_and(|cm| {
                let mut after = before.clone();
                after.make(&cm);
                !after.is_game_over() && king_capturable(&after)
            });
            if clean.best_score <= -DECIDED_SCORE {
                s.hung_king_already_lost += 1;
            } else if clean_hangs {
                s.hung_king_unseen += 1;
            } else {
                s.hung_king_avoidable += 1;
            }
        }
        if repeated && !pos.game.is_game_over() {
            break (0, Ending::Repetition);
        }
    };
    GameRecord {
        result,
        plies: moves.len() as u32,
        ending,
        moves,
        evals,
    }
}

/// Append `record` to `out` as one JSON line:
///
/// ```json
/// {"white": "L7", "black": "L8:temp=2", "white_config": {...}, "black_config": {...},
///  "opening_plies": 4, "moves": "<base64>", "evals": [null, ..., 12, -3],
///  "result": "1-0" | "0-1" | "1/2-1/2", "ending": "king captured", "plies": 97}
/// ```
///
/// `moves` is the same encoding `/engine-move-game` takes (`docs/PROTOCOL.md`):
/// each move a little-endian `u16` (`Move::to_u16`), concatenated, base64.
/// `evals[i]` is the score of move `i` from White's point of view (see
/// `GameRecord::evals`); `null` for the random opening.
fn record_game(
    out: &mut impl Write,
    white: &Side,
    black: &Side,
    opening_plies: usize,
    record: &GameRecord,
) -> std::io::Result<()> {
    let bytes: Vec<u8> = record
        .moves
        .iter()
        .flat_map(|mv| mv.to_u16().to_le_bytes())
        .collect();
    let config = |c: &SearchConfig| {
        json!({
            "max_depth": c.max_depth,
            "noise_temperature": c.noise_temperature,
            "blunder_chance": c.blunder_chance,
            "blunder_depth": c.blunder_depth,
            "decided_slip_chance": c.decided_slip_chance,
            "quiescence": c.use_quiescence,
            "killers": c.use_killers,
        })
    };
    let line = json!({
        "white": white.label,
        "black": black.label,
        "white_config": config(&white.config),
        "black_config": config(&black.config),
        "opening_plies": opening_plies,
        "moves": general_purpose::STANDARD.encode(bytes),
        "evals": record.evals,
        "result": match record.result { 1 => "1-0", -1 => "0-1", _ => "1/2-1/2" },
        "ending": record.ending.to_string(),
        "plies": record.plies,
    });
    writeln!(out, "{line}")?;
    out.flush()
}

/// Elo difference for an expected score `p` in `(0, 1)`.
fn elo(p: f64) -> f64 {
    -400.0 * (1.0 / p - 1.0).log10()
}

fn fmt_elo(p: f64) -> String {
    if p <= 0.0 {
        "-inf".into()
    } else if p >= 1.0 {
        "+inf".into()
    } else {
        format!("{:+.0}", elo(p))
    }
}

fn run_match(args: &MatchArgs) {
    let (a, b) = (&args.a, &args.b);
    println!("A = {a}");
    println!("B = {b}");
    let mut rng = Rng::new();
    let mut stats = [SideStats::default(), SideStats::default()]; // [A, B]
    let mut scores = Vec::with_capacity(args.games as usize); // A's score per game
    let mut lengths = Vec::with_capacity(args.games as usize);
    let mut endings: BTreeMap<Ending, u32> = BTreeMap::new();
    let mut opening = Vec::new();
    let mut recorder = args.record.as_ref().map(|path| {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap_or_else(|e| {
                eprintln!("cannot open {}: {e}", path.display());
                std::process::exit(2);
            });
        std::io::BufWriter::new(file)
    });

    for g in 0..args.games {
        if g % 2 == 0 {
            opening = random_opening(args.random_plies, &mut rng);
        }
        let a_white = g % 2 == 0;
        let (white, black) = if a_white { (a, b) } else { (b, a) };
        let mut game_stats = [SideStats::default(), SideStats::default()];
        let record = play_game(white, black, &opening, args.max_plies, &mut game_stats);
        if let Some(out) = recorder.as_mut() {
            if let Err(e) = record_game(out, white, black, opening.len(), &record) {
                eprintln!("cannot write game record: {e}");
                std::process::exit(2);
            }
        }
        let (w, bl) = (&game_stats[0], &game_stats[1]);
        let (sa, sb) = if a_white { (w, bl) } else { (bl, w) };
        stats[0].add(sa);
        stats[1].add(sb);
        let a_result = if a_white {
            record.result
        } else {
            -record.result
        };
        scores.push((a_result + 1) as f64 / 2.0);
        lengths.push(record.plies);
        *endings.entry(record.ending).or_default() += 1;
        let outcome = match a_result {
            1 => "A wins",
            -1 => "B wins",
            _ => "draw",
        };
        eprintln!(
            "game {:>3}: A as {}, {outcome} in {} plies ({})",
            g + 1,
            if a_white { "White" } else { "Black" },
            record.plies,
            record.ending
        );
    }

    let n = scores.len() as f64;
    let total: f64 = scores.iter().sum();
    let p = total / n;
    let wins = scores.iter().filter(|&&s| s == 1.0).count();
    let draws = scores.iter().filter(|&&s| s == 0.5).count();
    let losses = scores.len() - wins - draws;
    // 95% interval on the expected score (normal approximation over games),
    // mapped through the Elo curve.
    let var = scores.iter().map(|s| (s - p).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    let half = 1.96 * (var / n).sqrt();
    let mut sorted = lengths.clone();
    sorted.sort_unstable();

    println!();
    println!(
        "A score {total}/{n} ({:.1}%)  W/D/L {wins}/{draws}/{losses}",
        p * 100.0
    );
    println!(
        "Elo(A - B) {}  (95%: {} .. {})",
        fmt_elo(p),
        fmt_elo(p - half),
        fmt_elo(p + half)
    );
    println!(
        "plies: min {} / median {} / max {}  {lengths:?}",
        sorted[0],
        sorted[sorted.len() / 2],
        sorted[sorted.len() - 1]
    );
    let endings: Vec<String> = endings.iter().map(|(e, c)| format!("{e} {c}")).collect();
    println!("endings: {}", endings.join(", "));
    for (name, s) in [("A", &stats[0]), ("B", &stats[1])] {
        println!(
            "{name}: {} moves, {:.0} ms/move; king left en prise: {} avoidable, {} already lost, {} unseen; missed king captures: {}",
            s.moves,
            s.time.as_secs_f64() * 1000.0 / s.moves.max(1) as f64,
            s.hung_king_avoidable,
            s.hung_king_already_lost,
            s.hung_king_unseen,
            s.missed_king_capture
        );
    }
}

fn run_quality(args: &QualityArgs) {
    let side = &args.side;
    println!("{side}");
    let clean = noise_free(&side.config);
    let mut losses: Vec<i32> = Vec::new();
    let mut time = Duration::ZERO;
    for g in 0..args.games {
        let mut pos = Playout::start();
        for _ in 0..args.max_plies {
            if pos.game.is_game_over() {
                break;
            }
            let best = root_search(&pos.game, &clean, &pos.history, None).best_score;
            let start = Instant::now();
            let played = root_search(&pos.game, &side.config, &pos.history, None);
            time += start.elapsed();
            let Some(mv) = played.best_move else { break };
            losses.push(best - played.best_score);
            if pos.play(&mv) {
                break;
            }
        }
        eprintln!(
            "game {}/{} done, {} moves so far",
            g + 1,
            args.games,
            losses.len()
        );
    }
    if losses.is_empty() {
        println!("no moves played");
        return;
    }
    let n = losses.len() as f64;
    let pct =
        |pred: &dyn Fn(i32) -> bool| losses.iter().filter(|&&l| pred(l)).count() as f64 / n * 100.0;
    // Cap at a rook-and-a-half so a handful of king losses don't swamp the mean.
    let capped_mean = losses.iter().map(|&l| l.min(100) as f64).sum::<f64>() / n;
    println!(
        "{} moves, {:.0} ms/move",
        losses.len(),
        time.as_secs_f64() * 1000.0 / n
    );
    println!("best move played        {:5.1}%", pct(&|l| l <= 0));
    println!(
        "loss >= {SOLDIER_VALUE:<4} (soldier)  {:5.1}%",
        pct(&|l| l >= SOLDIER_VALUE)
    );
    println!(
        "loss >= {GUARD_VALUE:<4} (guard)    {:5.1}%",
        pct(&|l| l >= GUARD_VALUE)
    );
    println!(
        "loss >= {BISHOP_VALUE:<4} (bishop)   {:5.1}%",
        pct(&|l| l >= BISHOP_VALUE)
    );
    println!(
        "loss >= {:<4} (king)     {:5.1}%",
        KING_VALUE / 2,
        pct(&|l| l >= KING_VALUE / 2)
    );
    println!("mean loss (capped 100)  {capped_mean:5.1}");
}
