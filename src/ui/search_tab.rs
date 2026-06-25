use eframe::egui::{self, Color32, FontId, RichText, Stroke};
use crate::app::App;
use crate::types::{SearchState, CATS};
use crate::ui::{components::{lbl, outline_btn}, filter_bar, results_table, detail_panel};

pub fn draw(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context, state: &SearchState) {
    let fs = app.cfg.font_size;
    let busy = *state == SearchState::Searching;

    ui.add_space(10.0);
    let mut bar_rect = egui::Rect::NOTHING;

    // ── Search bar ────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        let resp = ui.add(
            egui::TextEdit::singleline(&mut app.query)
                .id(egui::Id::new("search_query"))
                .desired_width(ui.available_width() - 310.0)
                .hint_text("Search torrents — movies, shows, games, software, anime…")
                .font(FontId::proportional(fs + 2.0)),
        );
        bar_rect = resp.rect;
        if resp.gained_focus() && !app.cfg.history.is_empty() { app.show_hist = true; }
        if resp.changed() && app.query.is_empty() { app.show_hist = false; }
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) { app.do_search(); }

        ui.add_space(6.0);
        egui::ComboBox::from_id_source("cat_combo")
            .selected_text(RichText::new(&app.search_cat).font(FontId::proportional(fs)))
            .width(115.0)
            .show_ui(ui, |ui| {
                for &c in CATS {
                    ui.selectable_value(&mut app.search_cat, c.into(),
                        RichText::new(c).font(FontId::proportional(fs)));
                }
            });

        ui.add_space(6.0);
        if ui.add_enabled(!busy,
            egui::Button::new(
                RichText::new(if busy { "  Scanning…  " } else { "    Search    " })
                    .font(FontId::proportional(fs)).strong().color(Color32::WHITE))
                .fill(if busy { crate::theme::rgb(6,100,130) } else { app.pal.accent })
                .rounding(6.0)
                .min_size(egui::Vec2::new(0.0, 36.0))
        ).clicked() { app.do_search(); }

        if !app.query.is_empty() {
            if ui.add(egui::Button::new(RichText::new("✕").size(13.0).color(app.pal.sub))
                .fill(Color32::TRANSPARENT).rounding(4.0)).on_hover_text("Clear").clicked() {
                app.query.clear();
                app.show_hist = false;
            }
        }
    });

    draw_history_dropdown(app, ctx, bar_rect, fs);
    ui.add_space(8.0);
    filter_bar::draw(app, ui);
    ui.add_space(8.0);

    // ── State-specific content ────────────────────────────────────────────
    match state {
        SearchState::Idle => draw_idle(app, ui),

        SearchState::Searching => {
            ui.add_space(70.0);
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(12.0);
                lbl(ui, "Scanning all Jackett indexers…", app.pal.sub, 16.0);
                ui.add_space(4.0);
                lbl(ui, "This usually takes 10–30 seconds", app.pal.dim, fs);
            });
        }

        SearchState::Error(err) => {
            ui.add_space(10.0);
            let err = err.clone();
            egui::Frame::none()
                .fill(crate::theme::tint(app.pal.red, 10))
                .stroke(Stroke::new(1.0, crate::theme::tint(app.pal.red, 70)))
                .rounding(8.0)
                .inner_margin(egui::Margin::symmetric(20.0, 14.0))
                .outer_margin(egui::Margin::symmetric(12.0, 0.0))
                .show(ui, |ui| {
                    for line in err.lines() {
                        lbl(ui, line, app.pal.red, fs);
                    }
                    ui.add_space(8.0);
                    let accent = app.pal.accent;
                    if outline_btn(ui, "Open Settings", accent) { app.show_settings = true; }
                });
        }

        SearchState::Done => draw_results(app, ui, ctx),
    }
}

// ─── Idle / welcome ────────────────────────────────────────────────────────

fn draw_idle(app: &mut App, ui: &mut egui::Ui) {
    let fs = app.cfg.font_size;
    ui.add_space(50.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("TorrentX")
            .font(FontId::proportional(40.0)).strong()
            .color(crate::theme::tint(app.pal.accent, 90)));
        ui.add_space(6.0);
        lbl(ui, "Search all your Jackett indexers in one shot", app.pal.sub, fs + 1.0);
        ui.add_space(3.0);
        lbl(ui, "Movies  ·  TV  ·  Music  ·  Games  ·  Software  ·  Anime  ·  Books",
            app.pal.dim, fs - 1.0);
        ui.add_space(28.0);

        if !app.cfg.history.is_empty() {
            lbl(ui, "Recent searches", app.pal.dim, fs - 1.0);
            ui.add_space(10.0);
            let hist: Vec<_> = app.cfg.history.iter().take(12).cloned().collect();
            let mut clicked: Option<String> = None;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                for h in &hist {
                    if ui.add(egui::Button::new(
                        RichText::new(h.as_str()).font(FontId::proportional(fs)).color(app.pal.sub))
                        .fill(app.pal.surface)
                        .stroke(Stroke::new(1.0, app.pal.border))
                        .rounding(14.0).min_size(egui::vec2(0.0, 28.0))
                    ).clicked() { clicked = Some(h.clone()); }
                }
            });
            if let Some(h) = clicked { app.query = h; app.do_search(); }
        } else {
            // First-time help box
            ui.add_space(24.0);
            egui::Frame::none()
                .fill(crate::theme::tint(app.pal.accent, 12)).rounding(10.0)
                .stroke(Stroke::new(1.0, crate::theme::tint(app.pal.accent, 50)))
                .inner_margin(egui::Margin::symmetric(24.0, 16.0))
                .show(ui, |ui| {
                    ui.set_max_width(480.0);
                    ui.label(RichText::new("Getting started")
                        .font(FontId::proportional(fs + 1.0)).color(app.pal.accent).strong());
                    ui.add_space(6.0);
                    lbl(ui, "1. Make sure Jackett is running  (localhost:9117)", app.pal.sub, fs - 1.0);
                    lbl(ui, "2. Click ⚙ Settings and paste your API key", app.pal.sub, fs - 1.0);
                    lbl(ui, "3. Search for anything!", app.pal.sub, fs - 1.0);
                    lbl(ui, "4. Add RSS Feeds for auto-updated torrent streams", app.pal.sub, fs - 1.0);
                    ui.add_space(10.0);
                    let accent = app.pal.accent;
                    if crate::ui::components::outline_btn(ui, "Open Settings", accent) {
                        app.show_settings = true;
                    }
                });
        }
    });
}

// ─── Results view ─────────────────────────────────────────────────────────

fn draw_results(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    let fs = app.cfg.font_size;
    let raw    = app.all_results();
    let sorted = app.filtered(&raw);
    let total  = sorted.len();

    app.selected = app.selected.filter(|&i| i < total);
    if app.selected.is_none() { app.detail_open = false; }

    if total == 0 {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            lbl(ui, "No results match your filters", app.pal.sub, 17.0);
            if !raw.is_empty() {
                lbl(ui, &format!("{} results hidden by filters", raw.len()), app.pal.dim, fs);
            }
        });
        return;
    }

    let max_p  = app.max_pages(total);
    let page_s = app.page_slice(&sorted).to_vec();
    let page_n = page_s.len();

    // ── Stats bar ─────────────────────────────────────────────────────────
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        let active: usize  = sorted.iter().filter(|r| r.seeders.unwrap_or(0) > 0).count();
        let seeds: u32     = sorted.iter().map(|r| r.seeders.unwrap_or(0)).sum();
        let trackers: std::collections::HashSet<_> =
            sorted.iter().filter_map(|r| r.tracker.as_deref()).collect();
        lbl(ui, &format!("Showing {page_n} of {total}  ·  {active} active  ·  \
                          {seeds} seeds  ·  {} trackers", trackers.len()),
            app.pal.sub, fs - 1.0);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(12.0);
            let sc = sorted.clone();
            if outline_btn(ui, "Export CSV", app.pal.sub) {
                app.export_csv(&sc);
                let green = app.pal.green;
                app.toast("Exported to Downloads ✓", green);
            }
        });
    });

    // ── Category chips ───────────────────────────────────────────────────
    if app.cfg.show_cat_bar {
        let chips = App::cat_chips(&sorted);
        if !chips.is_empty() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                for (cat, count, col) in &chips {
                    let sel = app.filters.text == *cat;
                    egui::Frame::none()
                        .fill(crate::theme::tint(*col, if sel { 50 } else { 20 })).rounding(10.0)
                        .stroke(Stroke::new(if sel { 1.5 } else { 1.0 },
                            crate::theme::tint(*col, if sel { 200 } else { 80 })))
                        .inner_margin(egui::Margin::symmetric(7.0, 2.0))
                        .show(ui, |ui| {
                            if ui.add(egui::Label::new(
                                RichText::new(format!("{cat}  {count}"))
                                    .font(FontId::proportional(11.0)).color(*col)
                            ).sense(egui::Sense::click()))
                                .on_hover_text("Click to filter by category").clicked() {
                                if app.filters.text == *cat { app.filters.text.clear(); }
                                else { app.filters.text = cat.clone(); }
                            }
                        });
                    ui.add_space(3.0);
                }
            });
        }
    }
    ui.add_space(4.0);

    // ── Keyboard nav ──────────────────────────────────────────────────────
    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
        app.selected     = Some(app.selected.map_or(0, |s| (s + 1).min(page_n.saturating_sub(1))));
        app.detail_open  = true;
    }
    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
        app.selected     = Some(app.selected.map_or(0, |s| s.saturating_sub(1)));
        app.detail_open  = true;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        if let Some(i) = app.selected {
            if let Some(r) = page_s.get(i) {
                if let Some(m) = &r.magnet_uri {
                    let _ = open::that(m);
                    let accent = app.pal.accent;
                    app.toast("Opening magnet…", accent);
                }
            }
        }
    }
    if ui.input(|i| i.key_pressed(egui::Key::D)) {
        if app.selected.is_some() { app.detail_open = !app.detail_open; }
    }
    if ui.input(|i| i.key_pressed(egui::Key::F)) {
        if let Some(i) = app.selected {
            if let Some(r) = page_s.get(i).cloned() { app.add_fav(&r); }
        }
    }
    if ui.input(|i| i.key_pressed(egui::Key::M)) {
        if let Some(i) = app.selected {
            if let Some(r) = page_s.get(i) {
                if let Some(m) = &r.magnet_uri {
                    let _ = open::that(m);
                    let accent = app.pal.accent;
                    app.toast("Opening magnet…", accent);
                }
            }
        }
    }
    if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C) && app.detail_open) {
        if let Some(i) = app.selected {
            if let Some(r) = page_s.get(i) {
                if let Some(m) = &r.magnet_uri {
                    ctx.output_mut(|o| o.copied_text = m.clone());
                    let green = app.pal.green;
                    app.toast("Magnet copied ✓", green);
                }
            }
        }
    }

    // ── Pagination ────────────────────────────────────────────────────────
    if max_p > 1 {
        egui::TopBottomPanel::bottom("pages")
            .exact_height(34.0)
            .frame(egui::Frame::none().fill(app.pal.bg)
                .stroke(Stroke::new(1.0, app.pal.border))
                .inner_margin(egui::Margin::symmetric(12.0, 5.0)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let pg = app.page;
                    if ui.add_enabled(pg > 0, egui::Button::new(
                        RichText::new("← Prev").font(FontId::proportional(fs - 1.0)).color(app.pal.sub))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0, app.pal.border)).rounding(4.0)
                    ).clicked() { app.page -= 1; app.selected = None; }
                    ui.add_space(6.0);
                    for p in 0..max_p {
                        let near = p == 0 || p == max_p - 1 || p.abs_diff(pg) <= 2;
                        if !near {
                            if p == 1 || p == max_p - 2 {
                                lbl(ui, "…", app.pal.dim, fs - 1.0);
                            }
                            continue;
                        }
                        let on = p == pg;
                        if ui.add(egui::SelectableLabel::new(on,
                            RichText::new(format!("{}", p + 1))
                                .font(FontId::proportional(fs - 1.0))
                                .color(if on { app.pal.accent } else { app.pal.sub })
                        )).clicked() { app.page = p; app.selected = None; }
                    }
                    ui.add_space(6.0);
                    if ui.add_enabled(pg + 1 < max_p, egui::Button::new(
                        RichText::new("Next →").font(FontId::proportional(fs - 1.0)).color(app.pal.sub))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0, app.pal.border)).rounding(4.0)
                    ).clicked() { app.page += 1; app.selected = None; }
                    lbl(ui, &format!("  Page {} of {max_p}", pg + 1), app.pal.dim, fs - 1.0);
                });
            });
    }

    // ── Detail panel ──────────────────────────────────────────────────────
    if app.detail_open {
        if let Some(idx) = app.selected {
            if let Some(r) = page_s.get(idx).cloned() {
                egui::SidePanel::right("detail_panel")
                    .resizable(true).default_width(295.0).min_width(240.0)
                    .frame(egui::Frame::none()
                        .fill(app.pal.surface)
                        .stroke(Stroke::new(1.0, app.pal.border)))
                    .show_inside(ui, |ui| { detail_panel::draw(app, ui, &r); });
            }
        }
    }

    results_table::draw(app, ui, &page_s);
}

// ─── History dropdown ─────────────────────────────────────────────────────

fn draw_history_dropdown(
    app: &mut App, ctx: &egui::Context, bar_rect: egui::Rect, fs: f32,
) {
    if !app.show_hist || app.cfg.history.is_empty() { return; }
    let pos = egui::pos2(bar_rect.min.x, bar_rect.max.y + 4.0);
    let w   = bar_rect.width();
    let hist = app.cfg.history.clone();
    let mut clicked: Option<String> = None;
    let mut deleted: Option<String> = None;
    let mut clear_all = false;

    egui::Area::new(egui::Id::new("history_dropdown"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(app.pal.surface).rounding(8.0)
                .stroke(Stroke::new(1.0, app.pal.accent))
                .shadow(egui::epaint::Shadow { offset: [0.0,4.0].into(), blur:12.0, spread:0.0,
                    color: crate::theme::rgba(0,0,0,70) })
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(w.max(280.0));
                    ui.horizontal(|ui| {
                        lbl(ui, "Recent searches", app.pal.dim, 11.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("clear all").size(11.0).color(app.pal.dim))
                                .fill(Color32::TRANSPARENT).frame(false)).clicked() {
                                clear_all = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    for h in hist.iter().take(10) {
                        ui.horizontal(|ui| {
                            if ui.add(egui::Button::new(
                                RichText::new(h.as_str()).font(FontId::proportional(fs)).color(app.pal.text))
                                .fill(Color32::TRANSPARENT).frame(false)
                                .min_size(egui::vec2(w.max(280.0) - 50.0, 26.0))
                            ).clicked() { clicked = Some(h.clone()); }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new("✕").size(10.0).color(app.pal.dim))
                                    .fill(Color32::TRANSPARENT).frame(false)
                                    .min_size(egui::vec2(18.0, 18.0))
                                ).on_hover_text("Remove").clicked() {
                                    deleted = Some(h.clone());
                                }
                            });
                        });
                    }
                });
        });

    if let Some(h) = clicked { app.query = h; app.show_hist = false; app.do_search(); }
    if let Some(h) = deleted {
        app.cfg.history.retain(|x| x != &h);
        crate::config::save_cfg(&app.cfg);
    }
    if clear_all {
        app.cfg.history.clear();
        crate::config::save_cfg(&app.cfg);
        app.show_hist = false;
    }
}
