// ─── Direct play ───────────────────────────────────────────────────────────
// The app deliberately does no torrent I/O of its own: webtorrent-cli fetches
// pieces on demand (so seeking works) and mpv/vlc decode. All this module owns
// is the argv, one child process, and an honest error when it is missing.

use crate::lock_or_recover;
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

/// Read the file list out of `webtorrent info <torrent>`.
///
/// `--select` with no index used to print the list and exit; webtorrent-cli 6
/// ignores it and starts DOWNLOADING instead (measured: a local pack gave
/// "torrent downloaded successfully", and a real 546-seeder magnet sat at
/// "verifying existing torrent data" until the deadline — i.e. the picker was
/// silently pulling the whole torrent). `info` prints JSON and exits.
#[derive(serde::Deserialize)]
struct InfoFile {
    path: String,
    length: u64,
}

pub(crate) fn parse_info_json(out: &str) -> Vec<FileEntry> {
    #[derive(serde::Deserialize)]
    struct Info {
        #[serde(default)]
        files: Vec<InfoFile>,
    }
    let Ok(info) = serde_json::from_str::<Info>(out) else {
        return vec![];
    };
    info.files
        .into_iter()
        .enumerate()
        .map(|(idx, f)| FileEntry {
            idx,
            name: f.path,
            size: crate::jackett::fmt_size(f.length),
        })
        .collect()
}

/// The torrents cache directory (same one `torrent_file` writes to).
fn torrents_dir() -> PathBuf {
    dirs_next::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("torrentx")
        .join("torrents")
}

/// The lower-case hex infohash of a magnet, if it has a v1 `btih`.
fn magnet_hash(magnet: &str) -> Option<String> {
    let i = magnet.find("urn:btih:")? + "urn:btih:".len();
    let hash: String = magnet[i..]
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect();
    (hash.len() == 40).then(|| hash.to_ascii_lowercase())
}

/// Fetch a magnet's metadata and return the `.torrent` webtorrent saved.
///
/// `webtorrent downloadmeta <magnet>` writes `<infohash>.torrent` into the
/// working directory — pointing that at the torrents cache means the picker and
/// the subsequent Play share one file (and Play stops paying the metadata round
/// trip). Needs peers, so it runs under a budget.
fn fetch_metadata(magnet: &str) -> Option<PathBuf> {
    let dir = torrents_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let mut child = Command::new("webtorrent")
        .args(["downloadmeta", magnet])
        .current_dir(&dir)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(100))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    // downloadmeta names the file after the infohash, so look for exactly that
    // first — the cache holds other torrents and "newest wins" could hand back
    // the previous pick.
    if let Some(hash) = magnet_hash(magnet) {
        let want = dir.join(format!("{hash}.torrent"));
        if std::fs::metadata(&want).is_ok_and(|m| m.len() > 64) {
            return Some(want);
        }
    }
    std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "torrent"))
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// The torrent's file list, from metadata only — no data is downloaded.
///
/// Runs off the UI thread. A magnet needs peers for its metadata, so a dead
/// swarm reports that instead of an empty picker.
pub(crate) fn list_files(
    magnet: String,
    link: Option<String>,
    tx: std::sync::mpsc::Sender<Result<Vec<FileEntry>, String>>,
) {
    std::thread::spawn(move || {
        let source = torrent_file(&magnet, link.as_deref()).or_else(|| fetch_metadata(&magnet));
        let Some(path) = source else {
            let _ = tx.send(Err(
                "No metadata — the torrent's peers are unreachable".to_string()
            ));
            // The UI loop parks when nothing is happening; without this the
            // picker only appeared once the user moved the mouse.
            crate::wake_ui();
            return;
        };
        let out = Command::new("webtorrent")
            .args(["info", &path.to_string_lossy()])
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        let files = match out {
            Ok(o) => parse_info_json(&String::from_utf8_lossy(&o.stdout)),
            Err(e) => {
                let _ = tx.send(Err(format!("Could not read the torrent: {e}")));
                return;
            }
        };
        let _ = tx.send(if files.is_empty() {
            Err("This torrent has no files to choose from".to_string())
        } else {
            Ok(files)
        });
        crate::wake_ui();
    });
}

/// A local copy of the result's `.torrent`, fetched from the Jackett link.
/// Jackett serves it from its own cache, so this is a file fetch — no swarm
/// metadata round trip — and webtorrent can name/list files immediately.
/// None when there is no usable link; the caller falls back to the magnet.
pub(crate) fn torrent_file(magnet: &str, link: Option<&str>) -> Option<PathBuf> {
    // The cache is consulted FIRST: a magnet-only result (no Jackett link) still
    // has a .torrent here if the picker fetched the metadata.
    // Key the cache on the INFOHASH: `fetch_metadata` writes `<btih>.torrent` when
    // the picker pulls metadata for a magnet, so Play can reuse that file instead
    // of paying the peer round trip again (it used to hash the magnet string, so
    // the two paths never met).
    let hash = magnet_hash(magnet)?;
    let path = torrents_dir().join(format!("{hash}.torrent"));
    // ponytail: no TTL — a torrent file for one infohash never changes.
    if let Ok(md) = std::fs::metadata(&path)
        && md.len() > 64
    {
        return Some(path);
    }
    let link = link?;
    if !link.starts_with("http") {
        return None;
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

/// Where webtorrent writes the pieces it streams.
fn stream_dir() -> PathBuf {
    dirs_next::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("torrentx")
        .join("stream")
}

/// Delete what a finished stream downloaded.
///
/// webtorrent keeps every piece it fetched in `--out`; the app only ever streams
/// one thing at a time, so once a stream stops that data is garbage — and without
/// this it stayed: one season-pack session left 3.1 GB behind.
fn clear_stream_data(dir: &std::path::Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let _ = if p.is_dir() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
    }
}

/// Clear the stream cache on a background thread — it can be gigabytes, and the
/// UI thread must not wait for a recursive delete.
pub(crate) fn prune_stream_cache() {
    std::thread::spawn(|| clear_stream_data(&stream_dir()));
}

/// A free localhost port for webtorrent's streaming server.
///
/// Its default is 8000, and a leftover webtorrent (or anything else) on that port
/// makes it hang at "verifying existing torrent data…" with no player and no
/// error — measured on this box, where another WebTorrent held 8000 for two
/// hours. Bind-and-drop leaves a small race, which is fine: the failure is a
/// missing player, not a wrong file.
fn free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()?
        .local_addr()
        .ok()
        .map(|a| a.port())
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
    let source = torrent_file(magnet, link)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| magnet.to_string());
    let args = play_args(&source, player, file)
        .ok_or_else(|| "This result has no playable magnet".to_string())?;
    // One stream at a time: replacing the previous one avoids two players and
    // two torrents competing for bandwidth. webtorrent exits with its player.
    stop_play();
    // webtorrent streams into /tmp by default; on this box /tmp is a 7.8 GB tmpfs,
    // so a season pack either filled RAM or stalled re-verifying gigabytes of
    // sparse files on the next attempt ("verifying existing torrent data…" and no
    // player). Stream into the cache partition instead.
    let out_dir = stream_dir();
    let _ = std::fs::create_dir_all(&out_dir);
    let (out, err) = log_stdio();
    let mut child_cmd = Command::new("webtorrent");
    child_cmd.args(&args).arg("--out").arg(&out_dir);
    if let Some(port) = free_port() {
        child_cmd.arg("--port").arg(port.to_string());
    }
    let child = child_cmd
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
    *lock_or_recover(&PLAYER) = Some(child);
    Ok(())
}

/// Whether a stream we started is still running. Reaps a streamer that already
/// exited (its player was closed) so the UI stops offering "Stop".
pub(crate) fn is_playing() -> bool {
    let mut p = lock_or_recover(&PLAYER);
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
    let mut p = lock_or_recover(&PLAYER);
    let stopped = if let Some(mut c) = p.take() {
        let _ = c.kill();
        let _ = c.wait();
        true
    } else {
        false
    };
    drop(p);
    // The child is gone, so its data is garbage: a 1.5 GB episode left 3.1 GB of
    // pieces behind and nothing ever removed them.
    if stopped {
        prune_stream_cache();
    }
}

#[cfg(test)]
mod stream_cache_tests {
    use super::clear_stream_data;

    /// Streamed pieces are garbage once the stream stops: one session left 3.1 GB
    /// in the cache dir, and the next Play then stalled re-verifying it.
    #[test]
    fn pruning_empties_the_stream_dir_and_leaves_the_dirs_above_it() {
        let root = std::env::temp_dir().join(format!("txstream{}", std::process::id()));
        let dir = root.join("stream");
        let nested = dir.join("Some.Torrent.Name");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("ep1.mkv"), b"pieces").unwrap();
        std::fs::write(dir.join("loose.part"), b"x").unwrap();

        clear_stream_data(&dir);

        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "dir is empty");
        assert!(dir.exists(), "the stream dir itself stays");
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PLAYERS, is_playing, log_path, parse_info_json, play_args, player_flag, start_play,
        stop_play, torrent_file,
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
        assert!(
            !play_args(&m, "mpv", None)
                .unwrap()
                .iter()
                .any(|a| a == "--select")
        );
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
                // >64 bytes: the cache refuses a tiny/truncated file, so a real
                // .torrent (which is tens of KB) is what the threshold targets.
                let body: &[u8] = if bad {
                    b"<html>not a torrent</html>"
                } else {
                    concat!(
                        "d8:announce4:http7:comment80:",
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "4:infod4:name1:x6:lengthi1eee"
                    )
                    .as_bytes()
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
        let p =
            torrent_file(&m, Some(&format!("http://{addr}/ok"))).expect("bencoded body is cached");
        assert_eq!(std::fs::read(&p).unwrap().first(), Some(&b'd'));
        // Second call is served from the cache (the server only had two answers).
        assert_eq!(
            torrent_file(&m, Some(&format!("http://{addr}/ok"))),
            Some(p.clone())
        );

        let other = format!("magnet:?xt=urn:btih:{}&dn=other", "d".repeat(40));
        assert!(torrent_file(&other, Some(&format!("http://{addr}/bad"))).is_none());
        // Non-HTTP links never reach the network.
        assert!(torrent_file(&other, Some("file:///tmp/x.torrent")).is_none());
        // A magnet-only result (no Jackett link) still finds the .torrent the
        // picker's metadata fetch cached under its infohash.
        assert_eq!(
            torrent_file(&m, None),
            Some(p.clone()),
            "cached file reused"
        );
    }

    #[test]
    fn info_json_becomes_the_picker_entries() {
        // Real shape of `webtorrent info <torrent>`; sizes come from bytes.
        let json = r#"{
          "name": "Lanterns.2026.S01.MULTi",
          "files": [
            {"path": "Lanterns.2026.S01E01.1080p.WEB.H264-FRQC.mkv", "name": "E01.mkv", "length": 2115271393},
            {"path": "Lanterns.2026.S01E02.1080p.WEB.H264-FRQC.mkv", "name": "E02.mkv", "length": 1707249500}
          ],
          "pieceLength": 16384
        }"#;
        let f = parse_info_json(json);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].idx, 0);
        assert!(f[0].name.ends_with("S01E01.1080p.WEB.H264-FRQC.mkv"));
        assert_eq!(f[0].size, crate::jackett::fmt_size(2115271393));
        assert_eq!(f[1].idx, 1);
        // Anything that is not the info JSON is an empty list, not a panic.
        assert!(parse_info_json("fetching torrent metadata from 2 peers").is_empty());
        assert!(parse_info_json("").is_empty());
        assert!(parse_info_json(r#"{"name":"x"}"#).is_empty());
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
        // SAFETY: test-only; the helper threads it starts read PATH after
        // this write, and no other test touches PATH concurrently.
        unsafe { std::env::set_var("PATH", format!("{}:{old_path}", dir.display())) };

        let m = magnet();
        assert!(start_play(&m, None, "mpv", None).is_ok());
        // Wait for the *contents*, not for the file to exist: the shell creates
        // the redirect target before it writes, so an existence check passes on
        // an empty file and the test flakes on a slow (CI) machine.
        let recorded = || std::fs::read_to_string(&args_file).unwrap_or_default();
        assert!(
            wait_for(|| recorded().lines().count() >= 3, 5),
            "stub never recorded argv (got {:?}); PATH={}",
            recorded(),
            std::env::var("PATH").unwrap_or_default()
        );
        let rec: Vec<String> = recorded().lines().map(str::to_string).collect();
        assert_eq!(
            &rec[..3],
            [m.as_str(), "--mpv", "--not-on-top"],
            "got {rec:?}"
        );
        // Streams land in the cache partition, not webtorrent's /tmp default.
        assert_eq!(rec[3], "--out", "got {rec:?}");
        assert!(
            rec[4].ends_with("torrentx/stream"),
            "unexpected --out {}",
            rec[4]
        );
        // A free streaming port: 8000 is webtorrent's default and a squatter there
        // hangs the stream with no player and no error.
        assert_eq!(rec[5], "--port", "got {rec:?}");
        assert!(
            rec[6].parse::<u16>().is_ok_and(|p| p > 1024),
            "unexpected port {}",
            rec[6]
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

        // SAFETY: restores the value written above, same single-threaded test.
        unsafe { std::env::set_var("PATH", old_path) };
        let _ = std::fs::remove_dir_all(&dir);
    }
}
