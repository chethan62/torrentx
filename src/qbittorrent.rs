//! Send a magnet to a qBittorrent Web API (v2).
//!
//! Two calls: `auth/login` (form user/password, must carry a `Referer` or
//! qBittorrent answers 401 even with the right password) and `torrents/add`
//! (`urls=<magnet>`). The session cookie is kept by the client's cookie store,
//! so one [`Qbt::add_magnet`] is self-contained — no state to hold between rows.
//!
//! Deliberately qBittorrent only: it is the client most Jackett users run, its
//! API is stable, and Transmission/Deluge speak different protocols that would
//! each need their own crate or JSON-RPC session handling.

use std::time::Duration;

use crate::config::Config;

/// `application/x-www-form-urlencoded` body. Nested magnets carry `&`, `?` and
/// `:` — hand-concatenating them would truncate the request at the first `&`.
fn form_body(pairs: &[(&str, &str)]) -> String {
    let mut ser = form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs {
        ser.append_pair(k, v);
    }
    ser.finish()
}

/// Connection settings, taken from the config when `qbittorrent_url` is set.
#[derive(Clone)]
pub(crate) struct Qbt {
    base: String,
    user: String,
    pass: String,
}

impl Qbt {
    /// `None` when no URL is configured, i.e. the feature is off.
    pub(crate) fn from_cfg(cfg: &Config) -> Option<Qbt> {
        let base = cfg.qbittorrent_url.trim().trim_end_matches('/').to_string();
        if base.is_empty() {
            return None;
        }
        Some(Qbt {
            base,
            user: cfg.qbittorrent_user.clone(),
            pass: cfg.qbittorrent_pass.clone(),
        })
    }

    /// Log in and hand the magnet to the client.
    pub(crate) fn add_magnet(&self, magnet: &str) -> Result<(), String> {
        // The session cookie is threaded by hand: reqwest's cookie store is a
        // non-default feature, and one `SID` value is the whole protocol.
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("qBittorrent client: {e}"))?;

        let login = client
            .post(format!("{}/api/v2/auth/login", self.base))
            .header("Referer", &self.base)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form_body(&[
                ("username", &self.user),
                ("password", &self.pass),
            ]))
            .send()
            .map_err(|e| format!("qBittorrent unreachable at {}: {e}", self.base))?;
        let status = login.status();
        // Take whatever cookie the login set, not a name we guessed: qBittorrent
        // 4.x calls it `SID`, 5.x `QBT_SID_<port>`. The whole `name=value` pair
        // goes back verbatim.
        let session = login
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|c| c.split(';').next())
            .map(str::trim)
            .find(|c| !c.is_empty())
            .map(str::to_string)
            .ok_or_else(|| "qBittorrent did not return a session cookie".to_string())?;
        let body = login.text().unwrap_or_default();
        // 4.x answers 200 + "Ok."; 5.x answers 204 with no body. "Fails." is the
        // wrong-password answer in both, always with 200.
        let trimmed = body.trim();
        let logged_in =
            status.is_success() && (trimmed.is_empty() || trimmed.eq_ignore_ascii_case("ok."));
        if !logged_in {
            // qBittorrent answers 200 + "Fails." for a wrong password, and 403
            // when the client is banned for failed logins.
            return Err(if status == reqwest::StatusCode::FORBIDDEN {
                "qBittorrent refused the login (too many attempts — restart it)".to_string()
            } else {
                "qBittorrent login failed (check the user and password)".to_string()
            });
        }

        let add = client
            .post(format!("{}/api/v2/torrents/add", self.base))
            .header("Referer", &self.base)
            .header("Cookie", session)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form_body(&[("urls", magnet)]))
            .send()
            .map_err(|e| format!("qBittorrent add failed: {e}"))?;
        let st = add.status();
        let msg = add.text().unwrap_or_default();
        // 200 is success; a 415 means the magnet was rejected as not a torrent.
        if st.is_success() || st == reqwest::StatusCode::UNSUPPORTED_MEDIA_TYPE {
            Ok(())
        } else {
            Err(format!("qBittorrent rejected the magnet ({st} {msg})"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Minimal HTTP mock: answers the login with `Ok.` + a cookie, records every
    /// request (line, headers, body) so the test can assert what was sent.
    fn mock(reply_login: &'static str) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut s = stream.unwrap();
                let mut buf = Vec::new();
                let mut chunk = [0u8; 1024];
                // Read headers, then exactly Content-Length more bytes.
                loop {
                    let n = s.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(hdr_end) = text.find("\r\n\r\n") {
                        let len: usize = text
                            .lines()
                            .find_map(|l| {
                                l.strip_prefix("content-length: ")
                                    .or_else(|| l.strip_prefix("Content-Length: "))
                            })
                            .and_then(|v| v.trim().parse().ok())
                            .unwrap_or(0);
                        if buf.len() >= hdr_end + 4 + len {
                            break;
                        }
                    }
                }
                let req = String::from_utf8_lossy(&buf).to_string();
                let login = req.starts_with("POST /api/v2/auth/login");
                log.lock().unwrap().push(req);
                let body = if login { reply_login } else { "Ok." };
                let cookie = if login {
                    "set-cookie: SID=abc123; path=/\r\n"
                } else {
                    ""
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\ncontent-type: text/plain\r\n{cookie}connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(resp.as_bytes());
            }
        });
        (format!("http://127.0.0.1:{port}"), seen)
    }

    fn qbt(base: String) -> Qbt {
        Qbt {
            base,
            user: "admin".into(),
            pass: "secret".into(),
        }
    }

    #[test]
    fn login_then_add_sends_the_magnet_with_the_session_cookie() {
        let (base, seen) = mock("Ok.");
        qbt(base)
            .add_magnet("magnet:?xt=urn:btih:abc&dn=x")
            .unwrap();
        let reqs = seen.lock().unwrap().clone();
        assert_eq!(reqs.len(), 2, "one login, one add: {reqs:?}");
        assert!(reqs[0].contains("username=admin"), "{}", reqs[0]);
        assert!(reqs[0].contains("password=secret"), "{}", reqs[0]);
        assert!(reqs[0].contains("referer: http://127.0.0.1"), "{}", reqs[0]);
        assert!(
            reqs[1].starts_with("POST /api/v2/torrents/add"),
            "{}",
            reqs[1]
        );
        assert!(
            reqs[1].contains("urls=magnet%3A%3Fxt%3Durn%3Abtih%3Aabc%26dn%3Dx"),
            "the magnet must be form-encoded in the body: {}",
            reqs[1]
        );
        assert!(
            reqs[1].contains("cookie: SID=abc123"),
            "the add call must carry the session cookie: {}",
            reqs[1]
        );
    }

    #[test]
    fn a_wrong_password_is_reported_not_swallowed() {
        let (base, seen) = mock("Fails.");
        let err = qbt(base).add_magnet("magnet:?xt=urn:btih:abc").unwrap_err();
        assert!(err.contains("login failed"), "{err}");
        assert_eq!(seen.lock().unwrap().len(), 1, "no add after a failed login");
    }

    /// Live check against a real qBittorrent (the mock above proves the request
    /// shape; this proves qBittorrent accepts it). Ignored by default:
    ///
    /// ```text
    /// TX_QBT_URL=http://localhost:8080 TX_QBT_USER=admin TX_QBT_PASS=… \
    ///   cargo test --lib qbittorrent -- --ignored
    /// ```
    #[test]
    #[ignore = "needs a running qBittorrent; set TX_QBT_URL/USER/PASS"]
    fn a_real_webui_accepts_the_magnet() {
        let (Ok(url), Ok(user), Ok(pass)) = (
            std::env::var("TX_QBT_URL"),
            std::env::var("TX_QBT_USER"),
            std::env::var("TX_QBT_PASS"),
        ) else {
            panic!("set TX_QBT_URL, TX_QBT_USER and TX_QBT_PASS");
        };
        let q = Qbt {
            base: url,
            user,
            pass,
        };
        q.add_magnet(
            "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=torrentx-live-test",
        )
        .expect("qBittorrent should accept the magnet");
    }

    #[test]
    fn no_url_means_the_feature_is_off() {
        let mut cfg = Config::default();
        assert!(Qbt::from_cfg(&cfg).is_none());
        cfg.qbittorrent_url = "http://localhost:8080/".into();
        let q = Qbt::from_cfg(&cfg).expect("configured");
        assert_eq!(q.base, "http://localhost:8080", "trailing slash trimmed");
    }
}
