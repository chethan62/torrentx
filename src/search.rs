use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;

use crate::types::{JackettResponse, SearchState, TorrentResult};
use crate::utils::urlenc;

fn set_state(st: &Arc<Mutex<SearchState>>, s: SearchState) {
    if let Ok(mut g) = st.lock() { *g = s; }
}

pub fn start_search(
    url:     String,
    key:     String,
    query:   String,
    cat:     String,
    timeout: u64,
    results: Arc<Mutex<Vec<TorrentResult>>>,
    state:   Arc<Mutex<SearchState>>,
    count:   Arc<Mutex<usize>>,
) {
    thread::spawn(move || {
        set_state(&state, SearchState::Searching);

        let mut ep = format!(
            "{}/api/v2.0/indexers/all/results?apikey={}&Query={}",
            url.trim_end_matches('/'),
            urlenc(&key),
            urlenc(&query),
        );
        if cat != "All" {
            ep.push_str(&format!("&Category[]={}", urlenc(&cat)));
        }

        let client = match Client::builder().timeout(Duration::from_secs(timeout)).build() {
            Ok(c)  => c,
            Err(e) => { set_state(&state, SearchState::Error(format!("Client error: {e}"))); return; }
        };

        match client.get(&ep).send() {
            Err(e) => {
                let msg = if e.is_connect() {
                    format!("Cannot reach Jackett at {url}\nCheck: sudo systemctl start jackett")
                } else if e.is_timeout() {
                    format!("Timed out after {timeout}s — increase timeout in Settings")
                } else {
                    format!("Network error: {e}")
                };
                set_state(&state, SearchState::Error(msg));
            }
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    match resp.json::<JackettResponse>() {
                        Ok(data) => {
                            let n = data.results.len();
                            if let Ok(mut r) = results.lock() { *r = data.results; }
                            if let Ok(mut c) = count.lock()   { *c = n; }
                            set_state(&state, SearchState::Done);
                        }
                        Err(e) => set_state(&state, SearchState::Error(format!("Parse error: {e}"))),
                    }
                } else {
                    let msg = match status.as_u16() {
                        401 => "Invalid API key — open Settings and paste your Jackett key.".into(),
                        403 => "Forbidden — check Jackett permissions.".into(),
                        404 => "Jackett endpoint not found — verify URL in Settings.".into(),
                        500 => "Jackett internal error — check Jackett logs.".into(),
                        n   => format!("HTTP {n} from Jackett"),
                    };
                    set_state(&state, SearchState::Error(msg));
                }
            }
        }
    });
}
