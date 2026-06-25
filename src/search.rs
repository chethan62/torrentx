use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;

use crate::types::{JackettResponse, SearchState, TorrentResult};
use crate::utils::urlenc;

fn set_state(st: &Arc<Mutex<SearchState>>, s: SearchState) {
    if let Ok(mut g) = st.lock() {
        *g = s;
    }
}

/// Fetch configured indexer IDs via Torznab XML endpoint.
fn fetch_indexer_ids(client: &Client, url: &str, key: &str) -> Result<Vec<String>, String> {
    let ep = format!(
        "{}/api/v2.0/indexers/all/results/torznab/api?apikey={}&t=indexers&configured=true",
        url.trim_end_matches('/'),
        key
    );
    let resp = client
        .get(&ep)
        .send()
        .map_err(|e| format!("Cannot reach Jackett: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} fetching indexers", resp.status().as_u16()));
    }
    let body = resp.text().map_err(|e| format!("Read error: {e}"))?;

    let mut ids = Vec::new();
    for chunk in body.split("<indexer ") {
        if let Some(id_start) = chunk.find("id=\"") {
            let rest = &chunk[id_start + 4..];
            if let Some(id_end) = rest.find('"') {
                let id = rest[..id_end].trim().to_string();
                if !id.is_empty() && id != "all" {
                    ids.push(id);
                }
            }
        }
    }
    Ok(ids)
}

pub fn start_search(
    url: String,
    key: String,
    query: String,
    cat: String,
    timeout: u64,
    results: Arc<Mutex<Vec<TorrentResult>>>,
    state: Arc<Mutex<SearchState>>,
    count: Arc<Mutex<usize>>,
    done_count: Arc<Mutex<usize>>,
    total_count: Arc<Mutex<usize>>,
) {
    thread::spawn(move || {
        set_state(&state, SearchState::Searching);
        if let Ok(mut r) = results.lock() {
            r.clear();
        }
        if let Ok(mut c) = count.lock() {
            *c = 0;
        }
        if let Ok(mut d) = done_count.lock() {
            *d = 0;
        }
        if let Ok(mut t) = total_count.lock() {
            *t = 0;
        }

        let cat_id = match cat.as_str() {
            "Movies" => "2000",
            "TV" => "5000",
            "Music" => "3000",
            "PC Games" => "4000",
            "Software" => "4000",
            "Anime" => "5070",
            "Books" => "7000",
            "XXX" => "6000",
            _ => "",
        };

        let client = match Client::builder()
            .timeout(Duration::from_secs(timeout))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                set_state(&state, SearchState::Error(format!("Client error: {e}")));
                return;
            }
        };

        let indexer_ids = match fetch_indexer_ids(&client, &url, &key) {
            Ok(ids) if !ids.is_empty() => ids,
            Ok(_) => {
                set_state(
                    &state,
                    SearchState::Error("No indexers configured in Jackett".into()),
                );
                return;
            }
            Err(e) => {
                set_state(&state, SearchState::Error(e));
                return;
            }
        };

        let n = indexer_ids.len();
        if let Ok(mut t) = total_count.lock() {
            *t = n;
        }

        let done_atomic = Arc::new(AtomicUsize::new(0));

        for indexer_id in indexer_ids {
            let url2 = url.clone();
            let key2 = key.clone();
            let query2 = query.clone();
            let cat_id2 = cat_id.to_string();
            let results2 = Arc::clone(&results);
            let state2 = Arc::clone(&state);
            let count2 = Arc::clone(&count);
            let done2 = Arc::clone(&done_count);
            let done_atomic2 = Arc::clone(&done_atomic);

            thread::spawn(move || {
                let per_timeout = timeout.min(20);
                let client = match Client::builder()
                    .timeout(Duration::from_secs(per_timeout))
                    .build()
                {
                    Ok(c) => c,
                    Err(_) => {
                        bump(&done_atomic2, &done2, &results2, &count2, &state2, n);
                        return;
                    }
                };

                let mut ep = format!(
                    "{}/api/v2.0/indexers/{}/results?apikey={}&Query={}",
                    url2.trim_end_matches('/'),
                    indexer_id,
                    urlenc(&key2),
                    urlenc(&query2),
                );
                if !cat_id2.is_empty() {
                    ep.push_str(&format!("&Category[]={}", cat_id2));
                }

                if let Ok(resp) = client.get(&ep).send() {
                    if resp.status().is_success() {
                        if let Ok(data) = resp.json::<JackettResponse>() {
                            if let Ok(mut r) = results2.lock() {
                                r.extend(data.results);
                            }
                        }
                    }
                }

                bump(&done_atomic2, &done2, &results2, &count2, &state2, n);
            });
        }
    });
}

fn bump(
    done_atomic: &Arc<AtomicUsize>,
    done_count: &Arc<Mutex<usize>>,
    results: &Arc<Mutex<Vec<TorrentResult>>>,
    count: &Arc<Mutex<usize>>,
    state: &Arc<Mutex<SearchState>>,
    total: usize,
) {
    let finished = done_atomic.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut d) = done_count.lock() {
        *d = finished;
    }
    let n = results.lock().map(|r| r.len()).unwrap_or(0);
    if let Ok(mut c) = count.lock() {
        *c = n;
    }
    if finished >= total {
        set_state(state, SearchState::Done);
    }
}
