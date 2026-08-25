//! favorites drawing methods.

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_favorites(&mut self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(format!("Favorites  ({})", self.cfg.favorites.len()))
                    .font(FontId::proportional(18.0))
                    .color(self.pal.text)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0);
                if !self.cfg.favorites.is_empty() && outline_btn(ui, "Clear all", self.pal.red) {
                    self.cfg.favorites.clear();
                    save_cfg(&self.cfg);
                }
            });
        });

        if self.cfg.favorites.is_empty() {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                lbl(ui, "No favorites yet", self.pal.sub, 20.0);
                ui.add_space(6.0);
                lbl(
                    ui,
                    "Click Fav on any result, or press F when a row is selected",
                    self.pal.dim,
                    fs,
                );
            });
            return;
        }

        // Search box
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            lbl(ui, "Search:", self.pal.dim, fs);
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.ui.fav_search)
                    .desired_width(FAV_SEARCH_W)
                    .hint_text("filter favorites…")
                    .font(FontId::proportional(fs)),
            );
            if !self.ui.fav_search.is_empty()
                && ui
                    .add(
                        egui::Button::new(svg_image(SvgIcon::Close, 12.0, self.pal.sub))
                            .fill(Color32::TRANSPARENT)
                            .frame(false),
                    )
                    .clicked()
            {
                self.ui.fav_search.clear();
            }
        });
        ui.add_space(8.0);

        let mut remove: Option<usize> = None;
        let mut open_mag: Option<String> = None;
        let mut open_link: Option<String> = None;
        let fq = self.ui.fav_search.to_lowercase();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let favs = self.cfg.favorites.clone();
            let mut row_i = 0usize;
            for (i, fav) in favs.iter().enumerate() {
                if !fq.is_empty()
                    && !fav.title.to_lowercase().contains(&fq)
                    && !fav
                        .tracker
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&fq)
                {
                    continue;
                }
                row_i += 1;
                let bg = if row_i.is_multiple_of(2) {
                    self.pal.row_odd
                } else {
                    self.pal.row_even
                };
                // Full-row click layer FIRST (covers the whole row rect, text
                // included). Drawn before content so text stays clickable.
                let row_rect = ui.max_rect();
                let row_resp =
                    ui.interact(row_rect, egui::Id::new(("favrow", i)), egui::Sense::click());
                if row_resp.clicked() {
                    if let Some(m) = &fav.magnet {
                        if is_magnet(m) {
                            open_mag = Some(m.clone());
                        }
                    }
                }
                if row_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                egui::Frame::NONE
                    .fill(bg)
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                // Never negative — actions row takes ~130px;
                                // at very narrow widths let the title wrap.
                                ui.set_min_width((ui.available_width() - 130.0).max(60.0));
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&fav.title)
                                            .font(FontId::proportional(fs))
                                            .color(self.pal.text),
                                    )
                                    .truncate(),
                                );
                                ui.horizontal(|ui| {
                                    if let Some(t) = &fav.tracker {
                                        lbl(ui, t, self.pal.sub, fs - 1.5);
                                    }
                                    if let Some(s) = fav.size {
                                        lbl(
                                            ui,
                                            &format!("·  {}", fmt_size(s)),
                                            self.pal.dim,
                                            fs - 1.5,
                                        );
                                    }
                                    if let Some(s) = fav.seeders {
                                        lbl(ui, &format!("·  {} seeds", s), seed_col(s), fs - 1.5);
                                    }
                                    if !fav.saved_at.is_empty() {
                                        lbl(
                                            ui,
                                            &format!("·  saved {}", fav.saved_at),
                                            self.pal.dim,
                                            fs - 2.0,
                                        );
                                    }
                                });
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    if fav.magnet.as_deref().map(is_magnet).unwrap_or(false)
                                        && svg_btn(
                                            ui,
                                            SvgIcon::Magnet,
                                            "Open magnet",
                                            self.pal.accent,
                                        )
                                    {
                                        open_mag = fav.magnet.clone();
                                    }
                                    if fav.link.is_some()
                                        && svg_btn(
                                            ui,
                                            SvgIcon::Download,
                                            "Download .torrent",
                                            self.pal.green,
                                        )
                                    {
                                        open_link = fav.link.clone();
                                    }
                                    if svg_btn(ui, SvgIcon::Close, "Remove", self.pal.red) {
                                        remove = Some(i);
                                    }
                                },
                            );
                        });
                    });
                ui.separator();
            }
        });

        if let Some(i) = remove {
            self.cfg.favorites.remove(i);
            save_cfg(&self.cfg);
        }
        if let Some(m) = open_mag {
            let _ = safe_open(m);
            self.toast("Opening magnet…", self.pal.accent);
        }
        if let Some(l) = open_link {
            let _ = safe_open(l);
            self.toast("Downloading…", self.pal.green);
        }
    }

    // ─── RSS Tab ────────────────────────────────────────────────────────
}
