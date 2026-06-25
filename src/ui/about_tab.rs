use eframe::egui::{self, FontId, RichText, Stroke};
use crate::app::App;
use crate::ui::components::lbl;
use crate::theme::tint;

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let pal = app.pal.clone();
    let fs  = app.cfg.font_size;

    egui::ScrollArea::vertical().id_salt("about_scroll").show(ui, |ui| {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("TorrentX")
                .font(FontId::proportional(38.0)).strong().color(pal.accent));
            ui.label(RichText::new("v7  ·  Jackett-Powered Torrent Search")
                .font(FontId::proportional(15.0)).color(pal.dim));
            ui.add_space(28.0);

            ui.set_max_width(720.0);

            // Features card
            card(ui, &pal, fs, "Features", &[
                ("🔍", "Multi-indexer search", "Searches all your Jackett indexers simultaneously via the Torznab API."),
                ("📡", "RSS / Torznab feeds",  "Auto-refreshing feeds from any Jackett indexer — configure query, category, and refresh rate."),
                ("🎨", "19 themes",             "Tokyo Night, Cyberpunk, Dracula, Catppuccin, Nord, Gruvbox, and more."),
                ("⚡", "Instant filtering",    "Filter by text, seeders, size, year, tracker, or health — all client-side, instant."),
                ("★",  "Favorites",            "Save torrents with one click. Magnet links and download links saved locally."),
                ("📋", "Magnet & DL",           "Open magnets in your client, download .torrent files, or copy magnet URIs."),
                ("↕",  "Sortable table",        "Click any column header to sort. All columns are resizable."),
                ("📄", "CSV export",            "Export filtered results to CSV in one click."),
                ("🔑", "Search history",        "Remembers your last 20 searches with click-to-repeat."),
            ]);

            ui.add_space(20.0);

            // Keyboard shortcuts card
            card_table(ui, &pal, fs, "Keyboard Shortcuts", &[
                ("Ctrl+F",   "Focus search box"),
                ("Ctrl+R",   "Re-run search"),
                ("Enter",    "Search / open magnet"),
                ("↑ / ↓",   "Navigate results"),
                ("D",        "Toggle detail panel"),
                ("F",        "Add selected to Favorites"),
                ("M",        "Open magnet for selected"),
                ("Ctrl+C",   "Copy magnet (in detail panel)"),
                ("Esc",      "Close panel / clear query"),
            ]);

            ui.add_space(20.0);

            // RSS guide
            card_prose(ui, &pal, fs, "Setting up Jackett RSS Feeds", &[
                "1.  Start Jackett and add at least one indexer in the Jackett dashboard.",
                "2.  Copy your Jackett API key from the top-right of the Jackett UI.",
                "3.  In TorrentX Settings, paste the API key and confirm the URL (default: http://localhost:9117).",
                "4.  Go to the RSS Feeds tab and click  + Add Feed.",
                "5.  Enter a name, the indexer slug (or \"all\"), and optionally a search query and category IDs.",
                "6.  Enable  Auto-refresh  to have TorrentX poll the feed automatically.",
                "7.  Common Torznab category IDs:  2000=Movies  5000=TV  3000=Music  4000=PC  7000=Books",
            ]);

            ui.add_space(20.0);

            // Tips
            card_prose(ui, &pal, fs, "Tips & Tricks", &[
                "• Dedupe (Settings) collapses near-duplicate titles — useful when many indexers return the same torrent.",
                "• Use the Health filter to show only well-seeded (HOT / GOOD) results.",
                "• The ratio bar in the table shows seed/leech balance at a glance.",
                "• Resize columns by dragging the header separators.",
                "• Dark / light themes are remembered between sessions.",
                "• RSS feeds survive restarts — they auto-refresh after the configured interval.",
            ]);

            ui.add_space(20.0);

            // Footer
            lbl(ui, "Built with  egui · eframe · reqwest · quick-xml · Rust 🦀", pal.dim, fs - 1.0);
            ui.add_space(6.0);
            lbl(ui, "Config saved to  ~/.config/torrentx/config.json", pal.dim, fs - 2.0);
        });
    });
}

fn card(
    ui: &mut egui::Ui,
    pal: &crate::theme::Pal,
    fs: f32,
    title: &str,
    items: &[(&str, &str, &str)],
) {
    egui::Frame::NONE
        .fill(pal.surface).corner_radius(12.0)
        .stroke(Stroke::new(1.0, pal.border))
        .inner_margin(egui::Margin::same(18))
        .show(ui, |ui| {
            ui.label(RichText::new(title)
                .font(egui::FontId::proportional(fs + 2.0)).strong().color(pal.accent));
            ui.add_space(12.0);
            for (icon, header, detail) in items {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(*icon).font(egui::FontId::proportional(fs + 1.0)));
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(*header).font(egui::FontId::proportional(fs)).strong().color(pal.text));
                        ui.label(RichText::new(*detail).font(egui::FontId::proportional(fs - 1.5)).color(pal.sub));
                    });
                });
                ui.add_space(6.0);
            }
        });
}

fn card_table(
    ui: &mut egui::Ui,
    pal: &crate::theme::Pal,
    fs: f32,
    title: &str,
    rows: &[(&str, &str)],
) {
    egui::Frame::NONE
        .fill(pal.surface).corner_radius(12.0)
        .stroke(Stroke::new(1.0, pal.border))
        .inner_margin(egui::Margin::same(18))
        .show(ui, |ui| {
            ui.label(RichText::new(title)
                .font(egui::FontId::proportional(fs + 2.0)).strong().color(pal.accent));
            ui.add_space(12.0);
            egui::Grid::new("shortcuts_grid")
                .num_columns(2)
                .spacing([20.0, 8.0])
                .show(ui, |ui| {
                    for (key, desc) in rows {
                        egui::Frame::NONE
                            .fill(tint(pal.accent, 18))
                            .corner_radius(5.0)
                            .stroke(Stroke::new(1.0, tint(pal.accent, 60)))
                            .inner_margin(egui::Margin::symmetric(8, 2))
                            .show(ui, |ui| {
                                ui.label(RichText::new(*key)
                                    .font(egui::FontId::monospace(fs - 1.0))
                                    .color(pal.accent));
                            });
                        ui.label(RichText::new(*desc)
                            .font(egui::FontId::proportional(fs)).color(pal.text));
                        ui.end_row();
                    }
                });
        });
}

fn card_prose(
    ui: &mut egui::Ui,
    pal: &crate::theme::Pal,
    fs: f32,
    title: &str,
    lines: &[&str],
) {
    egui::Frame::NONE
        .fill(pal.surface).corner_radius(12.0)
        .stroke(Stroke::new(1.0, pal.border))
        .inner_margin(egui::Margin::same(18))
        .show(ui, |ui| {
            ui.label(RichText::new(title)
                .font(egui::FontId::proportional(fs + 2.0)).strong().color(pal.accent));
            ui.add_space(10.0);
            for line in lines {
                ui.label(RichText::new(*line)
                    .font(egui::FontId::proportional(fs - 0.5)).color(pal.sub));
                ui.add_space(3.0);
            }
        });
}
