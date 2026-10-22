//! Results table + detail panel drawing methods (split out for readability).

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_idle(&mut self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        ui.add_space(SP6 + SP7);

        // A dead Jackett is the one failure that makes every search useless, and
        // the idle screen used to say nothing about it (only the header dot went
        // red). Put the reason and the way to fix it where the user is looking.
        if self.net.jackett_ok == Some(false) {
            ui.vertical_centered(|ui| {
                egui::Frame::NONE
                    .fill(tint(self.pal.red, 20))
                    .corner_radius(R_MD)
                    .stroke(Stroke::new(1.0_f32, tint(self.pal.red, 90)))
                    .inner_margin(margin(SP5, SP3))
                    .show(ui, |ui| {
                        ui.set_max_width(560.0);
                        ui.label(
                            RichText::new("Jackett is unreachable")
                                .font(FontId::proportional(fs + 1.0))
                                .strong()
                                .color(self.pal.red),
                        );
                        ui.add_space(SP2);
                        lbl(
                            ui,
                            &format!("Checked {}", self.cfg.jackett_url),
                            self.pal.sub,
                            fs - 1.0,
                        );
                        ui.add_space(SP3);
                        if icon_text_btn(
                            ui,
                            SvgIcon::Settings,
                            "Open Settings",
                            self.pal.accent,
                            true,
                        ) {
                            self.set_settings_open(true);
                        }
                    });
            });
            ui.add_space(SP5);
        }

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("TorrentX")
                    .font(FontId::proportional(40.0))
                    .strong()
                    .color(tint(self.pal.accent, 230)),
            );
            ui.add_space(SP3);
            lbl(
                ui,
                "Search all your Jackett indexers in one shot",
                self.pal.sub,
                fs + 1.0,
            );
            ui.add_space(SP1);
            lbl(
                ui,
                "Movies  ·  TV  ·  Music  ·  Games  ·  Software  ·  Anime  ·  Books",
                self.pal.dim,
                fs - 1.0,
            );
            ui.add_space(SP7);

            if !self.cfg.history.is_empty() {
                // Heading and pills share one centred column; a wrapped row
                // inside `vertical_centered` starts at the left edge, which read
                // as a misaligned block under a centred heading.
                let block_w = ui.available_width().min(560.0);
                let mut clicked: Option<String> = None;
                ui.allocate_ui_with_layout(
                    egui::vec2(block_w, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        lbl(ui, "Recent searches", self.pal.dim, fs - 1.0);
                        ui.add_space(SP4);
                        let hist: Vec<String> = self.cfg.history.iter().take(12).cloned().collect();
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                            for h in &hist {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(h.as_str())
                                                .font(FontId::proportional(fs))
                                                .color(self.pal.sub),
                                        )
                                        .fill(self.pal.surface)
                                        .stroke(Stroke::new(1.0_f32, self.pal.border))
                                        .corner_radius(R_LG)
                                        .min_size(egui::vec2(0.0, 28.0)),
                                    )
                                    .clicked()
                                {
                                    clicked = Some(h.clone());
                                }
                            }
                        });
                    },
                );
                if let Some(h) = clicked {
                    self.search.query = h;
                    self.do_search();
                }
            } else {
                lbl(ui, "Try searching:", self.pal.dim, fs - 1.0);
                ui.add_space(SP4);
                let suggestions = ["Linux Mint", "Ubuntu 24.04", "Blender", "GIMP", "Inkscape"];
                let mut clicked: Option<&str> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    for s in &suggestions {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(*s)
                                        .font(FontId::proportional(fs))
                                        .color(self.pal.dim),
                                )
                                .fill(self.pal.surface)
                                .stroke(Stroke::new(1.0_f32, tint(self.pal.border, 140)))
                                .corner_radius(R_LG)
                                .min_size(egui::vec2(0.0, 28.0)),
                            )
                            .clicked()
                        {
                            clicked = Some(s);
                        }
                    }
                });
                if let Some(s) = clicked {
                    self.search.query = s.to_string();
                    self.do_search();
                }

                ui.add_space(SP7);
                egui::Frame::NONE
                    .fill(tint(self.pal.accent, 12))
                    .corner_radius(R_LG)
                    .stroke(Stroke::new(1.0_f32, tint(self.pal.accent, 50)))
                    .inner_margin(margin(SP6, SP5))
                    .show(ui, |ui| {
                        ui.set_max_width(480.0);
                        ui.label(
                            RichText::new("First time?")
                                .font(FontId::proportional(fs + 1.0))
                                .color(self.pal.accent)
                                .strong(),
                        );
                        ui.add_space(SP3);
                        lbl(
                            ui,
                            "1. Make sure Jackett is running  (localhost:9117)",
                            self.pal.sub,
                            fs - 1.0,
                        );
                        ui.horizontal(|ui| {
                            lbl(ui, "2. Click", self.pal.sub, fs - 1.0);
                            svg_icon(ui, SvgIcon::Settings, ICON_SM, self.pal.sub);
                            lbl(
                                ui,
                                "Settings and paste your API key",
                                self.pal.sub,
                                fs - 1.0,
                            );
                        });
                        lbl(ui, "3. Search for anything!", self.pal.sub, fs - 1.0);
                        ui.add_space(SP4);
                        if outline_btn(ui, "Open Settings", self.pal.accent) {
                            self.set_settings_open(true);
                        }
                    });
            }
        });
    }

    // ─── Detail panel ──────────────────────────────────────────────────────
}
