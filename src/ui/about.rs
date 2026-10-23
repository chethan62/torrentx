//! about drawing methods.

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_about(&mut self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(SP7);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("TorrentX")
                        .font(FontId::proportional(30.0))
                        .color(self.pal.text)
                        .strong(),
                );
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .font(FontId::proportional(fs))
                        .color(self.pal.accent),
                );
                ui.add_space(SP2);
                lbl(
                    ui,
                    "Native Rust + egui torrent search GUI powered by Jackett",
                    self.pal.sub,
                    fs + 1.0,
                );

                ui.add_space(SP6);
                let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
                // No hand-written dependency versions: they drift (this row said
                // "reqwest 0.12" while Cargo.toml had 0.13). Names only, and the
                // app's own version above comes from CARGO_PKG_VERSION.
                for (k, v) in [
                    ("Language", "Rust 2021 edition".to_string()),
                    ("GUI", "egui + egui_extras".to_string()),
                    ("Rendering", "GPU via wgpu / OpenGL (eframe)".to_string()),
                    ("HTTP", "reqwest (blocking)".to_string()),
                    ("Platform", platform),
                    (
                        "Build",
                        if cfg!(debug_assertions) {
                            "debug".to_string()
                        } else {
                            "release".to_string()
                        },
                    ),
                    ("Config", "~/.config/torrentx/config.json".to_string()),
                ] {
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() * 0.15);
                        lbl(ui, &format!("{k:<18}"), self.pal.dim, fs);
                        lbl(ui, &v, self.pal.sub, fs);
                    });
                    ui.add_space(SP1);
                }

                // Install GUID — click to copy (for bug reports)
                ui.add_space(SP3);
                ui.horizontal(|ui| {
                    ui.add_space(ui.available_width() * 0.15);
                    lbl(ui, &format!("{:<18}", "Install ID"), self.pal.dim, fs);
                    let gid = self.cfg.install_id.clone();
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(if gid.is_empty() { "(unassigned)" } else { &gid })
                                    .font(FontId::monospace(fs - 1.0))
                                    .color(self.pal.accent),
                            )
                            .fill(Color32::TRANSPARENT)
                            .frame(false),
                        )
                        .on_hover_text("Click to copy this install's anonymous ID")
                        .clicked()
                        && !gid.is_empty()
                    {
                        ui.ctx().copy_text(gid.clone());
                        self.toast("Install ID copied", self.pal.green);
                    }
                });

                // Theme swatches
                ui.add_space(SP6);
                lbl(
                    ui,
                    &format!("{} Themes", Theme::all().len()),
                    self.pal.accent,
                    fs + 1.0,
                );
                ui.add_space(SP4);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    ui.add_space(SP7);
                    for t in Theme::all() {
                        let col = t.accent_color();
                        let active = &self.cfg.theme == t;
                        egui::Frame::NONE
                            .fill(tint(col, if active { 45 } else { 20 }))
                            .corner_radius(R_MD)
                            .stroke(Stroke::new(
                                if active { 2.0_f32 } else { 1.0_f32 },
                                tint(col, if active { 220 } else { 90 }),
                            ))
                            .inner_margin(margin(SP3, SP2))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(t.name())
                                        .font(FontId::proportional(fs - 1.5))
                                        .color(col),
                                );
                            });
                    }
                });

                // Features
                ui.add_space(SP6);
                lbl(ui, "Features", self.pal.accent, fs + 1.0);
                ui.add_space(SP3);
                for f in [
                    "Search all Jackett indexers simultaneously",
                    "19 themes — 16 dark, 3 light — instant switching",
                    "Toggle columns: Tracker, Size, Leech, Ratio, Health, Date",
                    "Row density: Compact / Normal / Roomy",
                    "Font size: Small / Medium / Large",
                    "Filter by text, seeds, size, year, tracker, health status",
                    "Sort by Name, Tracker, Size, Seeds, Leechers, Date",
                    "Hover highlight per theme + selected row highlight",
                    "Animated spinner with elapsed time",
                    "Clickable category chips to filter by category",
                    "Search history with per-item delete",
                    "Favorites with search filter, timestamps, persistent storage",
                    "Detail side panel with seeder/leecher ratio bar",
                    "Deduplication across trackers",
                    "RSS feeds with background auto-refresh (configurable)",
                    "Per-indexer search — pick one Jackett indexer",
                    "Automatic update check on startup",
                    "Export filtered results to CSV",
                    "Pagination: 25 / 50 / 100 / All per page",
                    "Keyboard nav: Arrow keys, Enter, D, F, M, Ctrl+F, Ctrl+R, Esc",
                ] {
                    lbl(ui, &format!("  ·  {f}"), self.pal.sub, fs - 1.0);
                    ui.add_space(SP1);
                }

                // Shortcuts
                ui.add_space(SP6);
                lbl(ui, "Keyboard Shortcuts", self.pal.accent, fs + 1.0);
                ui.add_space(SP4);
                for (k, v) in [
                    ("Up / Down", "Navigate result rows"),
                    ("Enter", "Open magnet for selected row"),
                    ("D", "Toggle detail panel"),
                    ("F", "Add selected to Favorites"),
                    ("M", "Open magnet for selected row"),
                    ("Esc", "Close detail panel / clear search"),
                    ("Ctrl+F", "Focus search bar"),
                    ("Ctrl+R", "Re-run last search"),
                    ("Ctrl+C", "Copy magnet (detail panel open)"),
                ] {
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() * 0.12);
                        ui.add(
                            egui::Button::new(
                                RichText::new(k)
                                    .font(FontId::proportional(fs))
                                    .color(self.pal.accent),
                            )
                            .fill(self.pal.surface)
                            .stroke(Stroke::new(1.0_f32, self.pal.border))
                            .corner_radius(R_SM),
                        );
                        ui.add_space(SP3);
                        lbl(ui, v, self.pal.sub, fs);
                    });
                    ui.add_space(SP2);
                }

                // Credits
                ui.add_space(SP6);
                lbl(ui, "Credits", self.pal.accent, fs + 1.0);
                ui.add_space(SP3);
                for line in [
                    "Created by chethan62",
                    "Powered by Jackett — the open-source indexer proxy",
                    "Built with egui/eframe, reqwest, quick-xml, tray-icon, notify-rust",
                    "CJK fonts: Noto Sans CJK JP + KR (SIL OFL) · Icons: Lucide SVG",
                    "License: MIT OR Apache-2.0 — free to use, modify, and share",
                ] {
                    lbl(ui, &format!("  ·  {line}"), self.pal.sub, fs - 1.0);
                    ui.add_space(SP1);
                }

                // The OFL notice has to be viewable where the fonts are shipped,
                // and the release artifacts are a bare binary and an AppImage, so
                // "stand-alone text file" means in here as well as in the repo.
                ui.add_space(SP4);
                licence_section(
                    ui,
                    &self.pal,
                    fs,
                    "Bundled font licence (SIL OFL 1.1)",
                    "font_license",
                    crate::app::FONT_LICENSE,
                );
                ui.add_space(SP2);
                licence_section(
                    ui,
                    &self.pal,
                    fs,
                    "Bundled icon licence (ISC, Lucide)",
                    "icon_license",
                    crate::app::ICON_LICENSE,
                );

                // Links
                ui.add_space(SP4);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    ui.add_space(SP7);
                    for (label, url) in [
                        ("GitHub", "https://github.com/chethan62/torrentx"),
                        (
                            "Releases",
                            "https://github.com/chethan62/torrentx/releases/latest",
                        ),
                        (
                            "Report an Issue",
                            "https://github.com/chethan62/torrentx/issues/new",
                        ),
                        ("Jackett", "https://github.com/Jackett/Jackett"),
                    ] {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(label)
                                        .font(FontId::proportional(12.0))
                                        .color(self.pal.sub),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::new(1.0_f32, tint(self.pal.sub, 80)))
                                .corner_radius(R_SM),
                            )
                            .on_hover_text(url)
                            .clicked()
                        {
                            let _ = crate::safe_open(url);
                        }
                    }
                });

                ui.add_space(SP6);
                lbl(ui, "github.com/chethan62/torrentx", self.pal.dim, fs - 1.0);
            });
        });
    }

    // ─── Toast notifications ───────────────────────────────────────────────

    pub(crate) fn draw_toasts(&self, ctx: &egui::Context) {
        if self.ui.toasts.is_empty() {
            return;
        }
        let scr = ctx.input(|i| i.viewport_rect());
        let mut y = scr.max.y - 54.0;
        for toast in self.ui.toasts.iter().rev() {
            // Fade out during last 0.4s, slide in during first 0.15s
            let fade_a = ((toast.ttl.min(0.4) / 0.4) * 230.0) as u8;
            let slide_progress = (1.0 - toast.anim_progress.min(1.0)) * 30.0; // slide from right
            // Clamp to viewport so toasts never go off-screen at narrow widths
            let x_pos = (scr.max.x - 310.0 + slide_progress).max(8.0);

            egui::Area::new(egui::Id::new(format!("toast_{}", toast.msg)))
                .fixed_pos([x_pos, y])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(tint(self.pal.surface, fade_a))
                        .stroke(Stroke::new(1.5_f32, tint(toast.col, fade_a)))
                        .corner_radius(PANEL_RADIUS)
                        .inner_margin(margin(SP5, SP3))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 2],
                            blur: 8,
                            spread: 0,
                            color: rgba(0, 0, 0, 80),
                        })
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(&toast.msg)
                                    .font(FontId::proportional(13.5))
                                    .color(tint(toast.col, fade_a)),
                            );
                        });
                });
            y -= 46.0;
        }
    }
}

/// One licence notice, collapsed by default. The text is a legal notice, so it
/// uses the secondary text tier (not the dimmest) and is selectable: a user can
/// copy it out of the app instead of hunting for the file.
fn licence_section(
    ui: &mut egui::Ui,
    pal: &Pal,
    fs: f32,
    title: &str,
    id: &str,
    text: &'static str,
) {
    egui::CollapsingHeader::new(
        RichText::new(title)
            .font(FontId::proportional(fs - 1.0))
            .color(pal.sub),
    )
    .id_salt(id)
    .show(ui, |ui| {
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .id_salt(format!("{id}_scroll"))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(text)
                            .font(FontId::monospace(fs - 2.0))
                            .color(pal.sub),
                    )
                    .selectable(true)
                    .wrap(),
                );
            });
    });
}
