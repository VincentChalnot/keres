#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod base;
mod font;
mod icons;
mod platform_icon;
mod render;
mod rules;
mod save;
mod settings;
mod splash;
mod symbols;
// Only X11 sets its window icon from this buffer at runtime (see
// platform_icon.rs) — Windows gets its icon from the PE resource build.rs
// embeds, macOS from the .app bundle's .icns.
#[cfg(target_os = "linux")]
mod window_icon;

use app::{move_choice_label, App, Mode, Screen};
use keres_engine::{Move, Piece, Position};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use render::{Canvas, BOARD_PX_H, BOARD_PX_W, TILE_H, TILE_W};

struct Rect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Rect {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }
    fn cx(&self) -> i32 {
        (self.x0 + self.x1) / 2
    }
    fn cy(&self) -> i32 {
        (self.y0 + self.y1) / 2
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsAction {
    Threats,
    Coords,
    RotateIcons,
}

/// Full-width toggle buttons for the sidebar's SETTINGS tab (see
/// `sidebar_tab_rects`), laid out the same way as `sidebar_buttons` but
/// scoped to that tab's content area (`CONTENT_TOP`) instead of the
/// always-visible button stack above it.
fn settings_tab_buttons(app: &App) -> [(Rect, String, SettingsAction); 3] {
    let x0 = render::board_w(app.show_coords) + SIDEBAR_PAD;
    let x1 = render::logical_w(app.show_coords) - SIDEBAR_PAD;
    let rect = |i: i32| Rect {
        x0,
        y0: CONTENT_TOP + i * (render::BTN_H + BTN_GAP),
        x1,
        y1: CONTENT_TOP + i * (render::BTN_H + BTN_GAP) + render::BTN_H,
    };
    [
        (
            rect(0),
            if app.show_threats {
                "HIDE THREATS".to_string()
            } else {
                "SHOW THREATS".to_string()
            },
            SettingsAction::Threats,
        ),
        (
            rect(1),
            if app.show_coords {
                "HIDE COORDS".to_string()
            } else {
                "SHOW COORDS".to_string()
            },
            SettingsAction::Coords,
        ),
        (
            rect(2),
            if app.rotate_opponent_icons {
                "UNROTATE OPPONENT ICONS".to_string()
            } else {
                "ROTATE OPPONENT ICONS".to_string()
            },
            SettingsAction::RotateIcons,
        ),
    ]
}

enum SidebarAction {
    MainMenu,
    SwitchSides,
    Undo,
    Resign,
    Rules,
}

/// The four buttons on the simplified main menu.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    NewGame,
    LoadGame,
    Rules,
    Exit,
}

/// NEW GAME / LOAD GAME / RULES / EXIT, stacked in the same slot the old
/// mode buttons used to occupy — see `draw_menu`.
fn menu_buttons(show_coords: bool) -> [(Rect, &'static str, MenuAction); 4] {
    let bw = 460;
    let bh = render::BTN_H;
    let cx = render::logical_w(show_coords) / 2;
    let gap = 10;
    let top = 260;
    let rect = |i: i32| Rect {
        x0: cx - bw / 2,
        y0: top + i * (bh + gap),
        x1: cx + bw / 2,
        y1: top + i * (bh + gap) + bh,
    };
    [
        (rect(0), "NEW GAME", MenuAction::NewGame),
        (rect(1), "LOAD GAME", MenuAction::LoadGame),
        (rect(2), "RULES", MenuAction::Rules),
        (rect(3), "EXIT", MenuAction::Exit),
    ]
}

fn menu_mode_buttons(show_coords: bool) -> [(Rect, &'static str, Mode); 3] {
    let bw = 460;
    let bh = render::BTN_H;
    let cx = render::logical_w(show_coords) / 2;
    let gap = 10;
    let top = 260;
    [
        (
            Rect {
                x0: cx - bw / 2,
                y0: top,
                x1: cx + bw / 2,
                y1: top + bh,
            },
            "VS AI - PLAY WHITE",
            Mode::VsAiWhite,
        ),
        (
            Rect {
                x0: cx - bw / 2,
                y0: top + bh + gap,
                x1: cx + bw / 2,
                y1: top + 2 * bh + gap,
            },
            "VS AI - PLAY BLACK",
            Mode::VsAiBlack,
        ),
        (
            Rect {
                x0: cx - bw / 2,
                y0: top + 2 * (bh + gap),
                x1: cx + bw / 2,
                y1: top + 3 * bh + 2 * gap,
            },
            "HOTSEAT (SAME KEYBOARD)",
            Mode::Hotseat,
        ),
    ]
}

/// Ten clickable boxes acting as a discrete 1-10 "slider" for AI strength
/// (`App::level`), laid out between the subtitle text and the mode buttons.
fn menu_level_boxes(show_coords: bool) -> [Rect; 10] {
    let bw = 40;
    let bh = 32;
    let gap = 6;
    let cx = render::logical_w(show_coords) / 2;
    let total_w = 10 * bw + 9 * gap;
    let x0 = cx - total_w / 2;
    let top = 214;
    std::array::from_fn(|i| {
        let x = x0 + i as i32 * (bw + gap);
        Rect {
            x0: x,
            y0: top,
            x1: x + bw,
            y1: top + bh,
        }
    })
}

/// "BACK" button on the "new game" page, in the slot right below the 3 mode
/// buttons — leaves enough room above the "ESC TO GO BACK" hint at the
/// bottom of the screen (see `draw_new_game`) that the two never overlap.
fn new_game_back_button_rect(show_coords: bool) -> Rect {
    let bw = 460;
    let bh = render::BTN_H;
    let gap = 10;
    let cx = render::logical_w(show_coords) / 2;
    let top = 260 + 3 * bh + 2 * gap + 12;
    Rect {
        x0: cx - bw / 2,
        y0: top,
        x1: cx + bw / 2,
        y1: top + bh,
    }
}

const LOAD_ROW_W: i32 = 560;
const LOAD_ROW_H: i32 = 44;
const LOAD_ROW_GAP: i32 = 8;
const LOAD_LIST_TOP: i32 = 130;
const LOAD_BOTTOM_PAD: i32 = 90;

fn load_row_rect(index: i32, show_coords: bool) -> Rect {
    let cx = render::logical_w(show_coords) / 2;
    let y0 = LOAD_LIST_TOP + index * (LOAD_ROW_H + LOAD_ROW_GAP);
    Rect {
        x0: cx - LOAD_ROW_W / 2,
        y0,
        x1: cx + LOAD_ROW_W / 2,
        y1: y0 + LOAD_ROW_H,
    }
}

fn load_visible_rows(show_coords: bool) -> i32 {
    let lh = render::logical_h(show_coords);
    ((lh - LOAD_LIST_TOP - LOAD_BOTTOM_PAD) / (LOAD_ROW_H + LOAD_ROW_GAP)).max(1)
}

fn load_back_button_rect(show_coords: bool) -> Rect {
    let bw = 200;
    let bh = render::BTN_H;
    let cx = render::logical_w(show_coords) / 2;
    let lh = render::logical_h(show_coords);
    Rect {
        x0: cx - bw / 2,
        y0: lh - LOAD_BOTTOM_PAD + 20,
        x1: cx + bw / 2,
        y1: lh - LOAD_BOTTOM_PAD + 20 + bh,
    }
}

/// `YYYYMMDDHHMMSS` (as parsed from a save filename) to `YYYY-MM-DD
/// HH:MM:SS` for display.
fn format_save_timestamp(ts: &str) -> String {
    if ts.len() != 14 {
        return ts.to_string();
    }
    format!(
        "{}-{}-{} {}:{}:{}",
        &ts[0..4],
        &ts[4..6],
        &ts[6..8],
        &ts[8..10],
        &ts[10..12],
        &ts[12..14]
    )
}

/// Unix seconds (UTC) to `YYYY-MM-DD HH:MM:SS` for display.
fn format_unix_timestamp(secs: i64) -> String {
    format_save_timestamp(&save::format_timestamp(secs))
}

fn side_label(mode: Mode) -> &'static str {
    match mode {
        Mode::VsAiWhite => "WHITE (VS AI)",
        Mode::VsAiBlack => "BLACK (VS AI)",
        Mode::Hotseat => "HOTSEAT",
    }
}

/// "X" button that closes the fullstack/unstack disambiguation dialog,
/// cancelling the move — top-right corner of the board area.
fn close_button_rect(show_coords: bool) -> Rect {
    let size = 26;
    let pad = 14;
    let x1 = render::board_w(show_coords) - pad;
    let y0 = pad;
    Rect {
        x0: x1 - size,
        y0,
        x1,
        y1: y0 + size,
    }
}

/// YES/NO buttons for the "return to main menu?" confirmation prompt.
fn confirm_menu_buttons(show_coords: bool) -> (Rect, Rect) {
    let bw = 160;
    let bh = render::BTN_H;
    let gap = 20;
    let cx = render::board_w(show_coords) / 2;
    let cy = render::logical_h(show_coords) / 2 + 10;
    let total_w = 2 * bw + gap;
    let x0 = cx - total_w / 2;
    let yes = Rect {
        x0,
        y0: cy,
        x1: x0 + bw,
        y1: cy + bh,
    };
    let no = Rect {
        x0: x0 + bw + gap,
        y0: cy,
        x1: x0 + bw + gap + bw,
        y1: cy + bh,
    };
    (yes, no)
}

fn choice_buttons(n: usize, show_coords: bool) -> Vec<Rect> {
    let bw = 340;
    let bh = render::BTN_H;
    let gap = 10;
    let cx = render::board_w(show_coords) / 2;
    let total_h = n as i32 * bh + (n as i32 - 1) * gap;
    let top = (render::logical_h(show_coords) - total_h) / 2;
    (0..n)
        .map(|i| {
            let y0 = top + i as i32 * (bh + gap);
            Rect {
                x0: cx - bw / 2,
                y0,
                x1: cx + bw / 2,
                y1: y0 + bh,
            }
        })
        .collect()
}

const SIDEBAR_PAD: i32 = 14;
const BTN_GAP: i32 = 6;
const SIDEBAR_BUTTONS_TOP: i32 = 120;

fn sidebar_button_rect(index: i32, show_coords: bool) -> Rect {
    let x0 = render::board_w(show_coords) + SIDEBAR_PAD;
    let x1 = render::logical_w(show_coords) - SIDEBAR_PAD;
    let y0 = SIDEBAR_BUTTONS_TOP + index * (render::BTN_H + BTN_GAP);
    Rect {
        x0,
        y0,
        x1,
        y1: y0 + render::BTN_H,
    }
}

fn sidebar_buttons(app: &App) -> [(Rect, String, SidebarAction, bool); 5] {
    [
        (
            sidebar_button_rect(0, app.show_coords),
            "MAIN MENU".to_string(),
            SidebarAction::MainMenu,
            true,
        ),
        (
            sidebar_button_rect(1, app.show_coords),
            "SWITCH SIDES".to_string(),
            SidebarAction::SwitchSides,
            true,
        ),
        (
            sidebar_button_rect(2, app.show_coords),
            "UNDO".to_string(),
            SidebarAction::Undo,
            app.can_undo(),
        ),
        (
            sidebar_button_rect(3, app.show_coords),
            "RESIGN".to_string(),
            SidebarAction::Resign,
            true,
        ),
        (
            sidebar_button_rect(4, app.show_coords),
            "RULES".to_string(),
            SidebarAction::Rules,
            true,
        ),
    ]
}

const HISTORY_TOP: i32 = 334;
const HISTORY_ROW_H: i32 = 13;
// Extra breathing room between the HELP|MOVES|SETTINGS tab strip and its
// content, so the content doesn't feel glued to the tab underline.
const TAB_CONTENT_GAP: i32 = 10;
const CONTENT_TOP: i32 = HISTORY_TOP + TAB_CONTENT_GAP;

fn history_visible_rows(show_coords: bool) -> i32 {
    (render::logical_h(show_coords) - CONTENT_TOP - SIDEBAR_PAD) / HISTORY_ROW_H
}

/// Screen-space (col, row) for a board position, honoring the board flip.
fn screen_coord(pos: Position, flipped: bool) -> (i32, i32) {
    if flipped {
        (8 - pos.x as i32, 8 - pos.y as i32)
    } else {
        (pos.x as i32, pos.y as i32)
    }
}

/// Inverse of `screen_coord`: board position from a screen column/row.
fn board_pos(col: i32, row: i32, flipped: bool) -> Position {
    if flipped {
        Position::new((8 - col) as usize, (8 - row) as usize)
    } else {
        Position::new(col as usize, row as usize)
    }
}

fn tile_at(lx: i32, ly: i32, flipped: bool, show_coords: bool) -> Option<Position> {
    let bx = lx - render::gutter(show_coords);
    let by = ly - render::topbar(show_coords);
    if bx < 0 || by < 0 || bx >= BOARD_PX_W || by >= BOARD_PX_H {
        return None;
    }
    let col = bx / TILE_W;
    let row = by / TILE_H;
    Some(board_pos(col, row, flipped))
}

/// The opponent's pieces are drawn upside down, like a physical two-player
/// board where each side's own tokens face them. "Opponent" here means
/// whichever color is rendered at the top of the screen — which flips with
/// `app.flipped`, not with whose turn it is.
fn is_upside_down(color: keres_engine::Color, flipped: bool, rotate_opponent_icons: bool) -> bool {
    if !rotate_opponent_icons {
        return false;
    }
    let near_side = if flipped {
        keres_engine::Color::Black
    } else {
        keres_engine::Color::White
    };
    color != near_side
}

fn move_notation(mv: &Move, is_capture: bool) -> String {
    format!(
        "{}-{}{}{}",
        mv.from,
        mv.to,
        if mv.unstack { "-" } else { "" },
        if is_capture { "*" } else { "" },
    )
}

/// Crest-over-wordmark composition shared by the splash screen and main
/// menu (see `draw_splash`/`draw_menu`) — `splash::LOGO` centered above
/// `splash::TITLE`, bottom-aligned to `bottom_y` so callers can drop it in
/// wherever the old bitmap-font "KERES" text used to sit. `gap` is the
/// logo-to-title spacing in logical pixels — tune per caller (see
/// `draw_splash`'s/`draw_menu`'s call sites just below).
fn draw_wordmark(c: &mut Canvas, cx: i32, bottom_y: i32, gap: i32, color: u32) {
    let title_y = bottom_y - splash::TITLE_H as i32;
    let logo_y = title_y - gap - splash::LOGO_H as i32;
    c.draw_wide_bitmap_centered(cx, logo_y, &splash::LOGO, splash::LOGO_W as i32, color);
    c.draw_wide_bitmap_centered(cx, title_y, &splash::TITLE, splash::TITLE_W as i32, color);
}

/// The simplified main menu: title, subtitle, and the four top-level actions
/// (NEW GAME / LOAD GAME / RULES / EXIT). AI difficulty and side selection
/// live on the dedicated `draw_new_game` page instead.
fn draw_menu(c: &mut Canvas, show_coords: bool, mouse: Option<(i32, i32)>) {
    let lw = render::logical_w(show_coords);
    let lh = render::logical_h(show_coords);
    let hovered = |r: &Rect| mouse.map(|(mx, my)| r.contains(mx, my)).unwrap_or(false);
    c.fill_rect(0, 0, lw, lh, render::COL_PAGE_BG);
    // Adjust these two to move the menu's logo/title/baseline block: raise
    // or lower the whole group by changing `wordmark_bottom_y`, or move just
    // the baseline relative to it via `baseline_y`.
    let wordmark_bottom_y = 148;
    let baseline_y = wordmark_bottom_y + 20;
    draw_wordmark(c, lw / 2, wordmark_bottom_y, 8, render::COL_STATUS);
    c.draw_text_centered(
        lw / 2,
        baseline_y,
        "THE ART OF ABSTRACT PLAY",
        1,
        render::COL_COORD,
    );
    let has_saves = save::any_exist();
    for (rect, label, action) in menu_buttons(show_coords) {
        let enabled = action != MenuAction::LoadGame || has_saves;
        let style = if action == MenuAction::NewGame {
            render::ButtonStyle::Primary
        } else {
            render::ButtonStyle::Normal
        };
        render::draw_button_styled(
            c,
            rect.x0,
            rect.y0,
            rect.x1,
            rect.y1,
            label,
            style,
            enabled,
            enabled && hovered(&rect),
        );
    }
    draw_footer_credit(c, lw, lh);
}

/// The dedicated "new game" page, opened from the main menu's NEW GAME
/// button: AI difficulty and side selection, previously on the main menu
/// itself (see `draw_menu`).
fn draw_new_game(c: &mut Canvas, show_coords: bool, level: u8, mouse: Option<(i32, i32)>) {
    let lw = render::logical_w(show_coords);
    let lh = render::logical_h(show_coords);
    let hovered = |r: &Rect| mouse.map(|(mx, my)| r.contains(mx, my)).unwrap_or(false);
    c.fill_rect(0, 0, lw, lh, render::COL_PAGE_BG);
    c.draw_text_centered(lw / 2, 60, "NEW GAME", 5, render::COL_STATUS);
    c.draw_text_centered(lw / 2, 190, "AI DIFFICULTY", 1, render::COL_COORD);
    for (i, rect) in menu_level_boxes(show_coords).into_iter().enumerate() {
        let selected = level as usize == i + 1;
        let color = if selected {
            render::COL_SELECT
        } else {
            render::COL_STATUS
        };
        if selected {
            c.fill_rect_alpha(rect.x0, rect.y0, rect.x1, rect.y1, render::COL_SELECT, 0.35);
        }
        c.stroke_rect(rect.x0, rect.y0, rect.x1, rect.y1, 2, color);
        c.draw_text_centered(rect.cx(), rect.cy() - 5, &(i + 1).to_string(), 1, color);
    }
    for (rect, label, _) in menu_mode_buttons(show_coords) {
        render::draw_button(
            c,
            rect.x0,
            rect.y0,
            rect.x1,
            rect.y1,
            label,
            true,
            hovered(&rect),
        );
    }
    let back = new_game_back_button_rect(show_coords);
    render::draw_button(
        c,
        back.x0,
        back.y0,
        back.x1,
        back.y1,
        "BACK",
        true,
        hovered(&back),
    );
    c.draw_text_centered(lw / 2, lh - 20, "ESC TO GO BACK", 1, render::COL_COORD);
}

/// "Created by ... - Play online at playkeres.com", shown on the splash
/// screen and the main menu — gone as soon as a game is in progress (see
/// `draw_board`'s Playing screen, which never calls this).
fn draw_footer_credit(c: &mut Canvas, lw: i32, lh: i32) {
    c.draw_text_centered(
        lw / 2,
        lh - 20,
        "CREATED BY VINCENT CHALNOT - PLAY ONLINE AT PLAYKERES.COM",
        1,
        render::COL_COORD,
    );
}

/// The splash screen: the game's pixel-art crest and wordmark (see
/// `draw_wordmark`) plus the footer credit line. Dismissed on a click or a
/// key press only — it never advances on its own.
fn draw_splash(c: &mut Canvas, show_coords: bool) {
    let lw = render::logical_w(show_coords);
    let lh = render::logical_h(show_coords);
    c.fill_rect(0, 0, lw, lh, render::COL_PAGE_BG);
    // Adjust these to move the splash screen's logo/title/baseline block:
    // `wordmark_bottom_y` raises/lowers the whole group, the `16` passed to
    // `draw_wordmark` is the logo-to-title gap, and `baseline_y`/
    // `prompt_y` are relative to the wordmark's bottom edge.
    let wordmark_bottom_y = lh / 2 - 30;
    let baseline_y = wordmark_bottom_y + 16;
    let prompt_y = baseline_y + 40;
    draw_wordmark(c, lw / 2, wordmark_bottom_y, 16, render::COL_STATUS);
    c.draw_text_centered(
        lw / 2,
        baseline_y,
        "THE ART OF ABSTRACT PLAY",
        1,
        render::COL_COORD,
    );
    c.draw_text_centered(
        lw / 2,
        prompt_y,
        "CLICK OR PRESS ANY KEY TO CONTINUE",
        1,
        render::COL_COORD,
    );
    draw_footer_credit(c, lw, lh);
}

fn draw_load_screen(c: &mut Canvas, app: &App, scroll: i32, mouse: Option<(i32, i32)>) {
    let show_coords = app.show_coords;
    let lw = render::logical_w(show_coords);
    let lh = render::logical_h(show_coords);
    c.fill_rect(0, 0, lw, lh, render::COL_PAGE_BG);
    c.draw_text_centered(lw / 2, 50, "LOAD GAME", 4, render::COL_STATUS);

    if app.load_entries.is_empty() {
        c.draw_text_centered(lw / 2, lh / 2, "NO SAVED GAMES", 1, render::COL_COORD);
    } else {
        let visible = load_visible_rows(show_coords);
        let max_scroll = (app.load_entries.len() as i32 - visible).max(0);
        let scroll = scroll.clamp(0, max_scroll);
        for (row, entry) in app
            .load_entries
            .iter()
            .skip(scroll as usize)
            .take(visible as usize)
            .enumerate()
        {
            let rect = load_row_rect(row as i32, show_coords);
            let over = entry.status != save::Status::InProgress;
            let color = if over {
                render::COL_COORD
            } else {
                render::COL_STATUS
            };
            c.stroke_rect(rect.x0, rect.y0, rect.x1, rect.y1, 2, color);
            let side = side_label(entry.mode);
            let top = if entry.mode == Mode::Hotseat {
                format!("{}   {}", side, entry.status.label())
            } else {
                format!("{}   LVL {}   {}", side, entry.level, entry.status.label())
            };
            c.draw_text(rect.x0 + 14, rect.y0 + 6, &top, 1, color);
            let bottom = format!(
                "START {}   LAST {}",
                format_save_timestamp(&entry.started),
                format_unix_timestamp(entry.last_move)
            );
            c.draw_text(rect.x0 + 14, rect.y0 + 24, &bottom, 1, render::COL_COORD);
        }
    }

    let back = load_back_button_rect(show_coords);
    let back_hovered = mouse.map(|(mx, my)| back.contains(mx, my)).unwrap_or(false);
    render::draw_button(
        c,
        back.x0,
        back.y0,
        back.x1,
        back.y1,
        "BACK",
        true,
        back_hovered,
    );
    c.draw_text_centered(lw / 2, lh - 20, "ESC TO GO BACK", 1, render::COL_COORD);
}

fn draw_sidebar(c: &mut Canvas, app: &App, history_scroll: i32, mouse: Option<(i32, i32)>) {
    let x0 = render::board_w(app.show_coords);
    let lw = render::logical_w(app.show_coords);
    let lh = render::logical_h(app.show_coords);
    c.fill_rect(x0, 0, lw, lh, render::COL_SIDEBAR_BG);
    // Same crest+wordmark art as the splash screen/main menu (see
    // `draw_wordmark`), shrunk to fit the sidebar — tune `SIDEBAR_TITLE_SCALE`
    // here to resize it.
    const SIDEBAR_TITLE_SCALE: f32 = 1.0;
    c.draw_wide_bitmap_scaled(
        x0 + SIDEBAR_PAD,
        20,
        &splash::TITLE,
        splash::TITLE_W as i32,
        SIDEBAR_TITLE_SCALE,
        render::COL_STATUS,
    );

    let status = if app.ai_thinking {
        format!("AI IS THINKING{}", ".".repeat(app.thinking_dots()))
    } else if app.pending.is_some() {
        "CHOOSE A MOVE".to_string()
    } else {
        match app.game.color_to_move() {
            keres_engine::Color::White => "WHITE TO MOVE".to_string(),
            keres_engine::Color::Black => "BLACK TO MOVE".to_string(),
        }
    };
    c.draw_text(x0 + SIDEBAR_PAD, 80, &status, 1, render::COL_STATUS);

    for (rect, label, _, enabled) in sidebar_buttons(app) {
        let hovered = enabled && mouse.map(|(mx, my)| rect.contains(mx, my)).unwrap_or(false);
        render::draw_button(
            c, rect.x0, rect.y0, rect.x1, rect.y1, &label, enabled, hovered,
        );
    }

    for (rect, label, tab) in sidebar_tab_rects(app) {
        let active = app.sidebar_tab == tab;
        let color = if active {
            render::COL_SELECT
        } else {
            render::COL_COORD
        };
        c.draw_text(rect.x0, rect.y0, label, 1, color);
        if active {
            c.fill_rect(rect.x0, rect.y1 - 1, rect.x1, rect.y1, render::COL_SELECT);
        }
    }
    c.stroke_rect(
        x0 + SIDEBAR_PAD,
        HISTORY_TOP - 4,
        lw - SIDEBAR_PAD,
        HISTORY_TOP - 3,
        1,
        render::COL_COORD,
    );

    match app.sidebar_tab {
        app::SidebarTab::Moves => {
            let pairs: Vec<_> = app
                .history
                .chunks(2)
                .enumerate()
                .map(|(i, chunk)| (i + 1, chunk.first(), chunk.get(1)))
                .collect();
            let visible = history_visible_rows(app.show_coords);
            let max_scroll = (pairs.len() as i32 - visible).max(0);
            let scroll = history_scroll.clamp(0, max_scroll);
            for (row, (n, white, black)) in pairs
                .iter()
                .skip(scroll as usize)
                .take(visible as usize)
                .enumerate()
            {
                let y = CONTENT_TOP + row as i32 * HISTORY_ROW_H;
                let mut line = format!("{}.", n);
                if let Some((mv, cap)) = white {
                    line.push(' ');
                    line.push_str(&move_notation(mv, *cap));
                }
                if let Some((mv, cap)) = black {
                    line.push(' ');
                    line.push_str(&move_notation(mv, *cap));
                }
                c.draw_text(x0 + SIDEBAR_PAD, y, &line, 1, render::COL_COORD);
            }
        }
        app::SidebarTab::Help => draw_sidebar_help(c, app, x0, lw),
        app::SidebarTab::Settings => {
            for (rect, label, _) in settings_tab_buttons(app) {
                let hovered = mouse.map(|(mx, my)| rect.contains(mx, my)).unwrap_or(false);
                render::draw_button(c, rect.x0, rect.y0, rect.x1, rect.y1, &label, true, hovered);
            }
        }
    }
}

/// Tab rects for the sidebar's HELP/MOVES/SETTINGS switcher, at
/// `HISTORY_TOP - 18`. HELP comes first — it's the default tab (see
/// `App::new`).
fn sidebar_tab_rects(app: &App) -> [(Rect, &'static str, app::SidebarTab); 3] {
    let x0 = render::board_w(app.show_coords) + SIDEBAR_PAD;
    let y0 = HISTORY_TOP - 18;
    let y1 = HISTORY_TOP - 6;
    [
        (
            Rect {
                x0,
                y0,
                x1: x0 + 45,
                y1,
            },
            "HELP",
            app::SidebarTab::Help,
        ),
        (
            Rect {
                x0: x0 + 55,
                y0,
                x1: x0 + 55 + 54,
                y1,
            },
            "MOVES",
            app::SidebarTab::Moves,
        ),
        (
            Rect {
                x0: x0 + 55 + 54 + 10,
                y0,
                x1: x0 + 55 + 54 + 10 + 78,
                y1,
            },
            "SETTINGS",
            app::SidebarTab::Settings,
        ),
    ]
}

/// The sidebar's HELP tab: the hovered square's piece movement/promotion
/// (both halves of a stack, see `App::hovered_piece_help`), or a prompt when
/// nothing relevant is hovered.
fn draw_sidebar_help(c: &mut Canvas, app: &App, x0: i32, lw: i32) {
    let text_x0 = x0 + SIDEBAR_PAD + icons::ICON_N as i32 + 8;
    let text_w = lw - text_x0 - SIDEBAR_PAD;
    let wrap = |c: &mut Canvas, y: i32, text: &str, color: u32| -> i32 {
        // Simple word-wrap: greedily pack words into lines no wider than
        // the sidebar's content area, since the bitmap font has no natural
        // line-breaking of its own. Indented to line up under the name,
        // past the icon column (see the `draw_icon` call below).
        let max_chars = (text_w / (font::FONT_W as i32 + 1)).max(1) as usize;
        let mut y = y;
        let mut line = String::new();
        for word in text.split(' ') {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", line, word)
            };
            if candidate.len() > max_chars && !line.is_empty() {
                c.draw_text(text_x0, y, &line, 1, color);
                y += 14;
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            c.draw_text(text_x0, y, &line, 1, color);
            y += 14;
        }
        y
    };

    let Some((bottom, top)) = app.hovered_piece_help() else {
        c.draw_text(
            x0 + SIDEBAR_PAD,
            CONTENT_TOP,
            "HOVER A PIECE TO SEE",
            1,
            render::COL_COORD,
        );
        c.draw_text(
            x0 + SIDEBAR_PAD,
            CONTENT_TOP + 14,
            "ITS MOVES",
            1,
            render::COL_COORD,
        );
        return;
    };

    let mut y = CONTENT_TOP;
    for piece in [Some(bottom), top].into_iter().flatten() {
        c.draw_icon(
            x0 + SIDEBAR_PAD,
            y,
            piece.icon,
            1,
            render::COL_STATUS,
            false,
        );
        c.draw_text(
            text_x0,
            y,
            &format!("{}  {}", piece.letter, piece.name),
            1,
            render::COL_STATUS,
        );
        let mut ty = wrap(c, y + 14, piece.movement, render::COL_COORD);
        if let Some(promotion) = piece.promotion {
            ty = wrap(c, ty, promotion, render::COL_SELECT);
        }
        y = ty.max(y + icons::ICON_N as i32) + 10;
    }
    if top.is_some() {
        c.draw_text(
            x0 + SIDEBAR_PAD,
            y,
            "A STACK MOVES AS EITHER PIECE.",
            1,
            render::COL_COORD,
        );
    }
}

fn draw_board(c: &mut Canvas, app: &App, history_scroll: i32, mouse: Option<(i32, i32)>) {
    let show_coords = app.show_coords;
    let gutter = render::gutter(show_coords);
    let topbar = render::topbar(show_coords);
    let lw = render::logical_w(show_coords);
    let lh = render::logical_h(show_coords);

    c.fill_rect(0, 0, lw, lh, render::COL_PAGE_BG);
    draw_sidebar(c, app, history_scroll, mouse);

    for y in 0..9i32 {
        for x in 0..9i32 {
            let light = (x + y) % 2 == 0;
            let color = if light {
                render::COL_LIGHT_SQ
            } else {
                render::COL_DARK_SQ
            };
            let px0 = gutter + x * TILE_W;
            let py0 = topbar + y * TILE_H;
            c.fill_rect(px0, py0, px0 + TILE_W, py0 + TILE_H, color);
        }
    }

    // Tile-highlight overlays, palette matched to the Web platform (see
    // render::COL_HL_*): a full-tile alpha wash rather than a dot/ring.
    // Layered weakest to strongest so a stronger highlight always wins where
    // they'd otherwise overlap.
    if let Some(mv) = app.last_move {
        for pos in [mv.from, mv.to] {
            let (cx, cy) = screen_coord(pos, app.flipped);
            let px0 = gutter + cx * TILE_W;
            let py0 = topbar + cy * TILE_H;
            c.fill_rect_alpha(
                px0,
                py0,
                px0 + TILE_W,
                py0 + TILE_H,
                render::COL_HL_LAST_MOVE,
                render::COL_HL_LAST_MOVE_A,
            );
        }
    }

    for pos in app.hover_threat_squares() {
        let (cx, cy) = screen_coord(pos, app.flipped);
        let px0 = gutter + cx * TILE_W;
        let py0 = topbar + cy * TILE_H;
        c.fill_rect_alpha(
            px0,
            py0,
            px0 + TILE_W,
            py0 + TILE_H,
            render::COL_HL_THREAT,
            render::COL_HL_THREAT_A,
        );
    }

    for pos in app.hover_preview_squares() {
        let (cx, cy) = screen_coord(pos, app.flipped);
        let px0 = gutter + cx * TILE_W;
        let py0 = topbar + cy * TILE_H;
        c.fill_rect_alpha(
            px0,
            py0,
            px0 + TILE_W,
            py0 + TILE_H,
            render::COL_HL_HOVER,
            render::COL_HL_HOVER_A,
        );
    }

    if let Some(sel) = app.selected {
        let (cx, cy) = screen_coord(sel, app.flipped);
        let px0 = gutter + cx * TILE_W;
        let py0 = topbar + cy * TILE_H;
        c.fill_rect_alpha(
            px0,
            py0,
            px0 + TILE_W,
            py0 + TILE_H,
            render::COL_HL_SELECTED,
            render::COL_HL_SELECTED_A,
        );
    }

    for (pos, _is_capture) in app.target_squares() {
        let (cx, cy) = screen_coord(pos, app.flipped);
        let px0 = gutter + cx * TILE_W;
        let py0 = topbar + cy * TILE_H;
        c.fill_rect_alpha(
            px0,
            py0,
            px0 + TILE_W,
            py0 + TILE_H,
            render::COL_HL_POTENTIAL,
            render::COL_HL_POTENTIAL_A,
        );
    }

    // Pieces are drawn in screen-row order (top to bottom), not board-row
    // order — the two differ once the board is flipped (see `board_pos`).
    // This is what a stack's overflow-above-the-tile art (see
    // `render::draw_piece`) relies on: a lower screen row is always drawn
    // after the row above it, so its stack correctly paints over that row's
    // content instead of being painted over by it.
    let anim_progress = app.anim.as_ref().and_then(|a| a.progress().map(|t| (a, t)));
    for sy in 0..9i32 {
        for sx in 0..9i32 {
            let board_p = board_pos(sx, sy, app.flipped);
            if anim_progress.is_some_and(|(a, _)| board_p == a.to) {
                // Drawn separately below, sliding in from `a.from`.
                continue;
            }
            if let Some(piece) = app.game.board.get_piece(&board_p) {
                let tx = gutter + sx * TILE_W;
                let ty = topbar + sy * TILE_H;
                let upside_down =
                    is_upside_down(piece.color, app.flipped, app.rotate_opponent_icons);
                render::draw_piece(c, tx, ty, piece as &Piece, upside_down);
            }
        }
    }

    // The just-moved piece (or stack), sliding from its source tile to its
    // destination — purely cosmetic (see `App::anim`), eased out so it
    // settles rather than stopping abruptly.
    if let Some((anim, t)) = anim_progress {
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        let (fx, fy) = screen_coord(anim.from, app.flipped);
        let (tx, ty) = screen_coord(anim.to, app.flipped);
        let px = gutter as f32 + (fx as f32 + (tx - fx) as f32 * eased) * TILE_W as f32;
        let py = topbar as f32 + (fy as f32 + (ty - fy) as f32 * eased) * TILE_H as f32;
        let upside_down = is_upside_down(anim.piece.color, app.flipped, app.rotate_opponent_icons);
        render::draw_piece(
            c,
            px.round() as i32,
            py.round() as i32,
            &anim.piece,
            upside_down,
        );
    }

    // Big "this creates a stack" arrow: when a piece is selected and the
    // hovered square is a legal stacking target (a friendly piece it could
    // land on), it's ambiguous whether clicking merges the stack or just
    // reselects — the arrow makes the stacking outcome unmistakable. Drawn
    // last so it sits on top of the square and piece underneath it.
    if app.hovered_stack_target() {
        if let Some(pos) = app.hovered {
            let (cx, cy) = screen_coord(pos, app.flipped);
            let tx = gutter + cx * TILE_W;
            let ty = topbar + cy * TILE_H;
            c.draw_bitmap(
                tx + 22,
                ty - 7,
                &symbols::symbol_bits(symbols::Symbol::DownArrow),
                symbols::SYMBOL_W as i32,
                render::COL_HL_THREAT,
            );
        }
    }

    if app.show_coords {
        for y in 0..9i32 {
            let (_, sy) = screen_coord(Position::new(0, y as usize), app.flipped);
            let rank = 9 - y;
            c.draw_text(
                6,
                topbar + sy * TILE_H + TILE_H / 2 - 5,
                &rank.to_string(),
                1,
                render::COL_COORD,
            );
        }
        for x in 0..9i32 {
            let (sx, _) = screen_coord(Position::new(x as usize, 0), app.flipped);
            let file = ((b'A' + x as u8) as char).to_string();
            c.draw_text_centered(
                gutter + sx * TILE_W + TILE_W / 2,
                topbar + BOARD_PX_H + 8,
                &file,
                1,
                render::COL_COORD,
            );
        }
    }

    if let Some(pending) = &app.pending {
        c.fill_rect_alpha(0, 0, render::board_w(show_coords), lh, 0x000000, 0.72);
        let rects = choice_buttons(pending.options.len(), show_coords);
        for (i, mv) in pending.options.iter().enumerate() {
            let rect = &rects[i];
            let hovered = mouse.map(|(mx, my)| rect.contains(mx, my)).unwrap_or(false);
            let label = move_choice_label(&app.game, mv);
            render::draw_button(c, rect.x0, rect.y0, rect.x1, rect.y1, &label, true, hovered);
        }
        let close = close_button_rect(show_coords);
        let close_hovered = mouse
            .map(|(mx, my)| close.contains(mx, my))
            .unwrap_or(false);
        render::draw_button(
            c,
            close.x0,
            close.y0,
            close.x1,
            close.y1,
            "X",
            true,
            close_hovered,
        );
    }

    if app.confirm_menu {
        c.fill_rect_alpha(0, 0, render::board_w(show_coords), lh, 0x000000, 0.72);
        c.draw_text_centered(
            render::board_w(show_coords) / 2,
            lh / 2 - 40,
            "RETURN TO MAIN MENU?",
            2,
            render::COL_STATUS,
        );
        let (yes, no) = confirm_menu_buttons(show_coords);
        for (rect, label) in [(&yes, "YES"), (&no, "NO")] {
            let hovered = mouse.map(|(mx, my)| rect.contains(mx, my)).unwrap_or(false);
            render::draw_button(c, rect.x0, rect.y0, rect.x1, rect.y1, label, true, hovered);
        }
    }
}

fn draw_game_over(c: &mut Canvas, app: &App, history_scroll: i32) {
    draw_board(c, app, history_scroll, None);
    let board_w = render::board_w(app.show_coords);
    let lh = render::logical_h(app.show_coords);
    c.fill_rect_alpha(0, 0, board_w, lh, 0x000000, 0.72);
    let msg = if app.game.is_draw() {
        "DRAW"
    } else if app.game.white_wins() {
        "WHITE WINS"
    } else {
        "BLACK WINS"
    };
    c.draw_text_centered(board_w / 2, lh / 2 - 40, msg, 4, render::COL_STATUS);
    c.draw_text_centered(
        board_w / 2,
        lh / 2 + 30,
        "CLICK TO RETURN TO MENU",
        1,
        render::COL_COORD,
    );
}

/// "CLOSE" button, top-right corner of the fullscreen rules modal.
fn rules_close_button_rect(show_coords: bool) -> Rect {
    let bw = 90;
    let bh = render::BTN_H;
    let pad = 14;
    let lw = render::logical_w(show_coords);
    Rect {
        x0: lw - pad - bw,
        y0: pad,
        x1: lw - pad,
        y1: pad + bh,
    }
}

const RULES_GRID_TOP: i32 = 136;
const RULES_CARD_H: i32 = 46;
const RULES_CARD_GAP: i32 = 6;
const RULES_PAD: i32 = 14;

/// One rules-modal piece card's rect, laid out 2 columns x 4 rows over
/// `rules::PIECES`.
fn rules_card_rect(index: usize, show_coords: bool) -> Rect {
    let lw = render::logical_w(show_coords);
    let col = (index % 2) as i32;
    let row = (index / 2) as i32;
    let card_gap_x = 12;
    let card_w = (lw - 2 * RULES_PAD - card_gap_x) / 2;
    let x0 = RULES_PAD + col * (card_w + card_gap_x);
    let y0 = RULES_GRID_TOP + row * (RULES_CARD_H + RULES_CARD_GAP);
    Rect {
        x0,
        y0,
        x1: x0 + card_w,
        y1: y0 + RULES_CARD_H,
    }
}

/// Fullscreen rules reference — general rules plus every piece's movement
/// and promotion, laid out to fit on one screen with no scrolling (see
/// `rules::GENERAL`/`rules::PIECES`). Drawn as an overlay on top of whatever
/// screen is behind it (menu or an in-progress game).
fn draw_rules_modal(c: &mut Canvas, show_coords: bool, mouse: Option<(i32, i32)>) {
    let lw = render::logical_w(show_coords);
    let lh = render::logical_h(show_coords);
    c.fill_rect(0, 0, lw, lh, render::COL_PAGE_BG);
    c.draw_text_centered(lw / 2, 16, "RULES", 3, render::COL_STATUS);

    for (i, line) in rules::GENERAL.iter().enumerate() {
        c.draw_text_centered(lw / 2, 54 + i as i32 * 13, line, 1, render::COL_COORD);
    }

    for (i, piece) in rules::PIECES.iter().enumerate() {
        let rect = rules_card_rect(i, show_coords);
        let icon_x = rect.x0 + 8;
        let icon_y = rect.y0 + 6;
        c.draw_icon(icon_x, icon_y, piece.icon, 1, render::COL_STATUS, false);
        let text_x = icon_x + icons::ICON_N as i32 + 8;
        c.draw_text(
            text_x,
            rect.y0 + 2,
            &format!("{}  {}", piece.letter, piece.name),
            1,
            render::COL_STATUS,
        );
        c.draw_text(text_x, rect.y0 + 14, piece.movement, 1, render::COL_COORD);
        if let Some(promotion) = piece.promotion {
            c.draw_text(text_x, rect.y0 + 26, promotion, 1, render::COL_SELECT);
        }
    }

    let close = rules_close_button_rect(show_coords);
    let hovered = mouse
        .map(|(mx, my)| close.contains(mx, my))
        .unwrap_or(false);
    render::draw_button(
        c, close.x0, close.y0, close.x1, close.y1, "CLOSE", true, hovered,
    );
    c.draw_text_centered(lw / 2, lh - 20, "ESC TO CLOSE", 1, render::COL_COORD);
}

/// YES/NO-style button pair for the first-launch mini help modal — "GOT IT"
/// dismisses it, "FULL RULES" dismisses it and opens the fullscreen
/// reference instead.
fn help_modal_buttons(show_coords: bool) -> (Rect, Rect) {
    let bw = 170;
    let bh = render::BTN_H;
    let gap = 20;
    let cx = render::board_w(show_coords) / 2;
    let cy = render::logical_h(show_coords) / 2 + 40;
    let total_w = 2 * bw + gap;
    let x0 = cx - total_w / 2;
    let got_it = Rect {
        x0,
        y0: cy,
        x1: x0 + bw,
        y1: cy + bh,
    };
    let full_rules = Rect {
        x0: x0 + bw + gap,
        y0: cy,
        x1: x0 + bw + gap + bw,
        y1: cy + bh,
    };
    (got_it, full_rules)
}

/// First-launch mini help modal (see `App::show_help`) — the subset of
/// rules that doesn't overlap with chess, so a player who already knows
/// chess can start playing immediately.
fn draw_help_modal(c: &mut Canvas, show_coords: bool, mouse: Option<(i32, i32)>) {
    let board_w = render::board_w(show_coords);
    let lh = render::logical_h(show_coords);
    c.fill_rect_alpha(0, 0, board_w, lh, 0x000000, 0.82);
    let cx = board_w / 2;
    c.draw_text_centered(cx, lh / 2 - 90, "BEFORE YOU BEGIN", 2, render::COL_STATUS);
    for (i, line) in rules::QUICK_TIPS.iter().enumerate() {
        c.draw_text_centered(cx, lh / 2 - 44 + i as i32 * 16, line, 1, render::COL_COORD);
    }
    let (got_it, full_rules) = help_modal_buttons(show_coords);
    for (rect, label) in [(&got_it, "GOT IT"), (&full_rules, "FULL RULES")] {
        let hovered = mouse.map(|(mx, my)| rect.contains(mx, my)).unwrap_or(false);
        render::draw_button(c, rect.x0, rect.y0, rect.x1, rect.y1, label, true, hovered);
    }
}

fn write_ppm(path: &str, buf: &[u32], w: i32, h: i32) {
    use std::io::Write;
    let mut out = std::fs::File::create(path).expect("create snapshot file");
    write!(out, "P6\n{} {}\n255\n", w, h).unwrap();
    let mut bytes = Vec::with_capacity((w * h * 3) as usize);
    for &px in buf {
        bytes.push(((px >> 16) & 0xff) as u8);
        bytes.push(((px >> 8) & 0xff) as u8);
        bytes.push((px & 0xff) as u8);
    }
    out.write_all(&bytes).unwrap();
}

/// Optional `KERES_MOUSE=x,y` override for the snapshot tool, so button
/// hover states (see `render::draw_button`) can be checked without a real
/// window.
fn snapshot_mouse() -> Option<(i32, i32)> {
    let raw = std::env::var("KERES_MOUSE").ok()?;
    let (x, y) = raw.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// Headless render-to-file path used only for local visual verification
/// (`KERES_SNAPSHOT=<path>.ppm KERES_SCREEN=menu|new_game|playing|selected|
/// stacked|gameover|flipped|threats|history|nocoords|hover_friendly|hover_threat|
/// hover_stack|last_move|confirm_menu|stacked_close_hover|load_game|rules|
/// rules_in_game|quick_help|help_tab|help_tab_stack|help_tab_empty|
/// move_anim_start|move_anim_mid`,
/// optionally with `KERES_MOUSE=x,y` to check dialog-button hover). Never
/// touches a window or the real display; save I/O is redirected to a throwaway
/// temp directory so it never writes into the developer's real save folder.
fn run_snapshot(path: &str) {
    use keres_engine::{Board, Color, PieceType};
    save::set_test_dir_override(std::env::temp_dir().join("keres_snapshot_saves"));
    settings::set_test_dir_override(std::env::temp_dir().join("keres_snapshot_settings"));
    let _ = std::fs::remove_file(
        std::env::temp_dir()
            .join("keres_snapshot_settings")
            .join("settings.bin"),
    );
    let mut app = App::new();
    let screen = std::env::var("KERES_SCREEN").unwrap_or_else(|_| "menu".to_string());
    match screen.as_str() {
        "splash" => {}
        "menu" => app.dismiss_splash(),
        "new_game" => {
            app.dismiss_splash();
            app.open_new_game_screen();
        }
        "rules" => app.open_rules(),
        "rules_in_game" => {
            app.start_game(Mode::Hotseat);
            app.dismiss_help();
            app.open_rules();
        }
        "quick_help" => app.start_game(Mode::Hotseat),
        "load_game" => {
            app.start_game(Mode::Hotseat);
            app.dismiss_help();
            app.click_square(Position::new(2, 6));
            app.click_square(Position::new(1, 5));
            app.back_to_menu();
            app.open_load_screen();
        }
        "playing" => app.start_game(Mode::Hotseat),
        "selected" => {
            app.start_game(Mode::Hotseat);
            app.click_square(Position::new(3, 6));
        }
        "stacked" => {
            app.start_game(Mode::Hotseat);
            app.game.board.set_piece(
                &Position::new(3, 6),
                Some(Piece::new(
                    Color::White,
                    PieceType::Soldier,
                    Some(PieceType::Bishop),
                )),
            );
            app.click_square(Position::new(3, 6));
        }
        "gameover" => {
            app.start_game(Mode::Hotseat);
            app.screen = Screen::GameOver;
        }
        "flipped" => {
            app.start_game(Mode::Hotseat);
            app.toggle_flip();
        }
        "threats" => {
            app.start_game(Mode::Hotseat);
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
                Some(Piece::new(Color::White, PieceType::Soldier, None)),
            );
            board.set_piece(
                &Position::new(3, 3),
                Some(Piece::new(Color::Black, PieceType::Bishop, None)),
            );
            app.game.board = board;
        }
        "history" => {
            app.start_game(Mode::Hotseat);
            let moves = [
                Move {
                    from: Position::new(2, 6),
                    to: Position::new(1, 5),
                    unstack: false,
                },
                Move {
                    from: Position::new(6, 2),
                    to: Position::new(7, 3),
                    unstack: false,
                },
                Move {
                    from: Position::new(4, 6),
                    to: Position::new(3, 5),
                    unstack: false,
                },
                Move {
                    from: Position::new(4, 2),
                    to: Position::new(3, 3),
                    unstack: false,
                },
            ];
            for mv in moves {
                app.game.make(&mv);
                let cap = false;
                app.history.push((mv, cap));
            }
            app.set_sidebar_tab(app::SidebarTab::Moves);
        }
        "nocoords" => {
            app.start_game(Mode::Hotseat);
            app.toggle_coords();
        }
        "hover_friendly" => {
            app.start_game(Mode::Hotseat);
            app.set_hovered(Some(Position::new(3, 6)));
        }
        "move_anim_start" => {
            app.start_game(Mode::Hotseat);
            app.dismiss_help();
            app.click_square(Position::new(2, 6));
            app.click_square(Position::new(1, 5));
        }
        "move_anim_mid" => {
            app.start_game(Mode::Hotseat);
            app.dismiss_help();
            app.click_square(Position::new(2, 6));
            app.click_square(Position::new(1, 5));
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        "help_tab" => {
            app.start_game(Mode::Hotseat);
            app.dismiss_help();
            app.set_sidebar_tab(app::SidebarTab::Help);
            app.set_hovered(Some(Position::new(3, 6)));
        }
        "settings_tab" => {
            app.start_game(Mode::Hotseat);
            app.dismiss_help();
            app.set_sidebar_tab(app::SidebarTab::Settings);
        }
        "help_tab_stack" => {
            app.start_game(Mode::Hotseat);
            app.dismiss_help();
            app.game.board.set_piece(
                &Position::new(3, 6),
                Some(Piece::new(
                    Color::White,
                    PieceType::Soldier,
                    Some(PieceType::Bishop),
                )),
            );
            app.set_sidebar_tab(app::SidebarTab::Help);
            app.set_hovered(Some(Position::new(3, 6)));
        }
        "help_tab_empty" => {
            app.start_game(Mode::Hotseat);
            app.dismiss_help();
            app.set_sidebar_tab(app::SidebarTab::Help);
        }
        "hover_threat" => {
            app.start_game(Mode::Hotseat);
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
                Some(Piece::new(Color::White, PieceType::Soldier, None)),
            );
            board.set_piece(
                &Position::new(3, 3),
                Some(Piece::new(Color::Black, PieceType::Bishop, None)),
            );
            app.game.board = board;
            app.set_hovered(Some(Position::new(3, 3)));
        }
        "hover_stack" => {
            app.start_game(Mode::Hotseat);
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
                Some(Piece::new(Color::White, PieceType::Rook, None)),
            );
            board.set_piece(
                &Position::new(4, 3),
                Some(Piece::new(Color::White, PieceType::Soldier, None)),
            );
            app.game.board = board;
            app.click_square(Position::new(4, 4));
            app.set_hovered(Some(Position::new(4, 3)));
        }
        "last_move" => {
            app.start_game(Mode::Hotseat);
            app.click_square(Position::new(2, 6));
            app.click_square(Position::new(1, 5));
        }
        "confirm_menu" => {
            app.start_game(Mode::Hotseat);
            app.request_menu_confirm();
        }
        "stacked_close_hover" => {
            app.start_game(Mode::Hotseat);
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
            app.game.board = board;
            app.click_square(Position::new(4, 4));
            app.click_square(Position::new(5, 3));
        }
        other => panic!("unknown KERES_SCREEN {other}"),
    }
    let lw = render::logical_w(app.show_coords);
    let lh = render::logical_h(app.show_coords);
    let mut buffer = vec![0u32; (lw * lh) as usize];
    let mut canvas = Canvas {
        buf: &mut buffer,
        w: lw,
        h: lh,
    };
    match app.screen {
        Screen::Splash => draw_splash(&mut canvas, app.show_coords),
        Screen::Menu => draw_menu(&mut canvas, app.show_coords, snapshot_mouse()),
        Screen::NewGame => draw_new_game(&mut canvas, app.show_coords, app.level, snapshot_mouse()),
        Screen::Playing => draw_board(&mut canvas, &app, 0, snapshot_mouse()),
        Screen::GameOver => draw_game_over(&mut canvas, &app, 0),
        Screen::LoadGame => draw_load_screen(&mut canvas, &app, 0, snapshot_mouse()),
    }
    if app.show_rules {
        draw_rules_modal(&mut canvas, app.show_coords, snapshot_mouse());
    } else if app.show_help {
        draw_help_modal(&mut canvas, app.show_coords, snapshot_mouse());
    }
    write_ppm(path, &buffer, lw, lh);
}

fn main() {
    if let Ok(path) = std::env::var("KERES_SNAPSHOT") {
        run_snapshot(&path);
        return;
    }

    // Live FPS counter, bottom-right of the window — only when explicitly
    // asked for, so it never shows up for a normal player.
    let debug = std::env::args().any(|a| a == "--debug");

    let mut app = App::new();

    let mut window = Window::new(
        "Keres",
        render::logical_w(app.show_coords) as usize,
        render::logical_h(app.show_coords) as usize,
        WindowOptions {
            resize: true,
            ..WindowOptions::default()
        },
    )
    .expect("unable to open window");
    window.set_target_fps(60);
    // minifb's icon API differs per platform (and does not exist on macOS) —
    // see platform_icon.rs. Best-effort: never gates window creation.
    platform_icon::set_window_icon(&mut window);

    let mut logical: Vec<u32> = Vec::new();
    let mut output: Vec<u32> = Vec::new();
    let mut blit_cache = render::BlitCache::new();
    let mut prev_mouse_down = false;
    let mut history_scroll: i32 = 0;
    let mut load_scroll: i32 = 0;
    // ESC never quits directly except from the main menu: from a game in
    // progress it opens the return-to-menu confirmation instead (see
    // App::request_menu_confirm), so the window close is driven by this flag
    // rather than the raw key state.
    let mut should_quit = false;
    let mut last_frame = std::time::Instant::now();
    // Exponential moving average so the number doesn't flicker every frame.
    let mut fps_ema: f32 = 60.0;

    while window.is_open() && !should_quit {
        app.poll_ai();
        app.tick_anim();

        if debug {
            let now = std::time::Instant::now();
            let dt = now.duration_since(last_frame).as_secs_f32();
            last_frame = now;
            if dt > 0.0 {
                fps_ema = fps_ema * 0.9 + (1.0 / dt) * 0.1;
            }
        }

        let show_coords = app.show_coords;
        let lw = render::logical_w(show_coords);
        let lh = render::logical_h(show_coords);
        if logical.len() != (lw * lh) as usize {
            logical.resize((lw * lh) as usize, 0);
        }

        let (win_w, win_h) = window.get_size();
        let (win_w, win_h) = (win_w as i32, win_h as i32);

        let mouse_down = window.get_mouse_down(MouseButton::Left);
        let clicked = mouse_down && !prev_mouse_down;
        prev_mouse_down = mouse_down;
        let mouse_pos = window.get_mouse_pos(MouseMode::Clamp);
        let logical_mouse = mouse_pos.and_then(|(mx, my)| {
            render::window_to_logical(mx as i32, my as i32, win_w, win_h, lw, lh)
        });

        // Live hover tracking drives the immediate move preview and the
        // "this creates a stack" arrow (see App::hover_preview_squares /
        // hovered_stack_target) — only meaningful while a move can actually
        // be made.
        if app.screen == Screen::Playing
            && !app.ai_thinking
            && app.pending.is_none()
            && !app.confirm_menu
        {
            let hovered =
                logical_mouse.and_then(|(lx, ly)| tile_at(lx, ly, app.flipped, show_coords));
            app.set_hovered(hovered);
        } else {
            app.set_hovered(None);
        }

        if let Some((_, dy)) = window.get_scroll_wheel() {
            if dy.abs() > 0.01 {
                match app.screen {
                    Screen::Playing => {
                        history_scroll -= dy.signum() as i32;
                        history_scroll = history_scroll.max(0);
                    }
                    Screen::LoadGame => {
                        load_scroll -= dy.signum() as i32;
                        load_scroll = load_scroll.max(0);
                    }
                    _ => {}
                }
            }
        }

        if clicked {
            if let Some((lx, ly)) = logical_mouse {
                if app.show_help {
                    let (got_it, full_rules) = help_modal_buttons(show_coords);
                    if got_it.contains(lx, ly) {
                        app.dismiss_help();
                    } else if full_rules.contains(lx, ly) {
                        app.dismiss_help();
                        app.open_rules();
                    }
                } else if app.show_rules {
                    if rules_close_button_rect(show_coords).contains(lx, ly) {
                        app.close_rules();
                    }
                } else {
                    match app.screen {
                        Screen::Splash => app.dismiss_splash(),
                        Screen::Menu => {
                            for (rect, _label, action) in menu_buttons(show_coords) {
                                if rect.contains(lx, ly) {
                                    match action {
                                        MenuAction::NewGame => app.open_new_game_screen(),
                                        MenuAction::LoadGame => {
                                            if save::any_exist() {
                                                app.open_load_screen();
                                                load_scroll = 0;
                                            }
                                        }
                                        MenuAction::Rules => app.open_rules(),
                                        MenuAction::Exit => should_quit = true,
                                    }
                                    break;
                                }
                            }
                        }
                        Screen::NewGame => {
                            let mut handled = false;
                            for (i, rect) in menu_level_boxes(show_coords).into_iter().enumerate() {
                                if rect.contains(lx, ly) {
                                    app.set_level(i as u8 + 1);
                                    handled = true;
                                    break;
                                }
                            }
                            for (rect, _label, mode) in menu_mode_buttons(show_coords) {
                                if rect.contains(lx, ly) {
                                    app.start_game(mode);
                                    history_scroll = 0;
                                    handled = true;
                                    break;
                                }
                            }
                            if !handled && new_game_back_button_rect(show_coords).contains(lx, ly) {
                                app.close_new_game_screen();
                            }
                        }
                        Screen::Playing => {
                            if let Some(pending) = &app.pending {
                                if close_button_rect(show_coords).contains(lx, ly) {
                                    app.cancel_choice();
                                } else {
                                    let rects = choice_buttons(pending.options.len(), show_coords);
                                    for (i, rect) in rects.iter().enumerate() {
                                        if rect.contains(lx, ly) {
                                            app.resolve_choice(i);
                                            break;
                                        }
                                    }
                                }
                            } else if app.confirm_menu {
                                let (yes, no) = confirm_menu_buttons(show_coords);
                                if yes.contains(lx, ly) {
                                    app.confirm_back_to_menu();
                                } else if no.contains(lx, ly) {
                                    app.cancel_menu_confirm();
                                }
                            } else {
                                let mut handled = false;
                                for (rect, _label, action, enabled) in sidebar_buttons(&app) {
                                    if enabled && rect.contains(lx, ly) {
                                        match action {
                                            SidebarAction::MainMenu => app.request_menu_confirm(),
                                            SidebarAction::SwitchSides => app.toggle_flip(),
                                            SidebarAction::Undo => app.undo(),
                                            SidebarAction::Resign => app.resign(),
                                            SidebarAction::Rules => app.open_rules(),
                                        }
                                        handled = true;
                                        break;
                                    }
                                }
                                if !handled {
                                    for (rect, _label, tab) in sidebar_tab_rects(&app) {
                                        if rect.contains(lx, ly) {
                                            app.set_sidebar_tab(tab);
                                            handled = true;
                                            break;
                                        }
                                    }
                                }
                                if !handled && app.sidebar_tab == app::SidebarTab::Settings {
                                    for (rect, _label, action) in settings_tab_buttons(&app) {
                                        if rect.contains(lx, ly) {
                                            match action {
                                                SettingsAction::Threats => app.toggle_threats(),
                                                SettingsAction::Coords => app.toggle_coords(),
                                                SettingsAction::RotateIcons => {
                                                    app.toggle_rotate_opponent_icons()
                                                }
                                            }
                                            handled = true;
                                            break;
                                        }
                                    }
                                }
                                if !handled {
                                    if let Some(pos) = tile_at(lx, ly, app.flipped, show_coords) {
                                        app.click_square(pos);
                                    }
                                }
                            }
                        }
                        Screen::GameOver => app.back_to_menu(),
                        Screen::LoadGame => {
                            if load_back_button_rect(show_coords).contains(lx, ly) {
                                app.close_load_screen();
                            } else {
                                let visible = load_visible_rows(show_coords);
                                let max_scroll = (app.load_entries.len() as i32 - visible).max(0);
                                let scroll = load_scroll.clamp(0, max_scroll);
                                for row in 0..visible {
                                    let idx = (scroll + row) as usize;
                                    if idx >= app.load_entries.len() {
                                        break;
                                    }
                                    if load_row_rect(row, show_coords).contains(lx, ly) {
                                        app.load_selected(idx);
                                        history_scroll = 0;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
            if app.show_help {
                app.dismiss_help();
            } else if app.show_rules {
                app.close_rules();
            } else {
                match app.screen {
                    Screen::Splash => app.dismiss_splash(),
                    // The only place ESC quits the app outright.
                    Screen::Menu => should_quit = true,
                    Screen::NewGame => app.close_new_game_screen(),
                    Screen::Playing => {
                        if app.pending.is_some() {
                            app.cancel_choice();
                        } else if app.confirm_menu {
                            app.cancel_menu_confirm();
                        } else {
                            app.request_menu_confirm();
                        }
                    }
                    Screen::GameOver => app.back_to_menu(),
                    Screen::LoadGame => app.close_load_screen(),
                }
            }
        }

        if app.screen == Screen::Splash && !window.get_keys_pressed(KeyRepeat::No).is_empty() {
            app.dismiss_splash();
        }
        if window.is_key_pressed(Key::U, KeyRepeat::No) && app.screen == Screen::Playing {
            app.undo();
        }
        if let Some(pending) = &app.pending {
            let n = pending.options.len();
            for (i, key) in [Key::Key1, Key::Key2, Key::Key3].iter().enumerate() {
                if i < n && window.is_key_pressed(*key, KeyRepeat::No) {
                    app.resolve_choice(i);
                    break;
                }
            }
        } else if app.screen == Screen::NewGame {
            for (key, mode) in [
                (Key::Key1, Mode::Hotseat),
                (Key::Key2, Mode::VsAiWhite),
                (Key::Key3, Mode::VsAiBlack),
            ] {
                if window.is_key_pressed(key, KeyRepeat::No) {
                    app.start_game(mode);
                    history_scroll = 0;
                }
            }
        }

        let mut canvas = Canvas {
            buf: &mut logical,
            w: lw,
            h: lh,
        };
        match app.screen {
            Screen::Splash => draw_splash(&mut canvas, show_coords),
            Screen::Menu => draw_menu(&mut canvas, show_coords, logical_mouse),
            Screen::NewGame => draw_new_game(&mut canvas, show_coords, app.level, logical_mouse),
            Screen::Playing => draw_board(&mut canvas, &app, history_scroll, logical_mouse),
            Screen::GameOver => draw_game_over(&mut canvas, &app, history_scroll),
            Screen::LoadGame => draw_load_screen(&mut canvas, &app, load_scroll, logical_mouse),
        }
        if app.show_rules {
            draw_rules_modal(&mut canvas, show_coords, logical_mouse);
        } else if app.show_help {
            draw_help_modal(&mut canvas, show_coords, logical_mouse);
        }
        if debug {
            let label = format!("{:.0} FPS", fps_ema);
            let w = Canvas::text_width(&label, 1);
            canvas.draw_text(lw - 8 - w, lh - 16, &label, 1, render::COL_COORD);
        }

        let out_len = (win_w.max(1) * win_h.max(1)) as usize;
        if output.len() != out_len {
            output.resize(out_len, 0);
        }
        render::blit_to_window_cached(
            &mut blit_cache,
            &logical,
            &mut output,
            win_w.max(1),
            win_h.max(1),
            lw,
            lh,
        );

        window
            .update_with_buffer(&output, win_w.max(1) as usize, win_h.max(1) as usize)
            .expect("update_with_buffer failed");
    }
}
