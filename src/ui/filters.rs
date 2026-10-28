//! The filter bar: five label+field cells, the health chips and the batch
//! selection group, laid out for a wide window and for a phone.

use super::*;

use crate::app::App;

/// One filter slot. Named so the wide flow and the narrow rows can draw the
/// same widgets without copying them.
#[derive(Clone, Copy, PartialEq)]
enum Slot {
    Text,
    Seeds,
    Size,
    Year,
    Tracker,
}

const ALL_SLOTS: [Slot; 5] = [
    Slot::Text,
    Slot::Seeds,
    Slot::Size,
    Slot::Year,
    Slot::Tracker,
];

impl App {
    /// A label and its field as ONE atom. As separate items in a wrapping row a
    /// narrow window put the caption on the line below its own box ("Max" over
    /// a field labelled "GB"), so which input belonged to which caption stopped
    /// being readable.
    fn filter_cell(&mut self, ui: &mut egui::Ui, slot: Slot, fs: f32, fill: bool) -> bool {
        let dim = self.pal.dim;
        let (label, field) = match slot {
            Slot::Text => (
                "Filter",
                egui::TextEdit::singleline(&mut self.search.f_text)
                    .desired_width(FILTER_TEXT_W)
                    .hint_text("within results"),
            ),
            Slot::Seeds => (
                "Seeds ≥",
                egui::TextEdit::singleline(&mut self.search.f_seed)
                    .desired_width(FILTER_NUM_W)
                    .hint_text("0"),
            ),
            Slot::Size => (
                "Max GB",
                egui::TextEdit::singleline(&mut self.search.f_size)
                    .desired_width(FILTER_NUM_W)
                    .hint_text("∞"),
            ),
            Slot::Year => (
                "Year ≥",
                egui::TextEdit::singleline(&mut self.search.f_year)
                    .desired_width(FILTER_YEAR_W)
                    .hint_text("any"),
            ),
            Slot::Tracker => (
                "Tracker",
                egui::TextEdit::singleline(&mut self.search.f_trk)
                    .desired_width(FILTER_TRK_W)
                    .hint_text("any"),
            ),
        };
        let mut changed = false;
        ui.horizontal(|ui| {
            lbl(ui, label, dim, fs);
            ui.add_space(SP1);
            let mut field = field.font(FontId::proportional(fs));
            // Phone: the field takes whatever the label left in its column, so
            // no cell is narrower than its own placeholder ("within results"
            // rendered as "wi…" at a fixed 115 px inside a two-column grid).
            if fill {
                field = field.desired_width(ui.available_width().max(40.0));
            }
            changed = ui.add(field).changed();
        });
        changed
    }

    fn filters_dirty(&self) -> bool {
        !self.search.f_text.is_empty()
            || !self.search.f_seed.is_empty()
            || !self.search.f_size.is_empty()
            || !self.search.f_year.is_empty()
            || !self.search.f_trk.is_empty()
            || self.search.f_hlth != Hlth::All
    }

    fn reset_filters(&mut self, ui: &mut egui::Ui) {
        if outline_icon_btn(ui, SvgIcon::Close, "Reset", self.pal.red) {
            self.search.f_text.clear();
            self.search.f_seed.clear();
            self.search.f_size.clear();
            self.search.f_year.clear();
            self.search.f_trk.clear();
            self.search.f_hlth = Hlth::All;
            self.search.page = 0;
            self.ui.sel_set.clear();
            self.ui.sel_mode = false;
        }
    }

    /// The five cells. Wide: one wrapping row. Narrow: three explicit rows.
    ///
    /// A nested `ui.horizontal` inside a wrapping row announces no width until
    /// its children have run, so egui cannot wrap before it — at 412 px the row
    /// ran off the right edge instead of flowing, taking `Year ≥`, `Tracker` and
    /// the `DEAD` chip with it. Packing the rows by hand removes the guess.
    fn filter_cells(&mut self, ui: &mut egui::Ui, fs: f32, tight: bool) -> bool {
        let mut changed = false;
        if tight {
            // Two equal columns (`columns`, not a Grid): every field then
            // takes the width its label left behind, so both columns end at
            // the same x and nothing truncates. A Grid sizes to content, and
            // the tracker cell (136 px) plus the filter cell (150 px) came to
            // more than the 312 px the card has at 360 px wide.
            changed |= self.filter_cell(ui, Slot::Text, fs, true);
            ui.columns(2, |c| {
                changed |= self.filter_cell(&mut c[0], Slot::Seeds, fs, true);
                changed |= self.filter_cell(&mut c[1], Slot::Size, fs, true);
            });
            ui.columns(2, |c| {
                changed |= self.filter_cell(&mut c[0], Slot::Year, fs, true);
                changed |= self.filter_cell(&mut c[1], Slot::Tracker, fs, true);
            });
            if self.filters_dirty() {
                ui.horizontal(|ui| self.reset_filters(ui));
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                for slot in ALL_SLOTS {
                    changed |= self.filter_cell(ui, slot, fs, false);
                    ui.add_space(SP3);
                }
                if self.filters_dirty() {
                    self.reset_filters(ui);
                }
            });
        }
        changed
    }

    /// Batch selection: an action group, not a filter.
    fn select_group(&mut self, ui: &mut egui::Ui, fs: f32, view: Option<&crate::app::ResultsView>) {
        if v_checkbox(ui, self.ui.sel_mode, "Select", self.pal.accent).clicked() {
            self.ui.sel_mode = !self.ui.sel_mode;
            self.ui.sel_set.clear();
            self.close_detail();
        }
        if self.ui.sel_mode {
            let n = self.ui.sel_set.len();
            let copy_label = format!("Copy {n} magnet{}", if n == 1 { "" } else { "s" });
            if n > 0
                && icon_text_btn(ui, SvgIcon::Copy, &copy_label, self.pal.green, true)
                && let Some(v) = view
            {
                self.copy_selected_magnets(ui, &v.sorted);
            }
            ui.add_space(SP2);
            let all_checked = !self.ui.sel_set.is_empty();
            if v_checkbox(ui, all_checked, "All", self.pal.accent)
                .on_hover_text("Select all results on this page")
                .clicked()
                && let Some(v) = view
            {
                if self.cfg.page_size == 0 {
                    self.ui.sel_set = (0..v.sorted.len()).collect();
                } else {
                    let base = self.search.page * self.cfg.page_size;
                    let end = (base + self.cfg.page_size).min(v.sorted.len());
                    self.ui.sel_set = (base..end).collect();
                }
            }
            if !self.ui.sel_set.is_empty()
                && ui
                    .add(
                        egui::Button::new(
                            RichText::new("  Clear")
                                .font(FontId::proportional(fs - 1.0))
                                .color(self.pal.sub),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0_f32, self.pal.border))
                        .corner_radius(R_SM),
                    )
                    .clicked()
            {
                self.ui.sel_set.clear();
            }
            ui.add_space(SP2);
        }
    }

    pub(crate) fn draw_filter_bar(
        &mut self,
        ui: &mut egui::Ui,
        fs: f32,
        view: Option<&crate::app::ResultsView>,
    ) {
        egui::Frame::NONE
            .fill(self.pal.surface)
            .corner_radius(PANEL_RADIUS)
            .stroke(Stroke::new(1.0_f32, self.pal.border))
            .inner_margin(margin(PANEL_MARGIN_X as f32, SP3))
            .outer_margin(margin(PANEL_MARGIN_X as f32, 0.0))
            .show(ui, |ui| {
                let tight = narrow(ui.available_width());
                // Track whether any filter input changed this frame so we can
                // drop batch selections whose indices went stale. TextEdit
                // responses report .changed() — no per-frame string clones.
                let filter_changed = self.filter_cells(ui, fs, tight);
                if filter_changed && self.ui.sel_mode {
                    self.ui.sel_set.clear();
                    self.ui.sel_mode = false;
                    self.toast("Selection cleared (filters changed)", self.pal.yellow);
                }

                ui.add_space(SP2);

                // Health chips wrap on their own; the batch group is parked at
                // the right edge on a wide window and given its own row when
                // narrow, where a right-to-left child would push the chips off.
                // Health chips: on a phone the caption gets its own line, so
                // every chip line starts at the card's left edge instead of
                // the wrapped second line starting under the caption's x.
                if tight {
                    lbl(ui, "Health", self.pal.dim, fs);
                    ui.add_space(SP1);
                }
                ui.horizontal_wrapped(|ui| {
                    if !tight {
                        lbl(ui, "Health", self.pal.dim, fs);
                        ui.add_space(SP2);
                    }
                    for hf in [
                        Hlth::All,
                        Hlth::Hot,
                        Hlth::Good,
                        Hlth::Slow,
                        Hlth::Dying,
                        Hlth::Dead,
                    ] {
                        let on = self.search.f_hlth == hf;
                        let chip_col = match hf {
                            Hlth::All => self.pal.sub,
                            Hlth::Hot | Hlth::Good => self.pal.green,
                            Hlth::Slow | Hlth::Dying => self.pal.yellow,
                            Hlth::Dead => self.pal.red,
                        };
                        if ui
                            .add(egui::Button::selectable(
                                on,
                                RichText::new(hf.label())
                                    .font(FontId::proportional(fs - 1.0))
                                    .color(if on { self.pal.accent } else { chip_col }),
                            ))
                            .clicked()
                        {
                            if self.search.f_hlth != hf {
                                self.search.f_hlth = hf;
                                self.ui.sel_set.clear();
                                self.ui.sel_mode = false; // filter changed
                            }
                            self.search.page = 0;
                        }
                        ui.add_space(SP1);
                    }
                    if !tight {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.horizontal(|ui| self.select_group(ui, fs, view));
                        });
                    }
                });
                if tight {
                    ui.add_space(SP1);
                    ui.horizontal(|ui| self.select_group(ui, fs, view));
                }
            });
    }
}
