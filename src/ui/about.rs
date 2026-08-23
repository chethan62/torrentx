//! about drawing methods.

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_about(&self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(30.0);
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
                ui.add_space(4.0);
                lbl(
                    ui,
                    "Native Rust + egui torrent search GUI powered by Jackett",
                    self.pal.sub,
                    fs + 1.0,
                );

                ui.add_space(24.0);
                for (k, v) in [
                    ("Language", "Rust 2021 edition"),
                    ("GUI", "egui 0.36 + egui_extras"),
                    ("Rendering", "GPU via wgpu / OpenGL (eframe)"),
                    ("HTTP", "reqwest (blocking)"),
                    ("Config", "~/.config/torrentx/config.json"),
                ] {
                    ui.horizontal(|ui| {
                        ui.add_space(ui.available_width() * 0.15);
                        lbl(ui, &format!("{k:<18}"), self.pal.dim, fs);
                        lbl(ui, v, self.pal.sub, fs);
                    });
                    ui.add_space(2.0);
                }

                // Theme swatches
                ui.add_space(24.0);
                lbl(
                    ui,
                    &format!("{} Themes", Theme::all().len()),
                    self.pal.accent,
                    fs + 1.0,
                );
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    ui.add_space(40.0);
                    for t in Theme::all() {
                        let col = t.accent_color();
                        let active = &self.cfg.theme == t;
                        egui::Frame::NONE
                            .fill(tint(col, if active { 45 } else { 20 }))
                            .corner_radius(6.0)
                            .stroke(Stroke::new(
                                if active { 2.0_f32 } else { 1.0_f32 },
                                tint(col, if active { 220 } else { 90 }),
                            ))
                            .inner_margin(egui::Margin::symmetric(9, 4))
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
                ui.add_space(24.0);
                lbl(ui, "Features", self.pal.accent, fs + 1.0);
                ui.add_space(8.0);
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
                    ui.add_space(1.0);
                }

                // Shortcuts
                ui.add_space(24.0);
                lbl(ui, "Keyboard Shortcuts", self.pal.accent, fs + 1.0);
                ui.add_space(10.0);
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
                            .corner_radius(4.0),
                        );
                        ui.add_space(8.0);
                        lbl(ui, v, self.pal.sub, fs);
                    });
                    ui.add_space(4.0);
                }

                ui.add_space(20.0);
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
                        .inner_margin(egui::Margin::symmetric(14, 9))
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
