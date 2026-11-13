//! The CLI surface: `--version` / `--help` / `--minimized`.
//!
//! These flags return before any GUI or config work, so the process exits in a
//! few milliseconds — a spawned-binary test is fast and needs no display. It is
//! also the only automated check of the CLI on Linux: the `print_cli` fix for
//! Windows release builds (GUI subsystem, no console) is proven by the
//! windows-native workflow, but nothing here would notice `--version` printing
//! the wrong thing or panicking on this side.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_torrentx");

#[test]
fn version_prints_the_crate_version() {
    let out = Command::new(BIN).arg("--version").output().expect("spawn");
    assert!(out.status.success(), "--version exited {}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        text.trim(),
        concat!("TorrentX ", env!("CARGO_PKG_VERSION")),
        "--version output changed: {text:?}"
    );
}

#[test]
fn help_lists_every_flag() {
    let out = Command::new(BIN).arg("--help").output().expect("spawn");
    assert!(out.status.success(), "--help exited {}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    for flag in ["--config", "--minimized", "--search", "--help", "--version"] {
        assert!(
            text.contains(flag),
            "--help does not mention {flag}: {text}"
        );
    }
}

#[test]
fn search_without_a_query_fails_fast() {
    // Exits before any network or config work, so this stays a sub-second test.
    let out = Command::new(BIN).arg("--search").output().expect("spawn");
    assert_eq!(out.status.code(), Some(2), "--search alone should exit 2");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("--search"),
        "error should name the flag: {err}"
    );
}

#[test]
fn sources_lists_origins_even_with_jackett_down() {
    // Point at a port nothing listens on: the command must still describe the
    // sources that do not need Jackett, and say so on stderr.
    let dir = std::env::temp_dir().join(format!("tx-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmp");
    let cfg = dir.join("config.json");
    std::fs::write(
        &cfg,
        r#"{"jackett_url": "http://127.0.0.1:1", "api_key": "x", "timeout_secs": 1}"#,
    )
    .expect("write cfg");

    let out = Command::new(BIN)
        .args(["--config", cfg.to_str().unwrap(), "--sources"])
        .output()
        .expect("spawn");
    let text = String::from_utf8_lossy(&out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "--sources exited {}: {err}",
        out.status
    );
    assert!(
        text.contains("apibay") && text.contains("built-in"),
        "{text}"
    );
    assert!(text.contains("local def"), "JSON defs not labelled: {text}");
    assert!(err.contains("jackett unreachable"), "no warning: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Minimal stand-in for a Jackett server: the indexers endpoint answers the given
/// `<indexer …/>` entries, every results endpoint answers an empty feed. Enough to
/// prove which sources a `--search` actually fans out over.
fn mock_jackett(indexers: &'static str) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock jackett");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut c) = conn else { break };
            let mut buf = [0u8; 4096];
            let n = c.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let body = if req.contains("t=indexers") {
                format!("<?xml version=\"1.0\"?><indexers>{indexers}</indexers>")
            } else {
                "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>mock</title></channel></rss>"
                    .to_string()
            };
            let _ = write!(
                c,
                "HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    url
}

/// The fan-out is Jackett's configured list **merged with the native ones**. The
/// CLI used to take Jackett's list alone whenever the server answered, so a
/// headless search silently covered fewer sources than the GUI and then reported
/// "N of M" — this pins the merge, and the id Jackett never returns (`apibay`) is
/// the assertion that fails if it regresses.
#[test]
fn search_fans_out_over_native_sources_and_jacketts_configured_ones() {
    let url = mock_jackett(r#"<indexer id="zzzjackett" configured="true"/>"#);
    let dir = std::env::temp_dir().join(format!("tx-cli-merge-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmp");
    let cfg = dir.join("config.json");
    // timeout_secs 1: the native sources will reach for their real hosts, which is
    // fine — a failure prints the same `failed <id>:` line a success prints rows for.
    std::fs::write(
        &cfg,
        format!(
            r#"{{"jackett_url": "{url}", "api_key": "x", "timeout_secs": 1, "dedupe": false}}"#
        ),
    )
    .expect("write cfg");

    let out = Command::new(BIN)
        .args(["--config", cfg.to_str().unwrap(), "--search", "dune"])
        .output()
        .expect("spawn");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("zzzjackett"),
        "Jackett's configured indexer never got searched:\n{err}"
    );
    assert!(
        err.contains("apibay"),
        "native sources were dropped from the fan-out:\n{err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
