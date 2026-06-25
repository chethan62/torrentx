use eframe::egui::{self, Color32, FontId, RichText, Stroke};
use crate::app::App;
use crate::rss::{FeedStatus, RssFeedConfig};
use crate::theme::tint;
use crate::ui::components::{lbl, outline_btn, wide_btn, act_btn, status_pill};
use crate::utils::{fmt_size, seed_color, time_ago};

pub fn draw(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    let pal = app.pal.clone();
    let fs  = app.cfg.font_size;

    if app.rss_feeds.is_empty() && !app.rss_add_mode {
        draw_empty(app, ui);
        return;
    }

    ui.horizontal_top(|ui| {
        // ── Left sidebar ──────────────────────────────────────────────────
        egui::SidePanel::left("rss_sidebar")
            .resizable(true)
            .default_width(220.0)
            .min_width(160.0)
            .frame(egui::Frame::none()
                .fill(pal.surface)
                .stroke(Stroke::new(1.0, pal.border)))
            .show_inside(ui, |ui| {
                draw_sidebar(app, ui, ctx);
            });

        // ── Right pane ────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.bg))
            .show_inside(ui, |ui| {
                if app.rss_add_mode {
                    draw_add_form(app, ui);
                } else if let Some(edit_idx) = app.rss_edit_idx {
                    draw_edit_form(app, ui, edit_idx);
                } else {
                    draw_feed_items(app, ui);
                }
            });
    });
}

// ─── Empty state ──────────────────────────────────────────────────────────

fn draw_empty(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal.clone();
    let fs  = app.cfg.font_size;

    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("📡").size(42.0));
        ui.add_space(12.0);
        lbl(ui, "No RSS Feeds yet", pal.sub, 18.0);
        ui.add_space(6.0);
        lbl(ui, "Add Jackett Torznab feeds to auto-refresh torrents", pal.dim, fs);
        ui.add_space(20.0);

        egui::Frame::none()
            .fill(tint(pal.accent, 12)).rounding(10.0)
            .stroke(Stroke::new(1.0, tint(pal.accent, 50)))
            .inner_margin(egui::Margin::symmetric(24.0, 16.0))
            .show(ui, |ui| {
                ui.set_max_width(420.0);
                lbl(ui, "How Torznab RSS works", pal.accent, fs);
                ui.add_space(6.0);
                lbl(ui, "Each indexer in Jackett exposes a Torznab API.", pal.sub, fs - 1.0);
                lbl(ui, "You can search any indexer and get live results", pal.sub, fs - 1.0);
                lbl(ui, "as an auto-refreshed feed — no browser needed.", pal.sub, fs - 1.0);
                ui.add_space(12.0);
                let accent = pal.accent;
                if outline_btn(ui, "+ Add Feed", accent) { app.rss_add_mode = true; }
            });
    });
}

// ─── Sidebar ──────────────────────────────────────────────────────────────

fn draw_sidebar(app: &mut App, ui: &mut egui::Ui, _ctx: &egui::Context) {
    let pal = app.pal.clone();
    let fs  = app.cfg.font_size;

    // Header
    egui::Frame::none()
        .fill(pal.hdr)
        .stroke(Stroke::new(1.0, pal.border))
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                lbl(ui, "RSS Feeds", pal.accent, fs);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(
                        RichText::new("⟳ All").font(FontId::proportional(fs - 1.5)).color(pal.sub))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0, pal.border)).rounding(4.0)
                    ).on_hover_text("Refresh all feeds").clicked() {
                        app.refresh_all_feeds();
                    }
                });
            });
            ui.add_space(4.0);
            ui.add(egui::TextEdit::singleline(&mut app.rss_filter)
                .desired_width(ui.available_width())
                .hint_text("Filter feeds…")
                .font(FontId::proportional(fs)));
        });

    // Feed list
    egui::ScrollArea::vertical().id_source("rss_feed_list").show(ui, |ui| {
        let filter = app.rss_filter.to_lowercase();
        let len    = app.rss_feeds.len();
        let mut refresh_idx: Option<usize> = None;
        let mut delete_idx:  Option<usize> = None;
        let mut edit_idx:    Option<usize> = None;
        let mut select_idx:  Option<usize> = None;

        for i in 0..len {
            let name  = app.rss_feeds[i].config.name.clone();
            let items = app.rss_feeds[i].items.len();
            let status = app.rss_feeds[i].status.clone();
            let enabled = app.rss_feeds[i].config.enabled;

            if !filter.is_empty() && !name.to_lowercase().contains(&filter) { continue; }

            let is_sel = app.rss_selected == i && !app.rss_add_mode && app.rss_edit_idx.is_none();
            let bg = if is_sel { tint(pal.accent, 22) } else { Color32::TRANSPARENT };

            egui::Frame::none()
                .fill(bg).rounding(6.0)
                .inner_margin(egui::Margin::symmetric(10.0, 7.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Status dot
                        let (dot_col, dot) = match status {
                            FeedStatus::Ok      => (pal.green, "●"),
                            FeedStatus::Loading => (pal.accent, "⟳"),
                            FeedStatus::Error   => (pal.red,   "✕"),
                            FeedStatus::Idle    => (pal.dim,   "○"),
                        };
                        lbl(ui, dot, dot_col, fs - 2.0);
                        ui.add_space(4.0);

                        // Name + count
                        let name_col = if enabled { pal.text } else { pal.dim };
                        if ui.add(egui::Label::new(
                            RichText::new(&name).font(FontId::proportional(fs - 0.5)).color(name_col)
                        ).truncate(true).sense(egui::Sense::click())).clicked() {
                            select_idx = Some(i);
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if items > 0 {
                                egui::Frame::none()
                                    .fill(tint(pal.accent, 25)).rounding(8.0)
                                    .inner_margin(egui::Margin::symmetric(5.0, 1.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(items.to_string())
                                            .font(FontId::proportional(fs - 3.0)).color(pal.accent));
                                    });
                            }
                        });
                    });

                    // Action row (visible on hover or select)
                    if is_sel {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if act_btn(ui, "⟳",    "Refresh",     pal.accent) { refresh_idx = Some(i); }
                            if act_btn(ui, "Edit",  "Edit feed",   pal.sub)    { edit_idx    = Some(i); }
                            if act_btn(ui, "✕",    "Delete feed", pal.red)    { delete_idx  = Some(i); }
                            // Toggle enabled
                            let en_col = if enabled { pal.green } else { pal.dim };
                            let en_lbl = if enabled { "On" } else { "Off" };
                            if act_btn(ui, en_lbl, "Toggle enabled", en_col) {
                                app.rss_feeds[i].config.enabled = !enabled;
                                app.sync_rss_configs();
                            }
                        });
                    }
                });
        }

        // Apply
        if let Some(i) = select_idx  { app.rss_selected = i; app.rss_add_mode = false; app.rss_edit_idx = None; app.rss_detail = None; }
        if let Some(i) = refresh_idx { app.refresh_feed(i); }
        if let Some(i) = delete_idx  {
            app.rss_feeds.remove(i);
            if app.rss_selected >= app.rss_feeds.len() && !app.rss_feeds.is_empty() {
                app.rss_selected = app.rss_feeds.len() - 1;
            }
            app.sync_rss_configs();
        }
        if let Some(i) = edit_idx {
            app.rss_edit_idx = Some(i);
            app.rss_add_mode = false;
        }
    });

    // Add button
    ui.add_space(8.0);
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
            let accent = pal.accent;
            if wide_btn(ui, "+ Add Feed", accent) {
                app.rss_add_mode  = true;
                app.rss_edit_idx  = None;
                app.rss_new_cfg   = RssFeedConfig::new_default();
            }
        });
}

// ─── Feed items table ─────────────────────────────────────────────────────

fn draw_feed_items(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal.clone();
    let fs  = app.cfg.font_size;
    let rh  = app.cfg.row_height;
    let sel = app.rss_selected;

    if app.rss_feeds.is_empty() { return; }
    if sel >= app.rss_feeds.len() { return; }

    let name   = app.rss_feeds[sel].config.name.clone();
    let status = app.rss_feeds[sel].status.clone();
    let items  = app.rss_feeds[sel].items.clone();
    let err    = app.rss_feeds[sel].error.clone();

    // Header bar
    egui::Frame::none()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.border))
        .inner_margin(egui::Margin::symmetric(14.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                lbl(ui, &name, pal.accent, fs + 1.0);
                ui.add_space(8.0);
                let (dot_col, dot_lbl) = match status {
                    FeedStatus::Ok      => (pal.green,  "● OK"),
                    FeedStatus::Loading => (pal.accent, "⟳ Loading"),
                    FeedStatus::Error   => (pal.red,    "✕ Error"),
                    FeedStatus::Idle    => (pal.dim,    "○ Idle"),
                };
                status_pill(ui, dot_lbl, dot_col);
                lbl(ui, &format!("  {} items", items.len()), pal.dim, fs - 1.0);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let accent = pal.accent;
                    if outline_btn(ui, "⟳ Refresh", accent) { app.refresh_feed(sel); }
                    ui.add_space(6.0);
                    let sub = pal.sub;
                    if outline_btn(ui, "Edit Feed", sub) { app.rss_edit_idx = Some(sel); }
                });
            });
            if let Some(e) = &err {
                ui.add_space(4.0);
                lbl(ui, &format!("Error: {e}"), pal.red, fs - 1.0);
            }
        });

    if status == FeedStatus::Loading && items.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.spinner();
            ui.add_space(10.0);
            lbl(ui, "Fetching Torznab feed…", pal.sub, fs);
        });
        return;
    }

    if items.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            lbl(ui, "No items yet", pal.dim, fs + 2.0);
            ui.add_space(8.0);
            lbl(ui, "Click Refresh to fetch the latest torrents", pal.sub, fs);
        });
        return;
    }

    // Detail panel
    if let Some(detail_i) = app.rss_detail {
        if let Some(item) = items.get(detail_i).cloned() {
            egui::SidePanel::right("rss_detail_panel")
                .resizable(true).default_width(280.0).min_width(220.0)
                .frame(egui::Frame::none()
                    .fill(pal.surface)
                    .stroke(Stroke::new(1.0, pal.border)))
                .show_inside(ui, |ui| {
                    draw_item_detail(app, ui, &item);
                });
        }
    }

    // Items table
    ui.add_space(2.0);
    let mut actions: Vec<(usize, &'static str)> = vec![];

    use egui_extras::{Column, TableBuilder};
    TableBuilder::new(ui)
        .striped(false)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::remainder().at_least(180.0).clip(true))
        .column(Column::initial(80.0).at_least(50.0))
        .column(Column::initial(60.0).at_least(44.0))
        .column(Column::initial(60.0).at_least(44.0))
        .column(Column::initial(80.0).at_least(60.0))
        .column(Column::initial(180.0).at_least(120.0))
        .header(28.0, |mut hdr| {
            for label in ["Title", "Tracker", "Size", "Seeds", "Date", "Actions"] {
                hdr.col(|ui| {
                    ui.label(RichText::new(label).font(FontId::proportional(fs - 1.0))
                        .color(pal.sub).strong());
                });
            }
        })
        .body(|mut body| {
            for (i, item) in items.iter().enumerate() {
                let is_sel = app.rss_detail == Some(i);
                let bg = if is_sel { tint(pal.accent, 20) }
                         else if i % 2 == 0 { pal.row_odd }
                         else { pal.row_even };

                body.row(rh, |mut row| {
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        let resp = ui.add(egui::Label::new(
                            RichText::new(&item.title)
                                .font(FontId::proportional(fs))
                                .color(if is_sel { pal.accent } else { pal.text })
                        ).truncate(true).sense(egui::Sense::click()));
                        if resp.clicked() { actions.push((i, "detail")); }
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        ui.add(egui::Label::new(
                            RichText::new(item.tracker.as_deref().unwrap_or("—"))
                                .font(FontId::proportional(fs - 1.0)).color(pal.sub)
                        ).truncate(true));
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        ui.label(RichText::new(item.size.map(fmt_size).unwrap_or_else(|| "—".into()))
                            .font(FontId::proportional(fs)).color(pal.sub));
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        let s = item.seeders.unwrap_or(0);
                        ui.label(RichText::new(s.to_string())
                            .font(FontId::proportional(fs)).color(seed_color(s)).strong());
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        let d = item.pub_date.as_deref().map(time_ago).unwrap_or_else(|| "—".into());
                        ui.label(RichText::new(d).font(FontId::proportional(fs)).color(pal.dim));
                    });
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        ui.horizontal(|ui| {
                            if item.magnet.is_some() {
                                if act_btn(ui, "Mag",  "Open magnet",     pal.accent) { actions.push((i, "mag")); }
                                if act_btn(ui, "Copy", "Copy magnet link", pal.sub)   { actions.push((i, "copy")); }
                            }
                            if item.link.is_some() {
                                if act_btn(ui, "DL",  "Download .torrent", pal.green) { actions.push((i, "dl")); }
                            }
                            if act_btn(ui, "★", "Add to Favorites", pal.yellow) { actions.push((i, "fav")); }
                        });
                    });
                });
            }
        });

    // Apply actions
    for (i, action) in actions {
        if let Some(item) = items.get(i).cloned() {
            match action {
                "detail" => {
                    app.rss_detail = if app.rss_detail == Some(i) { None } else { Some(i) };
                }
                "mag"  => { if let Some(m) = &item.magnet { let _ = open::that(m); let a = app.pal.accent; app.toast("Opening magnet…", a); } }
                "copy" => { if let Some(m) = &item.magnet { ui.output_mut(|o| o.copied_text = m.clone()); let g = app.pal.green; app.toast("Magnet copied ✓", g); } }
                "dl"   => { if let Some(l) = &item.link   { let _ = open::that(l); } }
                "fav"  => { let it = item.clone(); app.add_fav_from_rss(&it); }
                _ => {}
            }
        }
    }
}

// ─── Item detail panel ────────────────────────────────────────────────────

fn draw_item_detail(app: &mut App, ui: &mut egui::Ui, item: &crate::rss::RssItem) {
    let pal = app.pal.clone();
    let fs  = app.cfg.font_size;

    ui.add_space(10.0);
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(12.0, 0.0))
        .show(ui, |ui| {
            ui.add(egui::Label::new(
                RichText::new(&item.title).font(FontId::proportional(fs)).color(pal.text).strong()
            ).wrap(true));
            ui.add_space(12.0);

            egui::Grid::new("rss_item_grid")
                .num_columns(2).spacing([8.0, 5.0])
                .show(ui, |ui| {
                    if let Some(t) = &item.tracker {
                        crate::ui::components::grid_row(ui, "Tracker", t, pal.accent, pal.dim, fs);
                    }
                    if let Some(s) = item.size {
                        crate::ui::components::grid_row(ui, "Size", &fmt_size(s), pal.text, pal.dim, fs);
                    }
                    if let Some(s) = item.seeders {
                        crate::ui::components::grid_row(ui, "Seeders", &s.to_string(), seed_color(s), pal.dim, fs);
                    }
                    if let Some(l) = item.leechers {
                        crate::ui::components::grid_row(ui, "Leechers", &l.to_string(), pal.red, pal.dim, fs);
                    }
                    if let Some(d) = &item.pub_date {
                        crate::ui::components::grid_row(ui, "Published", &time_ago(d), pal.text, pal.dim, fs);
                    }
                    if let Some(c) = &item.category {
                        crate::ui::components::grid_row(ui, "Category", c, pal.text, pal.dim, fs);
                    }
                });

            ui.add_space(12.0);
            if item.magnet.is_some() {
                let accent = pal.accent;
                if wide_btn(ui, "⚡  Open Magnet", accent) {
                    if let Some(m) = &item.magnet { let _ = open::that(m); }
                }
                ui.add_space(4.0);
                let sub = pal.sub;
                if wide_btn(ui, "⎘  Copy Magnet", sub) {
                    if let Some(m) = &item.magnet { ui.output_mut(|o| o.copied_text = m.clone()); let g = app.pal.green; app.toast("Copied ✓", g); }
                }
                ui.add_space(4.0);
            }
            if item.link.is_some() {
                let green = pal.green;
                if wide_btn(ui, "↓  Download .torrent", green) {
                    if let Some(l) = &item.link { let _ = open::that(l); }
                }
                ui.add_space(4.0);
            }
            let yellow = pal.yellow;
            if wide_btn(ui, "★  Save to Favorites", yellow) {
                let it = item.clone();
                app.add_fav_from_rss(&it);
            }
        });
}

// ─── Add feed form ────────────────────────────────────────────────────────

fn draw_add_form(app: &mut App, ui: &mut egui::Ui) {
    draw_feed_form(app, ui, None);
}

fn draw_edit_form(app: &mut App, ui: &mut egui::Ui, idx: usize) {
    // Copy config into rss_new_cfg first call
    if app.rss_edit_idx != Some(idx) { return; }
    // We need a temporary buffer; we use app.rss_new_cfg
    draw_feed_form(app, ui, Some(idx));
}

fn draw_feed_form(app: &mut App, ui: &mut egui::Ui, edit_idx: Option<usize>) {
    let pal = app.pal.clone();
    let fs  = app.cfg.font_size;
    let is_edit = edit_idx.is_some();

    // On first open for edit, load config
    if is_edit {
        // Only initialise when edit_idx just changed
        // We'll use a flag: if rss_new_cfg.name is empty and we're editing, copy it
        if app.rss_new_cfg.name.is_empty() {
            if let Some(idx) = edit_idx {
                if let Some(feed) = app.rss_feeds.get(idx) {
                    app.rss_new_cfg = feed.config.clone();
                }
            }
        }
    }

    let title = if is_edit { "Edit Feed" } else { "Add New RSS Feed" };

    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.set_max_width(500.0);

        lbl(ui, title, pal.accent, fs + 3.0);
        ui.add_space(4.0);
        lbl(ui, "Connects to a Jackett Torznab indexer endpoint", pal.dim, fs - 1.0);
        ui.add_space(20.0);

        egui::Frame::none()
            .fill(pal.surface).rounding(10.0)
            .stroke(Stroke::new(1.0, pal.border))
            .inner_margin(egui::Margin::same(18.0))
            .show(ui, |ui| {
                egui::Grid::new("feed_form_grid")
                    .num_columns(2).spacing([12.0, 10.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Name").font(FontId::proportional(fs)).color(pal.sub));
                        ui.add(egui::TextEdit::singleline(&mut app.rss_new_cfg.name)
                            .desired_width(300.0).hint_text("e.g.  YTS Movies")
                            .font(FontId::proportional(fs)));
                        ui.end_row();

                        ui.label(RichText::new("Indexer slug").font(FontId::proportional(fs)).color(pal.sub));
                        ui.vertical(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut app.rss_new_cfg.indexer)
                                .desired_width(300.0).hint_text("all  (or specific slug, e.g. yts)")
                                .font(FontId::proportional(fs)));
                            ui.label(RichText::new("Use \"all\" to search every indexer, or a specific Jackett slug.")
                                .font(FontId::proportional(fs - 2.5)).color(pal.dim));
                        });
                        ui.end_row();

                        ui.label(RichText::new("Search query").font(FontId::proportional(fs)).color(pal.sub));
                        ui.vertical(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut app.rss_new_cfg.query)
                                .desired_width(300.0).hint_text("(empty = latest uploads)")
                                .font(FontId::proportional(fs)));
                            ui.label(RichText::new("Leave empty to get the latest uploads from this indexer.")
                                .font(FontId::proportional(fs - 2.5)).color(pal.dim));
                        });
                        ui.end_row();

                        ui.label(RichText::new("Category").font(FontId::proportional(fs)).color(pal.sub));
                        ui.vertical(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut app.rss_new_cfg.category)
                                .desired_width(300.0).hint_text("e.g. 2000,2010  (Torznab category IDs)")
                                .font(FontId::proportional(fs)));
                            ui.label(RichText::new("Common: 2000=Movies 5000=TV 3000=Music 4000=PC")
                                .font(FontId::proportional(fs - 2.5)).color(pal.dim));
                        });
                        ui.end_row();

                        ui.label(RichText::new("Options").font(FontId::proportional(fs)).color(pal.sub));
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut app.rss_new_cfg.enabled,      "Enabled");
                            ui.add_space(12.0);
                            ui.checkbox(&mut app.rss_new_cfg.auto_refresh, "Auto-refresh");
                        });
                        ui.end_row();
                    });

                ui.add_space(14.0);

                // Preview URL
                if !app.cfg.jackett_url.is_empty() {
                    let url = crate::rss::build_rss_url(
                        &app.cfg.jackett_url, "YOUR_KEY", &app.rss_new_cfg);
                    lbl(ui, "Preview URL:", pal.dim, fs - 2.0);
                    ui.add_space(3.0);
                    egui::Frame::none()
                        .fill(pal.hdr).rounding(5.0)
                        .stroke(Stroke::new(1.0, pal.border))
                        .inner_margin(egui::Margin::symmetric(8.0, 5.0))
                        .show(ui, |ui| {
                            ui.add(egui::Label::new(
                                RichText::new(&url).font(FontId::monospace(fs - 3.0)).color(pal.dim)
                            ).wrap(true));
                        });
                    ui.add_space(10.0);
                }

                // Buttons
                ui.horizontal(|ui| {
                    let accent = pal.accent;
                    let save_lbl = if is_edit { "Save Changes" } else { "Add Feed" };
                    if wide_btn(ui, save_lbl, accent) {
                        let cfg = app.rss_new_cfg.clone();
                        if let Some(idx) = edit_idx {
                            app.rss_feeds[idx].config = cfg;
                        } else {
                            app.rss_feeds.push(crate::rss::RssFeedState::new(cfg));
                            app.rss_selected = app.rss_feeds.len() - 1;
                        }
                        app.sync_rss_configs();
                        app.rss_add_mode  = false;
                        app.rss_edit_idx  = None;
                        app.rss_new_cfg   = RssFeedConfig::new_default();
                        // Auto-refresh new/edited feed
                        let idx2 = if let Some(i) = edit_idx { i } else { app.rss_feeds.len() - 1 };
                        app.refresh_feed(idx2);
                    }
                    ui.add_space(8.0);
                    let red = pal.red;
                    if outline_btn(ui, "Cancel", red) {
                        app.rss_add_mode  = false;
                        app.rss_edit_idx  = None;
                        app.rss_new_cfg   = RssFeedConfig::new_default();
                    }
                });
            });
    });
}
