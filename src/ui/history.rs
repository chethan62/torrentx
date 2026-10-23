//! search drawing methods (split out for readability).

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_history_dropdown(
        &mut self,
        ctx: &egui::Context,
        bar_rect: egui::Rect,
        fs: f32,
    ) {
        if self.ui.show_hist && !self.cfg.history.is_empty() {
            let scr = ctx.input(|i| i.viewport_rect());
            // Clamp the dropdown to the viewport: never start left of the
            // window and never extend past the right edge (overflow bug at
            // narrow widths / when the search bar is scrolled).
            let w = bar_rect.width().max(280.0).min(scr.width() - 16.0);
            let x = bar_rect.min.x.clamp(8.0, (scr.max.x - w - 8.0).max(8.0));
            let pos = egui::pos2(x, bar_rect.max.y + 4.0);
            let hist = self.cfg.history.clone();
            let mut clicked: Option<String> = None;
            let mut deleted: Option<String> = None;
            let mut clear_all = false;

            let area = egui::Area::new(egui::Id::new("hist_dd"))
                .fixed_pos(pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(self.pal.surface)
                        .corner_radius(PANEL_RADIUS)
                        .stroke(Stroke::new(1.0_f32, self.pal.accent))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 12,
                            spread: 0,
                            color: rgba(0, 0, 0, 70),
                        })
                        .inner_margin(margin(SP4, SP3))
                        .show(ui, |ui| {
                            ui.set_width(w);
                            ui.horizontal_wrapped(|ui| {
                                lbl(ui, "Recent searches", self.pal.dim, 11.0);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new("clear all")
                                                        .size(11.0)
                                                        .color(self.pal.dim),
                                                )
                                                .fill(Color32::TRANSPARENT)
                                                .frame(false),
                                            )
                                            .clicked()
                                        {
                                            clear_all = true;
                                        }
                                    },
                                );
                            });
                            ui.add_space(SP2);
                            // Show the whole stored history (20 entries), not the
                            // first 10 — the rest was unreachable. Capped in height
                            // so a full history cannot overflow the window.
                            egui::ScrollArea::vertical()
                                .id_salt("hist_list")
                                .max_height(320.0)
                                .show(ui, |ui| {
                                    for h in hist.iter() {
                                        // One row: the entry fills the width, the
                                        // remove × sits at its right edge. The
                                        // wrapping row + right-to-left layout pushed
                                        // the × onto its own line, so every entry
                                        // was twice as tall and misaligned.
                                        ui.horizontal(|ui| {
                                            let gap = ui.spacing().item_spacing.x;
                                            let bw = (ui.available_width() - 18.0 - gap).max(80.0);
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new(h.as_str())
                                                            .font(FontId::proportional(fs))
                                                            .color(self.pal.text),
                                                    )
                                                    .fill(Color32::TRANSPARENT)
                                                    .frame(false)
                                                    .min_size(egui::vec2(bw, 24.0)),
                                                )
                                                .clicked()
                                            {
                                                clicked = Some(h.clone());
                                            }
                                            if ui
                                                .add(
                                                    egui::Button::new(svg_image(
                                                        SvgIcon::Close,
                                                        10.0,
                                                        self.pal.dim,
                                                    ))
                                                    .fill(Color32::TRANSPARENT)
                                                    .frame(false)
                                                    .min_size(egui::vec2(18.0, 18.0)),
                                                )
                                                .on_hover_text("Remove")
                                                .clicked()
                                            {
                                                deleted = Some(h.clone());
                                            }
                                        });
                                    }
                                }); // end history ScrollArea
                        });
                });
            self.ui.hist_rect = Some(area.response.rect);
            if let Some(h) = clicked {
                self.search.query = h;
                self.ui.show_hist = false;
                self.do_search();
            }
            if let Some(h) = deleted {
                self.cfg.history.retain(|x| x != &h);
                let _ = save_cfg(&self.cfg);
            }
            if clear_all {
                self.cfg.history.clear();
                let _ = save_cfg(&self.cfg);
                self.ui.show_hist = false;
            }
        } else {
            self.ui.hist_rect = None;
        }
    }
}
