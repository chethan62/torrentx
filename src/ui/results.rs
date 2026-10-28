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

/// Width of the trailing Actions column (5 icon buttons + gaps).
const ACTION_COL_W: f32 = 190.0;
/// Phone width: the row drops Copy and Detail (a tap on the row opens the
/// panel), so three buttons fit in this instead of five.
const ACTION_COL_W_PHONE: f32 = 124.0;
/// Narrowest a column can be dragged, and the widest (Excel clamps to 255 chars).
const COL_MIN_W: f32 = 60.0;
const COL_MAX_W: f32 = 900.0;
/// Half-width of a column separator's grab band.
const COL_GRAB_W: f32 = 4.0;
/// Height reserved under the table for the horizontal scrollbar.
const HSCROLL_ROOM: f32 = 20.0;
/// Height of the draggable header band for column separators.
const COL_HANDLE_H: f32 = 30.0;

/// Height the table may use, with room reserved under it for the horizontal
/// scrollbar. Clamped at 1: opening the detail panel in a short window leaves
/// less than `HSCROLL_ROOM` and an unclamped subtraction handed egui a
/// negative height, which panics in its layout pass (`Negative size: [0.0 -20.0]`).
fn table_viewport_h(available: f32) -> f32 {
    (available - HSCROLL_ROOM).max(1.0)
}

/// Widths for the three columns the phone layout keeps — `(name, size, seeds)` —
/// fitted to `avail` so the table cannot come out wider than the scroll area: a
/// wider table brings back the horizontal bar and clips every title.
///
/// Every width is at least [`COL_MIN_W`], because `Column::exact` under a
/// column's `at_least` makes egui_extras panic (`min > max`) — which is exactly
/// how the first attempt at this crashed at 412 pt.
fn phone_col_widths(avail: f32, action_w: f32) -> (f32, f32, f32) {
    let size_w = 78.0;
    let seeds_w = 62.0;
    let name_w = (avail - size_w - seeds_w - action_w).max(COL_MIN_W);
    (name_w, size_w, seeds_w)
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

        // A 412 pt window cannot hold the stored column set: the table would be
        // Name plus a horizontal scrollbar, every title cut off. Keep the three
        // that answer "what is it, how big, how alive" and let Name take the rest.
        let phone = crate::ui::narrow(ui.available_width());
        let action_w = if phone {
            ACTION_COL_W_PHONE
        } else {
            ACTION_COL_W
        };

        // Visible columns in user-configured order
        let cols: Vec<TableCol> = self
            .cfg
            .col_order
            .iter()
            .filter_map(|n| TableCol::from_name(n))
            .filter(|c| {
                if phone {
                    matches!(c, TableCol::Name | TableCol::Size | TableCol::Seeds)
                } else {
                    match c {
                        TableCol::Name | TableCol::Seeds => true,
                        TableCol::Tracker => col_shown.0,
                        TableCol::Size => col_shown.1,
                        TableCol::Leech => col_shown.2,
                        TableCol::Ratio => col_shown.3,
                        TableCol::Health => col_shown.4,
                        TableCol::Date => col_shown.5,
                    }
                }
            })
            .collect();

        // egui_extras' table always sums to the width it is given and its own
        // scroll area is `ScrollArea::new([false, vscroll])` — no horizontal
        // scroll. The widths therefore live HERE (`UiState::col_w`), the table is
        // built from them with `Column::exact`, and a wrapping ScrollArea scrolls
        // whatever does not fit. That is what makes a drag change one column and
        // nothing else: egui_extras' own handles would squeeze the remainder.
        let sig = cols
            .iter()
            .fold(0u64, |h, c| h.wrapping_mul(31).wrapping_add(*c as u64 + 1));
        if self.ui.col_w_sig != Some(sig) || self.ui.col_w.len() != cols.len() {
            // Never below the floor: egui_extras builds a `min..=max` range from
            // exact+at_least and panics when a column's seed width is under it.
            // Per-column, keyed by name: a hidden or reordered column keeps its
            // width, and a config written before this existed just has no keys.
            self.ui.col_w = cols
                .iter()
                .map(|c| {
                    self.cfg
                        .col_widths
                        .get(c.key())
                        .map(|w| crate::config::clamp_col_width(*w))
                        .unwrap_or(c.width().max(COL_MIN_W))
                })
                .collect();
            self.ui.col_w_sig = Some(sig);
            self.ui.col_drag = None;
        }
        let viewport_w = ui.available_width();
        // Name stretches into the leftover space until the user resizes it, so a
        // fresh config still fills the window; once `name` has a stored width the
        // table is Excel-exact and may leave space on the right.
        let mut widths = self.ui.col_w.clone();
        let scroll_bar_w = ui.spacing().scroll.bar_inner_margin
            + ui.spacing().scroll.bar_outer_margin
            + ui.spacing().scroll.bar_width;
        if phone {
            // Stored widths are a desktop preference ("Excel-exact"); applied to
            // a 412 pt window they overflow it and clip every title, so phone
            // width fits the three columns it keeps to the viewport instead.
            let (name_w, size_w, seeds_w) = phone_col_widths(viewport_w - scroll_bar_w, action_w);
            for (i, c) in cols.iter().enumerate() {
                widths[i] = match c {
                    TableCol::Name => name_w,
                    TableCol::Size => size_w,
                    TableCol::Seeds => seeds_w,
                    _ => widths[i],
                };
            }
        }
        let base_now: f32 = widths.iter().sum::<f32>() + action_w;
        if !self.cfg.col_widths.contains_key(TableCol::Name.key())
            && let Some(i) = cols.iter().position(|c| *c == TableCol::Name)
            && (viewport_w - base_now) > 0.0
        {
            widths[i] += viewport_w - base_now;
        }
        // The horizontal bar is drawn just inside the scroll area, below the
        // table: reserve its thickness or the table fills the panel and the bar
        // is clipped away (which is exactly how it looked — no bar at all).
        // Horizontal scrollbar at the bottom of the scroll area gets clipped if the
        // table fills the panel. Give it 20px breathing room.
        let viewport_h = table_viewport_h(ui.available_height());
        // Handle band = the header row, measured from the cursor BEFORE the table
        // exists (the table's own rect extends past the scrollbar, so subtracting
        // a guessed margin from it kept landing on top of the bar).
        let header_top = ui.cursor().top();
        #[allow(unused_assignments)] // assigned inside the scroll closure, read after it
        let mut table_rect = egui::Rect::NOTHING;
        // egui_extras squeezes columns to the viewport (each column is a min..=max
        // range), so the RENDERED width is not the stored one. Dividers and grab
        // bands must sit where the columns actually landed — positioning them from
        // the stored widths put every handle in the wrong place, which is why the
        // lines looked draggable and were not. Filled by the header closure.
        let mut col_rects: Vec<egui::Rect> = Vec::with_capacity(cols.len());
        // `widths`, not the stored self.ui.col_w: on a phone the stored widths
        // are overridden above, and using them here would keep the table wider
        // than the window and bring the horizontal bar back.
        let base_w: f32 = widths.iter().sum::<f32>() + action_w;
        // Phone: fit the scroll area, so no horizontal bar. Desktop keeps the
        // Excel-exact width and scrolls.
        let table_w = if phone {
            base_w.max(viewport_w - scroll_bar_w)
        } else {
            base_w.max(viewport_w)
        };
        // The table needs a bar you can see and grab. egui's default
        // `ScrollStyle::floating()` draws a dormant 2px line in the handle colour
        // (here: the border colour) that only grows while hovered, so a table
        // wider than the window looked like it had no scrollbar at all. Solid
        // bars always allocate their 6px — HSCROLL_ROOM already reserves 20.
        let style_before = ui.spacing().scroll;
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        // Solid bars take their handle colour from `widgets.inactive.bg_fill`,
        // which this theme sets to `surface` — on the light themes that is within
        // ~1.1:1 of the panel background, so the bar is a 6px band you cannot
        // see. Use `dim` for the handle here only: the table's own widgets get
        // `surface` back at the top of the closure below.
        let inactive_before = ui.visuals().widgets.inactive.bg_fill;
        let hovered_before = ui.visuals().widgets.hovered.bg_fill;
        ui.visuals_mut().widgets.inactive.bg_fill = pal.dim;
        ui.visuals_mut().widgets.hovered.bg_fill = pal.sub;
        // Hand-formatted closure body — #[rustfmt::skip] keeps the compact
        // single-level indentation instead of rustfmt's +1 level reflow.
        #[rustfmt::skip]
        // Phone width fits the columns to the window, so a horizontal bar there
        // is pure noise (and it reappeared no matter how the reserve was
        // computed — the scroll area's own margins differ from available_width).
        let sa = if phone {
            egui::ScrollArea::vertical()
        } else {
            egui::ScrollArea::both()
        };
        let sa_out = sa
            .id_salt("results_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // Content keeps the normal widget fills; only the bars use `dim`.
                ui.visuals_mut().widgets.inactive.bg_fill = inactive_before;
                ui.visuals_mut().widgets.hovered.bg_fill = hovered_before;
                // Both bounds: a min_width alone leaves the table free to report a
                // narrower content rect, so the ScrollArea never saw the overflow and
                // never drew its bar. With a definite width the columns (all exact) fit
                // exactly and the scroll area measures the real width.
                ui.set_min_width(table_w);
                ui.set_max_width(table_w);
                ui.set_max_height(viewport_h);
                let mut tb = TableBuilder::new(ui)
                    .striped(false)
                    .resizable(false)
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
                // Every column has an owned, exact width: no remainder, so nothing
                // compensates for a resize. egui_extras' own drag handles are off — this
                // file draws its own at the boundaries below the table body.
                for (i, _c) in cols.iter().enumerate() {
                    tb = tb.column(
                        Column::exact(widths.get(i).copied().unwrap_or(120.0))
                            .at_least(COL_MIN_W)
                            .clip(true),
                    );
                }
                // Column::at_least must not exceed Column::exact, or egui_extras panics.
                tb = tb.column(
                    Column::exact(action_w)
                        .at_least(action_w.min(150.0))
                        .clip(true),
                ); // Actions
                tb.header(30.0, |mut header| {
                    new_sort = self.results_header(
                        &mut header,
                        &cols,
                        &pal,
                        fsz,
                        &s_col,
                        &s_dir,
                        &mut col_rects,
                    );
                })
                .body(|mut body| {
                    self.results_rows(
                        &mut body,
                        page_s,
                        base,
                        &cols,
                        &pal,
                        rh,
                        fsz,
                        sel,
                        det_open,
                        phone,
                        &mut actions,
                    );
                });
            }); // end ScrollArea::both
        ui.spacing_mut().scroll = style_before;
        ui.visuals_mut().widgets.inactive.bg_fill = inactive_before;
        ui.visuals_mut().widgets.hovered.bg_fill = hovered_before;

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
        self.run_row_actions(ui, page_s, actions);

        // Column separators, drawn after the body so they sit on top of every
        // cell's click layer.
        table_rect = ui.min_rect();
        // The divider is painted full height, so the grab band must be full
        // height too — a header-only strip made every line below the header look
        // draggable and do nothing. It stops above the reserved scrollbar room:
        // a band that reached it swallowed the horizontal scrollbar's drags.
        let band_bottom = (table_rect.bottom() - HSCROLL_ROOM).max(header_top + COL_HANDLE_H);
        // The divider belongs to the list, not to the whole scroll viewport:
        // `table_rect` spans the viewport, so a rule drawn to its bottom ran on
        // past the last row into empty space. `content_size` is header + rows.
        let list_bottom = (header_top + sa_out.content_size.y).min(band_bottom);
        // egui_extras shrinks the columns to the viewport, so a pointer delta of
        // 10 px must move a stored width by 10 * (stored / rendered) or the line
        // trails the cursor. 1.0 when nothing is squeezed.
        let rendered_total: f32 = col_rects.iter().map(|r| r.width()).sum();
        let drag_scale = if rendered_total > 1.0 {
            let stored_total: f32 = widths.iter().sum();
            (stored_total / rendered_total).max(1.0)
        } else {
            1.0
        };
        let mut guide: Option<f32> = None;
        for (i, w) in widths.iter().enumerate() {
            // Right edge of the column as drawn; the stored sum is only a fallback
            // for the frame before the header reports its rects.
            let x = col_rects
                .get(i)
                .map(|r| r.right())
                .unwrap_or_else(|| table_rect.left() + widths[..=i].iter().sum::<f32>());
            let handle = egui::Rect::from_min_max(
                egui::pos2(x - COL_GRAB_W, header_top),
                egui::pos2(x + COL_GRAB_W, list_bottom),
            );
            let resp = ui.interact(
                handle,
                egui::Id::unique(("colhandle", i)),
                egui::Sense::click_and_drag(),
            );
            // No permanent grid. A full-height rule at every boundary turned the
            // table into a spreadsheet, with lines running on past the last row.
            // The divider now exists only while the pointer is on the boundary
            // (thin, `sub`) or dragging it (accent, plus the rubber band below);
            // the resize cursor is what advertises the grab when none is drawn.
            let live = resp.hovered() || resp.dragged();
            if live {
                ui.painter().vline(
                    x,
                    egui::Rangef::new(header_top, list_bottom),
                    egui::Stroke::new(
                        if resp.dragged() { 2.0 } else { 1.0 },
                        if resp.dragged() { pal.accent } else { pal.sub },
                    ),
                );
            }
            if live {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeColumn);
            }
            if resp.drag_started()
                && let Some(start) = ui.input(|i| i.pointer.press_origin())
            {
                // Measure from the press point, not from where the pointer had
                // already travelled by the first drag frame: otherwise the edge
                // lands a few px behind the rubber band for the whole drag.
                self.ui.col_drag = Some((i, start.x, *w));
            }
            if resp.dragged()
                && let (Some(pos), Some((ci, start_x, start_w))) =
                    (ui.ctx().pointer_latest_pos(), self.ui.col_drag)
                && ci == i
            {
                let w = (start_w + (pos.x - start_x) * drag_scale).clamp(COL_MIN_W, COL_MAX_W);
                if let Some(slot) = self.ui.col_w.get_mut(i) {
                    *slot = w;
                }
                // The dragged edge is the rubber band: it follows the pointer,
                // and the guide below makes the grab legible when the live edge
                // sits under the cursor.
                guide = Some(pos.x);
            }
            if resp.drag_stopped() {
                self.ui.col_drag = None;
                // Persist once per drag, not per frame: `save_cfg` does a fsync +
                // rename, which is far too heavy for a 60 fps path.
                self.ui.col_w = widths.clone();
                self.cfg.col_widths = cols
                    .iter()
                    .zip(widths.iter())
                    .map(|(c, w)| (c.key().to_string(), *w))
                    .collect();
                if let Err(e) = crate::config::save_cfg(&self.cfg) {
                    eprintln!("save column widths: {e}");
                }
            }
        }

        // Rubber band: a full-height accent line at the pointer, plus a grip tab
        // at the header, drawn after every divider on the foreground layer.
        if let Some(gx) = guide {
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::unique("col_drag_guide"),
            ));
            painter.vline(
                gx,
                egui::Rangef::new(header_top, list_bottom),
                egui::Stroke::new(2.0_f32, pal.accent),
            );
            painter.rect_filled(
                egui::Rect::from_center_size(
                    egui::pos2(gx, header_top + 15.0),
                    egui::vec2(26.0, 5.0),
                ),
                R_SM,
                pal.accent,
            );
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

    /// Column header row: label, sort arrow, click-to-sort, and the screen rect
    /// of every column (declaration order) for the divider hit-tests.
    #[allow(clippy::too_many_arguments)]
    fn results_header(
        &self,
        header: &mut egui_extras::TableRow<'_, '_>,
        cols: &[TableCol],
        pal: &Pal,
        fsz: f32,
        s_col: &SortCol,
        s_dir: &SortDir,
        col_rects: &mut Vec<egui::Rect>,
    ) -> Option<(SortCol, bool)> {
        let mut new_sort: Option<(SortCol, bool)> = None;
        // Table header helper — label + SVG sort arrow (no glyphs).
        let hdr = |ui: &mut egui::Ui, l: &str, col: &SortCol| {
            let on = s_col == col;
            let col_c = if on { pal.accent } else { pal.sub };
            ui.label(
                RichText::new(l)
                    .font(FontId::proportional(fsz))
                    .color(col_c)
                    .strong(),
            );
            if on {
                let icon = if *s_dir == SortDir::Desc {
                    SvgIcon::SortDesc
                } else {
                    SvgIcon::SortAsc
                };
                svg_icon(ui, icon, ICON_XS, pal.accent);
            }
        };
        for c in cols {
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
                col_rects.push(ui.max_rect());
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
                        egui::Id::unique(("sort_hdr", c.label())),
                        egui::Sense::click(),
                    );
                    hdr(ui, c.label(), &sortcol);
                    if hresp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if hresp.clicked() {
                        new_sort = Some((sortcol.clone(), *s_col == sortcol));
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
        new_sort
    }

    /// One page of result rows: hover/selection band, per-column cells, the
    /// always-visible action buttons, and the right-click menu.
    #[allow(clippy::too_many_arguments)]
    fn results_rows(
        &mut self,
        body: &mut egui_extras::TableBody<'_>,
        page_s: &[TorrentResult],
        base: usize,
        cols: &[TableCol],
        pal: &Pal,
        rh: f32,
        fsz: f32,
        sel: Option<usize>,
        det_open: bool,
        phone: bool,
        actions: &mut Vec<(usize, &'static str)>,
    ) {
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
                            let hovered = ui
                                .ctx()
                                .pointer_latest_pos()
                                .is_some_and(|p| p.y >= band.top() && p.y < band.top() + rh);
                            if hovered {
                                self.ui.hovered = Some(i);
                                let a = pal.row_hov.a() as f32 / 255.0;
                                bg = Color32::from_rgba_unmultiplied(
                                    (pal.row_hov.r() as f32 * a + base_bg.r() as f32 * (1.0 - a))
                                        as u8,
                                    (pal.row_hov.g() as f32 * a + base_bg.g() as f32 * (1.0 - a))
                                        as u8,
                                    (pal.row_hov.b() as f32 * a + base_bg.b() as f32 * (1.0 - a))
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
                                let cell_id = egui::Id::unique(("namecell", gi));
                                let cell_resp =
                                    ui.interact(cell_rect(ui), cell_id, egui::Sense::click());
                                // The title cell is the row's primary hit area, so
                                // the right-click menu lives here.
                                let fav = self.is_fav(&r.title);
                                attach_row_menu(&cell_resp, i, r, fav, actions);
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
                                        if v_checkbox(ui, checked, "", self.pal.accent).clicked() {
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
                                                .color(if is_sel { pal.accent } else { pal.text }),
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
                                let cell_id = egui::Id::unique(("rowcell", gi, c.label()));
                                let cell_resp =
                                    ui.interact(cell_rect(ui), cell_id, egui::Sense::click());
                                if cell_resp.clicked() && !self.ui.sel_mode {
                                    actions.push((i, "select"));
                                }
                                if cell_resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                // Same right-click menu as the title cell — this
                                // cell already owns the full cell rect, so no
                                // extra hit-test layer is needed.
                                attach_row_menu(&cell_resp, i, r, self.is_fav(&r.title), actions);
                                draw_cell_content(ui, c, r, seed, leech, fsz, pal);
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
                            && svg_btn(ui, SvgIcon::Magnet, "Open in torrent client", pal.accent)
                        {
                            actions.push((i, "mag"));
                        }
                        if has_mag
                            && !phone
                            && svg_btn(ui, SvgIcon::Copy, "Copy magnet link", pal.sub)
                        {
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
                        if !phone
                            && svg_btn(
                                ui,
                                SvgIcon::Info,
                                "Detail panel (D)",
                                if is_sel && det_open {
                                    pal.accent
                                } else {
                                    pal.sub
                                },
                            )
                        {
                            actions.push((i, "info"));
                        }
                        let cell_id = egui::Id::unique(("rowhov", gi));
                        let _hover_resp = ui.interact(ui.max_rect(), cell_id, egui::Sense::hover());
                    });
                });
                // Right-click anywhere on the row: the actions are queued for
                // the shared handler below, so the menu needs no state of its own.
            });
        }
    }

    /// Apply the row actions queued while the table was built. Kept out of the
    /// table closure so no handler runs while the table still holds its borrow.
    fn run_row_actions(
        &mut self,
        ui: &egui::Ui,
        page_s: &[TorrentResult],
        actions: Vec<(usize, &'static str)>,
    ) {
        for (i, action) in actions {
            if action == "hover" {
                continue;
            } // already handled
            if let Some(r) = page_s.get(i).cloned() {
                match action {
                    "select" => {
                        if self.ui.selected == Some(i) && self.ui.detail_open {
                            self.close_detail();
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
                            self.pick_files(&r.title, &m, r.link.as_deref(), true);
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

#[cfg(test)]
mod viewport_tests {
    use super::*;

    /// The phone column set must fit the window — a single pixel of overflow
    /// brings back the horizontal bar (measured at 412 pt) — and no column may
    /// sit under the minimum, because that panics inside egui_extras.
    #[test]
    fn phone_columns_fit_the_window_and_respect_the_minimum() {
        for avail in [330.0, 360.5, 374.5, 480.0, 700.0] {
            let (name, size, seeds) = phone_col_widths(avail, ACTION_COL_W_PHONE);
            assert!(name >= COL_MIN_W, "name {name} at {avail}");
            assert!(size >= COL_MIN_W && seeds >= COL_MIN_W, "at {avail}");
            assert!(
                name + size + seeds + ACTION_COL_W_PHONE <= avail,
                "columns overflow {avail}: {}",
                name + size + seeds + ACTION_COL_W_PHONE
            );
        }
    }

    /// A short window (detail panel open) must not produce a negative height:
    /// that is what panicked in egui's layout pass.
    #[test]
    fn a_short_window_cannot_produce_a_negative_table_height() {
        assert_eq!(table_viewport_h(0.0), 1.0);
        assert_eq!(table_viewport_h(10.0), 1.0);
        assert_eq!(table_viewport_h(HSCROLL_ROOM), 1.0);
        assert!(table_viewport_h(500.0) > 400.0);
    }
}
