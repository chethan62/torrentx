use eframe::egui::{self, FontId, RichText, Vec2};
use egui_extras::{Column, TableBuilder};

use crate::app::App;
use crate::config::*;
use crate::types::{SortCol, SortDir, TorrentResult};
use crate::utils::{cat_color, fmt_size, health_label, seed_color, time_ago};
use crate::ui::components::act_btn;

pub fn draw(app: &mut App, ui: &mut egui::Ui, page_s: &[TorrentResult]) {
    let mut actions: Vec<(usize, &'static str)> = vec![];
    let pal     = app.pal.clone();
    let s_col   = app.sort_col.clone();
    let s_dir   = app.sort_dir.clone();
    let rh      = app.cfg.row_height;
    let fsz     = app.cfg.font_size;
    let cfg     = app.cfg.clone();
    let sel     = app.selected;
    let det     = app.detail_open;
    let mut new_sort: Option<(SortCol, bool)> = None;

    let hdr_lbl = |l: &str, col: &SortCol| {
        let on  = &s_col == col;
        let arr = if on { if s_dir == SortDir::Desc { "▼" } else { "▲" } } else { "" };
        RichText::new(format!("{l}{arr}")).font(FontId::proportional(fsz))
            .color(if on { pal.accent } else { pal.sub }).strong()
    };

    let mut tb = TableBuilder::new(ui)
        .striped(false)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(COL_NAME).at_least(160.0).clip(true));

    if cfg.col_tracker { tb = tb.column(Column::initial(COL_TRACKER).at_least(55.0)); }
    if cfg.col_size    { tb = tb.column(Column::initial(COL_SIZE).at_least(50.0)); }
    tb = tb.column(Column::initial(COL_SEEDS).at_least(44.0));
    if cfg.col_leech   { tb = tb.column(Column::initial(COL_LEECH).at_least(44.0)); }
    if cfg.col_ratio   { tb = tb.column(Column::initial(COL_RATIO).at_least(44.0)); }
    if cfg.col_health  { tb = tb.column(Column::initial(COL_HEALTH).at_least(50.0)); }
    if cfg.col_date    { tb = tb.column(Column::initial(COL_DATE).at_least(60.0)); }
    tb = tb.column(Column::remainder().at_least(160.0));

    tb.header(30.0, |mut hdr| {
            hdr.col(|ui| {
                if ui.add(egui::Label::new(hdr_lbl("Name", &SortCol::Name)).sense(egui::Sense::click())).clicked() {
                    new_sort = Some((SortCol::Name, s_col == SortCol::Name));
                }
            });
            if cfg.col_tracker { hdr.col(|ui| {
                if ui.add(egui::Label::new(hdr_lbl("Tracker", &SortCol::Tracker)).sense(egui::Sense::click())).clicked() {
                    new_sort = Some((SortCol::Tracker, s_col == SortCol::Tracker));
                }
            }); }
            if cfg.col_size { hdr.col(|ui| {
                if ui.add(egui::Label::new(hdr_lbl("Size", &SortCol::Size)).sense(egui::Sense::click())).clicked() {
                    new_sort = Some((SortCol::Size, s_col == SortCol::Size));
                }
            }); }
            hdr.col(|ui| {
                if ui.add(egui::Label::new(hdr_lbl("Seeds", &SortCol::Seeds)).sense(egui::Sense::click())).clicked() {
                    new_sort = Some((SortCol::Seeds, s_col == SortCol::Seeds));
                }
            });
            if cfg.col_leech { hdr.col(|ui| {
                if ui.add(egui::Label::new(hdr_lbl("Leech", &SortCol::Leech)).sense(egui::Sense::click())).clicked() {
                    new_sort = Some((SortCol::Leech, s_col == SortCol::Leech));
                }
            }); }
            if cfg.col_ratio { hdr.col(|ui| {
                ui.label(RichText::new("Ratio").font(FontId::proportional(fsz)).color(pal.sub).strong());
            }); }
            if cfg.col_health { hdr.col(|ui| {
                ui.label(RichText::new("Health").font(FontId::proportional(fsz)).color(pal.sub).strong());
            }); }
            if cfg.col_date { hdr.col(|ui| {
                if ui.add(egui::Label::new(hdr_lbl("Date", &SortCol::Date)).sense(egui::Sense::click())).clicked() {
                    new_sort = Some((SortCol::Date, s_col == SortCol::Date));
                }
            }); }
            hdr.col(|ui| {
                ui.label(RichText::new("Actions").font(FontId::proportional(fsz)).color(pal.sub).strong());
            });
        })
        .body(|mut body| {
            for (i, r) in page_s.iter().enumerate() {
                let is_sel = sel == Some(i);
                let is_hov = app.hovered == Some(i);
                let seed   = r.seeders.unwrap_or(0);
                let leech  = r.peers.unwrap_or(0);
                let bg = if is_sel       { pal.row_sel }
                         else if is_hov  { pal.row_hov }
                         else if i % 2 == 0 { pal.row_odd }
                         else             { pal.row_even };

                body.row(rh, |mut row| {
                    // Name
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        let resp = ui.add(egui::Label::new(
                            RichText::new(&r.title).font(FontId::proportional(fsz))
                                .color(if is_sel { pal.accent } else { pal.text })
                        ).truncate(true).sense(egui::Sense::click()));
                        if resp.clicked() { actions.push((i, "select")); }
                        if resp.hovered() {
                            app.hovered = Some(i);
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if rh >= 40.0 {
                            let cat = r.category_desc.as_deref().unwrap_or("Other");
                            ui.add(egui::Label::new(RichText::new(cat)
                                .font(FontId::proportional(fsz - 2.5))
                                .color(cat_color(cat))).truncate(true));
                        }
                    });

                    // Tracker
                    if cfg.col_tracker { row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        ui.add(egui::Label::new(RichText::new(r.tracker.as_deref().unwrap_or("—"))
                            .font(FontId::proportional(fsz - 1.0)).color(pal.sub)).truncate(true));
                    }); }

                    // Size
                    if cfg.col_size { row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        ui.label(RichText::new(r.size.map(fmt_size).unwrap_or_else(|| "—".into()))
                            .font(FontId::proportional(fsz)).color(pal.sub));
                    }); }

                    // Seeds
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        ui.label(RichText::new(seed.to_string())
                            .font(FontId::proportional(fsz)).color(seed_color(seed)).strong());
                    });

                    // Leechers
                    if cfg.col_leech { row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        ui.label(RichText::new(leech.to_string())
                            .font(FontId::proportional(fsz)).color(pal.red));
                    }); }

                    // Ratio bar
                    if cfg.col_ratio { row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        let tot = (seed + leech) as f32;
                        if tot > 0.0 {
                            let pct  = (seed as f32 / tot).clamp(0.0, 1.0);
                            let rect = ui.available_rect_before_wrap();
                            let bar  = egui::Rect::from_min_size(
                                rect.min + Vec2::new(2.0, (rect.height() - 7.0) / 2.0),
                                Vec2::new((rect.width() - 4.0).max(8.0), 7.0));
                            ui.painter().rect_filled(bar, 3.0, pal.border);
                            let mut filled = bar;
                            filled.max.x = bar.min.x + bar.width() * pct;
                            ui.painter().rect_filled(filled, 3.0, seed_color(seed));
                            ui.allocate_rect(bar, egui::Sense::hover())
                                .on_hover_text(format!("{:.0}% seeded", pct * 100.0));
                        } else {
                            ui.label(RichText::new("—").font(FontId::proportional(fsz - 1.0)).color(pal.dim));
                        }
                    }); }

                    // Health
                    if cfg.col_health { row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        let dot = if seed > 10 { "●" } else { "○" };
                        ui.label(RichText::new(format!("{dot} {}", health_label(seed)))
                            .font(FontId::proportional(fsz - 1.0)).strong().color(seed_color(seed)));
                    }); }

                    // Date
                    if cfg.col_date { row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        let d = r.publish_date.as_deref().map(time_ago).unwrap_or_else(|| "—".into());
                        ui.label(RichText::new(d).font(FontId::proportional(fsz)).color(pal.dim));
                    }); }

                    // Actions
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        ui.horizontal(|ui| {
                            ui.add_space(2.0);
                            if r.magnet_uri.is_some() {
                                if act_btn(ui, "Mag",  "Open in torrent client", pal.accent) { actions.push((i, "mag")); }
                                if act_btn(ui, "Copy", "Copy magnet link",       pal.sub)    { actions.push((i, "copy")); }
                            }
                            if r.link.is_some() {
                                if act_btn(ui, "DL",  "Download .torrent",      pal.green)  { actions.push((i, "dl")); }
                            }
                            if act_btn(ui, "Fav",  "Add to Favorites (F)",  pal.yellow) { actions.push((i, "fav")); }
                            if act_btn(ui, "Info", "Detail panel (D)",
                                if is_sel && det { pal.accent } else { pal.dim }) { actions.push((i, "info")); }
                            if r.details.is_some() {
                                if act_btn(ui, "Web", "Open in browser", pal.dim) { actions.push((i, "web")); }
                            }
                        });
                    });
                });
            }
        });

    // Apply sort change
    if let Some((col, same)) = new_sort {
        if same {
            app.sort_dir = if app.sort_dir == SortDir::Desc { SortDir::Asc } else { SortDir::Desc };
        } else {
            app.sort_col = col;
            app.sort_dir = SortDir::Desc;
        }
        app.page = 0;
    }

    // Process row actions
    for (i, action) in actions {
        if let Some(r) = page_s.get(i).cloned() {
            match action {
                "select" => {
                    if app.selected == Some(i) && app.detail_open {
                        app.selected = None; app.detail_open = false;
                    } else {
                        app.selected = Some(i); app.detail_open = true;
                    }
                }
                "mag"  => { if let Some(m) = &r.magnet_uri { let _ = open::that(m); let ac = app.pal.accent; app.toast("Opening magnet…", ac); } }
                "copy" => { if let Some(m) = &r.magnet_uri { ui.output_mut(|o| o.copied_text = m.clone()); let g = app.pal.green; app.toast("Magnet copied ✓", g); } }
                "dl"   => { if let Some(l) = &r.link        { let _ = open::that(l); let g = app.pal.green; app.toast("Downloading…", g); } }
                "fav"  => { app.add_fav(&r); }
                "info" => {
                    if app.selected == Some(i) && app.detail_open {
                        app.detail_open = false; app.selected = None;
                    } else {
                        app.selected = Some(i); app.detail_open = true;
                    }
                }
                "web"  => { if let Some(d) = &r.details { let _ = open::that(d); } }
                _ => {}
            }
        }
    }

    // Clear hover when pointer leaves
    if let Some(pos) = ui.ctx().pointer_hover_pos() {
        if !ui.min_rect().contains(pos) { app.hovered = None; }
    } else {
        app.hovered = None;
    }
}
