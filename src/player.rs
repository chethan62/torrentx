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

/// How long a leader may own its slot. Above `fetch_metadata_once`'s 30 s budget,
/// so a wedged leader (or one whose thread died) cannot block the picker forever.
const SLOT_TTL: std::time::Duration = std::time::Duration::from_secs(35);

/// State of one infohash's metadata fetch, shared by every caller that wants it.
#[derive(Default)]
enum FetchSlot {
    #[default]
    Idle,
    /// Owned since this instant; the lease expires after the TTL.
    Running {
        since: std::time::Instant,
        /// Callers parked on this slot right now.
        waiters: usize,
        /// Set when the last waiter gives up: the leader kills its child.
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    },
    Done(Option<PathBuf>),
}

type Slot = std::sync::Arc<(std::sync::Mutex<FetchSlot>, std::sync::Condvar)>;

static FETCHING: std::sync::Mutex<Option<std::collections::HashMap<String, Slot>>> =
    std::sync::Mutex::new(None);

fn slot_for(key: &str) -> Slot {
    let mut map = crate::lock_or_recover(&FETCHING);
    std::sync::Arc::clone(
        map.get_or_insert_with(Default::default)
            .entry(key.to_string())
            .or_insert_with(|| std::sync::Arc::new(Default::default())),
    )
}

/// Run `fetch` once per `key`, however many callers arrive at once.
///
/// The detail pane starts a listing when a row is selected and the picker click
/// starts another for the same magnet: two `downloadmeta` runs on one infohash
/// cost 10.2 s instead of 5.2 s, and the second threw the first's work away.
/// Different magnets still fetch in parallel — the lock is per infohash.
fn single_flight(key: &str, fetch: impl FnOnce(&Cancel) -> Option<PathBuf>) -> Option<PathBuf> {
    single_flight_within(key, SLOT_TTL, fetch)
}

/// Set by the last waiter that gives up. `fetch` polls it and abandons the work
/// (killing its child) so a closed picker leaves nothing downloading.
pub(crate) type Cancel = std::sync::atomic::AtomicBool;

fn single_flight_within(
    key: &str,
    ttl: std::time::Duration,
    fetch: impl FnOnce(&Cancel) -> Option<PathBuf>,
) -> Option<PathBuf> {
    use std::sync::atomic::Ordering;
    let gate = slot_for(key);
    let (state, cv) = &*gate;
    let mut slot = crate::lock_or_recover(state);
    loop {
        // `Reuse` carries the leader's answer; `Lead`/`Wait` decide under the lock.
        let step = match &mut *slot {
            FetchSlot::Done(v) => Some(Some(v.clone())),
            FetchSlot::Idle => None,
            // The lease expired: whoever gets here first takes over, so a leader
            // that wedged (or died between states) cannot own the slot forever.
            FetchSlot::Running { since, .. } if since.elapsed() > ttl => None,
            FetchSlot::Running { waiters, .. } => {
                *waiters += 1;
                let (guard, wait) = cv
                    .wait_timeout(slot, ttl)
                    .unwrap_or_else(|e| e.into_inner());
                slot = guard;
                let timed_out = wait.timed_out();
                let mut bailed = false;
                if let FetchSlot::Running {
                    waiters, cancel, ..
                } = &mut *slot
                {
                    *waiters = waiters.saturating_sub(1);
                    if timed_out && *waiters == 0 {
                        // Nobody is waiting any more: tell the leader to stop
                        // instead of leaving a webtorrent chewing a dead swarm.
                        cancel.store(true, Ordering::Relaxed);
                        bailed = true;
                    }
                }
                if bailed || (timed_out && matches!(&*slot, FetchSlot::Running { .. })) {
                    // Out of patience: report "no metadata" instead of hanging
                    // the picker behind someone else's fetch.
                    return None;
                }
                Some(None)
            }
        };
        match step {
            Some(Some(answer)) => return answer,
            // Woken (or arrived) onto a finished slot: read it on the next turn.
            Some(None) => continue,
            None => break,
        }
    }
    let cancel = std::sync::Arc::new(Cancel::new(false));
    *slot = FetchSlot::Running {
        since: std::time::Instant::now(),
        waiters: 0,
        cancel: std::sync::Arc::clone(&cancel),
    };
    drop(slot);

    // A panicking fetch must still release the slot, or every waiter blocks on a
    // condvar nobody will notify.
    let answer =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fetch(&cancel))).unwrap_or(None);

    *crate::lock_or_recover(state) = FetchSlot::Done(answer.clone());
    cv.notify_all();
    // Later callers find the .torrent on disk; the slot would only grow the map.
    if let Some(map) = crate::lock_or_recover(&FETCHING).as_mut() {
        map.remove(key);
    }
    answer
}

/// Abandon this torrent's metadata fetch. The picker calls it when the user moves
/// to a different row: the listing thread nobody will read stops waiting, and the
/// leader kills its `webtorrent` child instead of polling a dead swarm for 30 s.
pub(crate) fn abandon(magnet: &str) {
    let Some(hash) = magnet_hash(magnet) else {
        return;
    };
    let gate = slot_for(&hash);
    if let FetchSlot::Running { cancel, .. } = &*crate::lock_or_recover(&gate.0) {
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Fetch a magnet's metadata and return the `.torrent` webtorrent saved./// Fetch a magnet's metadata and return the `.torrent` webtorrent saved./// Fetch a magnet's metadata and return the `.torrent` webtorrent saved./// Fetch a magnet's metadata and return the `.torrent` webtorrent saved.
///
/// `webtorrent downloadmeta <magnet>` writes `<infohash>.torrent` into the
/// working directory — pointing that at the torrents cache means the picker and
/// the subsequent Play share one file (and Play stops paying the metadata round
/// trip). Needs peers, so it runs under a budget.
fn fetch_metadata(magnet: &str) -> Option<PathBuf> {
    match magnet_hash(magnet) {
        Some(hash) => single_flight(&hash, |cancel| fetch_metadata_once(magnet, cancel)),
        None => fetch_metadata_once(magnet, &Cancel::new(false)),
    }
}

/// The uncached body: one `webtorrent downloadmeta` under a 30 s budget, killed
/// early when `cancel` is set (every waiter gave up).
/// `downloadmeta`, retried once. Real swarms fail transiently: the same magnet took
/// 4 s and wrote the file on one run, exited a second later with "0 peers" on the
/// next, hung on a third. One retry inside the same 30 s budget turns that into a
/// listing instead of an error.
fn fetch_metadata_once(magnet: &str, cancel: &Cancel) -> Option<PathBuf> {
    // The budget covers both attempts, but each attempt gets its own cap: a
    // peerless magnet HANGS on "fetching torrent metadata from 0 peers" rather
    // than exiting, so a single 30 s attempt would spend the whole budget and make
    // the retry unreachable. 12 s + 12 s + 0.7 s stays inside the 30 s.
    let overall = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut attempt = 0;
    loop {
        attempt += 1;
        let deadline =
            (std::time::Instant::now() + std::time::Duration::from_secs(12)).min(overall);
        if let Some(path) = download_metadata(magnet, cancel, deadline) {
            return Some(path);
        }
        // Cancelled work is never retried, and the second attempt cannot run past
        // the budget.
        if cancel.load(std::sync::atomic::Ordering::Relaxed)
            || attempt >= 2
            || std::time::Instant::now() >= overall
        {
            return None;
        }
        // A beat before the second try: peers that answered "0 peers" need a moment
        // to answer differently, and it costs nothing on a healthy swarm.
        std::thread::sleep(std::time::Duration::from_millis(700));
    }
}

fn download_metadata(
    magnet: &str,
    cancel: &Cancel,
    deadline: std::time::Instant,
) -> Option<PathBuf> {
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
    let started = std::time::SystemTime::now();
    let mut killed = false;
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            killed = true;
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if !killed && std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(100))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                // A killed child may have left a partial .torrent: never hand it
                // back as if it were real.
                if killed {
                    return None;
                }
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
    // Fallback for a name we did not predict: only a file this run wrote counts.
    // "Newest wins" alone handed back the previous pick's torrent (and 46-byte
    // stubs), so the picker listed the wrong film's files.
    std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "torrent"))
        .filter(|p| written_since(p, started))
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// How many file listings are running right now. The detail pane starts one per
/// selected row, so holding ArrowDown would otherwise spawn a `webtorrent` process
/// per row, each burning a 30 s peer round trip.
pub(crate) fn metadata_in_flight() -> usize {
    IN_FLIGHT.load(std::sync::atomic::Ordering::Relaxed)
}

static IN_FLIGHT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Decrements `IN_FLIGHT` however the listing thread ends.
struct FlightGuard;
impl Drop for FlightGuard {
    fn drop(&mut self) {
        IN_FLIGHT.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
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
    IN_FLIGHT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::thread::spawn(move || {
        let _flying = FlightGuard;
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

/// A `.torrent` this run wrote: big enough to be real, and newer than `since`.
/// (`modified()` has 1 s granularity on some filesystems, so allow the boundary.)
fn written_since(path: &std::path::Path, since: std::time::SystemTime) -> bool {
    std::fs::metadata(path).is_ok_and(|m| {
        m.len() > 64
            && m.modified()
                .is_ok_and(|t| t + std::time::Duration::from_secs(2) >= since)
    })
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
    /// Tests that rewrite `PATH` must not run concurrently: `env::set_var` is
    /// process-wide, so two of them fight and one sees the other's stub.
    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
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

        let _serial = crate::lock_or_recover(&PATH_LOCK);

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

    /// Two callers for the same infohash must share ONE fetch (and both get its
    /// answer); two different infohashes must not block each other.
    #[test]
    fn single_flight_runs_one_fetch_per_key() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        let runs = Arc::new(AtomicUsize::new(0));
        let mut hs = Vec::new();
        for _ in 0..4 {
            let runs = Arc::clone(&runs);
            hs.push(std::thread::spawn(move || {
                super::single_flight("hash-a", |_| {
                    runs.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(120));
                    Some(std::path::PathBuf::from("/tmp/a.torrent"))
                })
            }));
        }
        let answers: Vec<_> = hs.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(
            runs.load(Ordering::SeqCst),
            1,
            "the fetch ran more than once"
        );
        assert!(
            answers
                .iter()
                .all(|a| a.as_deref() == Some(std::path::Path::new("/tmp/a.torrent"))),
            "a waiter got a different answer: {answers:?}"
        );

        // A second key is independent: it runs while the first is finished.
        let other = super::single_flight("hash-b", |_| {
            runs.fetch_add(1, Ordering::SeqCst);
            None
        });
        assert_eq!(other, None);
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    /// Moving to another row abandons the old listing: the leader must kill its
    /// child, not poll a swarm nobody is waiting on.
    #[test]
    fn abandon_stops_the_leader() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant};

        let hash = "ab".repeat(20);
        let magnet = format!("magnet:?xt=urn:btih:{hash}&dn=x");
        let saw = Arc::new(AtomicBool::new(false));
        let s2 = Arc::clone(&saw);
        let leader = std::thread::spawn(move || {
            super::single_flight_within(&hash, Duration::from_secs(30), |cancel| {
                let t0 = Instant::now();
                while t0.elapsed() < Duration::from_secs(3) {
                    if cancel.load(Ordering::Relaxed) {
                        s2.store(true, Ordering::SeqCst);
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Some(std::path::PathBuf::from("/tmp/late.torrent"))
            })
        });
        std::thread::sleep(Duration::from_millis(60));
        super::abandon(&magnet);
        assert_eq!(leader.join().unwrap(), None, "the leader kept fetching");
        assert!(
            saw.load(Ordering::SeqCst),
            "the leader never saw the cancel"
        );
    }

    /// When the last waiter gives up, the leader must notice and abandon its work
    /// (in the app: kill the webtorrent child) instead of fetching for nobody.
    #[test]
    fn the_last_waiter_bailing_cancels_the_leader() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, Instant};

        let aborted = Arc::new(AtomicBool::new(false));
        let a = Arc::clone(&aborted);
        let leader = std::thread::spawn(move || {
            super::single_flight_within("cancel-me", Duration::from_secs(30), |cancel| {
                let t0 = Instant::now();
                // Stands in for fetch_metadata_once's 100 ms poll loop.
                while t0.elapsed() < Duration::from_secs(3) {
                    if cancel.load(Ordering::Relaxed) {
                        a.store(true, Ordering::SeqCst);
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Some(std::path::PathBuf::from("/tmp/late.torrent"))
            })
        });
        std::thread::sleep(Duration::from_millis(50));
        // One waiter with a short lease: it times out, is the last one, so it cancels.
        let gave_up = super::single_flight_within("cancel-me", Duration::from_millis(80), |_| {
            Some(std::path::PathBuf::from("/tmp/waiter.torrent"))
        });
        let answer = leader.join().unwrap();
        assert_eq!(gave_up, None, "the bailing waiter must not lead the fetch");
        assert_eq!(answer, None, "the leader kept fetching for nobody");
        assert!(
            aborted.load(Ordering::SeqCst),
            "the leader never saw the cancel"
        );
    }

    /// A leader that wedges past its lease must not own the slot forever: the next
    /// caller takes over instead of waiting behind dead work.
    #[test]
    fn an_expired_lease_is_taken_over() {
        use std::time::Duration;
        let ttl = Duration::from_millis(80);
        let slow = std::thread::spawn(|| {
            super::single_flight_within("lease-a", std::time::Duration::from_millis(80), |_| {
                std::thread::sleep(std::time::Duration::from_millis(500));
                Some(std::path::PathBuf::from("/tmp/slow.torrent"))
            })
        });
        std::thread::sleep(Duration::from_millis(150));
        let taken = super::single_flight_within("lease-a", ttl, |_| {
            Some(std::path::PathBuf::from("/tmp/taken.torrent"))
        });
        assert_eq!(
            taken.as_deref(),
            Some(std::path::Path::new("/tmp/taken.torrent")),
            "the expired lease was not taken over"
        );
        let _ = slow.join();
    }

    /// A panicking fetch used to leave the slot Running for good, so every waiter
    /// blocked on a condvar nobody would notify.
    #[test]
    fn a_panicking_fetch_releases_the_waiter() {
        use std::time::{Duration, Instant};
        let first = std::thread::spawn(|| {
            let _ = super::single_flight_within("boom", Duration::from_secs(30), |_| {
                // Panic only once the waiter is certainly attached, or it would
                // find an Idle slot (the finished one is removed) and lead itself.
                std::thread::sleep(Duration::from_millis(200));
                panic!("leader died mid-fetch");
            });
        });
        std::thread::sleep(Duration::from_millis(50));
        let t0 = Instant::now();
        let answer = super::single_flight_within("boom", Duration::from_secs(30), |_| {
            Some(std::path::PathBuf::from("/tmp/never.torrent"))
        });
        assert!(t0.elapsed() < Duration::from_secs(2), "the waiter hung");
        assert_eq!(answer, None, "a dead leader must report no metadata");
        let _ = first.join();
    }

    /// A `downloadmeta` that exits without metadata (the "0 peers" case real swarms
    /// hit) must be retried once, and the retry's file must be the answer.
    #[test]
    fn a_metadata_run_that_exits_empty_is_retried_once() {
        use std::path::PathBuf;

        let _serial = crate::lock_or_recover(&PATH_LOCK);

        // Unique per run so it cannot collide with another test's cached torrent.
        let hash = format!("{:040x}", (std::process::id() as u64) * 7919);
        let dir: PathBuf = std::env::temp_dir().join(format!("tx-meta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let counter = dir.join("count");
        let stub = dir.join("webtorrent");
        // First call: "no peers", exit without writing. Second: write a real-sized
        // .torrent into the working directory, which the app sets to the cache.
        std::fs::write(
            &stub,
            format!(
                "#!/bin/sh\nn=$(cat '{c}' 2>/dev/null || echo 0)\nn=$((n+1))\necho $n > '{c}'\nif [ \"$1\" = downloadmeta ] && [ \"$n\" -ge 2 ]; then head -c 2048 /dev/zero | tr '\\0' 'd' > '{h}.torrent'; fi\nexit 0\n",
                c = counter.display(),
                h = hash
            ),
        )
        .unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let old_path = std::env::var("PATH").unwrap_or_default();
        // SAFETY: test-only; the stub spawned here is the only consumer of PATH.
        unsafe { std::env::set_var("PATH", format!("{}:{}", dir.display(), old_path)) };
        let got = super::fetch_metadata_once(
            &format!("magnet:?xt=urn:btih:{hash}&dn=x"),
            &super::Cancel::new(false),
        );
        unsafe { std::env::set_var("PATH", &old_path) };

        let runs: u32 = std::fs::read_to_string(&counter)
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(0);
        let cached = super::torrents_dir().join(format!("{hash}.torrent"));
        let _ = std::fs::remove_file(&cached);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(runs, 2, "the empty run was not retried");
        assert_eq!(
            got.as_deref(),
            Some(cached.as_path()),
            "the retry's file was not returned"
        );
    }

    /// The "newest .torrent" fallback used to accept a file from a previous pick
    /// (or a 46-byte stub), so the picker listed the wrong torrent's files.
    #[test]
    fn a_metadata_fallback_only_trusts_a_file_this_run_wrote() {
        let dir = std::env::temp_dir().join(format!("tx-pf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let stub = dir.join("stub.torrent");
        let real = dir.join("real.torrent");
        std::fs::write(&stub, vec![b'd'; 46]).unwrap();
        std::fs::write(&real, vec![b'd'; 4096]).unwrap();

        let since = std::time::SystemTime::now();
        assert!(
            super::written_since(&real, since),
            "a real file written now counts"
        );
        assert!(
            !super::written_since(&stub, since),
            "a 46-byte stub must never count"
        );

        // Old file: predate it by an hour.
        let old = since - std::time::Duration::from_secs(3600);
        let f = std::fs::File::options().write(true).open(&real).unwrap();
        f.set_modified(old).unwrap();
        assert!(
            !super::written_since(&real, since),
            "a stale torrent must not count"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
