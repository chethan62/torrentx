# Plan — Direct Play (stream a torrent to mpv/vlc)

## Goal

Add a **Play** action to TorrentX that streams the biggest file in a torrent result to
the local video player without downloading the whole torrent first.

## Current context / assumptions

- Repo: `/home/chethan/projects/torrentx` (Rust 2021, egui 0.36 / eframe, `cargo 1.97.1`).
  HEAD is `40d3dcb` (RSS fixes), tree clean, branch `main`. History is direct commits to
  `main` — no PR flow.
- The app has **no video decoder** and egui cannot play video. Playback must be handed to an
  external player. `mpv` (`/usr/sbin/mpv`) and `vlc` (`/usr/sbin/vlc`) are installed on this
  machine; **no torrent-streaming CLI is installed** (`webtorrent`, `peerflix` absent), but
  `node v22.23.1` + npm are.
- Streaming is done by **`webtorrent-cli`**: `webtorrent <magnet> --mpv --not-on-top` serves the
  torrent over `http://localhost:8000` with HTTP range support and launches mpv on that URL.
  Verified from `webtorrent-cli/master/bin/cmd.js`:
  - no `--select` ⇒ it picks the **largest file** (`if no index specified, use largest file`);
  - `--mpv`, `--vlc`, `--not-on-top` are real options;
  - the player quits ⇒ webtorrent exits (no `--no-quit`).
- The app already spawns and owns a child process for Jackett (`src/jackett.rs`:
  `JACKETT_CHILD`, `stop_local()`), so the same lifecycle pattern is reused — no new crate,
  no tokio, no in-process BitTorrent stack.
- **Interpretation assumed:** "direct play" = watch while downloading (streaming to an
  external player). In-app embedded playback is listed under *Open questions* and is out of scope.

## Architecture / proposed approach

New leaf module `src/player.rs`: it validates the magnet, maps a player name to a fixed
webtorrent-cli flag, builds the argv (pure, unit-tested), spawns `webtorrent` and owns the
child so a second Play or app exit stops the previous stream. UI calls it from the detail
panel and from a `P` shortcut; `Config` gains one field (`player: "mpv" | "vlc"`).

Dependency direction stays acyclic: `player` → `jackett::is_magnet` + `std`; `ui` → `player`;
`player` imports nothing from `ui`/`app`.

## Step-by-step tasks

### Task 1 — `src/player.rs`: argv builder (TDD)

1. Write the file with a **stub** so the test compiles and fails:

```rust
// ─── Direct play (stream a magnet to an external player) ───────────────────
// No in-process BitTorrent stack: webtorrent-cli already fetches pieces
// on demand (range requests, seeking works) and mpv/vlc already decode.
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// Players we can drive, in menu order. The config value is matched against
/// this list; it is never interpolated into a command line.
pub(crate) const PLAYERS: [&str; 2] = ["mpv", "vlc"];

pub(crate) fn player_flag(_player: &str) -> Option<&'static str> {
    None
}

pub(crate) fn play_args(_magnet: &str, _player: &str) -> Option<Vec<String>> {
    None
}

#[cfg(test)]
mod tests {
    use super::{play_args, player_flag, PLAYERS};

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
    }
}
```

2. Register the module — `src/main.rs`, next to the other `mod` lines near the top:

```rust
mod player;
```

3. Run it and confirm it is **red**:

```bash
cd ~/projects/torrentx && cargo test --bin torrentx -- player 2>&1 | tail -15
```

Expected: `test result: FAILED. 0 passed; 2 failed` (both `play_args` returns `None`).

4. Implement minimally (replace the two stubs):

```rust
pub(crate) fn player_flag(player: &str) -> Option<&'static str> {
    match player {
        "mpv" => Some("--mpv"),
        "vlc" => Some("--vlc"),
        _ => None,
    }
}

/// Build the webtorrent-cli argv for one magnet. Pure — unit-tested without
/// spawning anything. `None` = not a magnet, or a player we don't support.
pub(crate) fn play_args(magnet: &str, player: &str) -> Option<Vec<String>> {
    let flag = player_flag(player)?;
    if !crate::jackett::is_magnet(magnet) {
        return None;
    }
    Some(vec![
        magnet.to_string(),
        flag.to_string(),
        // webtorrent-cli floats the player above other windows by default;
        // a stream should sit in the normal stack.
        "--not-on-top".to_string(),
    ])
}
```

5. Run again — expected `test result: ok. 2 passed`.

6. Commit: `git add src/player.rs src/main.rs && git commit -m "feat(player): build webtorrent-cli argv for direct play"`

### Task 2 — `src/player.rs`: spawn + stop

Append to the same file (kept in Task 1's commit if convenient; otherwise its own commit):

```rust
/// The stream we started, so a second Play (or app exit) can stop it.
static PLAYER: Mutex<Option<Child>> = Mutex::new(None);

/// Start streaming `magnet` to `player`. Err = message suitable for a toast.
pub(crate) fn start_play(magnet: &str, player: &str) -> Result<(), String> {
    let args =
        play_args(magnet, player).ok_or("This result has no playable magnet".to_string())?;
    // One stream at a time: playing again replaces the previous player
    // instead of stacking two torrents and two player windows.
    stop_play();
    let child = Command::new("webtorrent")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            "Direct play needs webtorrent-cli — run: npm install -g webtorrent-cli".to_string()
        })?;
    if let Ok(mut p) = PLAYER.lock() {
        *p = Some(child);
    }
    Ok(())
}

/// Stop the stream we started (kills the webtorrent process; the player sees
/// EOF and closes). No-op when nothing is playing.
pub(crate) fn stop_play() {
    let Ok(mut p) = PLAYER.lock() else { return };
    if let Some(mut c) = p.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}
```

Verification:

```bash
cd ~/projects/torrentx && cargo test --bin torrentx -- player 2>&1 | tail -5 && cargo build 2>&1 | tail -3
```

Expected: `test result: ok. 2 passed`, then `Finished dev profile`.

### Task 3 — Config: `player` field

1. **Test first** — in `src/config.rs`, inside `mod tests`, add to
   `config_defaults_apply_for_missing_fields` (the test that loads an old JSON without the
   newer fields):

```rust
        assert_eq!(old.player, "mpv"); // serde default: mpv
```

Run: `cargo test --bin torrentx -- config_defaults_apply_for_missing_fields 2>&1 | tail -8`
Expected: **compile error** `no field 'player' on type 'Config'` (this is the red step).

2. Implement — three edits in `src/config.rs`:

```rust
    /// Player driven by direct play ("mpv" | "vlc"); matched against
    /// `player::PLAYERS`, never passed to a shell.
    #[serde(default = "default_player")]
    pub(crate) player: String,
```

(add after `pub(crate) check_updates: bool,`); in `impl Default for Config` add
`player: default_player(),`; and near `fn default_true()`:

```rust
fn default_player() -> String {
    "mpv".into()
}
```

3. Run: `cargo test --bin torrentx -- config 2>&1 | tail -8` — expected `ok. 8 passed`
(config module tests; count may be 8–9).

### Task 4 — Play icon asset + `SvgIcon::Play`

1. Create `assets/icons/play.svg` (Lucide `play`, stroke = currentColor, same shape as the
   other icons):

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="6 3 20 12 6 21 6 3"/></svg>
```

2. In `src/main.rs`, three mechanical edits:

- enum: add `Play,` after `Download,` in `pub(crate) enum SvgIcon`;
- `uri()`: `SvgIcon::Play => "bytes://tx/play.svg",`
- `svg()`: `SvgIcon::Play => include_str!("../assets/icons/play.svg"),`

3. Verify:

```bash
cd ~/projects/torrentx && cargo build 2>&1 | tail -3
```

Expected: `Finished dev profile` with no `non-exhaustive patterns` error.

### Task 5 — Detail-panel Play button

`src/ui/results.rs`, inside `draw_detail`, in the existing magnet block (anchor:
`if let Some(mag) = r.magnet_uri.clone() {` around line 751). Insert **immediately after**
the `Open Magnet` button's `ui.add_space(4.0);` — keep `mc` untouched:

```rust
                    if wide_icon_btn(ui, SvgIcon::Play, "Play", self.pal.accent) {
                        match crate::player::start_play(&mag, &self.cfg.player) {
                            Ok(()) => self.toast("Streaming…", self.pal.accent),
                            Err(e) => self.toast(&e, self.pal.red),
                        }
                    }
                    ui.add_space(4.0);
```

Verify: `cargo build 2>&1 | tail -3` → `Finished dev profile`.

Manual check (display is live): `cargo run` → search something → open a row's detail panel →
a **Play** button sits under “Open Magnet”.

### Task 6 — `P` keyboard shortcut

`src/ui/search.rs`, after the existing `M` handler (`if !typing && ui.input(|i| i.key_pressed(egui::Key::M)) { … }`,
ends before `// Pagination`). Add:

```rust
                if !typing && ui.input(|i| i.key_pressed(egui::Key::P)) {
                    if let Some(i) = self.ui.selected {
                        if let Some(r) = page_s.get(i) {
                            if let Some(m) = &r.magnet_uri {
                                match crate::player::start_play(m, &self.cfg.player) {
                                    Ok(()) => self.toast("Streaming…", self.pal.accent),
                                    Err(e) => self.toast(&e, self.pal.red),
                                }
                            }
                        }
                    }
                }
```

Verify: `cargo build 2>&1 | tail -3` → `Finished dev profile`.

### Task 7 — Settings: “Play with” picker

`src/ui/header.rs`, inside `draw_settings_panel`, at the end of **Row 1** — anchor is the RSS
info tooltip block ending with `.on_hover_text(`…`);` followed by `                });`.
Insert just before that closing:

```rust
                    ui.add_space(8.0);
                    lbl(ui, "PLAY WITH", self.pal.dim, 10.0);
                    egui::ComboBox::from_id_salt("player_cb")
                        .selected_text(self.cfg.player.clone())
                        .width(64.0)
                        .show_ui(ui, |ui| {
                            for p in crate::player::PLAYERS {
                                if ui
                                    .selectable_value(&mut self.cfg.player, p.to_string(), p)
                                    .changed()
                                {
                                    let _ = save_cfg(&self.cfg);
                                }
                            }
                        });
```

Verify: `cargo build 2>&1 | tail -3` → `Finished dev profile`.

### Task 8 — Stop the stream when the app exits

`src/main.rs`, at the end of `fn main()` next to `jackett::stop_local();`:

```rust
    // Stop the stream we started, if any (the player sees EOF and closes).
    player::stop_play();
```

Verify: `cargo build 2>&1 | tail -3` → `Finished dev profile`.

### Task 9 — Full gate

```bash
cd ~/projects/torrentx && cargo fmt --all --check && cargo test --all-targets 2>&1 | grep -E "^test result|FAILED" && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3
```

Expected: `fmt` silent (exit 0), every suite `ok` (44 unit + 12 integration tests, plus the 2
new player tests → unit count 46), clippy `Finished … (0 warnings)`.

### Task 10 — Real end-to-end (the only proof that streaming works)

```bash
npm install -g webtorrent-cli          # ~30 s; installs to /home/chethan/.local/bin
command -v webtorrent                  # expected: /home/chethan/.local/bin/webtorrent
webtorrent --version                   # expected: a version line
```

Then start the app and press Play on a legal public-domain torrent (Sintel or Big Buck Bunny
from webtorrent.io), and verify **without looking at the screen**:

```bash
pgrep -x mpv                           # expected: a pid (player running)
ss -ltn | grep ':8000'                 # expected: LISTEN 127.0.0.1:8000 (webtorrent HTTP)
curl -sI http://localhost:8000/ | head -1   # expected: HTTP/1.1 200 OK
```

Press Play again on another result → exactly one `mpv` pid remains (`pgrep -x mpv | wc -l` → `1`).
Quit the app → `pgrep -x webtorrent` empty and `pgrep -x mpv` empty within ~5 s.

Negative path: run the app with `PATH` lacking webtorrent
(`PATH=/usr/bin:/bin cargo run`) and press Play → red toast:
`Direct play needs webtorrent-cli — run: npm install -g webtorrent-cli`.

### Task 11 — Docs + changelog, then commit

- `README.md` — Features list, after the “Detail panel” bullet:
  `- **Direct play** — stream a result to mpv/vlc without downloading the whole torrent (needs \`webtorrent-cli\`)`
- `GUIDE.md` — add `- [Direct play](#direct-play)` after the *Batch actions* TOC line; a
  **## Direct play** section before *Settings reference* with the prerequisite
  (`npm install -g webtorrent-cli`), the `P` shortcut, and the note that the largest file is
  streamed and quitting the app stops it; add `| Play with | mpv / vlc | external player for direct play |`
  to the Settings table; add `| P | Stream selected row (direct play) |` to the shortcuts table.
- `CHANGELOG.md` — under `## [Unreleased]`, add:

```markdown
### Added
- **Direct play.** A Play button in the detail panel (and `P`) streams a result's largest
  file to mpv or vlc through `webtorrent-cli`, without downloading the whole torrent first.
  The player is chosen in Settings; quitting the app stops the stream. Requires
  `npm install -g webtorrent-cli` (the button says so when it is missing).
```

Commit:

```bash
git add -A && git commit -m "feat(player): direct play — stream a magnet to mpv/vlc

Play in the detail panel or P streams the torrent's largest file to the
configured player through webtorrent-cli, stopping the previous stream and
on app exit. Missing webtorrent-cli is reported as an actionable toast."
```

## Tests / validation

| Check | Command | Expected |
|---|---|---|
| argv mapping | `cargo test -- player` | 2 passed |
| rejects bad input | same | `rejects_non_magnets_and_unknown_players` passes |
| config default | `cargo test -- config` | all config tests pass, `player == "mpv"` |
| whole suite | `cargo test --all-targets` | 46 unit + 12 integration, 0 failed |
| lint | `cargo clippy --all-targets --all-features -- -D warnings` | 0 warnings |
| format | `cargo fmt --all --check` | exit 0 |
| stream live | `pgrep -x mpv` + `ss -ltn \| grep :8000` | one pid, listener present |
| single stream | press Play twice, `pgrep -x mpv \| wc -l` | `1` |
| cleanup on quit | quit app, `pgrep -x webtorrent` | empty |

Every new test must be shown red before the implementation (Task 1 step 3, Task 3 step 1) —
a test that cannot fail is not evidence.

## Risks, tradeoffs, open questions

- **New external runtime dependency.** `webtorrent-cli` (npm) is not bundled. Missing → clear
  toast, no crash. Upgrade path if that is unacceptable: `librqbit` in-process (adds tokio +
  a large crate tree — rejected for now as the laziest option that streams).
- **Desktop-launched PATH.** A `.desktop` session may not include `~/.local/bin`; Play would
  then report the install message even though it is installed. Upgrade path: resolve the
  binary once via `npm prefix -g` and fall back to the absolute path.
- **Fixed port 8000.** webtorrent-cli defaults to it; a busy port makes the player fail to
  open the stream. Upgrade path: pass `--port 0` after confirming webtorrent-cli accepts it.
- **Wayland.** mpv/vlc open their own windows; no embedding into the egui window (embedding
  needs X11 `--wid`). Accepted.
- **`stop_play()` kills webtorrent**, so mpv may briefly show a dead stream before it exits;
  normal player behaviour is to close on EOF.
- **Open question (product):** was "direct play" meant as in-app embedded playback? That path
  needs an ffmpeg decode → wgpu texture pipeline (days, not hours) and is deliberately not
  planned here.
- **Play is not a row action.** The Actions column is fixed-width (190 px) with five buttons;
  a sixth clips (covered by `tests/table_remainder.rs`). Adding it there means widening the
  column to ~225 px and re-checking those tests — deferred until asked.
