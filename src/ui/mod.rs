//! UI drawing — split into per-screen modules. Shared imports, constants,

//! and helper widgets re-exported here so submodules can `use super::*;`.


mod about;
mod favorites;
mod header;
mod rss;
mod search;
mod widgets;


pub(crate) use crate::{act_btn, grid_row, labeled_input, lbl, outline_btn, outline_icon_btn, status_pill, svg_btn, svg_icon, svg_image, v_checkbox, wide_btn, wide_icon_btn, SvgIcon, CATS, MARGIN_DEFAULT};
pub(crate) use crate::config::{save_cfg, ROW_HEIGHT_COMPACT, ROW_HEIGHT_NORMAL, ROW_HEIGHT_ROOMY};
pub(crate) use crate::jackett::{cat_col, fmt_size, hlth_lbl, is_magnet, seed_col, time_ago, Hlth, SearchState, SortCol, SortDir, Tab, TableCol, TorrentResult};
pub(crate) use crate::rss::{FeedStatus, RssFeedConfig, RssFeedState, RssItem};
pub(crate) use crate::themes::{rgb, rgba, tint, Pal, Theme};
pub(crate) use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
pub(crate) use egui_extras::{Column, TableBuilder};


// ─── UI tuning constants ───────────────────────────────────────────────────
/// Filter-bar input widths (px), in row order.
pub(crate) const FILTER_TEXT_W: f32 = 115.0; // "within results"
pub(crate) const FILTER_NUM_W: f32 = 38.0;   // Seeds ≥ / Max GB
pub(crate) const FILTER_YEAR_W: f32 = 44.0;  // Year ≥
pub(crate) const FILTER_TRK_W: f32 = 86.0;   // Tracker
/// Settings-panel input widths (px).
pub(crate) const SETTINGS_URL_W: f32 = 172.0;
pub(crate) const SETTINGS_KEY_W: f32 = 210.0;
pub(crate) const SETTINGS_SMALL_W: f32 = 40.0;
/// RSS form input width (px).
pub(crate) const RSS_FORM_W: f32 = 260.0;
/// Favorites search input width (px).
pub(crate) const FAV_SEARCH_W: f32 = 220.0;
/// Standard panel corner radius (px).
pub(crate) const PANEL_RADIUS: f32 = 8.0;
/// Standard panel side margin (px, i8 for egui Margin).
pub(crate) const PANEL_MARGIN_X: i8 = 12;


// Re-export the shared cell renderer so `search.rs` (and any submodule) can
// reach it via `use super::*;`.
pub(crate) use widgets::draw_cell_content;

