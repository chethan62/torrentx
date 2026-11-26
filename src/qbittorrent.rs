//! Send a magnet to a qBittorrent Web API (v2).
//!
//! `torrents/add` (`urls=<magnet>`) is the only call that matters; how it is
//! authenticated depends on what the client offers, in this order:
//!
//! 1. **API key** (qBittorrent 5.2+): `Authorization: Bearer ***, stateless —
//!    no login call, no cookie, and qBittorrent skips its CSRF check for it.
//! 2. **Localhost bypass** (`WebUI/LocalHostAuth=false`): the client already
//!    treats a local request as authenticated, so no header at all.
//! 3. **Login** (every version): `auth/login` with a form body, which must carry
//!    a `Referer` or qBittorrent answers 401 even with the right password, then
//!    echo back whatever `Set-Cookie` it returned.
//!
//! With no URL configured, the connection details come from the local client's
//! own `qBittorrent.conf` (see [`local`]) — the Web UI is off by default in GUI
//! builds and its password is a one-time random string, so asking the user to
//! type either is asking them to fail.
//!
//! Deliberately qBittorrent only: it is the client most Jackett users run, its
//! API is stable, and Transmission/Deluge speak different protocols that would
//! each need their own crate or JSON-RPC session handling.

use std::sync::Mutex;
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

/// What a local qBittorrent's own config says about its Web UI. Read-only:
/// TorrentX never writes another app's config (qBittorrent rewrites it on exit,
/// so an edit made while it runs would silently vanish anyway).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LocalQbt {
    pub port: u16,
    /// `WebUI/APIKey`, already format-checked (`qbt_` + 28 chars).
    pub api_key: Option<String>,
    /// `WebUI/LocalHostAuth`: `true` = auth required, `false` = localhost bypass.
    pub local_auth: bool,
    /// `WebUI/Enabled`: `false` by default in GUI builds, `true` for `-nox`.
    pub enabled: bool,
}

fn conf_path() -> Option<std::path::PathBuf> {
    let p = dirs_next::config_dir()?
        .join("qBittorrent")
        .join("qBittorrent.conf");
    p.is_file().then_some(p)
}

/// QSettings writes `WebUI/Port` as `[WebUI]` + `Port=`, so a bare `Port=` under
/// another section must not win. Only `[WebUI]` is read; `None` when the file has
/// no such section (i.e. it is not a qBittorrent config).
pub(crate) fn parse_local_conf(text: &str) -> Option<LocalQbt> {
    let (mut section, mut saw) = (String::new(), false);
    let (mut port, mut api_key, mut local_auth, mut enabled) = (8080u16, None, true, false);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_string();
            continue;
        }
        if section != "WebUI" {
            continue;
        }
        saw = true;
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim();
        match k.trim() {
            "Port" => port = v.parse().unwrap_or(8080),
            // Utils::APIKey::isValid: prefix "qbt_" + 28 chars. A hand-edited
            // junk value must fall through to login, not 401 the add.
            "APIKey" => {
                api_key = Some(v.to_string()).filter(|k| k.starts_with("qbt_") && k.len() == 32)
            }
            "LocalHostAuth" => local_auth = !v.eq_ignore_ascii_case("false"),
            "Enabled" => enabled = v.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    saw.then_some(LocalQbt {
        port,
        api_key,
        local_auth,
        enabled,
    })
}

/// The local client's Web UI settings, or `None` when there is no local client.
///
/// Cached on the file's mtime: [`Qbt::from_cfg`] runs every frame from the detail
/// panel, and the user may enable the Web UI while TorrentX is open — so the
/// cache has to notice the change rather than live for the whole process.
pub(crate) fn local() -> Option<LocalQbt> {
    static CACHE: Mutex<Option<(u64, Option<LocalQbt>)>> = Mutex::new(None);
    let path = conf_path()?;
    let mtime = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((stamp, hit)) = cache.as_ref()
        && *stamp == mtime
    {
        return hit.clone();
    }
    let hit = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| parse_local_conf(&t));
    *cache = Some((mtime, hit.clone()));
    hit
}

/// Why there is no client to send to. Shown when the row action is clicked with
/// nothing configured — "unreachable" is the wrong story for both cases.
pub(crate) fn not_configured_hint() -> String {
    match local() {
        Some(l) if !l.enabled => {
            "qBittorrent's Web UI is off — Tools → Options → Web UI → enable it".to_string()
        }
        Some(_) => "Set a qBittorrent URL in Settings first".to_string(),
        None => "qBittorrent not found — start it once, then enable Tools → Options → Web UI"
            .to_string(),
    }
}

/// Connection settings: the config's URL when set, otherwise the detected local
/// client. The API key is held in memory only — never written to `config.json`,
/// never logged.
#[derive(Clone)]
pub(crate) struct Qbt {
    base: String,
    user: String,
    pass: String,
    api_key: Option<String>,
    local_auth: bool,
}

impl Qbt {
    /// `None` when there is nothing to send to, i.e. the feature is off.
    pub(crate) fn from_cfg(cfg: &Config) -> Option<Qbt> {
        Self::from_cfg_with(cfg, local())
    }

    /// Split from [`Qbt::from_cfg`] so the detection branch is testable without a
    /// real `~/.config/qBittorrent` on the machine running the tests.
    fn from_cfg_with(cfg: &Config, local: Option<LocalQbt>) -> Option<Qbt> {
        let base = cfg.qbittorrent_url.trim().trim_end_matches('/').to_string();
        if !base.is_empty() {
            return Some(Qbt {
                base,
                user: cfg.qbittorrent_user.clone(),
                pass: cfg.qbittorrent_pass.clone(),
                api_key: None,
                local_auth: true,
            });
        }
        // A detected client whose Web UI is off cannot be used; `not_configured_hint`
        // is what explains that on click.
        let l = local.filter(|l| l.enabled)?;
        Some(Qbt {
            base: format!("http://localhost:{}", l.port),
            user: cfg.qbittorrent_user.clone(),
            pass: cfg.qbittorrent_pass.clone(),
            api_key: l.api_key,
            local_auth: l.local_auth,
        })
    }

    /// Log in and return the `name=value` session cookie.
    fn login(&self, client: &reqwest::blocking::Client) -> Result<String, String> {
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
        Ok(session)
    }

    /// Hand the magnet to the client, authenticating however it allows.
    pub(crate) fn add_magnet(&self, magnet: &str) -> Result<(), String> {
        // The session cookie is threaded by hand: reqwest's cookie store is a
        // non-default feature, and one `SID` value is the whole protocol.
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("qBittorrent client: {e}"))?;

        let add = client
            .post(format!("{}/api/v2/torrents/add", self.base))
            .header("Referer", &self.base)
            .header("Content-Type", "application/x-www-form-urlencoded");
        let add = match (&self.api_key, self.local_auth) {
            (Some(key), _) => add.header("Authorization", format!("Bearer {key}")),
            // Localhost bypass: the client already treats us as authenticated.
            (None, false) => add,
            (None, true) => add.header("Cookie", self.login(&client)?),
        };

        let resp = add
            .body(form_body(&[("urls", magnet)]))
            .send()
            .map_err(|e| format!("qBittorrent add failed: {e}"))?;
        let st = resp.status();
        let msg = resp.text().unwrap_or_default();
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
            api_key: None,
            local_auth: true,
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
            api_key: None,
            local_auth: true,
        };
        q.add_magnet(
            "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=torrentx-live-test",
        )
        .expect("qBittorrent should accept the magnet");
    }

    /// Live check that the detection plumbing finds a real config file — the
    /// parser above is unit-tested, but a wrong path would silently do nothing.
    ///
    /// ```text
    /// XDG_CONFIG_HOME=/tmp/qbt-live/cfg cargo test --lib \
    ///   qbittorrent::tests::local_detection -- --ignored
    /// ```
    #[test]
    #[ignore = "needs a real qBittorrent config; point XDG_CONFIG_HOME at one"]
    fn local_detection_reads_the_real_config() {
        let l = local().expect("no qBittorrent.conf under XDG_CONFIG_HOME");
        assert!(l.port > 0, "{l:?}");
        assert!(l.enabled, "the probe config enables the Web UI: {l:?}");
    }

    /// 5.2+ API key: one request, Bearer auth, no login and no cookie.
    #[test]
    fn api_key_add_is_one_request_with_bearer() {
        let (base, seen) = mock("Ok.");
        let mut q = qbt(base);
        q.api_key = Some("qbt_0123456789abcdefghijklmnopqr".into());
        q.add_magnet("magnet:?xt=urn:btih:abc&dn=x").unwrap();
        let reqs = seen.lock().unwrap().clone();
        assert_eq!(reqs.len(), 1, "no login call: {reqs:?}");
        assert!(
            reqs[0].starts_with("POST /api/v2/torrents/add"),
            "{}",
            reqs[0]
        );
        assert!(
            reqs[0].contains("authorization: Bearer qbt_"),
            "{}",
            reqs[0]
        );
        assert!(!reqs[0].contains("cookie:"), "{}", reqs[0]);
    }

    /// `WebUI/LocalHostAuth=false`: the client trusts localhost, so no auth at all.
    #[test]
    fn localhost_bypass_sends_no_auth() {
        let (base, seen) = mock("Ok.");
        let mut q = qbt(base);
        q.local_auth = false;
        q.add_magnet("magnet:?xt=urn:btih:abc&dn=x").unwrap();
        let reqs = seen.lock().unwrap().clone();
        assert_eq!(reqs.len(), 1, "no login call: {reqs:?}");
        assert!(!reqs[0].contains("authorization:"), "{}", reqs[0]);
        assert!(!reqs[0].contains("cookie:"), "{}", reqs[0]);
    }

    #[test]
    fn parses_only_the_webui_section() {
        let t = "[BitTorrent]\nPort=9999\n[WebUI]\nEnabled=true\nPort=18080\n\
                 APIKey=qbt_0123456789abcdefghijklmnopqr\nLocalHostAuth=false\n";
        let l = parse_local_conf(t).unwrap();
        assert_eq!(l.port, 18080, "a Port under [BitTorrent] must not win");
        assert!(l.enabled);
        assert!(!l.local_auth);
        assert_eq!(
            l.api_key.as_deref(),
            Some("qbt_0123456789abcdefghijklmnopqr")
        );
    }

    #[test]
    fn defaults_match_qbittorrent_and_junk_keys_are_dropped() {
        let l = parse_local_conf("[WebUI]\nAPIKey=nope\n").unwrap();
        assert_eq!((l.port, l.enabled, l.local_auth), (8080, false, true));
        assert_eq!(l.api_key, None, "a malformed key must fall back to login");
        assert!(
            parse_local_conf("[BitTorrent]\nPort=1\n").is_none(),
            "no [WebUI] section = not a qBittorrent config"
        );
    }

    #[test]
    fn detection_fills_an_empty_url_and_a_disabled_webui_is_not_offered() {
        let cfg = Config::default();
        let detected = LocalQbt {
            port: 18080,
            api_key: Some("qbt_0123456789abcdefghijklmnopqr".into()),
            local_auth: false,
            enabled: true,
        };
        let q = Qbt::from_cfg_with(&cfg, Some(detected.clone())).expect("detected");
        assert_eq!(q.base, "http://localhost:18080");
        assert_eq!(q.api_key, detected.api_key);
        assert!(!q.local_auth);

        let off = LocalQbt {
            enabled: false,
            ..detected
        };
        assert!(
            Qbt::from_cfg_with(&cfg, Some(off)).is_none(),
            "a disabled Web UI is not a usable client"
        );
        assert!(Qbt::from_cfg_with(&cfg, None).is_none());
    }

    #[test]
    fn an_explicit_url_wins_over_detection() {
        let cfg = Config {
            qbittorrent_url: "http://localhost:8080/".into(),
            ..Config::default()
        };
        let detected = LocalQbt {
            port: 18080,
            api_key: Some("qbt_0123456789abcdefghijklmnopqr".into()),
            local_auth: false,
            enabled: true,
        };
        let q = Qbt::from_cfg_with(&cfg, Some(detected)).expect("configured");
        assert_eq!(q.base, "http://localhost:8080", "trailing slash trimmed");
        assert_eq!(
            q.api_key, None,
            "the detected key belongs to another instance"
        );
        assert!(q.local_auth, "an explicit URL always logs in");
    }
}
