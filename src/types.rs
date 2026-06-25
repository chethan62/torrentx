use eframe::egui::Color32;
use serde::Deserialize;

// ─── Jackett JSON ──────────────────────────────────────────────────────────

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub struct JackettResponse {
    #[serde(default)]
    pub results: Vec<TorrentResult>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct TorrentResult {
    #[serde(default)]
    pub title: String,
    pub tracker: Option<String>,
    pub category_desc: Option<String>,
    pub size: Option<u64>,
    pub seeders: Option<u32>,
    pub peers: Option<u32>,
    pub publish_date: Option<String>,
    pub magnet_uri: Option<String>,
    pub link: Option<String>,
    pub details: Option<String>,
}

// ─── UI State Enums ────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
pub enum SortCol { Name, Tracker, Size, Seeds, Leech, Date }

#[derive(Clone, PartialEq, Debug)]
pub enum SortDir { Asc, Desc }

#[derive(Clone, PartialEq, Debug)]
pub enum Tab { Search, Rss, Favorites, About }

#[derive(Clone, PartialEq, Debug)]
pub enum SearchState { Idle, Searching, Done, Error(String) }

#[derive(Clone, PartialEq, Debug, Default)]
pub enum HealthFilter { #[default] All, Hot, Good, Slow, Dead }

impl HealthFilter {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All  => "All",
            Self::Hot  => "HOT",
            Self::Good => "GOOD",
            Self::Slow => "SLOW",
            Self::Dead => "DEAD",
        }
    }
    pub fn matches(&self, seeds: u32) -> bool {
        match self {
            Self::All  => true,
            Self::Hot  => seeds > 500,
            Self::Good => (101..=500).contains(&seeds),
            Self::Slow => (11..=100).contains(&seeds),
            Self::Dead => seeds <= 10,
        }
    }
}

// ─── Filter State ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct FilterState {
    pub text:     String,
    pub min_seeds: String,
    pub max_gb:   String,
    pub min_year: String,
    pub tracker:  String,
    pub health:   HealthFilter,
}

impl FilterState {
    pub fn is_dirty(&self) -> bool {
        !self.text.is_empty()
            || !self.min_seeds.is_empty()
            || !self.max_gb.is_empty()
            || !self.min_year.is_empty()
            || !self.tracker.is_empty()
            || self.health != HealthFilter::All
    }
    pub fn reset(&mut self) { *self = Self::default(); }
}

// ─── Toast ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Toast {
    pub msg: String,
    pub ttl: f32,
    pub col: Color32,
}

// ─── Category metadata ─────────────────────────────────────────────────────

pub const CATS: &[&str] = &[
    "All", "Movies", "TV", "Music", "PC Games",
    "Software", "Anime", "Books", "XXX",
];

pub const SPINNER_FRAMES: &[&str] = &["⣾","⣽","⣻","⢿","⡿","⣟","⣯","⣷"];
