use eframe::egui::{self, FontId, RichText, Stroke};
use egui_extras::{Column, TableBuilder};
use crate::app::App;
use crate::config::save_cfg;
use crate::ui::components::{act_btn, lbl, outline_btn};
use crate::utils::fmt_size;

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal.clone();
    let fs  = app.cfg.font_size;
    let rh  = app.cfg.row_height;

    if app.cfg.favorites.is_empty() {
        ui.add_space(80.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("★").size(48.0).color(app.pal.yellow));
            ui.add_space(12.0);
            lbl(ui, "No favorites yet", pal.sub, 18.0);
            ui.add_space(6.0);
            lbl(ui, "Hit the  ★ Fav  button on any search result to save it here", pal.dim, fs);
        });
        return;
    }

    // ── Header ────────────────────────────────────────────────────────────
    egui::Frame::none()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.border))
        .inner_margin(egui::Margin::symmetric(14.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                lbl(ui, &format!("Favorites  ({})", app.cfg.favorites.len()), pal.accent, fs + 1.0);
                ui.add_space(14.0);
                ui.add(egui::TextEdit::singleline(&mut app.fav_search)
                    .desired_width(200.0)
                    .hint_text("Filter…")
                    .font(FontId::proportional(fs)));
                if !app.fav_search.is_empty() {
                    let red = pal.red;
                    if outline_btn(ui, "✕", red) { app.fav_search.clear(); }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let red = pal.red;
                    if outline_btn(ui, "Clear All", red) {
                        app.cfg.favorites.clear();
                        save_cfg(&app.cfg);
                    }
                });
            });
        });

    // ── Table ─────────────────────────────────────────────────────────────
    let filter = app.fav_search.to_lowercase();
    let favs   = app.cfg.favorites.clone();
    let shown: Vec<(usize, &crate::config::Favorite)> = favs.iter().enumerate()
        .filter(|(_, f)| {
            filter.is_empty()
                || f.title.to_lowercase().contains(&filter)
                || f.tracker.as_deref().unwrap_or("").to_lowercase().contains(&filter)
        })
        .collect();

    if shown.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            lbl(ui, "No favorites match that filter", pal.dim, fs + 2.0);
        });
        return;
    }

    let mut delete_idx: Option<usize> = None;
    let mut mag_action: Option<usize> = None;
    let mut cpy_action: Option<usize> = None;
    let mut dl_action:  Option<usize> = None;

    TableBuilder::new(ui)
        .striped(false)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::remainder().at_least(200.0).clip(true))
        .column(Column::initial(90.0).at_least(60.0))
        .column(Column::initial(80.0).at_least(60.0))
        .column(Column::initial(60.0).at_least(44.0))
        .column(Column::initial(100.0).at_least(70.0))
        .column(Column::initial(200.0).at_least(140.0))
        .header(28.0, |mut hdr| {
            for lbl_str in ["Title", "Tracker", "Size", "Seeds", "Saved", "Actions"] {
                hdr.col(|ui| {
                    ui.label(RichText::new(lbl_str).font(FontId::proportional(fs))
                        .color(pal.sub).strong());
                });
            }
        })
        .body(|mut body| {
            for (orig_i, fav) in &shown {
                let i = *orig_i;
                let row_bg = if i % 2 == 0 { pal.row_odd } else { pal.row_even };
                body.row(rh, |mut row| {
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_bg);
                        ui.add(egui::Label::new(RichText::new(&fav.title)
                            .font(FontId::proportional(fs)).color(pal.text)).truncate(true));
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_bg);
                        ui.add(egui::Label::new(RichText::new(fav.tracker.as_deref().unwrap_or("—"))
                            .font(FontId::proportional(fs - 1.0)).color(pal.sub)).truncate(true));
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_bg);
                        ui.label(RichText::new(fav.size.map(fmt_size).unwrap_or_else(|| "—".into()))
                            .font(FontId::proportional(fs)).color(pal.sub));
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_bg);
                        let s = fav.seeders.unwrap_or(0);
                        ui.label(RichText::new(s.to_string())
                            .font(FontId::proportional(fs))
                            .color(crate::utils::seed_color(s)).strong());
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_bg);
                        ui.label(RichText::new(fav.saved_at.get(..10).unwrap_or(&fav.saved_at))
                            .font(FontId::proportional(fs)).color(pal.dim));
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_bg);
                        ui.horizontal(|ui| {
                            if fav.magnet.is_some() {
                                if act_btn(ui, "Mag",  "Open in torrent client", pal.accent)  { mag_action = Some(i); }
                                if act_btn(ui, "Copy", "Copy magnet link",        pal.sub)     { cpy_action = Some(i); }
                            }
                            if fav.link.is_some() {
                                if act_btn(ui, "DL",  "Download .torrent",       pal.green)   { dl_action  = Some(i); }
                            }
                            if act_btn(ui, "✕", "Remove from favorites",        pal.red)     { delete_idx = Some(i); }
                        });
                    });
                });
            }
        });

    // Apply actions
    if let Some(i) = mag_action {
        if let Some(f) = app.cfg.favorites.get(i) {
            if let Some(m) = &f.magnet { let _ = open::that(m); let a = app.pal.accent; app.toast("Opening magnet…", a); }
        }
    }
    if let Some(i) = cpy_action {
        if let Some(f) = app.cfg.favorites.get(i) {
            if let Some(m) = &f.magnet { ui.output_mut(|o| o.copied_text = m.clone()); let g = app.pal.green; app.toast("Magnet copied ✓", g); }
        }
    }
    if let Some(i) = dl_action {
        if let Some(f) = app.cfg.favorites.get(i) {
            if let Some(l) = &f.link { let _ = open::that(l); }
        }
    }
    if let Some(i) = delete_idx {
        if i < app.cfg.favorites.len() {
            app.cfg.favorites.remove(i);
            save_cfg(&app.cfg);
        }
    }
}
