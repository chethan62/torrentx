// ─── Direct play ───────────────────────────────────────────────────────────
// The app deliberately does no torrent I/O of its own: webtorrent-cli fetches
// pieces on demand (so seeking works) and mpv/vlc decode. All this module owns
// is the argv, one child process, and an honest error when it is missing.

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

/// argv for one stream. Pure and unit-tested; no process is spawned here.
/// `None` = not a magnet, or a player we cannot drive.
pub(crate) fn play_args(magnet: &str, player: &str) -> Option<Vec<String>> {
    let flag = player_flag(player)?;
    if !crate::jackett::is_magnet(magnet) {
        return None;
    }
    Some(vec![
        magnet.to_string(),
        flag.to_string(),
        // Without this, webtorrent-cli floats the player above every window.
        "--not-on-top".to_string(),
    ])
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
pub(crate) fn start_play(magnet: &str, player: &str) -> Result<(), String> {
    let args = play_args(magnet, player)
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
    use super::{log_path, play_args, player_flag, start_play, stop_play, PLAYERS};

    fn magnet() -> String {
        format!("magnet:?xt=urn:btih:{}", "a".repeat(40))
    }

    #[test]
    fn builds_mpv_and_vlc_invocations() {
        let m = magnet();
        assert_eq!(
            play_args(&m, "mpv").unwrap(),
            vec![m.clone(), "--mpv".to_string(), "--not-on-top".to_string()]
        );
        assert_eq!(
            play_args(&m, "vlc").unwrap(),
            vec![m.clone(), "--vlc".to_string(), "--not-on-top".to_string()]
        );
        // Every advertised player has a webtorrent-cli flag.
        for p in PLAYERS {
            assert!(player_flag(p).is_some(), "{p} has no webtorrent flag");
        }
    }

    #[test]
    fn rejects_non_magnets_and_unknown_players() {
        assert!(play_args("https://example.com/x.torrent", "mpv").is_none());
        assert!(play_args("magnet:?xt=urn:btih:tooshort", "mpv").is_none());
        assert!(play_args(&magnet(), "sh -c evil").is_none());
        assert!(play_args(&magnet(), "").is_none());
        // A magnet embedded in a longer string is not a magnet URI.
        assert!(play_args(&format!("x {}", magnet()), "mpv").is_none());
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
        assert!(start_play(&m, "mpv").is_ok());
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

        let pid: u32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        stop_play();
        assert!(
            wait_for(|| !PathBuf::from(format!("/proc/{pid}")).exists(), 5),
            "stop_play must end the streamer (pid {pid} still alive)"
        );

        std::env::set_var("PATH", old_path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
