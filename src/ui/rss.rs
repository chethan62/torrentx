//! rss drawing methods.

use super::*;

use crate::app::App;


impl App {

    pub(crate) fn draw_rss_detail_panel(&mut self, ui: &mut egui::Ui) {
        if self.ui.tab != Tab::Rss { return; }
        let Some(di) = self.rss.rss_detail else { return };
        // Items live in the active feed's state; rebuild the index the same
        // way draw_rss does so the panel and table agree.
        let Some(feed) = self.rss.rss_feeds.get(self.rss.rss_selected) else { return };
        let items = feed.items.clone();
        if let Some(item) = items.get(di).cloned() {
            egui::Panel::right("rss_detail_pnl")
                .resizable(true)
                .default_size(300.0)
                .size_range(240.0..=640.0)
                .frame(egui::Frame::NONE
                    .fill(self.pal.surface)
                    .stroke(Stroke::new(1.0_f32, self.pal.border))
                    .inner_margin(egui::Margin::symmetric(PANEL_MARGIN_X, 8)))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.draw_rss_item_detail(ui, &item));
                });
        }
    }

    pub(crate) fn draw_rss(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let pal = self.pal.clone();
        let fs = self.cfg.font_size;

        if self.rss.rss_feeds.is_empty() && !self.rss.rss_add_mode {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("📡").size(42.0));
                ui.add_space(12.0);
                lbl(ui, "No RSS Feeds yet", pal.sub, 18.0);
                ui.add_space(6.0);
                lbl(ui, "Add Jackett Torznab feeds to auto-refresh torrents", pal.dim, fs);
                ui.add_space(20.0);
                egui::Frame::NONE
                    .fill(tint(pal.accent, 12)).corner_radius(10.0)
                    .stroke(Stroke::new(1.0_f32, tint(pal.accent, 50)))
                    .inner_margin(egui::Margin::symmetric(24, 16))
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);
                        lbl(ui, "How Torznab RSS works", pal.accent, fs);
                        ui.add_space(6.0);
                        lbl(ui, "Each indexer in Jackett exposes a Torznab API.", pal.sub, fs - 1.0);
                        lbl(ui, "You can search any indexer and get live results", pal.sub, fs - 1.0);
                        lbl(ui, "as an auto-refreshed feed — no browser needed.", pal.sub, fs - 1.0);
                        ui.add_space(12.0);
                        if outline_btn(ui, "+ Add Feed", pal.accent) { self.rss.rss_add_mode = true; }
                    });
            });
            return;
        }

        ui.horizontal_top(|ui| {
            // Sidebar as a fixed-width Frame (NOT egui::Panel::left — panels
            // inside a Ui don't reserve space in egui 0.36 and collapse/push
            // content; a plain frame in a horizontal layout is reliable).
            // allocate_ui with an exact size forces the 215px width; plain
            // set_width() lets inner content expand the frame.
            let sb_w = 215.0_f32;
            let sb_h = ui.available_height();
            ui.allocate_ui_with_layout(
                egui::vec2(sb_w, sb_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::Frame::NONE
                        .fill(pal.surface)
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .inner_margin(egui::Margin::symmetric(2, 0))
                        .show(ui, |ui| {
                            self.draw_rss_sidebar(ui);
                        });
                },
            );

            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                if self.rss.rss_add_mode { self.draw_rss_form(ui, None); }
                else if let Some(idx) = self.rss.rss_edit_idx { self.draw_rss_form(ui, Some(idx)); }
                else { self.draw_rss_items(ui); }
            });
        });
    }

    pub(crate) fn draw_rss_sidebar(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal.clone(); let fs = self.cfg.font_size;
        egui::Frame::NONE
            .fill(pal.hdr).stroke(Stroke::new(1.0_f32, pal.border))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    lbl(ui, "RSS Feeds", pal.accent, fs);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let sub = pal.sub;
                        if ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            svg_icon(ui, SvgIcon::Refresh, 13.0, sub);
                            ui.add(egui::Button::new(RichText::new("All").font(FontId::proportional(fs - 1.5)).color(sub))
                                .fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0_f32, pal.border)).corner_radius(4.0)
                            ).on_hover_text("Refresh all feeds").clicked()
                        }).inner { self.refresh_all_feeds(); }
                    });
                });
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut self.rss.rss_filter)
                    .desired_width(ui.available_width()).hint_text("Filter feeds…").font(FontId::proportional(fs)));
            });

        egui::ScrollArea::vertical().id_salt("rss_feed_list")
            .max_height(ui.available_height())
            .auto_shrink([false, true])
            .show(ui, |ui| {
            let filter = self.rss.rss_filter.to_lowercase();
            let len = self.rss.rss_feeds.len();
            let mut sel: Option<usize> = None;
            let mut del: Option<usize> = None;
            let mut ed: Option<usize> = None;
            let mut refr: Option<usize> = None;

            for i in 0..len {
                let name = self.rss.rss_feeds[i].config.name.clone();
                let n = self.rss.rss_feeds[i].items.len();
                let st = self.rss.rss_feeds[i].status.clone();
                let en = self.rss.rss_feeds[i].config.enabled;
                if !filter.is_empty() && !name.to_lowercase().contains(&filter) { continue; }

                let is_sel = self.rss.rss_selected == i && !self.rss.rss_add_mode && self.rss.rss_edit_idx.is_none();
                let bg = if is_sel { tint(pal.accent, 22) } else { Color32::TRANSPARENT };

                egui::Frame::NONE.fill(bg).corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        // Full-row click layer — clicking anywhere in the feed
                        // row (name, count badge, empty space) selects it, not
                        // just the name text.
                        let row_resp = ui.interact(ui.max_rect(), egui::Id::new(("feedrow", i)), egui::Sense::click());
                        if row_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                        if row_resp.clicked() { sel = Some(i); }
                        ui.horizontal(|ui| {
                            let (dc, icon) = match st {
                                FeedStatus::Ok => (pal.green, SvgIcon::Circle),
                                FeedStatus::Loading => (pal.accent, SvgIcon::Refresh),
                                FeedStatus::Error => (pal.red, SvgIcon::Close),
                                FeedStatus::Idle => (pal.dim, SvgIcon::CircleDot),
                            };
                            svg_icon(ui, icon, 10.0, dc);
                            ui.add_space(4.0);
                            let nc = if en { pal.text } else { pal.dim };
                            ui.add(egui::Label::new(RichText::new(&name).font(FontId::proportional(fs - 0.5)).color(nc))
                                .truncate());
                            // Auto-refresh marker
                            if self.rss.rss_feeds[i].config.auto_refresh {
                                let ac = if en { pal.accent } else { pal.dim };
                                ui.add(svg_image(SvgIcon::Refresh, 10.0, ac))
                                    .on_hover_text(format!("Auto-refreshes every {} min", self.cfg.rss_refresh_secs / 60));
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if n > 0 {
                                    egui::Frame::NONE.fill(tint(pal.accent, 25)).corner_radius(PANEL_RADIUS)
                                        .inner_margin(egui::Margin::symmetric(5, 1))
                                        .show(ui, |ui| { ui.label(RichText::new(n.to_string()).font(FontId::monospace(fs - 3.0)).color(pal.accent)); });
                                }
                            });
                        });
                        if is_sel {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if svg_btn(ui, SvgIcon::Refresh, "Refresh", pal.accent) { refr = Some(i); }
                                if act_btn(ui, "Edit", "Edit feed", pal.sub) { ed = Some(i); }
                                if svg_btn(ui, SvgIcon::Close, "Delete feed", pal.red) { del = Some(i); }
                                let ec = if en { pal.green } else { pal.dim };
                                let el = if en { "On" } else { "Off" };
                                if act_btn(ui, el, "Toggle enabled", ec) {
                                    self.rss.rss_feeds[i].config.enabled = !en; self.sync_rss_configs();
                                }
                            });
                        }
                    });
            }
            if let Some(i) = sel  { self.rss.rss_selected = i; self.rss.rss_add_mode = false; self.rss.rss_edit_idx = None; self.rss.rss_detail = None; }
            if let Some(i) = refr { self.refresh_feed(i); }
            if let Some(i) = del  { self.rss.rss_feeds.remove(i); if self.rss.rss_selected >= self.rss.rss_feeds.len() && !self.rss.rss_feeds.is_empty() { self.rss.rss_selected = self.rss.rss_feeds.len() - 1; } self.sync_rss_configs(); }
            if let Some(i) = ed   { self.rss.rss_edit_idx = Some(i); self.rss.rss_add_mode = false; }
        });

        ui.add_space(8.0);
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            if wide_btn(ui, "+ Add Feed", pal.accent) { self.rss.rss_add_mode = true; self.rss.rss_edit_idx = None; self.rss.rss_new_cfg = RssFeedConfig::new_default(); }
        });
    }

    pub(crate) fn draw_rss_items(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal.clone(); let fs = self.cfg.font_size; let rh = self.cfg.row_height;
        let sel = self.rss.rss_selected;
        if self.rss.rss_feeds.is_empty() || sel >= self.rss.rss_feeds.len() { return; }

        let name = self.rss.rss_feeds[sel].config.name.clone();
        let status = self.rss.rss_feeds[sel].status.clone();
        let items = self.rss.rss_feeds[sel].items.clone();
        let err = self.rss.rss_feeds[sel].error.clone();

        egui::Frame::NONE.fill(pal.surface).stroke(Stroke::new(1.0_f32, pal.border))
            .inner_margin(egui::Margin::symmetric(14, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    lbl(ui, &name, pal.accent, fs + 1.0); ui.add_space(8.0);
                    let (dc, icon, label) = match status {
                        FeedStatus::Ok => (pal.green, SvgIcon::Circle, "OK"),
                        FeedStatus::Loading => (pal.accent, SvgIcon::Refresh, "Loading"),
                        FeedStatus::Error => (pal.red, SvgIcon::Close, "Error"),
                        FeedStatus::Idle => (pal.dim, SvgIcon::CircleDot, "Idle"),
                    };
                    status_icon_pill(ui, icon, label, dc);
                    lbl(ui, &format!("  {} items", items.len()), pal.dim, fs - 1.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            svg_icon(ui, SvgIcon::Refresh, 13.0, pal.accent);
                            outline_btn(ui, "Refresh", pal.accent)
                        }).inner { self.refresh_feed(sel); }
                        ui.add_space(6.0);
                        if outline_btn(ui, "Edit Feed", pal.sub) { self.rss.rss_edit_idx = Some(sel); }
                    });
                });
                if let Some(e) = &err { ui.add_space(4.0); lbl(ui, &format!("Error: {e}"), pal.red, fs - 1.0); }
            });

        if status == FeedStatus::Loading && items.is_empty() {
            ui.add_space(40.0); ui.vertical_centered(|ui| { ui.spinner(); ui.add_space(10.0); lbl(ui, "Fetching Torznab feed…", pal.sub, fs); });
            return;
        }
        if items.is_empty() {
            ui.add_space(40.0); ui.vertical_centered(|ui| { lbl(ui, "No items yet", pal.dim, fs + 2.0); ui.add_space(8.0); lbl(ui, "Click Refresh to fetch the latest torrents", pal.sub, fs); });
            return;
        }

        if let Some(di) = self.rss.rss_detail {
            if let Some(item) = items.get(di).cloned() {
                // NOTE: rendering moved to draw_rss_detail_panel() (frame
                // level, before CentralPanel) so it appears BESIDE the table.
                let _ = (di, item);
            }
        }

        use egui_extras::{Column, TableBuilder};
        let mut actions: Vec<(usize, &'static str)> = vec![];
        ui.add_space(2.0);
        egui::ScrollArea::vertical().id_salt("rss_items_scroll").auto_shrink([false, true]).show(ui, |ui| {
        TableBuilder::new(ui).striped(false).resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(180.0).clip(true))
            .column(Column::initial(80.0).at_least(50.0))
            .column(Column::initial(60.0).at_least(44.0))
            .column(Column::initial(60.0).at_least(44.0))
            .column(Column::initial(80.0).at_least(60.0))
            .column(Column::initial(180.0).at_least(120.0))
            .header(28.0, |mut hdr| {
                for label in ["Title", "Tracker", "Size", "Seeds", "Date", "Actions"] {
                    hdr.col(|ui| { ui.label(RichText::new(label).font(FontId::proportional(fs - 1.0)).color(pal.sub).strong()); });
                }
            })
            .body(|mut body| {
                for (i, item) in items.iter().enumerate() {
                    let is_sel = self.rss.rss_detail == Some(i);
                    let bg = if is_sel { tint(pal.accent, 20) } else if i % 2 == 0 { pal.row_odd } else { pal.row_even };
                    body.row(rh, |mut row| {
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            // Full-cell click — anywhere in the row opens detail.
                            let cell_resp = ui.interact(ui.max_rect(), egui::Id::new(("rsscell", i)), egui::Sense::click());
                            if cell_resp.clicked() { actions.push((i, "detail")); }
                            if cell_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            ui.add_space(6.0);
                            ui.add(egui::Label::new(RichText::new(&item.title).font(FontId::proportional(fs)).color(if is_sel { pal.accent } else { pal.text }))
                                .truncate());
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            // Full-cell click — every column opens detail.
                            let cell_resp = ui.interact(ui.max_rect(), egui::Id::new(("rsstracker", i)), egui::Sense::click());
                            if cell_resp.clicked() { actions.push((i, "detail")); }
                            ui.add_space(4.0);
                            ui.add(egui::Label::new(RichText::new(item.tracker.as_deref().unwrap_or("—")).font(FontId::proportional(fs - 1.0)).color(pal.sub)).truncate());
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            let cell_resp = ui.interact(ui.max_rect(), egui::Id::new(("rsssize", i)), egui::Sense::click());
                            if cell_resp.clicked() { actions.push((i, "detail")); }
                            ui.add_space(4.0);
                            ui.label(RichText::new(item.size.map(fmt_size).unwrap_or_else(|| "—".into())).font(FontId::monospace(fs - 0.5)).color(pal.sub));
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            let cell_resp = ui.interact(ui.max_rect(), egui::Id::new(("rssseeds", i)), egui::Sense::click());
                            if cell_resp.clicked() { actions.push((i, "detail")); }
                            ui.add_space(4.0);
                            let s = item.seeders.unwrap_or(0);
                            ui.label(RichText::new(s.to_string()).font(FontId::monospace(fs - 0.5)).color(seed_col(s)).strong());
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            let cell_resp = ui.interact(ui.max_rect(), egui::Id::new(("rssdate", i)), egui::Sense::click());
                            if cell_resp.clicked() { actions.push((i, "detail")); }
                            ui.add_space(4.0);
                            let d = item.pub_date.as_deref().map(time_ago).unwrap_or_else(|| "—".into());
                            ui.label(RichText::new(d).font(FontId::monospace(fs - 0.5)).color(pal.dim));
                        });
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            ui.horizontal(|ui| {
                                ui.add_space(2.0);
                                ui.spacing_mut().item_spacing.x = 5.0;
                                if item.magnet.as_deref().map(is_magnet).unwrap_or(false) {
                                    if  svg_btn(ui, SvgIcon::Magnet, "Open magnet", pal.accent) { actions.push((i, "mag")); }
                                    if  svg_btn(ui, SvgIcon::Copy, "Copy magnet link", pal.sub) { actions.push((i, "copy")); }
                                }
                                if item.link.is_some()
                                    &&  svg_btn(ui, SvgIcon::Download, "Download .torrent", pal.green) { actions.push((i, "dl")); }
                                if  svg_btn(ui, SvgIcon::Info, "Item details", pal.sub) { actions.push((i, "detail")); }
                                if  svg_btn(ui, SvgIcon::Star, "Add to Favorites", pal.yellow) { actions.push((i, "fav")); }
                                let cell_id = egui::Id::new(("rsshov", i));
                                let hover_resp = ui.interact(ui.max_rect(), cell_id, egui::Sense::hover());
                                if hover_resp.hovered() {
                                    self.ui.hovered = Some(i);
                                }
                            });
                        });
                    });
                }
            });

        for (i, action) in actions {
            if let Some(item) = items.get(i).cloned() {
                match action {
                    "detail" => { self.rss.rss_detail = if self.rss.rss_detail == Some(i) { None } else { Some(i) }; }
                    "mag" => { if let Some(m) = &item.magnet { let _ = open::that(m); self.toast("Opening magnet…", pal.accent); } }
                    "copy" => { if let Some(m) = &item.magnet { ui.ctx().copy_text(m.clone()); self.toast("Magnet copied ✓", pal.green); } }
                    "dl" => { if let Some(l) = &item.link { let _ = open::that(l); } }
                    "fav" => { self.add_fav_from_rss(&item); }
                    _ => {}
                }
            }
        }
        }); // end ScrollArea
    }

    pub(crate) fn draw_rss_item_detail(&mut self, ui: &mut egui::Ui, item: &RssItem) {
        let pal = self.pal.clone(); let fs = self.cfg.font_size;
        ui.add_space(10.0);
        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(PANEL_MARGIN_X, 0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                lbl(ui, "Item details", pal.accent, fs + 1.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(svg_image(SvgIcon::Close, 14.0, pal.sub))
                        .fill(Color32::TRANSPARENT).corner_radius(4.0))
                        .on_hover_text("Close").clicked() {
                        self.rss.rss_detail = None;
                    }
                });
            });
            ui.separator();
            ui.add_space(8.0);
            ui.add(egui::Label::new(RichText::new(&item.title).font(FontId::proportional(fs)).color(pal.text).strong()).wrap());
            ui.add_space(12.0);
            egui::Grid::new("rss_item_grid").num_columns(2).spacing([8.0, 5.0]).show(ui, |ui| {
                if let Some(t) = &item.tracker { grid_row(ui, "Tracker", t, pal.accent, &pal, fs); }
                if let Some(s) = item.size { grid_row(ui, "Size", &fmt_size(s), pal.text, &pal, fs); }
                if let Some(s) = item.seeders { grid_row(ui, "Seeders", &s.to_string(), seed_col(s), &pal, fs); }
                if let Some(l) = item.leechers { grid_row(ui, "Leechers", &l.to_string(), pal.red, &pal, fs); }
                if let Some(d) = &item.pub_date { grid_row(ui, "Published", &time_ago(d), pal.text, &pal, fs); }
                if let Some(c) = &item.category { grid_row(ui, "Category", c, pal.text, &pal, fs); }
            });
            ui.add_space(12.0);
            if item.magnet.is_some() {
                if wide_icon_btn(ui, SvgIcon::Magnet, "Open Magnet", pal.accent) { if let Some(m) = &item.magnet { let _ = open::that(m); } }
                ui.add_space(4.0);
                if wide_icon_btn(ui, SvgIcon::Copy, "Copy Magnet", pal.sub) { if let Some(m) = &item.magnet { ui.ctx().copy_text(m.clone()); self.toast("Copied ✓", pal.green); } }
                ui.add_space(4.0);
            }
            if item.link.is_some() { if wide_icon_btn(ui, SvgIcon::Download, "Download .torrent", pal.green) { if let Some(l) = &item.link { let _ = open::that(l); } } ui.add_space(4.0); }
            if wide_icon_btn(ui, SvgIcon::Star, "Save to Favorites", pal.yellow) { let it = item.clone(); self.add_fav_from_rss(&it); }
        });
    }

    pub(crate) fn draw_rss_form(&mut self, ui: &mut egui::Ui, edit_idx: Option<usize>) {
        let pal = self.pal.clone(); let fs = self.cfg.font_size;
        let is_edit = edit_idx.is_some();

        if is_edit && self.rss.rss_new_cfg.name.is_empty() {
            if let Some(idx) = edit_idx {
                if let Some(feed) = self.rss.rss_feeds.get(idx) { self.rss.rss_new_cfg = feed.config.clone(); }
            }
        }

        let title = if is_edit { "Edit Feed" } else { "Add New RSS Feed" };
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(500.0);
            lbl(ui, title, pal.accent, fs + 3.0); ui.add_space(4.0);
            lbl(ui, "Connects to a Jackett Torznab indexer endpoint", pal.dim, fs - 1.0);
            ui.add_space(20.0);

            labeled_input(ui, "Name:", &mut self.rss.rss_new_cfg.name, RSS_FORM_W, "My Feed", fs, pal.dim);
            ui.add_space(6.0);
            labeled_input(ui, "Indexer:", &mut self.rss.rss_new_cfg.indexer, RSS_FORM_W, "all (Jackett slug)", fs, pal.dim);
            ui.add_space(6.0);
            labeled_input(ui, "Query:", &mut self.rss.rss_new_cfg.query, RSS_FORM_W, "empty = latest torrents", fs, pal.dim);
            ui.add_space(6.0);
            labeled_input(ui, "Category:", &mut self.rss.rss_new_cfg.category, RSS_FORM_W, "Torznab cat numbers", fs, pal.dim);
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.checkbox(&mut self.rss.rss_new_cfg.enabled, "");
                lbl(ui, "Enabled", pal.text, fs);
                ui.add_space(20.0);
                ui.checkbox(&mut self.rss.rss_new_cfg.auto_refresh, "");
                lbl(ui, "Auto-refresh", pal.text, fs);
            });
            ui.add_space(16.0);

            ui.horizontal(|ui| {
                if outline_btn(ui, "Cancel", pal.red) { self.rss.rss_add_mode = false; self.rss.rss_edit_idx = None; }
                ui.add_space(12.0);
                if wide_btn(ui, "Save", pal.accent) {
                    if is_edit {
                        if let Some(idx) = edit_idx { self.rss.rss_feeds[idx].config = self.rss.rss_new_cfg.clone(); }
                    } else {
                        self.rss.rss_feeds.push(RssFeedState::new(self.rss.rss_new_cfg.clone()));
                        self.rss.rss_selected = self.rss.rss_feeds.len() - 1;
                    }
                    self.sync_rss_configs();
                    self.rss.rss_add_mode = false; self.rss.rss_edit_idx = None;
                    self.rss.rss_new_cfg = RssFeedConfig::new_default();
                }
            });
        });
    }

    // ─── About tab ─────────────────────────────────────────────────────────

}

