//! Per-game autosave: every move is written to disk immediately, so a game
//! in progress always survives quitting the app, and every game ever played
//! is kept as a permanent record that can be browsed and resumed from the
//! main menu's LOAD GAME screen.
//!
//! Games live in a dedicated folder under the platform's standard data
//! directory (via `dirs_next::data_dir()`), e.g.
//! `~/.local/share/keres/games` on Linux. Each game gets its own file named
//! `<side>-<start-timestamp>.keres`, where `<side>` is whichever side the
//! human plays (`white`, `black`, or `hotseat` when both sides are human)
//! and the timestamp (`YYYYMMDDHHMMSS`, UTC) is fixed at game start — every
//! move simply overwrites that same file.
//!
//! Format (`[u8 version][u8 mode][u8 level][u8 status][i64 LE last_move]
//! [u16 LE move_count][u16 LE move; move_count]`) is versioned: `version`
//! is a sentinel outside the range of any byte the original, metadata-free
//! format could have produced as its first byte (that format's first byte
//! was always `mode` ∈ {0, 1, 2}), so a file from that short-lived format
//! is unambiguously rejected — `load`/`list` just drop it rather than
//! misreading it as (invalid) v2 data. Since this whole feature predates
//! any real release, that's fine: no migration path is needed.

use crate::app::Mode;
use keres_engine::Move;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// First header byte. Chosen outside {0, 1, 2} (every value the prior
/// metadata-free format could have written as its first, `mode`, byte) so
/// files from that format are always rejected rather than misparsed.
const FORMAT_VERSION: u8 = 200;
const HEADER_LEN: usize = 14; // version + mode + level + status + i64 last_move + u16 move_count

/// How a game ended, or that it hasn't. Checkmate and draw are technically
/// re-derivable by replaying the move list and asking `Game`, but
/// resignation is *not* — nothing in the moves themselves records it — so
/// this is stored explicitly for all five outcomes rather than only the one
/// that strictly needs it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    InProgress,
    WhiteWinsCheckmate,
    BlackWinsCheckmate,
    Draw,
    WhiteResigned,
    BlackResigned,
}

impl Status {
    fn to_byte(self) -> u8 {
        match self {
            Status::InProgress => 0,
            Status::WhiteWinsCheckmate => 1,
            Status::BlackWinsCheckmate => 2,
            Status::Draw => 3,
            Status::WhiteResigned => 4,
            Status::BlackResigned => 5,
        }
    }

    fn from_byte(b: u8) -> Option<Status> {
        match b {
            0 => Some(Status::InProgress),
            1 => Some(Status::WhiteWinsCheckmate),
            2 => Some(Status::BlackWinsCheckmate),
            3 => Some(Status::Draw),
            4 => Some(Status::WhiteResigned),
            5 => Some(Status::BlackResigned),
            _ => None,
        }
    }

    /// Short all-caps label for the LOAD GAME list.
    pub fn label(self) -> &'static str {
        match self {
            Status::InProgress => "IN PROGRESS",
            Status::WhiteWinsCheckmate => "WHITE WINS",
            Status::BlackWinsCheckmate => "BLACK WINS",
            Status::Draw => "DRAW",
            Status::WhiteResigned => "WHITE RESIGNED",
            Status::BlackResigned => "BLACK RESIGNED",
        }
    }
}

thread_local! {
    static DIR_OVERRIDE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Pin this thread's games folder to an isolated directory instead of the
/// real save folder — used by tests (parallel tests must never race on the
/// real folder) and by the `KERES_SNAPSHOT` headless render path (so taking
/// a snapshot never writes into the developer's actual save history).
pub fn set_test_dir_override(p: PathBuf) {
    DIR_OVERRIDE.with(|o| *o.borrow_mut() = Some(p));
}

fn games_dir() -> PathBuf {
    if let Some(p) = DIR_OVERRIDE.with(|o| o.borrow().clone()) {
        return p;
    }
    dirs_next::data_dir()
        .map(|d| d.join("keres").join("games"))
        .unwrap_or_else(|| PathBuf::from("keres_games"))
}

fn side_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Hotseat => "hotseat",
        Mode::VsAiWhite => "white",
        Mode::VsAiBlack => "black",
    }
}

fn mode_byte(mode: Mode) -> u8 {
    match mode {
        Mode::Hotseat => 0,
        Mode::VsAiWhite => 1,
        Mode::VsAiBlack => 2,
    }
}

fn mode_from_byte(b: u8) -> Option<Mode> {
    match b {
        0 => Some(Mode::Hotseat),
        1 => Some(Mode::VsAiWhite),
        2 => Some(Mode::VsAiBlack),
        _ => None,
    }
}

/// Days since the Unix epoch to a proleptic-Gregorian (year, month, day),
/// UTC. Howard Hinnant's `civil_from_days` algorithm — see
/// <http://howardhinnant.github.io/date_algorithms.html#civil_from_days>
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// `secs` (Unix time, UTC) formatted as `YYYYMMDDHHMMSS`.
pub fn format_timestamp(secs: i64) -> String {
    let (y, mo, d) = civil_from_days(secs.div_euclid(86400));
    let sod = secs.rem_euclid(86400);
    format!(
        "{:04}{:02}{:02}{:02}{:02}{:02}",
        y,
        mo,
        d,
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

/// A fresh file path for a game starting now. Nothing is written to disk
/// until the first move (see `save`).
pub fn new_path(mode: Mode) -> PathBuf {
    games_dir().join(format!(
        "{}-{}.keres",
        side_label(mode),
        format_timestamp(unix_now())
    ))
}

pub fn save(path: &Path, mode: Mode, level: u8, status: Status, moves: &[Move]) {
    let mut bytes = Vec::with_capacity(HEADER_LEN + moves.len() * 2);
    bytes.push(FORMAT_VERSION);
    bytes.push(mode_byte(mode));
    bytes.push(level);
    bytes.push(status.to_byte());
    bytes.extend_from_slice(&unix_now().to_le_bytes());
    bytes.extend_from_slice(&(moves.len() as u16).to_le_bytes());
    for mv in moves {
        bytes.extend_from_slice(&mv.to_u16().to_le_bytes());
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::File::create(path) {
        let _ = f.write_all(&bytes);
    }
}

pub fn delete(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// A fully-parsed save file: everything needed to resume the game exactly
/// as it was left (see `App::load_selected`/`resume_game`).
pub struct SaveRecord {
    pub mode: Mode,
    pub level: u8,
    pub status: Status,
    pub last_move: i64,
    pub moves: Vec<Move>,
}

pub fn load(path: &Path) -> Option<SaveRecord> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).ok()?;
    if bytes.len() < HEADER_LEN || bytes[0] != FORMAT_VERSION {
        return None;
    }
    let mode = mode_from_byte(bytes[1])?;
    let level = bytes[2];
    if !(keres_engine::engine::constants::MIN_LEVEL..=keres_engine::engine::constants::MAX_LEVEL)
        .contains(&level)
    {
        return None;
    }
    let status = Status::from_byte(bytes[3])?;
    let last_move = i64::from_le_bytes(bytes[4..12].try_into().ok()?);
    let count = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
    if bytes.len() != HEADER_LEN + count * 2 {
        return None;
    }
    let mut moves = Vec::with_capacity(count);
    for i in 0..count {
        let off = HEADER_LEN + i * 2;
        // Strict decode: a corrupt or hand-edited file can hold a square
        // index outside 0..=80, which `Move::from_u16` would panic on.
        // Reject the file instead — `load` already returns `None` for every
        // other kind of damage.
        moves.push(Move::try_from_u16(u16::from_le_bytes([
            bytes[off],
            bytes[off + 1],
        ]))?);
    }
    Some(SaveRecord {
        mode,
        level,
        status,
        last_move,
        moves,
    })
}

/// One saved game as listed from the games folder, for the LOAD GAME
/// screen.
pub struct SaveEntry {
    pub path: PathBuf,
    /// Start time parsed from the filename, `YYYYMMDDHHMMSS`.
    pub started: String,
    pub mode: Mode,
    pub level: u8,
    pub status: Status,
    /// Unix time (UTC) the most recent move was saved.
    pub last_move: i64,
}

fn filename_shape_ok(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("keres") {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let Some((_, ts)) = stem.rsplit_once('-') else {
        return false;
    };
    ts.len() == 14 && ts.bytes().all(|b| b.is_ascii_digit())
}

fn entry_from_path(path: &Path) -> Option<SaveEntry> {
    if !filename_shape_ok(path) {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    let (_, ts) = stem.rsplit_once('-')?;
    let record = load(path)?;
    Some(SaveEntry {
        path: path.to_path_buf(),
        started: ts.to_string(),
        mode: record.mode,
        level: record.level,
        status: record.status,
        last_move: record.last_move,
    })
}

/// All saved games, most recently played first.
pub fn list() -> Vec<SaveEntry> {
    let Ok(read) = std::fs::read_dir(games_dir()) else {
        return Vec::new();
    };
    let mut entries: Vec<SaveEntry> = read
        .filter_map(|e| e.ok())
        .filter_map(|e| entry_from_path(&e.path()))
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.last_move));
    entries
}

/// Cheap existence check for the menu's LOAD GAME button, without opening
/// and parsing every file.
pub fn any_exist() -> bool {
    let Ok(read) = std::fs::read_dir(games_dir()) else {
        return false;
    };
    read.filter_map(|e| e.ok())
        .any(|e| filename_shape_ok(&e.path()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use keres_engine::Position;

    #[test]
    fn save_load_roundtrip() {
        set_test_dir_override(std::env::temp_dir().join("keres_test_save_roundtrip"));
        let moves = vec![
            Move {
                from: Position::new(2, 6),
                to: Position::new(3, 5),
                unstack: false,
            },
            Move {
                from: Position::new(6, 1),
                to: Position::new(6, 4),
                unstack: true,
            },
        ];
        let path = new_path(Mode::VsAiBlack);
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("black-"));
        save(&path, Mode::VsAiBlack, 7, Status::InProgress, &moves);
        let record = load(&path).expect("save should be readable back");
        assert_eq!(record.mode, Mode::VsAiBlack);
        assert_eq!(record.level, 7);
        assert_eq!(record.status, Status::InProgress);
        assert_eq!(record.moves, moves);
        delete(&path);
        assert!(load(&path).is_none());
    }

    #[test]
    fn a_pre_metadata_file_is_rejected_not_misread() {
        set_test_dir_override(std::env::temp_dir().join("keres_test_save_legacy"));
        let path = games_dir().join("white-20250101120000.keres");
        let _ = std::fs::create_dir_all(games_dir());
        // The original format's first byte was the raw mode byte (0, 1 or
        // 2) with no version tag at all — simulate that directly.
        std::fs::write(&path, [1u8, 0, 0]).unwrap();
        assert!(load(&path).is_none());
        assert!(list().is_empty());
        delete(&path);
    }

    #[test]
    fn list_finds_saved_games_most_recently_played_first() {
        set_test_dir_override(std::env::temp_dir().join("keres_test_save_list"));
        let _ = std::fs::remove_dir_all(games_dir());
        let older = games_dir().join("white-20250101120000.keres");
        let newer = games_dir().join("black-20260202130000.keres");
        save(&older, Mode::VsAiWhite, 3, Status::InProgress, &[]);
        save(&newer, Mode::VsAiBlack, 9, Status::WhiteWinsCheckmate, &[]);
        // Force distinguishable `last_move` values regardless of clock
        // resolution: rewrite the newer file with an explicit later time.
        let mut bytes = std::fs::read(&newer).unwrap();
        bytes[4..12].copy_from_slice(&(unix_now() + 1000).to_le_bytes());
        std::fs::write(&newer, &bytes).unwrap();

        let entries = list();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, newer);
        assert_eq!(entries[0].mode, Mode::VsAiBlack);
        assert_eq!(entries[0].level, 9);
        assert_eq!(entries[0].status, Status::WhiteWinsCheckmate);
        assert_eq!(entries[1].path, older);
        assert!(any_exist());
        let _ = std::fs::remove_dir_all(games_dir());
    }
}
