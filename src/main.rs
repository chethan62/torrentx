#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

mod app;
mod config;
mod theme;
mod types;
mod search;
mod rss;
mod utils;
mod ui;

use eframe::egui;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "TorrentX",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("TorrentX")
                .with_inner_size([1380.0, 820.0])
                .with_min_inner_size([960.0, 580.0]),
            ..Default::default()
        },
        Box::new(|_cc| Box::new(app::App::default())),
    )
}
