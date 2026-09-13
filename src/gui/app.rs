//! Game state machine: menu / playing / game-over, click handling and the
//! background AI-search thread.

use keres_engine::{Color, Game, Move, MoveGenerator, Piece, Position, PotentialMove};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Instant;

/// How long a piece takes to slide from its source tile to its destination
/// (see `App::anim`) — short enough to never feel like it's in the way of
/// the next move, long enough to actually read as motion.
pub const MOVE_ANIM_MS: u64 = 150;

/// An in-flight slide animation for the piece (or stack) that just moved —
/// purely cosmetic, doesn't gate input: a human can select/move again while
/// it's still playing, same as before animations existed.
pub struct MoveAnim {
    pub from: Position,
    pub to: Position,
    /// The piece as it ends up at `to` after the move (post-merge,
    /// post-promotion) — what actually gets drawn sliding in.
    pub piece: Piece,
    started: Instant,
}

impl MoveAnim {
    /// Linear 0.0..=1.0 progress, clamped; `None` once the animation has run
    /// its course (the caller should drop it and draw the real board as-is).
    pub fn progress(&self) -> Option<f32> {
        let t = self.started.elapsed().as_secs_f32() / (MOVE_ANIM_MS as f32 / 1000.0);
        if t >= 1.0 {
            None
        } else {
            Some(t)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Hotseat,
    VsAiWhite, // human plays White, engine plays Black
    VsAiBlack, // human plays Black, engine plays White
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Splash,
    Menu,
    NewGame,
    Playing,
    GameOver,
    LoadGame,
}

/// Which content the sidebar's lower half shows during a game: the move
/// list, or the inline per-piece movement reference (see
/// `App::hovered_piece_help`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SidebarTab {
    Help,
    Moves,
    Settings,
}

pub struct PendingChoice {
    pub options: Vec<Move>,
}

pub struct App {
    pub screen: Screen,
    pub mode: Mode,
    pub game: Game,
    pub selected: Option<Position>,
    pub legal: Vec<PotentialMove>,
    /// Board square currently under the mouse, tracked live each frame (see
    /// `set_hovered` in `main.rs`) regardless of selection state — needed
    /// even while something is selected, to drive `hovered_stack_target`.
    pub hovered: Option<Position>,
    pub pending: Option<PendingChoice>,
    ai_rx: Option<Receiver<Option<Move>>>,
    pub ai_thinking: bool,
    /// When the current AI search started, used to animate the "AI IS
    /// THINKING..." dots (see `thinking_dots`) at a steady one-per-second
    /// pace regardless of how often the frame loop polls it.
    ai_thinking_since: Option<Instant>,
    /// The most recently applied move (by either side), so the board can
    /// highlight where it came from and landed. Cleared on a fresh/resumed
    /// game and rewound by `undo`.
    pub last_move: Option<Move>,
    /// In-flight slide animation for the piece that just moved, see
    /// `MoveAnim`.
    pub anim: Option<MoveAnim>,
    /// True while the "return to main menu?" confirmation prompt is shown
    /// over the Playing screen (see `request_menu_confirm`).
    pub confirm_menu: bool,
    pub flipped: bool,
    pub show_threats: bool,
    pub show_coords: bool,
    /// Whether the far side's piece icons render upside down — see
    /// `main.rs::is_upside_down`, toggled from the sidebar's SETTINGS tab.
    pub rotate_opponent_icons: bool,
    /// AI strength, `MIN_LEVEL`..=`MAX_LEVEL`. Chosen from the menu before
    /// starting a game; see `keres_engine::engine::SearchConfig::for_level`.
    pub level: u8,
    /// Whether the first-launch mini help modal has ever been shown —
    /// persisted (see `crate::settings`) so it never reappears after the
    /// very first game a player starts, across restarts.
    help_seen: bool,
    /// True while the first-launch mini help modal is on screen (see
    /// `start_game`/`dismiss_help`).
    pub show_help: bool,
    /// True while the fullscreen rules reference is on screen — openable
    /// from both the main menu and an in-progress game.
    pub show_rules: bool,
    /// Which content the sidebar's lower half shows (see `SidebarTab`).
    pub sidebar_tab: SidebarTab,
    /// Applied moves with whether each was a capture, for the history panel.
    pub history: Vec<(Move, bool)>,
    undo_stack: Vec<(Move, keres_engine::UndoInfo)>,
    /// File the current game is (or will be, once the first move lands)
    /// autosaved to — fixed for the life of the game, see `crate::save`.
    /// `None` on the menu, before any game has started or been resumed.
    save_path: Option<std::path::PathBuf>,
    /// Populated by `open_load_screen`, rendered by the LOAD GAME screen.
    pub load_entries: Vec<crate::save::SaveEntry>,
    /// True when a move just landed and it's the AI's turn next, but the
    /// slide animation for that move hasn't finished yet — see
    /// `apply_move`/`tick_anim`. The AI search is a multi-threaded,
    /// CPU-saturating Rayon job; starting it immediately competes with the
    /// render thread for CPU and is what makes the ~150ms slide stutter.
    /// Delaying the search until the animation completes keeps that one
    /// frame smooth at the cost of a barely-perceptible delay before
    /// "AI IS THINKING" appears.
    ai_start_pending: bool,
}

impl App {
    pub fn new() -> Self {
        let settings = crate::settings::load();
        App {
            screen: Screen::Splash,
            mode: Mode::Hotseat,
            game: Game::new(),
            selected: None,
            legal: Vec::new(),
            hovered: None,
            pending: None,
            ai_rx: None,
            ai_thinking: false,
            ai_thinking_since: None,
            last_move: None,
            anim: None,
            confirm_menu: false,
            flipped: false,
            show_threats: settings.show_threats,
            show_coords: settings.show_coords,
            rotate_opponent_icons: settings.rotate_opponent_icons,
            level: settings.level,
            help_seen: settings.help_seen,
            show_help: false,
            show_rules: false,
            sidebar_tab: SidebarTab::Help,
            history: Vec::new(),
            undo_stack: Vec::new(),
            save_path: None,
            load_entries: Vec::new(),
            ai_start_pending: false,
        }
    }

    fn persist_settings(&self) {
        crate::settings::save(&crate::settings::Settings {
            level: self.level,
            show_coords: self.show_coords,
            show_threats: self.show_threats,
            help_seen: self.help_seen,
            rotate_opponent_icons: self.rotate_opponent_icons,
        });
    }

    pub fn start_game(&mut self, mode: Mode) {
        self.mode = mode;
        self.flipped = mode == Mode::VsAiBlack;
        self.game = Game::new();
        self.selected = None;
        self.legal.clear();
        self.hovered = None;
        self.pending = None;
        self.ai_rx = None;
        self.ai_thinking = false;
        self.ai_thinking_since = None;
        self.last_move = None;
        self.anim = None;
        self.ai_start_pending = false;
        self.confirm_menu = false;
        self.history.clear();
        self.undo_stack.clear();
        self.screen = Screen::Playing;
        self.save_path = Some(crate::save::new_path(mode));
        self.note_game_launched();
        self.maybe_start_ai();
    }

    /// Show the first-launch mini help modal exactly once, ever (see
    /// `show_help`/`help_seen`) — marked seen and persisted immediately, so
    /// even quitting mid-modal doesn't bring it back on the next launch.
    fn note_game_launched(&mut self) {
        if !self.help_seen {
            self.help_seen = true;
            self.show_help = true;
            self.persist_settings();
        }
    }

    /// Open the LOAD GAME screen, listing every game found in the save
    /// folder (newest first).
    pub fn open_load_screen(&mut self) {
        self.load_entries = crate::save::list();
        self.screen = Screen::LoadGame;
    }

    /// Leave the LOAD GAME screen without resuming anything.
    pub fn close_load_screen(&mut self) {
        self.screen = Screen::Menu;
    }

    /// Resume the `index`-th entry from the last `open_load_screen` listing.
    /// Ignored if the index is out of range or the file can no longer be
    /// read (e.g. deleted or corrupted since the listing was built).
    pub fn load_selected(&mut self, index: usize) {
        let Some(entry) = self.load_entries.get(index) else {
            return;
        };
        let path = entry.path.clone();
        if let Some(record) = crate::save::load(&path) {
            self.save_path = Some(path);
            self.level = record.level;
            self.resume_game(record.mode, record.moves, record.status);
        }
    }

    /// Rebuild a game from a saved (mode, move list, outcome) triple,
    /// replaying every move so history/undo state and any in-progress AI
    /// turn are correct. Checkmate/draw are naturally reflected in
    /// `self.game` by replaying the moves, but resignation isn't — nothing
    /// in the move list records it — so `status` is applied explicitly
    /// afterwards to avoid e.g. resuming a resigned game as still playable.
    fn resume_game(&mut self, mode: Mode, moves: Vec<Move>, status: crate::save::Status) {
        self.mode = mode;
        self.game = Game::new();
        self.selected = None;
        self.legal.clear();
        self.hovered = None;
        self.pending = None;
        self.ai_rx = None;
        self.ai_thinking = false;
        self.ai_thinking_since = None;
        self.last_move = None;
        self.anim = None;
        self.ai_start_pending = false;
        self.confirm_menu = false;
        self.history.clear();
        self.undo_stack.clear();
        self.screen = Screen::Playing;
        self.note_game_launched();
        for mv in moves {
            let is_capture = self
                .game
                .board
                .get_piece(&mv.to)
                .map(|p| p.color != self.game.color_to_move())
                .unwrap_or(false);
            // Save files are just bytes on disk: a truncated write, a bad
            // sector or a hand-edited file can hold a move that is illegal in
            // the position it replays into, and `make` would panic on it.
            // Resume the longest valid prefix instead of taking the whole app
            // down.
            let Ok(undo) = self.game.try_make(&mv) else {
                break;
            };
            self.history.push((mv, is_capture));
            self.undo_stack.push((mv, undo));
            self.last_move = Some(mv);
        }
        match status {
            crate::save::Status::WhiteResigned => {
                self.game.set_game_over(true, false, false);
                self.screen = Screen::GameOver;
            }
            crate::save::Status::BlackResigned => {
                self.game.set_game_over(true, true, false);
                self.screen = Screen::GameOver;
            }
            _ => self.maybe_start_ai(),
        }
    }

    /// The current position's outcome, for the autosave header. Checkmate
    /// and draw fall straight out of `Game`; resignation is never reflected
    /// here since it isn't a board state — callers that resign must pass
    /// the resignation status to `crate::save::save` themselves.
    fn outcome_status(&self) -> crate::save::Status {
        if !self.game.is_game_over() {
            crate::save::Status::InProgress
        } else if self.game.is_draw() {
            crate::save::Status::Draw
        } else if self.game.white_wins() {
            crate::save::Status::WhiteWinsCheckmate
        } else {
            crate::save::Status::BlackWinsCheckmate
        }
    }

    pub fn is_ai_turn(&self) -> bool {
        match self.mode {
            Mode::Hotseat => false,
            Mode::VsAiWhite => self.game.color_to_move() == Color::Black,
            Mode::VsAiBlack => self.game.color_to_move() == Color::White,
        }
    }

    fn maybe_start_ai(&mut self) {
        if self.game.is_game_over() {
            self.screen = Screen::GameOver;
            return;
        }
        if self.is_ai_turn() {
            self.ai_thinking = true;
            self.ai_thinking_since = Some(Instant::now());
            let game_clone = self.game.clone();
            let config = self.ai_search_config();
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let mv = keres_engine::engine::find_best_move(&game_clone, config, None);
                let _ = tx.send(mv);
            });
            self.ai_rx = Some(rx);
        }
    }

    // The default full-depth search can take longer than a shared CI
    // runner's CPU budget allows, which made timing-based tests below
    // flaky. Tests only care that a move is produced and applied, so they
    // use a shallow depth regardless of the chosen level; real gameplay
    // uses the level-derived config.
    #[cfg(not(test))]
    fn ai_search_config(&self) -> Option<keres_engine::engine::SearchConfig> {
        Some(keres_engine::engine::SearchConfig::for_level(self.level))
    }

    #[cfg(test)]
    fn ai_search_config(&self) -> Option<keres_engine::engine::SearchConfig> {
        Some(keres_engine::engine::SearchConfig {
            max_depth: 2,
            ..Default::default()
        })
    }

    /// Call once per frame: applies the engine's move once the background
    /// search has finished.
    pub fn poll_ai(&mut self) {
        let Some(rx) = &self.ai_rx else { return };
        if let Ok(mv) = rx.try_recv() {
            self.ai_thinking = false;
            self.ai_thinking_since = None;
            self.ai_rx = None;
            if let Some(mv) = mv {
                self.apply_move(mv);
            }
        }
    }

    /// Per-frame housekeeping: drops the move-slide animation once it's run
    /// its course. Call once per frame, same as `poll_ai`.
    pub fn tick_anim(&mut self) {
        if let Some(anim) = &self.anim {
            if anim.progress().is_none() {
                self.anim = None;
                if self.ai_start_pending {
                    self.ai_start_pending = false;
                    self.maybe_start_ai();
                }
            }
        }
    }

    /// Number of dots (1..=3) to show after "AI IS THINKING", cycling once
    /// per second so the message visibly animates while the search runs.
    pub fn thinking_dots(&self) -> usize {
        match self.ai_thinking_since {
            Some(t) => (t.elapsed().as_secs() % 3) as usize + 1,
            None => 1,
        }
    }

    fn apply_move(&mut self, mv: Move) {
        let is_capture = self
            .game
            .board
            .get_piece(&mv.to)
            .map(|p| p.color != self.game.color_to_move())
            .unwrap_or(false);
        let undo = self.game.make(&mv);
        self.history.push((mv, is_capture));
        self.undo_stack.push((mv, undo));
        self.last_move = Some(mv);
        // Animate whatever actually ended up on the destination square —
        // already reflects any stack merge or promotion `make` just did.
        if let Some(piece) = self.game.board.get_piece(&mv.to) {
            self.anim = Some(MoveAnim {
                from: mv.from,
                to: mv.to,
                piece: *piece,
                started: Instant::now(),
            });
        }
        self.selected = None;
        self.legal.clear();
        self.pending = None;
        if let Some(path) = &self.save_path {
            let moves: Vec<Move> = self.history.iter().map(|(m, _)| *m).collect();
            crate::save::save(path, self.mode, self.level, self.outcome_status(), &moves);
        }
        if self.game.is_game_over() {
            self.screen = Screen::GameOver;
        } else if self.anim.is_some() && self.is_ai_turn() {
            // Don't spawn the (CPU-saturating, multi-threaded) AI search
            // until the slide animation for this move has finished playing
            // — see `ai_start_pending`'s doc comment.
            self.ai_start_pending = true;
        } else {
            self.maybe_start_ai();
        }
    }

    /// Undo the human's last move. When playing the AI, also undoes the
    /// engine's reply first so control always returns to the human.
    pub fn undo(&mut self) {
        if self.ai_thinking
            || self.ai_start_pending
            || self.pending.is_some()
            || self.confirm_menu
            || self.undo_stack.is_empty()
        {
            return;
        }
        while let Some((mv, undo)) = self.undo_stack.pop() {
            self.game.unmake(&mv, undo);
            self.history.pop();
            if self.mode == Mode::Hotseat || !self.is_ai_turn() || self.undo_stack.is_empty() {
                break;
            }
        }
        self.last_move = self.undo_stack.last().map(|(mv, _)| *mv);
        self.anim = None;
        self.ai_start_pending = false;
        self.selected = None;
        self.legal.clear();
        self.pending = None;
        self.screen = Screen::Playing;
        if let Some(path) = &self.save_path {
            if self.history.is_empty() {
                crate::save::delete(path);
            } else {
                let moves: Vec<Move> = self.history.iter().map(|(m, _)| *m).collect();
                crate::save::save(path, self.mode, self.level, self.outcome_status(), &moves);
            }
        }
    }

    /// The current side to move resigns; the opponent wins immediately.
    pub fn resign(&mut self) {
        if self.screen != Screen::Playing || self.pending.is_some() || self.confirm_menu {
            return;
        }
        let resigning = self.game.color_to_move();
        self.game
            .set_game_over(true, resigning == Color::Black, false);
        self.screen = Screen::GameOver;
        self.ai_rx = None;
        self.ai_thinking = false;
        self.ai_start_pending = false;
        if let Some(path) = &self.save_path {
            let status = if resigning == Color::White {
                crate::save::Status::WhiteResigned
            } else {
                crate::save::Status::BlackResigned
            };
            let moves: Vec<Move> = self.history.iter().map(|(m, _)| *m).collect();
            crate::save::save(path, self.mode, self.level, status, &moves);
        }
    }

    pub fn toggle_flip(&mut self) {
        self.flipped = !self.flipped;
    }

    pub fn toggle_threats(&mut self) {
        self.show_threats = !self.show_threats;
        self.persist_settings();
    }

    pub fn toggle_coords(&mut self) {
        self.show_coords = !self.show_coords;
        self.persist_settings();
    }

    pub fn toggle_rotate_opponent_icons(&mut self) {
        self.rotate_opponent_icons = !self.rotate_opponent_icons;
        self.persist_settings();
    }

    pub fn set_level(&mut self, level: u8) {
        self.level = level.clamp(
            keres_engine::engine::constants::MIN_LEVEL,
            keres_engine::engine::constants::MAX_LEVEL,
        );
        self.persist_settings();
    }

    /// Leave the splash screen for the main menu — on a click or a key press
    /// only; it never advances on its own.
    pub fn dismiss_splash(&mut self) {
        if self.screen == Screen::Splash {
            self.screen = Screen::Menu;
        }
    }

    /// Open the dedicated "new game" page (AI difficulty + mode select) from
    /// the simplified main menu.
    pub fn open_new_game_screen(&mut self) {
        self.screen = Screen::NewGame;
    }

    /// Leave the "new game" page without starting anything.
    pub fn close_new_game_screen(&mut self) {
        self.screen = Screen::Menu;
    }

    /// Dismiss the first-launch mini help modal (see `show_help`).
    pub fn dismiss_help(&mut self) {
        self.show_help = false;
    }

    /// Open the fullscreen rules reference — from the main menu or, via the
    /// sidebar, from an in-progress game.
    pub fn open_rules(&mut self) {
        self.show_rules = true;
    }

    pub fn close_rules(&mut self) {
        self.show_rules = false;
    }

    pub fn set_sidebar_tab(&mut self, tab: SidebarTab) {
        self.sidebar_tab = tab;
    }

    /// The rules of the currently-hovered square's piece (bottom, then top
    /// if stacked) — for the sidebar's HELP tab. `None` when nothing
    /// relevant is hovered.
    pub fn hovered_piece_help(
        &self,
    ) -> Option<(
        &'static crate::rules::PieceRule,
        Option<&'static crate::rules::PieceRule>,
    )> {
        let pos = self.hovered?;
        let piece = self.game.board.get_piece(&pos)?;
        let bottom = crate::rules::find(crate::render::letter_for(piece.bottom));
        let top = piece
            .top
            .map(|t| crate::rules::find(crate::render::letter_for(t)));
        Some((bottom, top))
    }

    pub fn can_undo(&self) -> bool {
        !self.ai_thinking
            && !self.ai_start_pending
            && self.pending.is_none()
            && !self.undo_stack.is_empty()
    }

    /// Update the board square currently under the mouse. Tracked live every
    /// frame regardless of selection state (see the field doc on `hovered`).
    pub fn set_hovered(&mut self, pos: Option<Position>) {
        self.hovered = pos;
    }

    /// Moves for the hovered piece, deduped by destination square, or empty
    /// if nothing relevant is hovered.
    fn hovered_piece_moves(&self, only_color: Color) -> Vec<Position> {
        let Some(pos) = self.hovered else {
            return Vec::new();
        };
        let Some(piece) = self.game.board.get_piece(&pos) else {
            return Vec::new();
        };
        if piece.color != only_color {
            return Vec::new();
        }
        let gen = MoveGenerator::new(&self.game.board, piece.color == Color::White);
        let mut out: Vec<Position> = Vec::new();
        for pm in gen.get_moves(&pos) {
            if !out.contains(&pm.to) {
                out.push(pm.to);
            }
        }
        out
    }

    /// Squares highlighted by hovering a friendly piece when nothing is
    /// selected — an immediate, lightweight preview of that piece's moves.
    pub fn hover_preview_squares(&self) -> Vec<Position> {
        if self.selected.is_some() {
            return Vec::new();
        }
        self.hovered_piece_moves(self.game.color_to_move())
    }

    /// Squares highlighted by hovering an enemy piece — its own possible
    /// moves, shown only when "show threats" is enabled (the sole purpose
    /// of that option) and nothing is selected.
    pub fn hover_threat_squares(&self) -> Vec<Position> {
        if self.selected.is_some() || !self.show_threats {
            return Vec::new();
        }
        let opponent = match self.game.color_to_move() {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        self.hovered_piece_moves(opponent)
    }

    /// True when the hovered square holds a friendly piece that the current
    /// selection could legally stack onto — used to warn the player (with a
    /// big arrow, see `main.rs`) that clicking it merges the stack rather
    /// than just reselecting that piece.
    pub fn hovered_stack_target(&self) -> bool {
        let Some(sel) = self.selected else {
            return false;
        };
        let Some(pos) = self.hovered else {
            return false;
        };
        if pos == sel || !self.legal.iter().any(|pm| pm.to == pos) {
            return false;
        }
        self.game
            .board
            .get_piece(&pos)
            .map(|p| p.color == self.game.color_to_move())
            .unwrap_or(false)
    }

    /// Handle a click on board square `pos` while in the Playing screen.
    pub fn click_square(&mut self, pos: Position) {
        if self.ai_thinking || self.ai_start_pending || self.pending.is_some() || self.confirm_menu
        {
            return;
        }
        if let Some(sel) = self.selected {
            let mut matches: Vec<Move> = Vec::new();
            for pm in self.legal.iter().filter(|pm| pm.to == pos) {
                for mv in pm.to_moves() {
                    if !matches.iter().any(|m: &Move| m.unstack == mv.unstack) {
                        matches.push(mv);
                    }
                }
            }
            if !matches.is_empty() {
                if matches.len() == 1 {
                    self.apply_move(matches[0]);
                } else {
                    self.pending = Some(PendingChoice { options: matches });
                }
                return;
            }
            self.selected = None;
            self.legal.clear();
            if pos != sel {
                self.try_select(pos);
            }
        } else {
            self.try_select(pos);
        }
    }

    fn try_select(&mut self, pos: Position) {
        if let Some(piece) = self.game.board.get_piece(&pos) {
            if piece.color == self.game.color_to_move() {
                let moves = self.game.get_moves(&pos);
                if !moves.is_empty() {
                    self.selected = Some(pos);
                    self.legal = moves;
                }
            }
        }
    }

    pub fn resolve_choice(&mut self, index: usize) {
        if let Some(choice) = self.pending.take() {
            if let Some(&mv) = choice.options.get(index) {
                self.apply_move(mv);
            } else {
                self.pending = None;
            }
        }
    }

    pub fn cancel_choice(&mut self) {
        self.pending = None;
        self.selected = None;
        self.legal.clear();
    }

    /// Deduplicated set of legal destination squares for the current
    /// selection, with whether landing there captures an enemy piece.
    pub fn target_squares(&self) -> Vec<(Position, bool)> {
        let mut out: Vec<(Position, bool)> = Vec::new();
        for pm in &self.legal {
            let is_capture = self
                .game
                .board
                .get_piece(&pm.to)
                .map(|p| p.color != self.game.color_to_move())
                .unwrap_or(false);
            if !out.iter().any(|(p, _)| *p == pm.to) {
                out.push((pm.to, is_capture));
            }
        }
        out
    }

    pub fn back_to_menu(&mut self) {
        self.screen = Screen::Menu;
        self.ai_rx = None;
        self.ai_thinking = false;
        self.ai_thinking_since = None;
        self.ai_start_pending = false;
        self.confirm_menu = false;
        // Scoped to "a game just started" — don't let it linger into the
        // menu (normal click-routing already can't reach this button while
        // the modal is up, but this is a cheap belt-and-suspenders guard).
        self.show_help = false;
    }

    /// Open the "return to main menu?" prompt (ESC or the MAIN MENU button)
    /// — only while a game is actually in progress with no other dialog
    /// already up.
    pub fn request_menu_confirm(&mut self) {
        if self.screen == Screen::Playing && self.pending.is_none() {
            self.confirm_menu = true;
        }
    }

    /// User confirmed leaving the game in progress.
    pub fn confirm_back_to_menu(&mut self) {
        self.back_to_menu();
    }

    /// User dismissed the "return to main menu?" prompt without leaving.
    pub fn cancel_menu_confirm(&mut self) {
        self.confirm_menu = false;
    }
}

/// Human-readable label for one disambiguated stack-move choice.
pub fn move_choice_label(game: &Game, mv: &Move) -> String {
    let Some(piece) = game.board.get_piece(&mv.from) else {
        return "MOVE".to_string();
    };
    if mv.unstack {
        if let Some(top) = piece.top {
            return format!("MOVE {} ONLY", piece_name(top));
        }
    }
    if piece.top.is_some() {
        "MOVE WHOLE STACK".to_string()
    } else {
        format!("MOVE {}", piece_name(piece.bottom))
    }
}

fn piece_name(pt: keres_engine::PieceType) -> &'static str {
    use keres_engine::PieceType::*;
    match pt {
        Soldier => "SOLDIER",
        Bishop => "BISHOP",
        Rook => "ROOK",
        Paladin => "PALADIN",
        Guard => "GUARD",
        Knight => "KNIGHT",
        Ballista => "BALLISTA",
        King => "KING",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn ai_thread_moves_automatically_when_it_is_its_turn() {
        let mut app = App::new();
        // Human plays Black -> AI plays White and must move first.
        app.start_game(Mode::VsAiBlack);
        assert!(
            app.ai_thinking,
            "engine should start thinking on White's turn"
        );

        let deadline = Instant::now() + Duration::from_secs(10);
        while app.ai_thinking && Instant::now() < deadline {
            app.poll_ai();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!app.ai_thinking, "engine did not finish its move in time");
        assert!(app.game.moves_without_capture() >= 1 || app.game.is_game_over());
        // A move was actually applied: it is now Black's turn.
        assert_eq!(app.game.color_to_move(), Color::Black);
    }

    #[test]
    fn hotseat_never_starts_ai_thread() {
        let mut app = App::new();
        app.start_game(Mode::Hotseat);
        assert!(!app.ai_thinking);
        assert!(!app.is_ai_turn());
    }

    #[test]
    fn ambiguous_stack_move_offers_exactly_two_deduped_choices() {
        use keres_engine::{Board, Piece, PieceType};

        let mut board = Board::empty();
        board.set_piece(
            &Position::new(4, 8),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );
        board.set_piece(
            &Position::new(4, 0),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );
        board.set_piece(
            &Position::new(4, 4),
            Some(Piece::new(
                Color::White,
                PieceType::Soldier,
                Some(PieceType::Bishop),
            )),
        );
        let mut app = App::new();
        app.mode = Mode::Hotseat;
        app.game = Game::from_board(board);

        app.click_square(Position::new(4, 4));
        assert!(app.selected.is_some(), "stacked piece should be selectable");

        // One step diagonally forward is reachable both by the soldier
        // (bottom, whole stack only) and the bishop (top, either half).
        app.click_square(Position::new(5, 3));
        let pending = app
            .pending
            .as_ref()
            .expect("expected a disambiguation prompt");
        assert_eq!(
            pending.options.len(),
            2,
            "expected exactly one deduped choice per unstack flag"
        );
        assert!(pending.options.iter().any(|m| m.unstack));
        assert!(pending.options.iter().any(|m| !m.unstack));

        app.resolve_choice(0);
        assert!(app.pending.is_none());
        assert!(app.game.board.get_piece(&Position::new(5, 3)).is_some());
    }

    #[test]
    fn undo_restores_previous_position_and_history() {
        let mut app = App::new();
        app.start_game(Mode::Hotseat);
        let before = app.game.board_hash();
        app.click_square(Position::new(2, 6));
        app.click_square(Position::new(1, 5));
        assert_eq!(app.history.len(), 1);
        assert_ne!(app.game.board_hash(), before);

        app.undo();
        assert_eq!(
            app.game.board_hash(),
            before,
            "board should match pre-move state"
        );
        assert!(app.history.is_empty());
        assert!(!app.can_undo());
    }

    #[test]
    fn undo_vs_ai_returns_control_to_the_human() {
        let mut app = App::new();
        app.start_game(Mode::VsAiBlack); // AI plays White and moves first
        let deadline = Instant::now() + Duration::from_secs(10);
        while (app.ai_thinking || app.anim.is_some()) && Instant::now() < deadline {
            app.tick_anim();
            app.poll_ai();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(app.history.len(), 1, "AI should have made its opening move");

        // Human (Black) replies with any legal move, via the real click
        // path so undo_stack stays in sync with history.
        let pm = app.game.get_all_moves()[0];
        app.click_square(pm.from);
        app.click_square(pm.to);
        assert_eq!(app.history.len(), 2, "human reply should be recorded too");

        // The human's reply immediately triggers the AI's next search;
        // real UI disables Undo while ai_thinking, so wait it out too.
        let deadline2 = Instant::now() + Duration::from_secs(10);
        while (app.ai_thinking || app.anim.is_some()) && Instant::now() < deadline2 {
            app.tick_anim();
            app.poll_ai();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(app.history.len(), 3, "AI should have replied a second time");

        app.undo();
        // Undo must land back on the human's turn, not mid-AI-turn.
        assert!(!app.is_ai_turn());
        assert!(!app.ai_thinking);
    }

    #[test]
    fn resign_ends_the_game_for_the_opponent() {
        let mut app = App::new();
        app.start_game(Mode::Hotseat); // White to move
        app.resign();
        assert_eq!(app.screen, Screen::GameOver);
        assert!(app.game.is_game_over());
        assert!(
            !app.game.white_wins(),
            "White resigned, so Black should win"
        );
    }

    #[test]
    fn save_and_resume_round_trip_reproduces_the_position() {
        crate::save::set_test_dir_override(std::env::temp_dir().join("keres_test_save_app"));
        let mut app = App::new();
        app.start_game(Mode::Hotseat);
        app.click_square(Position::new(2, 6));
        app.click_square(Position::new(1, 5));
        app.click_square(Position::new(6, 2));
        app.click_square(Position::new(7, 3));
        let expected_hash = app.game.board_hash();
        let expected_history_len = app.history.len();

        let path = app.save_path.clone().expect("game should have a save path");
        let record = crate::save::load(&path).expect("apply_move should have autosaved");
        assert_eq!(record.mode, Mode::Hotseat);
        assert_eq!(record.status, crate::save::Status::InProgress);

        let mut resumed = App::new();
        resumed.resume_game(record.mode, record.moves, record.status);
        assert_eq!(resumed.game.board_hash(), expected_hash);
        assert_eq!(resumed.history.len(), expected_history_len);
        crate::save::delete(&path);
    }

    #[test]
    fn resign_persists_a_resignation_status_that_resume_honors() {
        crate::save::set_test_dir_override(std::env::temp_dir().join("keres_test_save_resign"));
        let mut app = App::new();
        app.start_game(Mode::Hotseat); // White to move
        app.click_square(Position::new(2, 6));
        app.click_square(Position::new(1, 5));
        let path = app.save_path.clone().expect("game should have a save path");
        app.resign(); // Black resigns (it's Black's move after White's 1st)

        let record = crate::save::load(&path).expect("resign should have autosaved");
        assert_eq!(record.status, crate::save::Status::BlackResigned);

        let mut resumed = App::new();
        resumed.resume_game(record.mode, record.moves, record.status);
        assert_eq!(
            resumed.screen,
            Screen::GameOver,
            "a resumed resignation must not look like an in-progress game"
        );
        assert!(resumed.game.is_game_over());
        assert!(
            resumed.game.white_wins(),
            "White should win on Black's resignation"
        );
        crate::save::delete(&path);
    }

    #[test]
    fn load_screen_lists_and_resumes_a_saved_game() {
        crate::save::set_test_dir_override(
            std::env::temp_dir().join("keres_test_save_load_screen"),
        );
        let mut app = App::new();
        app.start_game(Mode::VsAiBlack);
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.ai_thinking && Instant::now() < deadline {
            app.poll_ai();
            std::thread::sleep(Duration::from_millis(20));
        }
        let path = app.save_path.clone().expect("game should have a save path");
        app.back_to_menu();

        let mut fresh = App::new();
        fresh.open_load_screen();
        assert_eq!(fresh.screen, Screen::LoadGame);
        assert!(
            fresh.load_entries.iter().any(|e| e.path == path),
            "the saved game should appear in the listing"
        );
        let index = fresh
            .load_entries
            .iter()
            .position(|e| e.path == path)
            .unwrap();
        fresh.load_selected(index);
        assert_eq!(fresh.screen, Screen::Playing);
        assert_eq!(fresh.mode, Mode::VsAiBlack);
        assert_eq!(fresh.history.len(), 1);

        crate::save::delete(&path);
    }
}
