//! Results table + detail panel drawing methods (split out for readability).

use super::*;

use crate::app::App;

impl App {
    pub(crate) fn draw_detail(&mut self, ui: &mut egui::Ui, r: &TorrentResult) {
        let fs = self.cfg.font_size;
        ui.add_space(SP4);
        ui.horizontal(|ui| {
            ui.add_space(SP4);
            lbl(ui, "Details", self.pal.text, fs + 2.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(SP3);
                if ui
                    .add(
                        egui::Button::new(svg_image(SvgIcon::Close, ICON_MD, self.pal.sub))
                            .fill(Color32::TRANSPARENT)
                            .corner_radius(R_SM),
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
        ui.add_space(SP3);

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
                ui.add_space(SP3);

                let cat = r.category_desc.as_deref().unwrap_or("Unknown");
                egui::Frame::NONE
                    .fill(tint(cat_col(cat), 25))
                    .corner_radius(PANEL_RADIUS)
                    .inner_margin(margin(SP3, SP1))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(cat)
                                .font(FontId::proportional(fs - 1.0))
                                .color(cat_col(cat)),
                        );
                    });
                ui.add_space(SP4);

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
                    ui.add_space(SP3);
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
                    ui.add_space(SP1);
                    lbl(
                        ui,
                        &format!("{:.0}% seeded", pct * 100.0),
                        self.pal.dim,
                        fs - 2.0,
                    );
                }

                // Show magnet link (truncated) if present
                if let Some(mag) = &r.magnet_uri {
                    ui.add_space(SP3);
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
                                .corner_radius(R_SM),
                            )
                            .on_hover_text("Click to copy full magnet")
                            .clicked()
                        {
                            ui.ctx().copy_text(mag.clone());
                            self.toast("Magnet copied", self.pal.green);
                        }
                    });
                }

                ui.add_space(SP5);
                lbl(ui, "Actions", self.pal.dim, fs - 1.0);
                ui.add_space(SP3);

                if let Some(mag) = r.magnet_uri.clone() {
                    let mc = mag.clone();
                    // Snapshot the picker: the widgets below borrow `self`, so the
                    // list (small) is cloned rather than borrowed across them.
                    let pick = self.ui.pick.clone().filter(|p| p.key == r.title);
                    let chosen: Option<usize> = pick
                        .as_ref()
                        .and_then(|p| p.sel.and_then(|i| p.files.get(i)))
                        .map(|f| f.idx);
                    if wide_icon_btn(ui, SvgIcon::Play, "Play", self.pal.accent) {
                        match crate::player::start_play(
                            &mag,
                            r.link.as_deref(),
                            &self.cfg.player,
                            chosen,
                        ) {
                            Ok(()) => self.toast("Starting stream…", self.pal.accent),
                            Err(e) => self.toast(&e, self.pal.red),
                        }
                    }
                    if crate::player::is_playing()
                        && wide_icon_btn(ui, SvgIcon::Close, "Stop stream", self.pal.red)
                    {
                        crate::player::stop_play();
                        self.toast("Stream stopped", self.pal.sub);
                        ui.add_space(SP2);
                    }
                    ui.add_space(SP2);
                    // Multi-file torrents (season packs) stream their largest file
                    // unless a file is chosen; the chooser handles long lists with
                    // a filter.
                    match &pick {
                        Some(p) if p.loading => {
                            lbl(ui, "Listing files…", self.pal.dim, fs - 1.0);
                        }
                        Some(p) if !p.files.is_empty() => {
                            let label = match p.sel.and_then(|i| p.files.get(i)) {
                                Some(f) => format!("{} ({})", short_name(&f.name), f.size),
                                None => format!("Largest file (of {})", p.files.len()),
                            };
                            if wide_icon_btn(ui, SvgIcon::Search, &label, self.pal.sub)
                                && let Some(st) = self.ui.pick.as_mut()
                            {
                                st.window = true;
                            }
                        }
                        Some(p) => {
                            if let Some(e) = &p.err {
                                lbl(ui, e, self.pal.red, fs - 1.5);
                            }
                            if wide_icon_btn(ui, SvgIcon::Refresh, "Retry file list", self.pal.sub)
                            {
                                self.pick_files(&r.title, &mag, r.link.as_deref());
                            }
                        }
                        None => {
                            if wide_icon_btn(
                                ui,
                                SvgIcon::Search,
                                "Choose episode/file…",
                                self.pal.sub,
                            ) {
                                self.pick_files(&r.title, &mag, r.link.as_deref());
                            }
                        }
                    }
                    ui.add_space(SP2);
                    if wide_icon_btn(ui, SvgIcon::Magnet, "Open Magnet", self.pal.accent) {
                        let _ = safe_open(mag);
                        self.toast("Opening magnet…", self.pal.accent);
                    }
                    ui.add_space(SP2);
                    if wide_icon_btn(ui, SvgIcon::Copy, "Copy Magnet Link", self.pal.sub) {
                        ui.ctx().copy_text(mc);
                        self.toast("Copied", self.pal.green);
                    }
                    ui.add_space(SP2);
                }
                if let Some(link) = r.link.clone() {
                    if wide_icon_btn(ui, SvgIcon::Download, "Download .torrent", self.pal.green) {
                        let _ = safe_open(link);
                        self.toast("Downloading…", self.pal.green);
                    }
                    ui.add_space(SP2);
                }
                if let Some(det) = r.details.clone() {
                    if wide_icon_btn(ui, SvgIcon::Web, "Open in Browser", self.pal.sub) {
                        let _ = safe_open(det);
                    }
                    ui.add_space(SP2);
                }
                let r2 = r.clone();
                if wide_icon_btn(ui, SvgIcon::Star, "Add to Favorites", self.pal.yellow) {
                    self.add_fav(&r2);
                }
            });

        // File chooser: drawn as a top-level window, but the list lives in
        // `ui.pick` so the choice survives closing it and the panel scrolling.
        if let Some(mut p) = self.ui.pick.clone()
            && p.window
            && !p.files.is_empty()
        {
            let mut open = true;
            egui::Window::new(format!("Files — {}", short_name(&p.key)))
                .id(egui::Id::new(("pick_win", p.key.clone())))
                .open(&mut open)
                .resizable(true)
                .collapsible(false)
                .default_width(560.0)
                .show(ui.ctx(), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut p.filter)
                            .hint_text("filter files…")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(SP2);
                    let needle = p.filter.to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(340.0)
                        .show(ui, |ui| {
                            if ui
                                .selectable_label(p.sel.is_none(), "Largest file (default)")
                                .clicked()
                            {
                                p.sel = None;
                                p.window = false;
                            }
                            for (i, f) in p.files.iter().enumerate() {
                                if !needle.is_empty() && !f.name.to_lowercase().contains(&needle) {
                                    continue;
                                }
                                if ui
                                    .selectable_label(
                                        p.sel == Some(i),
                                        format!("{:>3}  {}  ({})", f.idx, f.name, f.size),
                                    )
                                    .clicked()
                                {
                                    p.sel = Some(i);
                                    p.window = false;
                                }
                            }
                        });
                });
            if !open {
                p.window = false;
            }
            self.ui.pick = Some(p);
        }
    }
}

/// Short middle/start-trimmed file name for the picker's closed state: the
/// distinguishing part of an episode name is usually its tail.
fn short_name(n: &str) -> String {
    const MAX: usize = 34;
    if n.chars().count() <= MAX {
        return n.to_string();
    }
    let tail: String = n
        .chars()
        .skip(n.chars().count().saturating_sub(MAX - 1))
        .collect();
    format!("…{tail}")
}
