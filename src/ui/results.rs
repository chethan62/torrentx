//! Results table + detail panel drawing methods.

use super::*;

use crate::app::App;

/// The clickable part of a table cell.
///
/// egui_extras puts each column's resize handle in the gap to the cell's right and
/// registers it BEFORE the cell closure runs, so a full-cell `ui.interact` added
/// inside the closure sits on top of it: dragging the divider selected a row (or
/// re-sorted the table) instead of resizing, and the cursor never showed the
/// resize arrow. Excluding the handle band gives the drag back to the table.
fn cell_rect(ui: &egui::Ui) -> egui::Rect {
    let grab = ui.style().interaction.resize_grab_radius_side + 1.0;
    let mut r = ui.max_rect();
    r.max.x = (r.max.x - grab).max(r.min.x + 1.0);
    r
}

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
                    SvgIcon::SortDesc
                } else {
                    SvgIcon::SortAsc
                };
                svg_icon(ui, icon, ICON_XS, pal.accent);
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

        // egui_extras 0.36 hardcodes hscroll OFF inside the table; at
        // narrow widths the fixed columns + Actions would clip off the
        // right edge of the window. Wrap in a both-axis ScrollArea so the
        // columns scroll horizontally instead.
        // Hand-formatted closure body — #[rustfmt::skip] keeps the compact
        // single-level indentation instead of rustfmt's +1 level reflow.
        #[rustfmt::skip]
        egui::ScrollArea::both()
            .id_salt("results_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
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
                // Name is the ONLY remainder column — it must stay
                // non-resizable so egui_extras re-computes it every frame to
                // fill the current available width. A resizable remainder
                // locks its width on the first frame (TableState::load:
                // resizable columns keep `Size::exact(prev_width)`), so after
                // a resize/maximize the table would end early and leave dead
                // space after the Actions column.
                tb = tb.column(Column::remainder().at_least(120.0).resizable(false));
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
                    ui.add_space(CELL_PAD_X);
                    if *c == TableCol::Health {
                        ui.label(
                            RichText::new("Health")
                                .font(FontId::proportional(fsz))
                                .color(pal.sub)
                                .strong(),
                        );
                    } else {
                        let hresp = ui.interact(
                            cell_rect(ui),
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
                // The icon buttons in this column are right-aligned; centre the
                // header over them instead of leaving it stranded on the left.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(CELL_PAD_X);
                    ui.label(
                        RichText::new("Actions")
                            .font(FontId::proportional(fsz))
                            .color(pal.sub)
                            .strong(),
                    );
                });
            });
        })
        .body(|mut body| {
            self.ui.hovered = None;
            for (i, r) in page_s.iter().enumerate() {
                let gi = base + i; // global index into the filtered results
                let is_sel = sel == Some(i);
                let seed = r.seeders.unwrap_or(0);
                let leech = r.peers.unwrap_or(0);

                let base_bg = if is_sel {
                    pal.row_sel
                } else if i % 2 == 0 {
                    pal.row_odd
                } else {
                    pal.row_even
                };
                // Hover is derived from the pointer at paint time (not from the
                // previous frame's cell hit-tests), so the highlight is on the row
                // under the cursor in the same frame and cannot stick after the
                // pointer leaves.
                let mut bg = base_bg;

                body.row(rh, |mut row| {
                    let last = cols.len(); // Actions is the trailing cell
                    for (ci, c) in cols.iter().enumerate() {
                        row.col(|ui| {
                            // Row band: inset at the row's OUTER edges so the
                            // hover/selection fill lines up with the padded text
                            // instead of running to the window edge.
                            let mut fill = ui.max_rect();
                            if ci == 0 {
                                let band = fill;
                                let hovered = ui.ctx().pointer_latest_pos().is_some_and(|p| {
                                    p.y >= band.top() && p.y < band.top() + rh
                                });
                                if hovered {
                                    self.ui.hovered = Some(i);
                                    let a = pal.row_hov.a() as f32 / 255.0;
                                    bg = Color32::from_rgba_unmultiplied(
                                        (pal.row_hov.r() as f32 * a
                                            + base_bg.r() as f32 * (1.0 - a))
                                            as u8,
                                        (pal.row_hov.g() as f32 * a
                                            + base_bg.g() as f32 * (1.0 - a))
                                            as u8,
                                        (pal.row_hov.b() as f32 * a
                                            + base_bg.b() as f32 * (1.0 - a))
                                            as u8,
                                        255,
                                    );
                                }
                            }
                            if ci == 0 {
                                fill.min.x += CELL_PAD_X;
                            }
                            if ci == last {
                                fill.max.x -= CELL_PAD_X;
                            }
                            ui.painter().rect_filled(fill, 0.0, bg);
                            if ci == 0 && is_sel {
                                // 2px accent bar in the gutter marking the row.
                                let cell = ui.max_rect();
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(cell.left() + SP2, cell.top() + SP1),
                                        egui::vec2(2.0, cell.height() - SP2),
                                    ),
                                    1.0,
                                    pal.accent,
                                );
                            }
                            ui.add_space(CELL_PAD_X);
                            match c {
                                TableCol::Name => {
                                    // Full-cell click layer — clicking ANYWHERE
                                    // in the Name cell (not just the text)
                                    // selects the row. Double-click opens magnet.
                                    let cell_id = egui::Id::new(("namecell", gi));
                                    let cell_resp = ui.interact(
                                        cell_rect(ui),
                                        cell_id,
                                        egui::Sense::click(),
                                    );
                                    // The title cell is the row's primary hit area, so
                                    // the right-click menu lives here.
                                    let fav = self.is_fav(&r.title);
                                    attach_row_menu(&cell_resp, i, r, fav, &mut actions);
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
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    // Draw the content (non-interactive label)
                                    ui.horizontal(|ui| {
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
                                            ui.add_space(SP2);
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
                                                    // Category sub-label: fs - 2.5 put it
                                                    // at 11.5 px at the default size, the
                                                    // smallest text on the screen for a
                                                    // value people actually read.
                                                    .font(FontId::proportional(fsz - 1.5))
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
                                    let cell_resp = ui.interact(
                                        cell_rect(ui),
                                        cell_id,
                                        egui::Sense::click(),
                                    );
                                    if cell_resp.clicked() && !self.ui.sel_mode {
                                        actions.push((i, "select"));
                                    }
                                    if cell_resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    // Same right-click menu as the title cell — this
                                    // cell already owns the full cell rect, so no
                                    // extra hit-test layer is needed.
                                    attach_row_menu(
                                        &cell_resp,
                                        i,
                                        r,
                                        self.is_fav(&r.title),
                                        &mut actions,
                                    );
                                    draw_cell_content(ui, c, r, seed, leech, fsz, &pal);
                                }
                            }
                        });
                    }
                    // Actions — fixed (always-visible) icon buttons, right-aligned.
                    row.col(|ui| {
                        let mut fill = ui.max_rect();
                        fill.max.x -= CELL_PAD_X;
                        ui.painter().rect_filled(fill, 0.0, bg);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Breathing room so icons never touch the cell edge
                            ui.add_space(CELL_PAD_X);
                            ui.spacing_mut().item_spacing.x = SP3;
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
                            let saved = self.is_fav(&r.title);
                            if svg_btn(
                                ui,
                                if saved {
                                    SvgIcon::StarFilled
                                } else {
                                    SvgIcon::Star
                                },
                                if saved {
                                    "Remove from Favorites (F)"
                                } else {
                                    "Add to Favorites (F)"
                                },
                                pal.yellow,
                            ) {
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
                            let _hover_resp =
                                ui.interact(ui.max_rect(), cell_id, egui::Sense::hover());
                        });
                    });
                    // Right-click anywhere on the row: the actions are queued for
                    // the shared handler below, so the menu needs no state of its own.

                });
            }
        });
        }); // end ScrollArea::both

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
                    "name" => {
                        ui.ctx().copy_text(r.title.clone());
                        self.toast("Name copied", self.pal.green);
                    }
                    "play" => {
                        if let Some(m) = &r.magnet_uri {
                            match crate::player::start_play(
                                m,
                                r.link.as_deref(),
                                &self.cfg.player,
                                None,
                            ) {
                                Ok(()) => self.toast("Starting stream…", self.pal.accent),
                                Err(e) => self.toast(&e, self.pal.red),
                            }
                        }
                    }
                    "pick" => {
                        if let Some(m) = r.magnet_uri.clone() {
                            // The listing can take 30 s, and its progress line
                            // lives in the detail panel — open it, otherwise the
                            // menu item looks like it did nothing.
                            self.ui.selected = Some(i);
                            self.ui.detail_open = true;
                            self.pick_files(&r.title, &m, r.link.as_deref());
                        }
                    }
                    "dl" => {
                        if let Some(l) = &r.link {
                            let _ = safe_open(l);
                            self.toast("Downloading…", self.pal.green);
                        }
                    }
                    "fav" => {
                        // The button's tooltip promises a toggle: a filled star
                        // means the row is saved, so clicking it removes.
                        if self.is_fav(&r.title) {
                            self.cfg.favorites.retain(|f| f.title != r.title);
                            let _ = save_cfg(&self.cfg);
                            self.toast("Removed from Favorites", self.pal.sub);
                        } else {
                            self.add_fav(&r);
                        }
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
}

/// Right-click menu for one result. Pushes into the deferred action queue so the
/// handlers (selection, copy, favourites, playback) stay in one place.
fn attach_row_menu(
    resp: &egui::Response,
    i: usize,
    r: &TorrentResult,
    is_fav: bool,
    actions: &mut Vec<(usize, &'static str)>,
) {
    resp.context_menu(|ui| {
        let mut item = |ui: &mut egui::Ui, label: &str, act: &'static str| {
            if ui.button(label).clicked() {
                actions.push((i, act));
                ui.close();
            }
        };
        item(ui, "Play (largest file)", "play");
        item(ui, "Choose episode / file…", "pick");
        ui.separator();
        item(ui, "Open details", "info");
        item(
            ui,
            if is_fav {
                "Remove from Favorites"
            } else {
                "Add to Favorites"
            },
            "fav",
        );
        ui.separator();
        item(ui, "Copy magnet link", "copy");
        item(ui, "Copy name", "name");
        if r.link.is_some() {
            item(ui, "Download .torrent", "dl");
        }
        if r.magnet_uri.is_some() {
            item(ui, "Open magnet in browser", "mag");
        }
    });
}
