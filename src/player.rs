// ─── Direct play ───────────────────────────────────────────────────────────
// The app deliberately does no torrent I/O of its own: webtorrent-cli fetches
// pieces on demand (so seeking works) and mpv/vlc decode. All this module owns
// is the argv, one child process, and an honest error when it is missing.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// Players webtorrent-cli can drive, in menu order. A configured value is
/// matched against this list — it never reaches a command line verbatim.
pub(crate) const PLAYERS: [&str; 2] = ["mpv", "vlc"];

/// webtorrent-cli's streaming flag for a player, or None for anything else.
pub(crate) fn player_flag(player: &str) -> Option<&'static str> {
    match player {
        "mpv" => Some("--mpv"),
        "vlc" => Some("--vlc"),
        _ => None,
    }
}

/// One file inside a torrent, as `webtorrent --select` lists it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FileEntry {
    pub(crate) idx: usize,
    pub(crate) name: String,
    pub(crate) size: String,
}

/// Parse the listing `webtorrent <magnet> --select` prints:
///
/// ```text
/// 0  Sintel.mkv (128 MB)
/// 1  Sintel.srt (12 KB)
/// ```
///
/// Names may contain " (", so the size is split off from the LAST one; header,
/// hint and blank lines have no leading index and are skipped. Pure + tested.
pub(crate) fn parse_file_list(out: &str) -> Vec<FileEntry> {
    let mut files = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        let Some((idx, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(idx) = idx.parse::<usize>() else {
            continue;
        };
        let rest = rest.trim_start();
        if !rest.ends_with(')') {
            continue;
        }
        let Some(open) = rest.rfind(" (") else {
            continue;
        };
        let name = rest[..open].trim();
        if name.is_empty() {
            continue;
        }
        files.push(FileEntry {
            idx,
            name: name.to_string(),
            size: rest[open + 2..rest.len() - 1].to_string(),
        });
    }
    files
}

/// Ask webtorrent for the torrent's file list (`--select` with no index lists
/// and exits). Runs off the UI thread; needs peers for a magnet, so a dead
/// swarm reports that instead of an empty picker.
pub(crate) fn list_files(
    magnet: String,
    link: Option<String>,
    tx: std::sync::mpsc::Sender<Result<Vec<FileEntry>, String>>,
) {
    use std::io::Read;
    std::thread::spawn(move || {
        // The .torrent from Jackett lists files without waiting on peers.
        let source = link
            .as_deref()
            .and_then(|l| torrent_file(&magnet, l))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(magnet);
        let mut child = match Command::new("webtorrent")
            .args([source.as_str(), "--select"])
            // chalk colours the index/size; NO_COLOR keeps the parser simple.
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Err(match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        "Listing files needs webtorrent-cli — run: npm install -g webtorrent-cli"
                            .to_string()
                    }
                    _ => format!("Could not start webtorrent: {e}"),
                }));
                return;
            }
        };
        // Metadata comes from peers, so give it a budget and then stop waiting.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
            }
        }
        let mut out = String::new();
        if let Some(mut s) = child.stdout.take() {
            let _ = s.read_to_string(&mut out);
        }
        let files = parse_file_list(&out);
        let _ = tx.send(if files.is_empty() {
            Err("No file list — the torrent's peers are unreachable".to_string())
        } else {
            Ok(files)
        });
    });
}

/// A local copy of the result's `.torrent`, fetched from the Jackett link.
/// Jackett serves it from its own cache, so this is a file fetch — no swarm
/// metadata round trip — and webtorrent can name/list files immediately.
/// None when there is no usable link; the caller falls back to the magnet.
pub(crate) fn torrent_file(magnet: &str, link: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    if !link.starts_with("http") {
        return None;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    magnet.hash(&mut h);
    let path = dirs_next::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("torrentx")
        .join("torrents")
        .join(format!("{:016x}.torrent", h.finish()));
    // ponytail: no TTL — Jackett's file for one infohash never changes.
    if let Ok(md) = std::fs::metadata(&path) {
        if md.len() > 64 {
            return Some(path);
        }
    }
    // Local Jackett: keep the budget short so a hung server cannot stall Play.
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;
    let resp = client.get(link).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().ok()?;
    // A bencoded dict starts with 'd'; anything else is an HTML error page.
    if bytes.first() != Some(&b'd') {
        return None;
    }
    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::write(&path, &bytes).ok()?;
    Some(path)
}

/// magnet or a `.torrent` path — the two things webtorrent-cli streams from.
fn is_playable(source: &str) -> bool {
    crate::jackett::is_magnet(source) || source.ends_with(".torrent")
}

/// argv for one stream. Pure and unit-tested; no process is spawned here.
/// `None` = not a magnet, or a player we cannot drive.
pub(crate) fn play_args(source: &str, player: &str, file: Option<usize>) -> Option<Vec<String>> {
    let flag = player_flag(player)?;
    if !is_playable(source) {
        return None;
    }
    let mut args = vec![
        source.to_string(),
        flag.to_string(),
        // Without this, webtorrent-cli floats the player above every window.
        "--not-on-top".to_string(),
    ];
    // A pack streams its largest file by default; --select pins the choice.
    if let Some(idx) = file {
        args.push("--select".to_string());
        args.push(idx.to_string());
    }
    Some(args)
}

/// The stream we started, so a second Play (or app exit) can stop it.
static PLAYER: Mutex<Option<Child>> = Mutex::new(None);

/// Where the streamer's own output goes. Without it, "nothing plays" leaves no
/// trail: webtorrent prints the peer count it is stuck at, which is the only
/// way to tell a dead swarm from a blocked network.
pub(crate) fn log_path() -> std::path::PathBuf {
    dirs_next::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("torrentx")
        .join("webtorrent.log")
}

/// stdout + stderr both appended to `log_path()`; null when the file cannot be
/// created (a missing log must never stop playback).
fn log_stdio() -> (Stdio, Stdio) {
    let path = log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::File::create(&path).and_then(|f| f.try_clone().map(|c| (f, c))) {
        Ok((a, b)) => (Stdio::from(a), Stdio::from(b)),
        Err(_) => (Stdio::null(), Stdio::null()),
    }
}

/// Start streaming `magnet` to `player`. Err is a message suitable for a toast.
pub(crate) fn start_play(
    magnet: &str,
    link: Option<&str>,
    player: &str,
    file: Option<usize>,
) -> Result<(), String> {
    // Jackett's .torrent means webtorrent has names/sizes before any peer answers,
    // so mpv opens on the first connection instead of after the metadata round trip.
    let source = link
        .and_then(|l| torrent_file(magnet, l))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| magnet.to_string());
    let args = play_args(&source, player, file)
        .ok_or_else(|| "This result has no playable magnet".to_string())?;
    // One stream at a time: replacing the previous one avoids two players and
    // two torrents competing for bandwidth. webtorrent exits with its player.
    stop_play();
    let (out, err) = log_stdio();
    let child = Command::new("webtorrent")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                "Direct play needs webtorrent-cli — run: npm install -g webtorrent-cli".to_string()
            }
            _ => format!("Could not start webtorrent: {e}"),
        })?;
    if let Ok(mut p) = PLAYER.lock() {
        *p = Some(child);
    }
    Ok(())
}

/// Whether a stream we started is still running. Reaps a streamer that already
/// exited (its player was closed) so the UI stops offering "Stop".
pub(crate) fn is_playing() -> bool {
    let Ok(mut p) = PLAYER.lock() else {
        return false;
    };
    match p.as_mut().map(|c| c.try_wait()) {
        Some(Ok(Some(_))) | Some(Err(_)) => {
            *p = None;
            false
        }
        Some(Ok(None)) => true,
        None => false,
    }
}

/// Stop the stream we started (the player sees EOF and closes). No-op when
/// nothing is playing.
pub(crate) fn stop_play() {
    let Ok(mut p) = PLAYER.lock() else { return };
    if let Some(mut c) = p.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_playing, log_path, parse_file_list, play_args, player_flag, start_play, stop_play,
        torrent_file, PLAYERS,
    };

    fn magnet() -> String {
        format!("magnet:?xt=urn:btih:{}", "a".repeat(40))
    }

    #[test]
    fn builds_mpv_and_vlc_invocations() {
        let m = magnet();
        assert_eq!(
            play_args(&m, "mpv", None).unwrap(),
            vec![m.clone(), "--mpv".to_string(), "--not-on-top".to_string()]
        );
        assert_eq!(
            play_args(&m, "vlc", None).unwrap(),
            vec![m.clone(), "--vlc".to_string(), "--not-on-top".to_string()]
        );
        // Every advertised player has a webtorrent-cli flag.
        for p in PLAYERS {
            assert!(player_flag(p).is_some(), "{p} has no webtorrent flag");
        }
    }

    #[test]
    fn rejects_non_magnets_and_unknown_players() {
        assert!(play_args("https://example.com/x.mkv", "mpv", None).is_none());
        // A cached .torrent path is a valid source.
        assert!(play_args("/tmp/x.torrent", "mpv", None).is_some());
        assert!(play_args("magnet:?xt=urn:btih:tooshort", "mpv", None).is_none());
        assert!(play_args(&magnet(), "sh -c evil", None).is_none());
        assert!(play_args(&magnet(), "", None).is_none());
        // A magnet embedded in a longer string is not a magnet URI.
        assert!(play_args(&format!("x {}", magnet()), "mpv", None).is_none());
    }

    #[test]
    fn select_index_is_passed_to_webtorrent() {
        let m = magnet();
        let args = play_args(&m, "mpv", Some(3)).unwrap();
        assert_eq!(&args[args.len() - 2..], ["--select", "3"]);
        // No index → no --select, webtorrent keeps its largest-file default.
        assert!(!play_args(&m, "mpv", None)
            .unwrap()
            .iter()
            .any(|a| a == "--select"));
    }

    /// Jackett's own .torrent is fetched once, cached by magnet, and an HTML
    /// error page is refused (a bencoded dict starts with 'd').
    #[test]
    fn torrent_file_caches_the_jackett_copy_and_refuses_html() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let srv = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = srv.local_addr().unwrap();
        std::thread::spawn(move || {
            for _ in 0..2 {
                let Ok((mut s, _)) = srv.accept() else { return };
                let mut buf = [0u8; 1024];
                let n = s.read(&mut buf).unwrap_or(0);
                let bad = String::from_utf8_lossy(&buf[..n]).contains("/bad");
                let body: &[u8] = if bad {
                    b"<html>not a torrent</html>"
                } else {
                    b"d8:announce4:http4:infod4:name1:x6:lengthi1eee"
                };
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(body);
            }
        });

        let m = format!("magnet:?xt=urn:btih:{}&dn=stub", "c".repeat(40));
        let p = torrent_file(&m, &format!("http://{addr}/ok")).expect("bencoded body is cached");
        assert_eq!(std::fs::read(&p).unwrap().first(), Some(&b'd'));
        // Second call is served from the cache (the server only had two answers).
        assert_eq!(torrent_file(&m, &format!("http://{addr}/ok")), Some(p));

        let other = format!("magnet:?xt=urn:btih:{}&dn=other", "d".repeat(40));
        assert!(torrent_file(&other, &format!("http://{addr}/bad")).is_none());
        // Non-HTTP links never reach the network.
        assert!(torrent_file(&other, "file:///tmp/x.torrent").is_none());
    }

    #[test]
    fn parses_the_select_listing() {
        // Shape printed by webtorrent-cli with NO_COLOR set.
        let out = "Select a file to download:\n0  Sintel.mkv (128 MB)\n1  Sintel.srt (12 KB)\n10  Episode (with parens) (1.2 GB)\n\nTo select a specific file, re-run `webtorrent` with \"--select [index]\"\n";
        let f = parse_file_list(out);
        assert_eq!(f.len(), 3);
        assert_eq!(
            (f[0].idx, f[0].name.as_str(), f[0].size.as_str()),
            (0, "Sintel.mkv", "128 MB")
        );
        assert_eq!(f[1].size, "12 KB");
        // A name containing " (" must not truncate at the first one.
        assert_eq!(f[2].idx, 10);
        assert_eq!(f[2].name, "Episode (with parens)");
        assert_eq!(f[2].size, "1.2 GB");
        // Indented output (some terminals) still parses.
        assert_eq!(parse_file_list("  2  a.mkv (1 MB)").len(), 1);
        // Junk lines are skipped, not guessed at.
        assert!(parse_file_list("").is_empty());
        assert!(parse_file_list("Magnet: magnet:?xt=urn:btih:aa").is_empty());
    }

    /// End-to-end over the process boundary: a stub `webtorrent` on PATH records
    /// its argv and pid. Nothing else in this test binary spawns `webtorrent`.
    #[cfg(unix)]
    #[test]
    fn start_play_runs_webtorrent_with_the_stream_argv_and_stops_it() {
        use std::path::PathBuf;
        use std::time::{Duration, Instant};

        fn wait_for<F: Fn() -> bool>(f: F, secs: u64) -> bool {
            let deadline = Instant::now() + Duration::from_secs(secs);
            while Instant::now() < deadline {
                if f() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            f()
        }

        let dir: PathBuf =
            std::env::temp_dir().join(format!("tx-play-spawn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let args_file = dir.join("argv.txt");
        let pid_file = dir.join("pid.txt");
        let stub = dir.join("webtorrent");
        // `sleep 30` keeps the process alive so stop_play has something to kill.
        std::fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\necho $$ > '{}'\necho stub-stream-line\nsleep 30\n",
                args_file.display(),
                pid_file.display()
            ),
        )
        .unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let old_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{}:{old_path}", dir.display()));

        let m = magnet();
        assert!(start_play(&m, None, "mpv", None).is_ok());
        assert!(
            wait_for(|| args_file.exists(), 5),
            "stub never ran; PATH={}",
            std::env::var("PATH").unwrap_or_default()
        );
        assert_eq!(
            std::fs::read_to_string(&args_file)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            vec![m.as_str(), "--mpv", "--not-on-top"]
        );

        // The streamer's own output must land in the log: that peer count is the
        // only evidence a user has when no player window appears.
        assert!(
            wait_for(
                || std::fs::read_to_string(log_path())
                    .map(|l| l.contains("stub-stream-line"))
                    .unwrap_or(false),
                5
            ),
            "webtorrent output must be redirected to {}",
            log_path().display()
        );

        assert!(is_playing(), "a spawned streamer is a running stream");
        let pid: u32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        stop_play();
        assert!(!is_playing(), "stop_play must clear the playing state");
        assert!(
            wait_for(|| !PathBuf::from(format!("/proc/{pid}")).exists(), 5),
            "stop_play must end the streamer (pid {pid} still alive)"
        );

        std::env::set_var("PATH", old_path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
