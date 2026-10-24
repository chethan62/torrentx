//! search drawing methods (split out for readability).

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_pagination(&mut self, ui: &mut egui::Ui, max_p: usize, pg: usize, fs: f32) {
        if max_p <= 1 {
            return;
        }
        egui::Panel::bottom("pages")
            .default_size(34.0)
            .frame(
                egui::Frame::NONE
                    .fill(self.pal.bg)
                    .stroke(Stroke::new(1.0_f32, self.pal.border))
                    .inner_margin(margin(PANEL_MARGIN_X as f32, SP2)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if icon_text_btn(ui, SvgIcon::ChevronLeft, "Prev", self.pal.sub, pg > 0) {
                        self.search.page -= 1;
                        // Page content changed: a listing for the old row has no
                        // reader left, so it is abandoned with the pane.
                        self.close_detail();
                        self.ui.detail_row = None;
                    }
                    ui.add_space(SP3);
                    for p in 0..max_p {
                        let near = p == 0 || p == max_p - 1 || p.abs_diff(pg) <= 2;
                        if !near {
                            if p == 1 || p == max_p - 2 {
                                lbl(ui, "…", self.pal.dim, fs - 1.0);
                            }
                            continue;
                        }
                        let on = p == pg;
                        if ui
                            .add(egui::Button::selectable(
                                on,
                                RichText::new(format!("{}", p + 1))
                                    .font(FontId::proportional(fs - 1.0))
                                    .color(if on { self.pal.accent } else { self.pal.sub }),
                            ))
                            .clicked()
                        {
                            self.search.page = p;
                            self.close_detail();
                            self.ui.detail_row = None;
                        }
                    }
                    ui.add_space(SP3);
                    if icon_text_btn(
                        ui,
                        SvgIcon::ChevronRight,
                        "Next",
                        self.pal.sub,
                        pg + 1 < max_p,
                    ) {
                        self.search.page += 1;
                        self.close_detail();
                        self.ui.detail_row = None;
                    }
                    lbl(
                        ui,
                        &format!("  Page {} of {max_p}", pg + 1),
                        self.pal.dim,
                        fs - 1.0,
                    );
                });
            });
    }
}
