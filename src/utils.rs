use eframe::egui::Color32;
use crate::theme::rgb;

pub fn fmt_size(b: u64) -> String {
    let b = b as f64;
    if b >= 1_073_741_824.0 { format!("{:.2} GB", b / 1_073_741_824.0) }
    else if b >= 1_048_576.0 { format!("{:.0} MB", b / 1_048_576.0) }
    else { format!("{:.0} KB", b / 1_024.0) }
}

/// Parse both RFC 3339 and RFC 2822 date strings into a human-readable age.
pub fn time_ago(s: &str) -> String {
    let dt = chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
        .or_else(|_| chrono::DateTime::parse_from_rfc2822(s))
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%a, %d %b %Y %H:%M:%S %z"));
    match dt {
        Ok(dt) => {
            let secs = chrono::Utc::now()
                .signed_duration_since(dt.with_timezone(&chrono::Utc))
                .num_seconds()
                .max(0);
            if secs < 3600      { format!("{}m ago",  secs / 60) }
            else if secs < 86400   { format!("{}h ago",  secs / 3600) }
            else if secs < 604800  { format!("{}d ago",  secs / 86400) }
            else                    { dt.format("%Y-%m-%d").to_string() }
        }
        Err(_) => s.get(..10).unwrap_or("?").to_string(),
    }
}

pub fn pub_year(s: &str) -> u32 {
    chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
        .or_else(|_| chrono::DateTime::parse_from_rfc2822(s))
        .ok()
        .and_then(|dt| dt.format("%Y").to_string().parse::<u32>().ok())
        .unwrap_or(0)
}

pub fn seed_color(s: u32) -> Color32 {
    if s > 500      { rgb(34,197,94) }
    else if s > 100 { rgb(74,222,128) }
    else if s > 10  { rgb(245,158,11) }
    else if s > 0   { rgb(249,115,22) }
    else            { rgb(239,68,68) }
}

pub fn health_label(s: u32) -> &'static str {
    if s > 500      { "HOT" }
    else if s > 100 { "GOOD" }
    else if s > 10  { "SLOW" }
    else if s > 0   { "DYING" }
    else            { "DEAD" }
}

pub fn cat_color(cat: &str) -> Color32 {
    match cat.split('/').next().unwrap_or("").trim() {
        "Movies"   => rgb(245,158,11),
        "TV"       => rgb(59,130,246),
        "Music"    => rgb(16,185,129),
        "Games"    => rgb(139,92,246),
        "Software" => rgb(6,182,212),
        "Anime"    => rgb(236,72,153),
        "Books"    => rgb(249,115,22),
        _          => rgb(100,116,139),
    }
}

/// Torznab numeric category → human label
pub fn torznab_cat(id: &str) -> &'static str {
    match id {
        s if s.starts_with("1") => "Console",
        s if s.starts_with("2") => "Movies",
        s if s.starts_with("3") => "Music",
        s if s.starts_with("4") => "PC",
        s if s.starts_with("5") => "TV",
        s if s.starts_with("6") => "XXX",
        s if s.starts_with("7") => "Books",
        s if s.starts_with("8") => "Other",
        _ => "Other",
    }
}

pub fn urlenc(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        ' ' => "+".into(),
        c   => format!("%{:02X}", c as u32),
    }).collect()
}

pub fn normalize_title(t: &str) -> String {
    let stop = ["1080p","720p","480p","4k","bluray","bdrip","webrip",
                "x264","x265","hevc","10bit","hdr","yify","yts","rarbg",
                "mkv","mp4","avi","remux"];
    let mut s = t.to_lowercase();
    for w in &stop { s = s.replace(w, " "); }
    s.split_whitespace().take(4).collect::<Vec<_>>().join(" ")
}

pub fn now_str() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string()
}
