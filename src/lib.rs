//! TorrentX as a library, so the same code can be the desktop binary and the
//! Android `.so` the platform loads. `src/main.rs` is a two-line desktop shell.

// ─── Modules ────────────────────────────────────────────────────────
mod app;
mod config;
mod indexers;
mod jackett;
mod player;
mod rss;
mod sources;
mod themes;
mod ui;

use app::App;

use themes::{Pal, tint};

use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};

use crate::ui::{ICON_LG, ICON_MD, ICON_SM, ICON_XS, R_MD, R_SM, SP1, SP2, SP3, margin};

// ─── Constants ─────────────────────────────────────────────────────────────

pub(crate) const MARGIN_DEFAULT: f32 = 12.0;

pub(crate) const CATS: &[&str] = &[
    "All", "Movies", "TV", "Music", "PC Games", "Software", "Anime", "Books", "XXX",
];

// ─── Single instance ───────────────────────────────────────────────────────
// Two copies of the app share one config file and each save rewrites the whole
// file (favourites, history, window size), so a second copy silently clobbers
// the first one's changes. A Unix socket next to the config is the lock: it is
// per-profile (so `--config` still gives a separate instance), and a *connect*
// to it answers whether the owner is alive — a stale file left by a crash
// refuses the connection and can be reclaimed.

/// Set when another copy was launched and asked this one to show its window.
pub(crate) static SHOW_REQ: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(unix)]
fn instance_socket() -> std::path::PathBuf {
    let cfg = crate::config::cfg_path();
    cfg.parent()
        .map(|d| d.join("torrentx.sock"))
        .unwrap_or_else(|| std::path::PathBuf::from("torrentx.sock"))
}

/// Claim the single-instance lock at `path`.
///
/// `None` means a live copy owns it and has been asked to show its window — the
/// caller should exit quietly. `Some(listener)` means we own it; the caller keeps
/// it alive for the process lifetime (dropping it releases the lock).
#[cfg(unix)]
fn claim_instance_at(path: &std::path::Path) -> Option<std::os::unix::net::UnixListener> {
    use std::io::Write;
    use std::os::unix::net::{UnixListener, UnixStream};

    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut s) = UnixStream::connect(path) {
        let _ = s.write_all(b"show\n");
        return None;
    }
    // Nothing answered: either no file, or a socket left behind by a crash.
    let _ = std::fs::remove_file(path);
    UnixListener::bind(path).ok()
}

/// Serve "show me" requests from later launches until the process exits.
#[cfg(unix)]
fn serve_show_requests(l: std::os::unix::net::UnixListener) {
    use std::io::Read;
    std::thread::spawn(move || {
        for conn in l.incoming() {
            let Ok(mut c) = conn else { break };
            let mut buf = [0u8; 16];
            let _ = c.read(&mut buf);
            SHOW_REQ.store(true, std::sync::atomic::Ordering::SeqCst);
            wake_ui();
        }
    });
}

/// Claim the lock for this profile (the socket lives beside the config, so
/// `--config` still gives a second, independent instance).
#[cfg(unix)]
fn claim_instance() -> bool {
    match claim_instance_at(&instance_socket()) {
        Some(l) => {
            serve_show_requests(l);
            true
        }
        None => false,
    }
}

#[cfg(not(unix))]
fn claim_instance() -> bool {
    true
}

// ─── Autostart (Linux) ─────────────────────────────────────────────────────
// TorrentX has no daemon, so RSS feeds only poll while the window is open.
// Starting at login is the whole feature: one XDG autostart file, no service.
// Everything here is Linux-only, pill and helpers alike: on another target the
// file would be written where nothing reads it, and an unused-code warning is the
// honest signal that the feature does not exist there.

#[cfg(target_os = "linux")]
fn autostart_file() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .map(|d| d.join("autostart/torrentx.desktop"))
}

#[cfg(target_os = "linux")]
pub(crate) fn autostart_on() -> bool {
    autostart_file().is_some_and(|p| p.exists())
}

/// The launcher line must be the AppImage, not the `/tmp/.mount_*/usr/bin`
/// binary inside it: that path is gone after the process exits.
#[cfg(target_os = "linux")]
fn autostart_exe() -> std::path::PathBuf {
    std::env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn desktop_entry(exe: &std::path::Path) -> String {
    format!(
        // --minimized: at login the app belongs in the tray, polling RSS, not
        // in front of whatever the user is doing.
        "[Desktop Entry]\nType=Application\nName=TorrentX\nComment=Search all your Jackett indexers\nExec=\"{}\" --minimized\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    )
}

#[cfg(target_os = "linux")]
pub(crate) fn set_autostart(on: bool) {
    let Some(path) = autostart_file() else { return };
    if !on {
        let _ = std::fs::remove_file(&path);
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, desktop_entry(&autostart_exe()));
}

/// CSV field escaping: wrap in quotes, double any embedded quotes.
pub(crate) fn csv_esc(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Open a URL/URI only if its scheme is safe. Indexer-supplied strings are
/// untrusted — never hand file:// or arbitrary schemes to xdg-open.
/// Lock a state mutex, recovering the data if a previous holder panicked.
///
/// These locks are only ever held for a field read or write, so a poisoned lock
/// carries no half-written structure worth aborting over — and taking the whole
/// GUI down because a worker thread died earlier is strictly worse for the user
/// than showing the previous value. `unwrap()` here turns one panic into a
/// second one on the next search; `if let Ok(..)` silently drops the update
/// instead. One convention: always go through here.
pub(crate) fn lock_or_recover<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Open a URL/URI only if its scheme is safe. Indexer-supplied strings are
/// untrusted — never hand file:// or arbitrary schemes to xdg-open.
pub(crate) fn safe_open(s: impl AsRef<str>) -> bool {
    let t = s.as_ref().trim();
    if safe_scheme(t) {
        open::that(t).is_ok()
    } else {
        false
    }
}

/// Scheme allow-list for `safe_open` (pure, unit-testable).
pub(crate) fn safe_scheme(s: &str) -> bool {
    ["http://", "https://", "magnet:"]
        .iter()
        .any(|p| s.starts_with(p))
}

/// CSV field escaping + formula-injection guard: spreadsheet apps execute
/// cells that begin with = + - @ (tab/CR too). Indexer-supplied titles are
/// untrusted, so prefix such payloads with `'`. The payload sits at index 1
/// (index 0 is the opening quote added by `csv_esc`).
pub(crate) fn csv_safe(s: &str) -> String {
    let mut s = csv_esc(s);
    if matches!(
        s.as_bytes().get(1),
        Some(b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r')
    ) {
        s.insert(1, '\'');
    }
    s
}

// ─── Small UI buttons ──────────────────────────────────────────────────────

pub(crate) fn act_btn(ui: &mut egui::Ui, label: &str, tip: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).size(11.5).color(color))
            .fill(tint(color, 18))
            .stroke(Stroke::new(1.0_f32, tint(color, 70)))
            .corner_radius(R_SM)
            .min_size(Vec2::new(0.0, 25.0)),
    )
    .on_hover_text(tip)
    .clicked()
}

/// A high-quality SVG icon button — renders an embedded Lucide SVG via
/// egui's image loader (resvg), tinted to the theme color. Professional
/// icon design, vector-crisp, no glyphs, no tofu. egui caches the
/// rasterized image by URI. Returns true when clicked.
pub(crate) fn svg_btn(ui: &mut egui::Ui, icon: SvgIcon, tip: &str, color: Color32) -> bool {
    let size = egui::vec2(32.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let rounding = egui::CornerRadius::same(R_SM as u8);
    let painter = ui.painter_at(rect);

    // Press feedback: shrink the hit surface slightly while held (tactile
    // confirmation the click registered — interface-design motion rule).
    let press_scale = if resp.is_pointer_button_down_on() {
        0.94
    } else {
        1.0
    };
    let rect = if press_scale < 1.0 {
        egui::Rect::from_center_size(rect.center(), rect.size() * press_scale)
    } else {
        rect
    };

    if resp.hovered() || resp.clicked() {
        painter.rect_filled(rect, rounding, tint(color, 26));
    } else {
        painter.rect_filled(rect, rounding, tint(color, 14));
    }
    painter.rect_stroke(
        rect.shrink(0.5),
        rounding,
        Stroke::new(1.0, tint(color, 70)),
        egui::StrokeKind::Outside,
    );

    // Draw the SVG tinted to the theme color (icon area centered in button).
    // Lucide uses stroke="currentColor"; resvg renders that black, and egui's
    // tint multiplies (black × color = black → invisible on dark themes). So
    // rewrite currentColor→white so tint() yields the actual theme color.
    let svg_white = icon.svg().replace("currentColor", "#fff");
    let icon_size = ICON_LG;
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(icon_size, icon_size));
    let img = egui::Image::from_bytes(icon.uri(), svg_white.into_bytes())
        .tint(color)
        .fit_to_exact_size(egui::vec2(icon_size, icon_size));
    ui.put(icon_rect, img);

    resp.on_hover_text(tip).clicked()
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum SvgIcon {
    Magnet,
    Copy,
    Download,
    Play,
    Star,
    StarFilled,
    Info,
    Web,
    Search,
    Rss,
    Settings,
    Close,
    Refresh,
    SortAsc,
    SortDesc,
    Circle,
    CircleDot,
    ChevronLeft,
    ChevronRight,
    Check,
}

impl SvgIcon {
    fn uri(&self) -> &'static str {
        match self {
            SvgIcon::Magnet => "bytes://tx/magnet.svg",
            SvgIcon::Copy => "bytes://tx/copy.svg",
            SvgIcon::Download => "bytes://tx/download.svg",
            SvgIcon::Play => "bytes://tx/play.svg",
            SvgIcon::Star => "bytes://tx/star.svg",
            SvgIcon::StarFilled => "bytes://tx/star-filled.svg",
            SvgIcon::Info => "bytes://tx/info.svg",
            SvgIcon::Web => "bytes://tx/web.svg",
            SvgIcon::Search => "bytes://tx/search.svg",
            SvgIcon::Rss => "bytes://tx/rss.svg",
            SvgIcon::Settings => "bytes://tx/settings.svg",
            SvgIcon::Close => "bytes://tx/x.svg",
            SvgIcon::Refresh => "bytes://tx/refresh-cw.svg",
            SvgIcon::SortAsc => "bytes://tx/arrow-up-narrow-wide.svg",
            SvgIcon::SortDesc => "bytes://tx/arrow-down-narrow-wide.svg",
            SvgIcon::Circle => "bytes://tx/circle.svg",
            SvgIcon::CircleDot => "bytes://tx/circle-dot.svg",
            SvgIcon::ChevronLeft => "bytes://tx/chevron-left.svg",
            SvgIcon::ChevronRight => "bytes://tx/chevron-right.svg",
            SvgIcon::Check => "bytes://tx/check.svg",
        }
    }
    fn svg(&self) -> &'static str {
        match self {
            SvgIcon::Magnet => include_str!("../assets/icons/magnet.svg"),
            SvgIcon::Copy => include_str!("../assets/icons/copy.svg"),
            SvgIcon::Download => include_str!("../assets/icons/download.svg"),
            SvgIcon::Play => include_str!("../assets/icons/play.svg"),
            SvgIcon::Star => include_str!("../assets/icons/star.svg"),
            SvgIcon::StarFilled => include_str!("../assets/icons/star-filled.svg"),
            SvgIcon::Info => include_str!("../assets/icons/info.svg"),
            SvgIcon::Web => include_str!("../assets/icons/web.svg"),
            SvgIcon::Search => include_str!("../assets/icons/search.svg"),
            SvgIcon::Rss => include_str!("../assets/icons/rss.svg"),
            SvgIcon::Settings => include_str!("../assets/icons/settings.svg"),
            SvgIcon::Close => include_str!("../assets/icons/x.svg"),
            SvgIcon::Refresh => include_str!("../assets/icons/refresh-cw.svg"),
            SvgIcon::SortAsc => include_str!("../assets/icons/arrow-up-narrow-wide.svg"),
            SvgIcon::SortDesc => include_str!("../assets/icons/arrow-down-narrow-wide.svg"),
            SvgIcon::Circle => include_str!("../assets/icons/circle.svg"),
            SvgIcon::CircleDot => include_str!("../assets/icons/circle-dot.svg"),
            SvgIcon::ChevronLeft => include_str!("../assets/icons/chevron-left.svg"),
            SvgIcon::ChevronRight => include_str!("../assets/icons/chevron-right.svg"),
            SvgIcon::Check => include_str!("../assets/icons/check.svg"),
        }
    }
}

/// Inline SVG icon (not a button) — renders the tinted icon at `size` px
/// into the current Ui. For headers, labels, status dots, etc.
pub(crate) fn svg_icon(ui: &mut egui::Ui, icon: SvgIcon, size: f32, color: Color32) {
    ui.add(svg_image(icon, size, color));
}

/// Returns an egui::Image widget for an SVG icon (tinted, sized). Usable
/// inside `Button::new(...)` or `ui.add(...)`.
pub(crate) fn svg_image(icon: SvgIcon, size: f32, color: Color32) -> egui::Image<'static> {
    let svg_white = icon.svg().replace("currentColor", "#fff");
    egui::Image::from_bytes(icon.uri(), svg_white.into_bytes())
        .tint(color)
        .fit_to_exact_size(egui::vec2(size, size))
}

/// A high-quality vector checkbox — drawn with the Painter (rounded rect +
/// check mark), NO font glyphs. Single allocation, whole-rect clickable.
/// Uses the accent color when checked. Returns the Response.
pub(crate) fn v_checkbox(
    ui: &mut egui::Ui,
    checked: bool,
    label: &str,
    accent: Color32,
) -> egui::Response {
    let icon_size = 15.0;
    let spacing = ui.spacing().item_spacing.x;

    // Measure label
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font_id, ui.visuals().text_color());

    // Single allocation for the whole row (icon + spacing + text)
    let desired = egui::vec2(
        icon_size + spacing + galley.size().x,
        icon_size.max(galley.size().y) + 2.0,
    );
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let p = ui.painter_at(rect);
        // Box
        let box_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.center().y - icon_size / 2.0),
            egui::vec2(icon_size, icon_size),
        );
        let rounding = egui::CornerRadius::same(R_SM as u8);
        if checked {
            p.rect_filled(box_rect, rounding, accent);
            // Check mark (two lines); contrast against the accent fill
            let check_col = if ui.visuals().dark_mode {
                Color32::from_rgb(12, 12, 16)
            } else {
                Color32::WHITE
            };
            let ck = egui::Stroke::new(2.2, check_col);
            let cc = box_rect.center();
            p.line_segment(
                [
                    egui::pos2(cc.x - 3.5, cc.y),
                    egui::pos2(cc.x - 0.8, cc.y + 3.0),
                ],
                ck,
            );
            p.line_segment(
                [
                    egui::pos2(cc.x - 0.8, cc.y + 3.0),
                    egui::pos2(cc.x + 4.0, cc.y - 3.5),
                ],
                ck,
            );
        } else {
            p.rect_filled(box_rect, rounding, Color32::TRANSPARENT);
            p.rect_stroke(
                box_rect,
                rounding,
                egui::Stroke::new(1.3, ui.visuals().text_color().gamma_multiply(0.6)),
                egui::StrokeKind::Inside,
            );
        }
        // Label
        let text_pos = egui::pos2(
            rect.left() + icon_size + spacing,
            rect.center().y - galley.size().y / 2.0,
        );
        p.galley(text_pos, galley, ui.visuals().text_color());
    }
    response
}

/// Compact icon + text pill. The complete surface is one native button,
/// so icon and label share the same hover/focus/click target.
pub(crate) fn icon_text_btn(
    ui: &mut egui::Ui,
    icon: SvgIcon,
    label: &str,
    color: Color32,
    enabled: bool,
) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::image_and_text(
            svg_image(icon, ICON_SM, color),
            RichText::new(label)
                .font(FontId::proportional(12.0))
                .color(color),
        )
        .fill(tint(color, 14))
        .stroke(Stroke::new(1.0, tint(color, 70)))
        .corner_radius(R_SM)
        .min_size(Vec2::new(0.0, 28.0)),
    )
    .clicked()
}

/// Icon + label as ONE clickable pill — the whole tab, not just its text.
/// `active` paints the accent fill and border the header uses for the current tab.
pub(crate) fn icon_text_tab(
    ui: &mut egui::Ui,
    icon: SvgIcon,
    label: &str,
    color: Color32,
    active: bool,
    accent: Color32,
) -> egui::Response {
    ui.add(
        egui::Button::image_and_text(
            svg_image(icon, ICON_MD, color),
            RichText::new(label)
                .font(FontId::proportional(14.0))
                .color(color),
        )
        .fill(if active {
            tint(accent, 22)
        } else {
            Color32::TRANSPARENT
        })
        .stroke(Stroke::new(if active { 1.0 } else { 0.0 }, accent))
        .corner_radius(R_MD)
        .min_size(Vec2::new(0.0, 28.0)),
    )
}

/// Non-interactive status pill with a tofu-proof SVG icon.
pub(crate) fn status_icon_pill(ui: &mut egui::Ui, icon: SvgIcon, label: &str, col: Color32) {
    egui::Frame::NONE
        .fill(tint(col, 20))
        .corner_radius(R_MD)
        .inner_margin(margin(SP3, SP1))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                svg_icon(ui, icon, ICON_XS, col);
                ui.label(
                    RichText::new(label)
                        .font(FontId::proportional(10.5))
                        .color(col),
                );
            });
        });
}

pub(crate) fn wide_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    let w = ui.available_width().max(200.0);
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .font(FontId::proportional(13.0))
                .color(color),
        )
        .fill(tint(color, 18))
        .stroke(Stroke::new(1.0_f32, tint(color, 80)))
        .corner_radius(R_MD)
        .min_size(Vec2::new(w, 34.0)),
    )
    .clicked()
}

pub(crate) fn outline_btn(ui: &mut egui::Ui, label: &str, color: Color32) -> bool {
    ui.add(
        egui::Button::new(
            RichText::new(label)
                .font(FontId::proportional(12.0))
                .color(color),
        )
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0_f32, tint(color, 80)))
        .corner_radius(R_SM),
    )
    .clicked()
}

/// Wide button with an SVG icon + text label (professional look, no glyphs).
/// Built with allocate + painter + horizontal layout so icon and text never
/// overlap and the whole button is clickable.
pub(crate) fn wide_icon_btn(ui: &mut egui::Ui, icon: SvgIcon, label: &str, color: Color32) -> bool {
    let w = ui.available_width().max(200.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 34.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let fill = if resp.hovered() {
            tint(color, 26)
        } else {
            tint(color, 18)
        };
        painter.rect_filled(rect, egui::CornerRadius::same(R_MD as u8), fill);
        painter.rect_stroke(
            rect.shrink(0.5),
            egui::CornerRadius::same(R_MD as u8),
            Stroke::new(1.0, tint(color, 80)),
            egui::StrokeKind::Outside,
        );
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        svg_icon(ui, icon, ICON_MD, color);
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(13.0))
                .color(color),
        );
    });
    resp.clicked()
}

/// Outline button with an SVG icon + text label (professional, no glyphs).
pub(crate) fn outline_icon_btn(
    ui: &mut egui::Ui,
    icon: SvgIcon,
    label: &str,
    color: Color32,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(160.0), 26.0),
        egui::Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_stroke(
            rect.shrink(0.5),
            egui::CornerRadius::same(R_SM as u8),
            Stroke::new(1.0, tint(color, 80)),
            egui::StrokeKind::Outside,
        );
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        svg_icon(ui, icon, ICON_SM, color);
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(12.0))
                .color(color),
        );
    });
    resp.clicked()
}

pub(crate) fn lbl(ui: &mut egui::Ui, text: &str, color: Color32, fs: f32) {
    ui.label(
        RichText::new(text)
            .font(FontId::proportional(fs))
            .color(color),
    );
}

/// A labeled single-line input: `Label: [input]` on one row.
pub(crate) fn labeled_input(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    width: f32,
    hint: &str,
    fs: f32,
    dim: Color32,
) {
    ui.horizontal(|ui| {
        ui.add_space(SP2);
        lbl(ui, label, dim, fs);
        ui.add_space(SP2);
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(width)
                .hint_text(hint)
                .font(FontId::proportional(fs)),
        );
    });
}

// ─── App methods ───────────────────────────────────────────────────────────

// ─── (logic methods moved to app.rs) ──────────────────────────────
// ─── eframe::App main loop ─────────────────────────────────────────────────

/// Which pixels-per-point base to use this frame.
///
/// `native` is the OS scale (`None` on platforms that do not report one), `base` the
/// value in use and `current` what we have applied. A native change re-bases (the window
/// moved to another monitor); `current` is only ever used as a first-frame fallback,
/// because it already contains the user's zoom — using it as the base compounds.
pub(crate) fn next_base(native: Option<f32>, base: f32, current: f32) -> f32 {
    match native {
        Some(n) if (n - base).abs() > 0.001 => n,
        None if base <= 0.0 => current,
        _ => base,
    }
}

impl App {
    /// Scale the whole interface by `cfg.ui_scale`.
    ///
    /// Everything is drawn in points, so one pixels-per-point change resizes text,
    /// rows, icons and spacing together — the alternative (a font size that only
    /// body text honours) left 10 px section labels and 12 px chips untouched.
    /// The base follows the monitor: `native_pixels_per_point` is the OS scale, so a
    /// window moved to a different-DPI screen re-bases instead of keeping the first
    /// screen's. (`pixels_per_point()` is what WE set — reading that back fed the zoom
    /// into its own base and made the interface creep larger every frame.)
    pub(crate) fn apply_ui_scale(&mut self, ctx: &egui::Context) {
        let native = ctx.input(|i| i.viewport().native_pixels_per_point);
        self.base_ppp = next_base(native, self.base_ppp, ctx.pixels_per_point());
        let want =
            (self.base_ppp * crate::config::clamp_scale(self.cfg.ui_scale) * 100.0).round() / 100.0;
        if (ctx.pixels_per_point() - want).abs() > 0.001 {
            ctx.set_pixels_per_point(want);
        }
    }
}

// ─── (UI draw methods moved to ui.rs) ──────────────────────────────
// ─── Helper for detail grid ─────────────────────────────────────────────────

pub(crate) fn grid_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    color: Color32,
    p: &Pal,
    fs: f32,
) {
    ui.label(
        RichText::new(format!("{label}:"))
            .font(FontId::proportional(fs - 1.5))
            .color(p.dim),
    );
    ui.label(
        RichText::new(value)
            .font(FontId::proportional(fs - 1.0))
            .color(color),
    );
    ui.end_row();
}

// ─── Entry point ────────────────────────────────────────────────────────────

/// The window as it should be created. Pure so `--minimized` can be pinned by a
/// test: the flag's whole promise is "no window", and a silent no-op would make
/// At login open its window again.
fn viewport_for(minimized: bool, saved: Option<[f32; 2]>) -> egui::ViewportBuilder {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("TorrentX")
        .with_inner_size([1300.0, 800.0])
        // Min width tuned so the header right-side controls + search bar
        // still fit without clipping on small screens (1280x720 laptops).
        .with_min_inner_size([820.0, 560.0]);
    if minimized {
        viewport = viewport.with_visible(false);
    }
    // Restore the last window size (persisted in the config; see the throttled
    // save in App::ui). Falls back to the 1300x800 default.
    if let Some([w, h]) = saved
        && w >= 820.0
        && h >= 560.0
    {
        viewport = viewport.with_inner_size([w, h]);
    }
    viewport
}

fn native_options() -> eframe::NativeOptions {
    let viewport = viewport_for(
        START_MINIMIZED.load(std::sync::atomic::Ordering::Relaxed),
        crate::config::load_cfg().win_size,
    );
    eframe::NativeOptions {
        viewport,
        // Required on Android: without the activity the event loop cannot be
        // built at all.
        #[cfg(target_os = "android")]
        android_app: ANDROID_APP.get().cloned(),
        ..Default::default()
    }
}

type AppCreator = Box<
    dyn FnOnce(
        &eframe::CreationContext,
    ) -> Result<Box<dyn eframe::App>, Box<dyn std::error::Error + Send + Sync>>,
>;

fn app_creator() -> AppCreator {
    Box::new(|cc| {
        // SVG icon support (egui_extras svg feature → resvg). Required for
        // the Lucide SVG action buttons.
        egui_extras::install_image_loaders(&cc.egui_ctx);
        Ok(Box::new(App::default()) as Box<dyn eframe::App>)
    })
}

/// Shared quit flag: set when the tray "Quit" is clicked, so the app exits
/// even from the tray (which runs outside the egui event loop).
pub(crate) static QUIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--minimized`: start with no visible window (the tray icon is the way in).
/// Used by the "At login" autostart entry so logging in does not throw the
/// search window in your face. Consumed once by the first frame on Wayland.
pub(crate) static START_MINIMIZED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Whether WE hid the X11 window (tray toggle or `--minimized`).
///
/// Focus is not a proxy: a hidden window keeps its last `focused` value, so the
/// old `Visible(!focused)` hid an already-hidden window a second time and the
/// app became unreachable from its own tray.
pub(crate) static WINDOW_HIDDEN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Hide the window at startup only when there is a tray to get it back from.
/// A hidden window with no tray is unreachable, so the flag yields.
fn should_start_hidden(requested: bool, tray: bool) -> bool {
    requested && tray
}

/// What a tray "Show / Hide" should do, given the current state: `true` = show.
/// Pure so the one-way-door failure is pinned by a test.
pub(crate) fn next_visible(hidden: bool) -> bool {
    hidden
}

/// Set when the tray "Show / Hide" is clicked; the app toggles window visibility.
pub(crate) static TOGGLE_VIS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// The running app's egui context. The tray thread needs it to wake the event
/// loop after setting a flag: the flags are polled in `App::ui`, so while the
/// app is idle (no repaint scheduled) a tray click would otherwise not be
/// observed until the window happened to receive an event — and a hidden
/// window receives none, so "Show" could never bring it back.
pub(crate) static UI_CTX: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

/// Wake the UI thread so it consumes a tray flag set by the tray thread.
pub(crate) fn wake_ui() {
    if let Some(ctx) = UI_CTX.get() {
        ctx.request_repaint();
    }
}

/// Whether this session runs on Wayland, where a window cannot be hidden or
/// raised by the client (see the tray "Show / Hide" handler).
pub(crate) fn wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var("WINIT_UNIX_BACKEND").map_or(true, |b| b != "x11")
}

/// Create the system tray icon + menu (Show/Hide, Quit) on a dedicated thread.
/// The `ksni` backend publishes a StatusNotifierItem over D-Bus and runs its
/// service on its own worker thread, so no GTK event loop (and no GTK at all)
/// is needed. Returns immediately; the tray lives for the app's lifetime.
/// Failure is non-fatal.
fn setup_tray() -> bool {
    // The tray is built on its own thread, and every failure path there used to
    // just `return` — so `--minimized` could hide the window on a machine whose
    // tray never came up, leaving no way back. Report the outcome instead.
    #[cfg(target_os = "linux")]
    {
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        std::thread::spawn(move || {
            use tray_icon::{
                Icon, TrayIconBuilder,
                menu::{Menu, MenuEvent, MenuItem},
            };

            // A 32x32 TorrentX glyph — a bold blue "T", transparent background —
            // rendered with per-pixel SDF coverage so edges are smooth at tray size.
            //
            // The original design was a near-black rounded tile with a thin blue
            // "T". Measured in the panel, the result was a dim slate mark (peak
            // (41,46,66)) with no trace of the icon blue: the tile merged into the
            // dark panel and the thin strokes anti-aliased away when Plasma scaled
            // 32px down to tray size, leaving the item effectively invisible.
            const TS: usize = 32;
            let mut rgba = Vec::with_capacity(TS * TS * 4);
            // Rounded-rect SDF: <0 inside, >0 outside.
            let rr = |x: f32, y: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32| {
                let qx = (x - cx).abs() - hw + r;
                let qy = (y - cy).abs() - hh + r;
                let ox = qx.max(0.0);
                let oy = qy.max(0.0);
                ox.hypot(oy) + qx.max(qy).min(0.0) - r
            };
            for y in 0..TS {
                for x in 0..TS {
                    let px = x as f32 + 0.5;
                    let py = y as f32 + 0.5;
                    let cov = |d: f32| (0.5 - d).clamp(0.0, 1.0);
                    // Thick strokes filling most of the frame: they must survive
                    // being scaled to ~20px.
                    let t = cov(rr(px, py, 16.0, 10.0, 12.0, 4.0, 2.0)) // T crossbar
                        .max(cov(rr(px, py, 16.0, 21.0, 4.0, 10.0, 2.0))); // T stem
                    if t <= 0.0 {
                        rgba.extend_from_slice(&[0, 0, 0, 0]);
                        continue;
                    }
                    // Bright blue: legible on dark panels (where a dark tile was
                    // invisible) and on light ones.
                    rgba.extend_from_slice(&[137, 180, 250, (t * 255.0).round() as u8]);
                }
            }
            let Ok(icon) = Icon::from_rgba(rgba, TS as u32, TS as u32) else {
                let _ = tx.send(false);
                return;
            };

            // On Wayland the window cannot be restored by the client, so the item
            // is an honest one-way "Minimize" there; X11 keeps the real toggle.
            let show = MenuItem::new(
                if wayland_session() {
                    "Minimize"
                } else {
                    "Show / Hide"
                },
                true,
                None,
            );
            let quit = MenuItem::new("Quit", true, None);
            let menu = Menu::new();
            if menu.append_items(&[&show, &quit]).is_err() {
                let _ = tx.send(false);
                return;
            }

            // Hold the handle: the tray is torn down when it is dropped (the
            // ksni backend stops its D-Bus service on Drop).
            // A STABLE id. Without it tray-icon generates "<pid>-<n>", so every
            // launch registers what the host sees as a brand-new item — and the SNI
            // spec says Id is how the host identifies an item across sessions (it is
            // what per-item placement/pinning state is keyed on).
            let Ok(_tray) = TrayIconBuilder::new()
                .with_id("torrentx")
                .with_menu(Box::new(menu))
                .with_tooltip("TorrentX")
                .with_icon(icon)
                .build()
            else {
                let _ = tx.send(false);
                return;
            };
            let _ = tx.send(true);

            // Menu events arrive on this thread's channel; signal the app.
            let show_id = show.id().clone();
            let quit_id = quit.id().clone();
            std::thread::spawn(move || {
                while let Ok(ev) = MenuEvent::receiver().recv() {
                    if ev.id == quit_id {
                        QUIT.store(true, std::sync::atomic::Ordering::SeqCst);
                        wake_ui();
                        break;
                    }
                    if ev.id == show_id {
                        TOGGLE_VIS.store(true, std::sync::atomic::Ordering::SeqCst);
                        wake_ui();
                    }
                }
            });

            // Park this thread for the app's lifetime so the tray handle above is
            // never dropped. The ksni backend runs its D-Bus service on its own
            // worker thread, so there is no GTK event loop to run here.
            loop {
                std::thread::park_timeout(std::time::Duration::from_secs(3600));
            }
        });
        // Bounded: a host without a tray, or one that never answers, must not
        // stall startup — it just means the window stays visible.
        rx.recv_timeout(std::time::Duration::from_secs(2))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Print CLI output without ever panicking.
///
/// A release build on Windows is a GUI-subsystem binary (`windows_subsystem =
/// "windows"`), so it starts with no console and stdout is not a usable handle:
/// `println!` panics with "failed printing to stdout: The pipe is being closed"
/// (os error 232) when a shell captures it. Attach to the parent console, then
/// write and ignore the error — `--version` from Explorer prints nowhere, which
/// is fine, and `--version` from a terminal prints normally.
fn print_cli(s: &str) {
    #[cfg(all(windows, not(debug_assertions)))]
    {
        // kernel32 is always linked; declaring the one symbol beats adding
        // windows-sys as a direct dependency for eight lines.
        // `extern` blocks are unsafe since Rust 2024; without the keyword this
        // block failed to compile on Windows only — the Linux CI never sees it,
        // and a multi-command pwsh build step hid the failure.
        unsafe extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
        // SAFETY: single documented call, no pointers; it either attaches this
        // process to the parent's console or fails harmlessly.
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
    use std::io::Write;
    let mut out = std::io::stdout();
    let _ = out.write_all(s.as_bytes());
    let _ = out.flush();
}

/// The Android activity handle. Set once by `android_main`; `native_options`
/// needs it to build the event loop, and the config path needs it to find the
/// app's writable storage (there is no XDG dir on a phone).
#[cfg(target_os = "android")]
static ANDROID_APP: std::sync::OnceLock<winit::platform::android::activity::AndroidApp> =
    std::sync::OnceLock::new();

/// The app's private storage on Android, or `None` before the activity exists.
#[cfg(target_os = "android")]
pub(crate) fn android_data_dir() -> Option<std::path::PathBuf> {
    ANDROID_APP.get()?.internal_data_path()
}

pub fn run_gui() -> eframe::Result<()> {
    run()
}

/// Android has no `main`: the OS calls this on the activity's thread, and it
/// must return before the process is killed, so the whole app runs inside it.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    let _ = ANDROID_APP.set(app);
    if let Err(e) = run() {
        log::error!("TorrentX exited: {e}");
    }
}

fn run() -> eframe::Result<()> {
    // Parse CLI flags before any GUI/config work.
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_cli(&format!(
            "TorrentX {} — native Jackett torrent-search GUI\n\n\
             USAGE:\n  torrentx [OPTIONS]\n\n\
             OPTIONS:\n  \
             --config <path>   Use an alternate config file (default: ~/.config/torrentx/config.json)\n  \
             --minimized       Start without a window (tray icon only; used by At login)\n  \
             -h, --help        Print this help and exit\n  \
             -V, --version     Print the version and exit\n",
            env!("CARGO_PKG_VERSION")
        ));
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        print_cli(&format!("TorrentX {}\n", env!("CARGO_PKG_VERSION")));
        return Ok(());
    }

    START_MINIMIZED.store(
        args.iter().any(|a| a == "--minimized"),
        std::sync::atomic::Ordering::Relaxed,
    );

    // Optional --config <path> override (before any config is loaded).
    if let Some(i) = args.iter().position(|a| a == "--config")
        && let Some(p) = args.get(i + 1)
    {
        config::set_config_override(std::path::PathBuf::from(p));
    }

    // One copy per profile: a second launch tells the running one to show its
    // window instead of starting a rival config writer.
    if !claim_instance() {
        eprintln!("TorrentX is already running — asked it to show its window.");
        return Ok(());
    }

    // System tray on a dedicated thread (optional; non-fatal if unavailable).
    // `--minimized` is only honoured when the tray actually came up.
    let tray = setup_tray();
    let requested_minimized = START_MINIMIZED.load(std::sync::atomic::Ordering::Relaxed);
    if requested_minimized && !tray {
        eprintln!("--minimized ignored: no system tray, so a hidden window could not be restored");
    }
    START_MINIMIZED.store(
        should_start_hidden(requested_minimized, tray),
        std::sync::atomic::Ordering::Relaxed,
    );

    // Try the normal (GPU-accelerated) run first.
    let result = match eframe::run_native("TorrentX", native_options(), app_creator()) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("GPU init failed ({e}); retrying with software rendering…");
            // Retry with a software-friendly renderer. These env vars are read
            // by egui-wgpu/Mesa at adapter-selection time — i.e. INSIDE the
            // run_native call below — so setting them here is effective:
            // WGPU_BACKEND=opengl + LIBGL_ALWAYS_SOFTWARE steer wgpu onto Mesa's
            // software GL (llvmpipe), fixing black windows on machines where
            // hardware Vulkan/GL can't initialize (VMs, NVIDIA+Wayland quirks).
            // Edition 2024 marks `set_var` unsafe because a concurrent getenv in
            // another thread is UB. This runs before the eframe event loop starts
            // and only the tray thread exists; it reads no environment. The
            // supported alternative is `WgpuConfiguration::supported_backends`,
            // but the *software* adapter on Mesa is still chosen by these same
            // variables, so the fallback keeps them.
            //
            // SAFETY: single set of writes, no other thread reads the environment
            // (the tray thread does not), and it happens during startup before any
            // library call can observe a partially updated environment.
            unsafe {
                std::env::set_var("WGPU_BACKEND", "opengl");
                std::env::set_var("WGPU_POWER_PREF", "low");
                std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
                std::env::set_var("GALLIUM_DRIVER", "llvmpipe");
            }
            eframe::run_native("TorrentX", native_options(), app_creator())
        }
    };
    // Stop the Jackett we started, if any (a no-op when the user runs their own).
    jackett::stop_local();
    // Stop the stream we started, if any (the player sees EOF and closes).
    player::stop_play();
    result
}

#[cfg(test)]
mod tests {
    use super::{
        csv_esc, csv_safe, desktop_entry, next_visible, safe_scheme, should_start_hidden,
        viewport_for,
    };

    #[test]
    fn csv_esc_quotes_and_doubles_embedded_quotes() {
        assert_eq!(csv_esc("plain"), "\"plain\"");
        assert_eq!(csv_esc("has \"quotes\""), "\"has \"\"quotes\"\"\"");
        assert_eq!(csv_esc(""), "\"\"");
        // Commas are harmless inside quotes.
        assert_eq!(csv_esc("a,b,c"), "\"a,b,c\"");
    }

    #[test]
    fn csv_safe_neutralizes_formula_injection() {
        // Benign values pass through unchanged.
        assert_eq!(csv_safe("Ubuntu 24.04"), "\"Ubuntu 24.04\"");
        assert_eq!(csv_safe(""), "\"\"");
        // Formula-leading characters get a `'` prefix (after the quote).
        assert_eq!(csv_safe("=cmd|' /C calc'!A0"), "\"'=cmd|' /C calc'!A0\"");
        assert_eq!(csv_safe("+1+1"), "\"'+1+1\"");
        assert_eq!(csv_safe("-2"), "\"'-2\"");
        assert_eq!(csv_safe("@SUM(1)"), "\"'@SUM(1)\"");
        assert_eq!(csv_safe("\tTab"), "\"'\tTab\"");
    }

    #[test]
    fn scheme_allow_list() {
        assert!(safe_scheme("magnet:?xt=urn:btih:abc"));
        assert!(safe_scheme("https://example.com/x"));
        assert!(safe_scheme("http://localhost:9117/y"));
        // Untrusted / dangerous schemes are rejected before xdg-open.
        assert!(!safe_scheme("file:///home/user/.ssh/id_rsa"));
        assert!(!safe_scheme("ftp://host/f"));
        assert!(!safe_scheme("javascript:alert(1)"));
        assert!(!safe_scheme(""));
    }

    /// A hidden window with no tray icon is unreachable: `--minimized` must yield
    /// when the tray failed to build (missing appindicator, no D-Bus watcher).
    #[test]
    fn minimized_is_ignored_without_a_tray() {
        assert!(
            should_start_hidden(true, true),
            "tray present: honour --minimized"
        );
        assert!(
            !should_start_hidden(true, false),
            "no tray: keep the window visible"
        );
        assert!(
            !should_start_hidden(false, true),
            "no flag: visible as usual"
        );
    }

    /// One copy per profile. Two copies share `config.json` and each save
    /// rewrites the whole file, so the second one silently clobbers the first
    /// one's favourites and history.
    #[cfg(unix)]
    #[test]
    fn a_second_launch_is_refused_and_told_to_show() {
        use super::{SHOW_REQ, claim_instance_at, serve_show_requests};
        use std::sync::atomic::Ordering;

        let dir = std::env::temp_dir().join(format!("tx-oneinst-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("torrentx.sock");

        let owner = claim_instance_at(&sock).expect("first claim owns the lock");
        serve_show_requests(owner);

        SHOW_REQ.store(false, Ordering::SeqCst);
        assert!(
            claim_instance_at(&sock).is_none(),
            "a second launch must be refused while the first is alive"
        );
        // The refused launch asked the owner to show its window.
        let mut waited = 0;
        while !SHOW_REQ.load(Ordering::SeqCst) && waited < 100 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            waited += 1;
        }
        assert!(
            SHOW_REQ.load(Ordering::SeqCst),
            "owner was not asked to show"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A crashed instance leaves its socket file behind. That file must not lock
    /// the user out of their own app forever.
    #[cfg(unix)]
    #[test]
    fn a_stale_socket_is_reclaimed() {
        use super::claim_instance_at;

        let dir = std::env::temp_dir().join(format!("tx-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("torrentx.sock");

        {
            let dead = std::os::unix::net::UnixListener::bind(&sock).unwrap();
            drop(dead); // process died: the file stays, nothing listens
        }
        assert!(sock.exists(), "the stale socket file should still be there");
        assert!(
            claim_instance_at(&sock).is_some(),
            "a stale socket must be reclaimed, not block startup"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The tray toggle must bring a hidden window BACK. The old branch read
    /// `focused` as the visibility source, and a hidden window reports its last
    /// focus state — so "Show" hid it again and `--minimized` was a one-way door.
    #[test]
    fn tray_toggle_shows_a_hidden_window() {
        assert!(next_visible(true), "hidden window must be shown");
        assert!(!next_visible(false), "visible window must be hidden");
    }

    /// `--minimized` is only honest if the window is created invisible; the flag
    /// also must not disturb the saved-size restore.
    #[test]
    fn minimized_creates_an_invisible_window_and_keeps_the_saved_size() {
        assert_eq!(
            viewport_for(true, None).visible,
            Some(false),
            "--minimized must start hidden"
        );
        assert_eq!(
            viewport_for(false, None).visible,
            None,
            "a normal start leaves visibility to eframe"
        );

        let saved = viewport_for(false, Some([1024.0, 700.0]));
        assert_eq!(saved.inner_size, Some(egui::vec2(1024.0, 700.0)));

        // Too small to be a real window (a corrupt or pre-min-size config):
        // fall back to the default rather than restoring a useless geometry.
        let tiny = viewport_for(false, Some([100.0, 80.0]));
        assert_eq!(tiny.inner_size, Some(egui::vec2(1300.0, 800.0)));
    }

    /// The autostart entry is the whole feature: it must name the AppImage (the
    /// binary inside the mount vanishes), carry Exec, and be a valid desktop
    /// entry — a malformed one is silently ignored by every desktop.
    #[test]
    #[cfg(target_os = "linux")]
    fn autostart_entry_names_the_appimage_and_is_a_desktop_entry() {
        let e = desktop_entry(std::path::Path::new(
            "/home/u/Applications/TorrentX.AppImage",
        ));
        assert!(e.starts_with("[Desktop Entry]\n"), "{e}");
        // Quoted per the desktop-entry spec: an unquoted path with a space is
        // split into argv words by every launcher. --minimized keeps a login
        // from throwing the window in front of the user.
        assert!(
            e.contains("\nExec=\"/home/u/Applications/TorrentX.AppImage\" --minimized\n"),
            "{e}"
        );
        assert!(e.contains("\nType=Application\n"), "{e}");
        assert!(e.ends_with('\n'), "desktop entries end with a newline");
        let spaced = desktop_entry(std::path::Path::new("/home/u/My Apps/TorrentX.AppImage"));
        assert!(
            spaced.contains("\nExec=\"/home/u/My Apps/TorrentX.AppImage\" --minimized\n"),
            "{spaced}"
        );
    }
}

#[cfg(test)]
mod ui_scale_tests {
    use super::next_base;

    #[test]
    fn the_base_follows_the_monitor_and_never_the_applied_zoom() {
        // Window moved to a 2x monitor: re-base.
        assert_eq!(next_base(Some(2.0), 1.0, 1.5), 2.0);
        // Our own applied zoom (1.2 = base 1.0 * scale 1.2) must NOT become the base,
        // or the interface would grow again every frame.
        assert_eq!(next_base(Some(1.0), 1.0, 1.2), 1.0);
        // Platform without a native value: capture once.
        assert_eq!(next_base(None, 0.0, 1.5), 1.5);
        assert_eq!(next_base(None, 1.5, 1.8), 1.5);
    }
}
