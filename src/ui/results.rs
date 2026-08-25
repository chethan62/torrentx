//! Results table + detail panel drawing methods.

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_results_table(
        &mut self,
        ui: &mut egui::Ui,
        page_s: &[TorrentResult],
        base: usize,
    ) {
        let mut actions: Vec<(usize, &'static str)> = vec![];
        let pal = self.pal.clone();
        let s_col = self.search.s_col.clone();
        let s_dir = self.search.s_dir.clone();
        let rh = self.cfg.row_height;
        let fsz = self.cfg.font_size;
        // Only the column-visibility flags are needed (NOT a full Config
        // clone — that would copy history/favorites/rss every frame).
        let col_shown = (
            self.cfg.col_tracker,
            self.cfg.col_size,
            self.cfg.col_leech,
            self.cfg.col_ratio,
            self.cfg.col_health,
            self.cfg.col_date,
        );
        let sel = self.ui.selected;
        let det_open = self.ui.detail_open;
        // Use visual tokens for animation easing
        let tokens = self.tokens;
        let hover_progress = self.ui.row_hover_anim;
        // Table content fade (page turns, filter/sort changes) — ease-out
        let table_fade = (tokens.easing)(self.ui.table_anim);

        let mut new_sort: Option<(SortCol, bool)> = None;

        // Table header helper — label + SVG sort arrow (no glyphs).
        let hdr = |ui: &mut egui::Ui, l: &str, col: &SortCol| {
            let on = &s_col == col;
            let col_c = if on { pal.accent } else { pal.sub };
            ui.label(
                RichText::new(l)
                    .font(FontId::proportional(fsz))
                    .color(col_c)
                    .strong(),
            );
            if on {
                let icon = if s_dir == SortDir::Desc {
                    SvgIcon::ArrowDown
                } else {
                    SvgIcon::ArrowUp
                };
                svg_icon(ui, icon, 10.0, pal.accent);
            }
        };

        // Visible columns in user-configured order
        let cols: Vec<TableCol> = self
            .cfg
            .col_order
            .iter()
            .filter_map(|n| TableCol::from_name(n))
            .filter(|c| match c {
                TableCol::Name | TableCol::Seeds => true,
                TableCol::Tracker => col_shown.0,
                TableCol::Size => col_shown.1,
                TableCol::Leech => col_shown.2,
                TableCol::Ratio => col_shown.3,
                TableCol::Health => col_shown.4,
                TableCol::Date => col_shown.5,
            })
            .collect();

        let mut tb = TableBuilder::new(ui)
            .striped(false)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
        // The Name column is the flexible one (absorbs window slack); all
        // other data columns are fixed-width. Actions is a normal resizable
        // column — NOT remainder — because egui pins a trailing remainder
        // column to fill all leftover space, which makes its separator
        // undraggable once moved (the stuck-drag bug).
        // Actions holds up to 5 icon buttons (32px each + gaps ≈ 190px), so
        // its floor must stay high; the Name remainder shrinks to absorb
        // narrow width. egui_extras 0.36 hardcodes hscroll OFF → columns clip
        // past the viewport, but every column is user-resizable.
        for c in &cols {
            if *c == TableCol::Name {
                tb = tb.column(Column::remainder().at_least(120.0));
            } else {
                tb = tb.column(Column::initial(c.width()).at_least(36.0));
            }
        }
        tb = tb.column(Column::initial(190.0).at_least(150.0)); // Actions (resizable)
        tb.header(30.0, |mut header| {
            for c in &cols {
                let sortcol = match c {
                    TableCol::Name => SortCol::Name,
                    TableCol::Tracker => SortCol::Tracker,
                    TableCol::Size => SortCol::Size,
                    TableCol::Seeds => SortCol::Seeds,
                    TableCol::Leech => SortCol::Leech,
                    TableCol::Ratio => SortCol::Ratio,
                    TableCol::Date => SortCol::Date,
                    TableCol::Health => SortCol::Seeds, // non-sortable; reuse Seeds
                };
                header.col(|ui| {
                    // Header padding — labels shouldn't touch the cell edge
                    ui.add_space(4.0);
                    if *c == TableCol::Health {
                        ui.label(
                            RichText::new("Health")
                                .font(FontId::proportional(fsz))
                                .color(pal.sub)
                                .strong(),
                        );
                    } else {
                        let hresp = ui.interact(
                            ui.max_rect(),
                            egui::Id::new(("sort_hdr", c.label())),
                            egui::Sense::click(),
                        );
                        hdr(ui, c.label(), &sortcol);
                        if hresp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if hresp.clicked() {
                            new_sort = Some((sortcol.clone(), s_col == sortcol));
                        }
                    }
                });
            }
            header.col(|ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Actions")
                        .font(FontId::proportional(fsz))
                        .color(pal.sub)
                        .strong(),
                );
            });
        })
        .body(|mut body| {
            for (i, r) in page_s.iter().enumerate() {
                let gi = base + i; // global index into the filtered results
                let is_sel = sel == Some(i);
                let is_hov = self.ui.hovered == Some(i);
                let seed = r.seeders.unwrap_or(0);
                let leech = r.peers.unwrap_or(0);

                // Animated hover background - smoothly transition between states
                let hover_t = if is_hov { hover_progress } else { 0.0 };
                let base_bg = if is_sel {
                    pal.row_sel
                } else if i % 2 == 0 {
                    pal.row_odd
                } else {
                    pal.row_even
                };
                // Lerp between base_bg and hover color (symmetric ease for enter+exit).
                // Blend in STRAIGHT (non-premultiplied) alpha: the hover color is a
                // low-alpha accent tint, so we composite it over the base row color
                // rather than lerping raw RGB (which produced a dark muddy highlight
                // on light themes).
                let bg = if hover_t > 0.0 {
                    let t = (tokens.easing_hover)(hover_t);
                    // Effective alpha of the hover tint at this point in the fade.
                    let hov_a = pal.row_hov.a() as f32 * t;
                    let hov_r = pal.row_hov.r() as f32;
                    let hov_g = pal.row_hov.g() as f32;
                    let hov_b = pal.row_hov.b() as f32;
                    // Straight-alpha "over" composite: dst = src*a + base*(1-a)
                    Color32::from_rgba_unmultiplied(
                        (hov_r * hov_a / 255.0 + base_bg.r() as f32 * (1.0 - hov_a / 255.0)) as u8,
                        (hov_g * hov_a / 255.0 + base_bg.g() as f32 * (1.0 - hov_a / 255.0)) as u8,
                        (hov_b * hov_a / 255.0 + base_bg.b() as f32 * (1.0 - hov_a / 255.0)) as u8,
                        255,
                    )
                } else {
                    base_bg
                };

                body.row(rh, |mut row| {
                    for c in &cols {
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                            match c {
                                TableCol::Name => {
                                    // Full-cell click layer — clicking ANYWHERE
                                    // in the Name cell (not just the text)
                                    // selects the row. Double-click opens magnet.
                                    let cell_id = egui::Id::new(("namecell", gi));
                                    let cell_resp =
                                        ui.interact(ui.max_rect(), cell_id, egui::Sense::click());
                                    if cell_resp.double_clicked() {
                                        if let Some(m) = r.magnet_uri.as_deref() {
                                            if is_magnet(m) {
                                                let _ = safe_open(m);
                                                self.toast(
                                                    "Opening in torrent client…",
                                                    self.pal.accent,
                                                );
                                            } else {
                                                self.toast("Invalid magnet link", self.pal.yellow);
                                            }
                                        } else {
                                            self.toast("No magnet link", self.pal.yellow);
                                        }
                                    } else if cell_resp.clicked() {
                                        if self.ui.sel_mode {
                                            if !self.ui.sel_set.insert(gi) {
                                                self.ui.sel_set.remove(&gi);
                                            }
                                        } else {
                                            actions.push((i, "select"));
                                        }
                                    }
                                    if cell_resp.hovered() {
                                        self.ui.hovered = Some(i);
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    // Draw the content (non-interactive label)
                                    ui.horizontal(|ui| {
                                        ui.add_space(6.0);
                                        if self.ui.sel_mode {
                                            let checked = self.ui.sel_set.contains(&gi);
                                            if v_checkbox(ui, checked, "", self.pal.accent)
                                                .clicked()
                                            {
                                                if checked {
                                                    self.ui.sel_set.remove(&gi);
                                                } else {
                                                    self.ui.sel_set.insert(gi);
                                                }
                                            }
                                            ui.add_space(4.0);
                                        }
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&r.title)
                                                    .font(FontId::proportional(fsz))
                                                    .color(if is_sel {
                                                        pal.accent
                                                    } else {
                                                        pal.text
                                                    }),
                                            )
                                            .truncate(),
                                        );
                                    });
                                    if rh >= 40.0 {
                                        let cat = r.category_desc.as_deref().unwrap_or("Other");
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(cat)
                                                    .font(FontId::proportional(fsz - 2.5))
                                                    .color(cat_col(cat)),
                                            )
                                            .truncate(),
                                        );
                                    }
                                }
                                TableCol::Tracker
                                | TableCol::Size
                                | TableCol::Seeds
                                | TableCol::Leech
                                | TableCol::Ratio
                                | TableCol::Health
                                | TableCol::Date => {
                                    // Full-cell click layer FIRST (covers the whole
                                    // cell rect), THEN draw the label as non-interactive
                                    // content. If drawn first, the label shrinks
                                    // ui.max_rect() and clicks on the text miss.
                                    let cell_id = egui::Id::new(("rowcell", gi, c.label()));
                                    let cell_resp =
                                        ui.interact(ui.max_rect(), cell_id, egui::Sense::click());
                                    if cell_resp.clicked() && !self.ui.sel_mode {
                                        actions.push((i, "select"));
                                    }
                                    if cell_resp.hovered() {
                                        self.ui.hovered = Some(i);
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    draw_cell_content(ui, c, r, seed, leech, fsz, &pal);
                                }
                            }
                        });
                    }
                    // Actions — fixed (always-visible) icon buttons, right-aligned.
                    row.col(|ui| {
                        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Breathing room so icons never touch the cell edge
                            ui.add_space(8.0);
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let has_mag = r.magnet_uri.as_deref().map(is_magnet).unwrap_or(false);
                            let has_link = r.link.is_some();
                            // All actions always visible (no hover-reveal).
                            if has_mag
                                && svg_btn(
                                    ui,
                                    SvgIcon::Magnet,
                                    "Open in torrent client",
                                    pal.accent,
                                )
                            {
                                actions.push((i, "mag"));
                            }
                            if has_mag && svg_btn(ui, SvgIcon::Copy, "Copy magnet link", pal.sub) {
                                actions.push((i, "copy"));
                            }
                            if has_link
                                && svg_btn(ui, SvgIcon::Download, "Download .torrent", pal.green)
                            {
                                actions.push((i, "dl"));
                            }
                            if svg_btn(ui, SvgIcon::Star, "Add to Favorites (F)", pal.yellow) {
                                actions.push((i, "fav"));
                            }
                            if svg_btn(
                                ui,
                                SvgIcon::Info,
                                "Detail panel (D)",
                                if is_sel && det_open {
                                    pal.accent
                                } else {
                                    pal.sub
                                },
                            ) {
                                actions.push((i, "info"));
                            }
                            let cell_id = egui::Id::new(("rowhov", gi));
                            let hover_resp =
                                ui.interact(ui.max_rect(), cell_id, egui::Sense::hover());
                            if hover_resp.hovered() {
                                self.ui.hovered = Some(i);
                            }
                        });
                    });
                });
            }
        });

        if let Some((col, same)) = new_sort {
            if same {
                self.search.s_dir = if self.search.s_dir == SortDir::Desc {
                    SortDir::Asc
                } else {
                    SortDir::Desc
                };
            } else {
                self.search.s_col = col;
                self.search.s_dir = SortDir::Desc;
            }
            self.search.page = 0;
        }

        // Process actions
        for (i, action) in actions {
            if action == "hover" {
                continue;
            } // already handled
            if let Some(r) = page_s.get(i).cloned() {
                match action {
                    "select" => {
                        if self.ui.selected == Some(i) && self.ui.detail_open {
                            self.ui.selected = None;
                            self.ui.detail_open = false;
                        } else {
                            self.ui.selected = Some(i);
                            self.ui.detail_open = true;
                        }
                    }
                    "mag" => {
                        if let Some(m) = &r.magnet_uri {
                            let _ = safe_open(m);
                            self.toast("Opening magnet…", self.pal.accent);
                        }
                    }
                    "copy" => {
                        if let Some(m) = &r.magnet_uri {
                            ui.ctx().copy_text(m.clone());
                            self.toast("Magnet copied", self.pal.green);
                        }
                    }
                    "dl" => {
                        if let Some(l) = &r.link {
                            let _ = safe_open(l);
                            self.toast("Downloading…", self.pal.green);
                        }
                    }
                    "fav" => {
                        self.add_fav(&r);
                    }
                    "info" => {
                        // Idempotent open: clicking Info always opens the detail
                        // panel for this row. (The row's own click may have already
                        // set selected+detail_open — don't toggle it closed here,
                        // or the button would flash-open/close in the same frame.)
                        self.ui.selected = Some(i);
                        self.ui.detail_open = true;
                    }
                    "web" => {
                        if let Some(d) = &r.details {
                            let _ = safe_open(d);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Clear hover when mouse leaves the table area
        if let Some(hover_pos) = ui.ctx().pointer_hover_pos() {
            if !ui.min_rect().contains(hover_pos) {
                self.ui.hovered = None;
            }
        } else {
            self.ui.hovered = None;
        }

        // Fade in the table content on page/filter/sort change (draw a
        // translucent overlay that fades out, instead of re-drawing rows
        // with opacity which would make text hard to read mid-fade).
        if table_fade < 1.0 {
            let rect = ui.min_rect();
            let a = ((1.0 - table_fade) * 120.0) as u8;
            ui.painter().rect_filled(rect, 0.0, tint(pal.surface, a));
        }
    }

    // ─── Idle / welcome ────────────────────────────────────────────────────

    pub(crate) fn draw_idle(&mut self, ui: &mut egui::Ui) {
        let fs = self.cfg.font_size;
        ui.add_space(50.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("TorrentX")
                    .font(FontId::proportional(40.0))
                    .strong()
                    .color(tint(self.pal.accent, 230)),
            );
            ui.add_space(6.0);
            lbl(
                ui,
                "Search all your Jackett indexers in one shot",
                self.pal.sub,
                fs + 1.0,
            );
            ui.add_space(3.0);
            lbl(
                ui,
                "Movies  ·  TV  ·  Music  ·  Games  ·  Software  ·  Anime  ·  Books",
                self.pal.dim,
                fs - 1.0,
            );
            ui.add_space(32.0);

            if !self.cfg.history.is_empty() {
                lbl(ui, "Recent searches", self.pal.dim, fs - 1.0);
                ui.add_space(10.0);
                let hist: Vec<String> = self.cfg.history.iter().take(12).cloned().collect();
                let mut clicked: Option<String> = None;
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
                                .corner_radius(14.0)
                                .min_size(egui::vec2(0.0, 28.0)),
                            )
                            .clicked()
                        {
                            clicked = Some(h.clone());
                        }
                    }
                });
                if let Some(h) = clicked {
                    self.search.query = h;
                    self.do_search();
                }
            } else {
                lbl(ui, "Try searching:", self.pal.dim, fs - 1.0);
                ui.add_space(10.0);
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
                                .corner_radius(14.0)
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

                ui.add_space(32.0);
                egui::Frame::NONE
                    .fill(tint(self.pal.accent, 12))
                    .corner_radius(10.0)
                    .stroke(Stroke::new(1.0_f32, tint(self.pal.accent, 50)))
                    .inner_margin(egui::Margin::symmetric(24, 16))
                    .show(ui, |ui| {
                        ui.set_max_width(480.0);
                        ui.label(
                            RichText::new("First time?")
                                .font(FontId::proportional(fs + 1.0))
                                .color(self.pal.accent)
                                .strong(),
                        );
                        ui.add_space(6.0);
                        lbl(
                            ui,
                            "1. Make sure Jackett is running  (localhost:9117)",
                            self.pal.sub,
                            fs - 1.0,
                        );
                        ui.horizontal(|ui| {
                            lbl(ui, "2. Click", self.pal.sub, fs - 1.0);
                            svg_icon(ui, SvgIcon::Settings, 12.0, self.pal.sub);
                            lbl(
                                ui,
                                "Settings and paste your API key",
                                self.pal.sub,
                                fs - 1.0,
                            );
                        });
                        lbl(ui, "3. Search for anything!", self.pal.sub, fs - 1.0);
                        ui.add_space(10.0);
                        if outline_btn(ui, "Open Settings", self.pal.accent) {
                            self.ui.show_settings = true;
                        }
                    });
            }
        });
    }

    // ─── Detail panel ──────────────────────────────────────────────────────

    pub(crate) fn draw_detail(&mut self, ui: &mut egui::Ui, r: &TorrentResult) {
        let fs = self.cfg.font_size;
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            lbl(ui, "Details", self.pal.text, fs + 2.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if ui
                    .add(
                        egui::Button::new(svg_image(SvgIcon::Close, 14.0, self.pal.sub))
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(4.0),
                    )
                    .on_hover_text("Close")
                    .clicked()
                {
                    self.ui.detail_open = false;
                    self.ui.selected = None;
                }
            });
        });
        ui.separator();
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .id_salt("det_scr")
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(&r.title)
                            .font(FontId::proportional(fs))
                            .color(self.pal.text)
                            .strong(),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);

                let cat = r.category_desc.as_deref().unwrap_or("Unknown");
                egui::Frame::NONE
                    .fill(tint(cat_col(cat), 25))
                    .corner_radius(PANEL_RADIUS)
                    .inner_margin(egui::Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(cat)
                                .font(FontId::proportional(fs - 1.0))
                                .color(cat_col(cat)),
                        );
                    });
                ui.add_space(12.0);

                // Use grid for aligned details
                egui::Grid::new("detail_grid")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        let seed = r.seeders.unwrap_or(0);
                        let leech = r.peers.unwrap_or(0);

                        if let Some(t) = &r.tracker {
                            grid_row(ui, "Tracker", t, self.pal.sub, &self.pal, fs);
                        }
                        if let Some(s) = r.size {
                            grid_row(ui, "Size", &fmt_size(s), self.pal.sub, &self.pal, fs);
                        }
                        grid_row(
                            ui,
                            "Seeders",
                            &seed.to_string(),
                            seed_col(seed),
                            &self.pal,
                            fs,
                        );
                        grid_row(
                            ui,
                            "Leechers",
                            &leech.to_string(),
                            self.pal.red,
                            &self.pal,
                            fs,
                        );
                        let ratio = if leech > 0 {
                            format!("{:.2}", seed as f64 / leech as f64)
                        } else {
                            "∞".into()
                        };
                        grid_row(ui, "Ratio", &ratio, self.pal.sub, &self.pal, fs);
                        grid_row(ui, "Health", hlth_lbl(seed), seed_col(seed), &self.pal, fs);
                        if let Some(d) = &r.publish_date {
                            grid_row(ui, "Published", &time_ago(d), self.pal.dim, &self.pal, fs);
                        }
                    });

                let seed = r.seeders.unwrap_or(0);
                let leech = r.peers.unwrap_or(0);
                let tot = (seed + leech) as f32;
                if tot > 0.0 {
                    let ratio_value = if leech > 0 {
                        format!("{:.2}", seed as f64 / leech as f64)
                    } else {
                        "∞".into()
                    };
                    let pct = (seed as f32 / tot).clamp(0.0, 1.0);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Ratio: {}  ", ratio_value))
                                .font(FontId::proportional(fs - 1.0))
                                .color(self.pal.sub),
                        );
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width() - 60.0, 8.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 4.0, self.pal.border);
                        let mut filled = rect;
                        filled.max.x = rect.min.x + rect.width() * pct;
                        ui.painter().rect_filled(filled, 4.0, seed_col(seed));
                    });
                    ui.add_space(2.0);
                    lbl(
                        ui,
                        &format!("{:.0}% seeded", pct * 100.0),
                        self.pal.dim,
                        fs - 2.0,
                    );
                }

                // Show magnet link (truncated) if present
                if let Some(mag) = &r.magnet_uri {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Magnet:")
                                .font(FontId::proportional(fs - 1.5))
                                .color(self.pal.dim),
                        );
                        // Truncate on a UTF-8 char boundary (magnets can carry
                        // non-ASCII in the dn= display name; byte-slicing [..57]
                        // panics on a mid-char cut). Uses the tested helper.
                        let truncated = crate::jackett::truncate_magnet(mag, 57);
                        // Full-width clickable copy target (button, not a text label).
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(truncated)
                                        .font(FontId::monospace(fs - 2.0))
                                        .color(self.pal.sub),
                                )
                                .fill(tint(self.pal.sub, 10))
                                .stroke(Stroke::new(1.0_f32, self.pal.border))
                                .corner_radius(4.0),
                            )
                            .on_hover_text("Click to copy full magnet")
                            .clicked()
                        {
                            ui.ctx().copy_text(mag.clone());
                            self.toast("Magnet copied", self.pal.green);
                        }
                    });
                }

                ui.add_space(16.0);
                lbl(ui, "Actions", self.pal.dim, fs - 1.0);
                ui.add_space(6.0);

                if let Some(mag) = r.magnet_uri.clone() {
                    let mc = mag.clone();
                    if wide_icon_btn(ui, SvgIcon::Magnet, "Open Magnet", self.pal.accent) {
                        let _ = safe_open(mag);
                        self.toast("Opening magnet…", self.pal.accent);
                    }
                    ui.add_space(4.0);
                    if wide_icon_btn(ui, SvgIcon::Copy, "Copy Magnet Link", self.pal.sub) {
                        ui.ctx().copy_text(mc);
                        self.toast("Copied", self.pal.green);
                    }
                    ui.add_space(4.0);
                }
                if let Some(link) = r.link.clone() {
                    if wide_icon_btn(ui, SvgIcon::Download, "Download .torrent", self.pal.green) {
                        let _ = safe_open(link);
                        self.toast("Downloading…", self.pal.green);
                    }
                    ui.add_space(4.0);
                }
                if let Some(det) = r.details.clone() {
                    if wide_icon_btn(ui, SvgIcon::Web, "Open in Browser", self.pal.sub) {
                        let _ = safe_open(det);
                    }
                    ui.add_space(4.0);
                }
                let r2 = r.clone();
                if wide_icon_btn(ui, SvgIcon::Star, "Add to Favorites", self.pal.yellow) {
                    self.add_fav(&r2);
                }
            });
    }
}
