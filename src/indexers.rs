//! User-definable JSON indexers.
//!
//! Jackett ships ~646 definitions written in Cardigann YAML, with HTML
//! selectors, cookie handling and per-site quirks. This is not that: it covers
//! the other case — a site whose search is *already JSON* — so a new source is a
//! small data file instead of a patch to the app.
//!
//! A definition lives in `~/.config/torrentx/indexers/<id>.json`:
//!
//! ```json
//! {
//!   "id": "eztv",
//!   "name": "EZTV",
//!   "category": "TV",
//!   "url": "https://eztvx.to/api/get-torrents?limit=100&q={q}",
//!   "rows": "torrents",
//!   "fields": {
//!     "title": "title", "magnet": "magnet_url", "hash": "hash",
//!     "size": "size_bytes", "seeders": "seeds", "leechers": "peers",
//!     "date": "date_released_unix"
//!   }
//! }
//! ```
//!
//! `{q}` in `url` is replaced with the URL-encoded query. `rows` is a dotted
//! path to the array of results (numbers index arrays); omit it when the body is
//! the array itself or a single result. `magnet` may be left out if the site
//! returns only an infohash — a magnet is then synthesised, exactly as apibay's
//! rows do. `category` is the picker bucket the source fills ("TV", "Movies",
//! "Music", "PC Games", "Software", "Books", "XXX"); omit it and the rows only
//! appear under All.
//!
//! A definition whose id matches a built-in source (apibay, knaben) replaces it.

use crate::jackett::TorrentResult;
use serde_json::Value;
use std::path::PathBuf;

/// Where a user's own definitions live (beside `config.json`).
pub(crate) fn dir() -> PathBuf {
    crate::config::cfg_path()
        .parent()
        .map(|p| p.join("indexers"))
        .unwrap_or_else(|| PathBuf::from("indexers"))
}

#[derive(serde::Deserialize, Clone, Debug)]
pub(crate) struct Fields {
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) magnet: Option<String>,
    #[serde(default)]
    pub(crate) hash: Option<String>,
    #[serde(default)]
    pub(crate) size: Option<String>,
    #[serde(default)]
    pub(crate) seeders: Option<String>,
    #[serde(default)]
    pub(crate) leechers: Option<String>,
    #[serde(default)]
    pub(crate) date: Option<String>,
    #[serde(default)]
    pub(crate) details: Option<String>,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub(crate) struct Def {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) category: Option<String>,
    pub(crate) url: String,
    #[serde(default)]
    pub(crate) rows: Option<String>,
    pub(crate) fields: Fields,
}

impl Def {
    pub(crate) fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.id.clone())
    }
}

/// An id has to survive being put in a URL and a picker: no spaces, no dots.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The one definition that ships with the app, so the mechanism has a worked
/// example and a source that is not Jackett's. A user file with the same id wins.
const BUNDLED: &str = include_str!("../assets/indexers/eztv.json");

fn parsed(json: &str) -> Option<Def> {
    let def: Def = serde_json::from_str(json).ok()?;
    valid_id(&def.id).then_some(def)
}

/// Bundled first, then `~/.config/torrentx/indexers/*.json`; a later definition
/// with the same id replaces an earlier one. Unreadable or invalid files are
/// skipped rather than guessed at.
pub(crate) fn load() -> Vec<Def> {
    let mut out: Vec<Def> = parsed(BUNDLED).into_iter().collect();
    let dir = dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    for f in files {
        let Ok(raw) = std::fs::read_to_string(&f) else {
            continue;
        };
        let Some(def) = parsed(&raw) else {
            continue;
        };
        match out.iter_mut().find(|d| d.id == def.id) {
            Some(slot) => *slot = def,
            None => out.push(def),
        }
    }
    out
}

pub(crate) fn ids() -> Vec<String> {
    load().into_iter().map(|d| d.id).collect()
}

pub(crate) fn find(id: &str) -> Option<Def> {
    load().into_iter().find(|d| d.id.eq_ignore_ascii_case(id))
}

/// Dotted path lookup: `"torrents"`, `"data.movies"`, `"items.0.torrents"`.
/// An empty path returns the value itself.
pub(crate) fn at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for step in path.split('.').filter(|s| !s.is_empty()) {
        cur = match cur {
            Value::Array(a) => a.get(step.parse::<usize>().ok()?)?,
            Value::Object(o) => o.get(step)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Sites send counts as numbers or as strings ("3482755813"), depending on the
/// writer's mood. Both parse; empty and negative mean "unknown".
fn as_u64(s: Option<&Value>) -> Option<u64> {
    match s? {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)),
        Value::String(s) => s.trim().parse::<f64>().ok().map(|f| f.max(0.0) as u64),
        _ => None,
    }
}

/// A date is either unix seconds or something date-shaped; the table sorts on a
/// parsed date, so unix seconds are converted to the RFC 2822 form the rest of
/// the app uses.
fn as_date(s: Option<&Value>) -> Option<String> {
    match s? {
        Value::Number(_) => as_u64(s)
            .and_then(|ts| chrono::DateTime::from_timestamp(ts as i64, 0).map(|d| d.to_rfc2822())),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else if let Ok(ts) = t.parse::<i64>() {
                chrono::DateTime::from_timestamp(ts, 0).map(|d| d.to_rfc2822())
            } else {
                Some(t.to_string())
            }
        }
        _ => None,
    }
}

/// Map one definition over a response body. Pure, so a definition can be tested
/// against a saved body without a network.
pub(crate) fn parse(def: &Def, body: &str) -> Vec<TorrentResult> {
    let Ok(root) = serde_json::from_str::<Value>(body) else {
        return vec![];
    };
    let path = def.rows.clone().unwrap_or_default();
    let container = at(&root, &path).unwrap_or(&root);
    let rows: Vec<&Value> = match container {
        Value::Array(a) => a.iter().collect(),
        Value::Object(_) => vec![container],
        _ => vec![],
    };
    let f = &def.fields;
    rows.into_iter()
        .filter_map(|row| {
            let title = at(row, &f.title)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim()
                .to_string();
            if title.is_empty() {
                return None;
            }
            let magnet = f
                .magnet
                .as_ref()
                .and_then(|p| at(row, p))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .filter(|m| m.starts_with("magnet:"));
            let magnet_uri = magnet.or_else(|| {
                let hash = f.hash.as_ref().and_then(|p| at(row, p))?.as_str()?;
                (!hash.is_empty()).then(|| crate::sources::magnet(hash, &title))
            });
            Some(TorrentResult {
                title,
                tracker: Some(def.label()),
                category_desc: def.category.clone(),
                size: as_u64(f.size.as_ref().and_then(|p| at(row, p))),
                seeders: as_u64(f.seeders.as_ref().and_then(|p| at(row, p)))
                    .map(|v| v.min(u32::MAX as u64) as u32),
                peers: as_u64(f.leechers.as_ref().and_then(|p| at(row, p)))
                    .map(|v| v.min(u32::MAX as u64) as u32),
                publish_date: as_date(f.date.as_ref().and_then(|p| at(row, p))),
                magnet_uri,
                details: f
                    .details
                    .as_ref()
                    .and_then(|p| at(row, p))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                ..Default::default()
            })
        })
        .collect()
}

/// One request: `{q}` is substituted, the body is parsed by the definition.
/// Blocking; the caller runs it on the fan-out thread.
pub(crate) fn fetch(
    def: &Def,
    query: &str,
    cat: &str,
    timeout: u64,
) -> Result<Vec<TorrentResult>, String> {
    let url = def.url.replace("{q}", &crate::jackett::urlenc(query));
    let body = crate::jackett::shared_client()
        .get(&url)
        .timeout(std::time::Duration::from_secs(timeout.min(30)))
        .send()
        .map_err(|e| format!("{}: {}", def.id, crate::sources::net_err(e, timeout)))
        .and_then(|r| {
            r.error_for_status()
                .map_err(|e| format!("{}: {}", def.id, crate::sources::net_err(e, timeout)))
        })
        .and_then(|r| {
            r.text()
                .map_err(|e| format!("{}: {}", def.id, crate::sources::net_err(e, timeout)))
        })?;
    Ok(parse(def, &body)
        .into_iter()
        .filter(|r| crate::sources::matches_cat(cat, r.category_desc.as_deref()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eztv() -> Def {
        parsed(BUNDLED).expect("the bundled definition must parse")
    }

    #[test]
    fn the_bundled_definition_is_usable() {
        let d = eztv();
        // "eztv" belongs to Jackett's own indexer; a definition must not shadow it.
        assert_eq!(d.id, "eztv-api");
        assert_eq!(d.label(), "EZTV (API)");
        assert_eq!(d.category.as_deref(), Some("TV"));
        assert!(d.url.contains("{q}"), "a search url needs a {{q}} slot");
    }

    #[test]
    fn dotted_paths_walk_objects_and_arrays() {
        let v: Value =
            serde_json::from_str(r#"{"a":{"b":[{"c":7}]},"n":1}"#).expect("fixture is json");
        assert_eq!(at(&v, "a.b.0.c").and_then(|x| x.as_u64()), Some(7));
        assert_eq!(at(&v, "n").and_then(|x| x.as_u64()), Some(1));
        assert_eq!(at(&v, ""), Some(&v));
        assert!(at(&v, "a.missing.c").is_none());
        assert!(
            at(&v, "a.b.9.c").is_none(),
            "out-of-range index is not a panic"
        );
        assert!(at(&v, "n.0").is_none(), "indexing a scalar yields nothing");
    }

    #[test]
    fn a_json_body_becomes_rows_with_sizes_and_dates() {
        // Shape taken from eztv's real /api/get-torrents answer: counts as
        // numbers, size as a *string*, date as unix seconds.
        let body = r#"{"torrents":[
          {"title":"Show S01E01 1080p","magnet_url":"magnet:?xt=urn:btih:AAA&dn=x",
           "hash":"aaaa","seeds":4,"peers":18,"size_bytes":"3482755813","date_released_unix":1793121040},
          {"title":"","magnet_url":"magnet:?xt=urn:btih:BBB"},
          {"title":"Only a hash","hash":"bbbb","seeds":0}
        ]}"#;
        let rows = parse(&eztv(), body);
        assert_eq!(rows.len(), 2, "an untitled row is not a result");
        assert_eq!(rows[0].title, "Show S01E01 1080p");
        assert_eq!(rows[0].tracker.as_deref(), Some("EZTV (API)"));
        assert_eq!(rows[0].category_desc.as_deref(), Some("TV"));
        assert_eq!(rows[0].seeders, Some(4));
        assert_eq!(rows[0].peers, Some(18));
        assert_eq!(
            rows[0].size,
            Some(3_482_755_813),
            "a string size still parses"
        );
        assert!(
            rows[0]
                .publish_date
                .as_deref()
                .unwrap_or("")
                .contains("2026")
        );
        // No magnet_url, but a hash: synthesised, with trackers, like apibay.
        let m = rows[1].magnet_uri.as_deref().unwrap_or("");
        assert!(m.starts_with("magnet:?xt=urn:btih:bbbb"), "{m}");
        assert!(m.contains("&tr=udp%3A%2F%2F"), "{m}");
        // A real zero survives as zero (unknown would be None).
        assert_eq!(rows[1].seeders, Some(0));
    }

    #[test]
    fn a_definition_can_point_at_a_single_result_or_the_body_itself() {
        let mut d = eztv();
        d.rows = None;
        let rows = parse(
            &d,
            r#"[{"title":"a","hash":"h1"},{"title":"b","hash":"h2"}]"#,
        );
        assert_eq!(rows.len(), 2, "no `rows` path means the body is the array");
        d.rows = Some("data.movie".into());
        let rows = parse(&d, r#"{"data":{"movie":{"title":"solo","hash":"h3"}}}"#);
        assert_eq!(rows.len(), 1, "a single object is one row");
    }

    #[test]
    fn the_category_bucket_is_the_definitions_own() {
        let d = eztv();
        assert!(crate::sources::matches_cat("TV", d.category.as_deref()));
        assert!(!crate::sources::matches_cat(
            "Movies",
            d.category.as_deref()
        ));
    }

    #[test]
    fn an_id_that_cannot_be_a_picker_entry_is_rejected() {
        let too_long = "x".repeat(33);
        for bad in ["", "has space", "dot.ted", "slash/ed", too_long.as_str()] {
            let json = format!(r#"{{"id":"{bad}","url":"u{{q}}","fields":{{"title":"t"}}}}"#);
            assert!(parsed(&json).is_none(), "rejected: {bad:?}");
        }
        let ok = r#"{"id":"my-site_2","url":"u{q}","fields":{"title":"t"}}"#;
        assert!(parsed(ok).is_some());
    }

    #[test]
    fn a_user_file_replaces_a_bundled_definition_by_id() {
        let dir = crate::indexers::dir();
        let ours = dir.join("eztv.json");
        let existed = ours.exists();
        let backup = std::fs::read_to_string(&ours).ok();
        std::fs::create_dir_all(&dir).expect("config dir is writable");
        std::fs::write(
            &ours,
            r#"{"id":"eztv","name":"EZTV (mine)","category":"TV","url":"https://x/{q}","fields":{"title":"t"}}"#,
        )
        .expect("write");
        let found = find("eztv").expect("still present");
        assert_eq!(found.label(), "EZTV (mine)");
        assert_eq!(found.url, "https://x/{q}");
        // leave the user's config exactly as it was
        match backup {
            Some(b) => std::fs::write(&ours, b).expect("restore"),
            None => {
                let _ = std::fs::remove_file(&ours);
            }
        }
        if !existed {
            let _ = std::fs::remove_dir(&dir);
        }
    }
}
