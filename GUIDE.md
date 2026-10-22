# TorrentX User Guide

Everything you need to run and use TorrentX effectively.

## Contents

- [Installation](#installation)
- [Quick start](#quick-start)
- [Searching](#searching)
- [Filters & sorting](#filters--sorting)
- [Batch actions](#batch-actions)
- [Favorites](#favorites)
- [RSS feeds](#rss-feeds)
- [Direct play](#direct-play)
- [Settings reference](#settings-reference)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Config file reference](#config-file-reference)
- [UI standards](#ui-standards)
- [CLI flags](#cli-flags)
- [Known limitations](#known-limitations)
- [Troubleshooting](#troubleshooting)
- [Privacy](#privacy)

## Installation

**AppImage** (recommended): grab `TorrentX-<version>-x86_64.AppImage` from the
[latest release](https://github.com/chethan62/torrentx/releases/latest),
`chmod +x` it, and run. No installation needed.

**Raw binary**: download `torrentx`, place it in your `PATH`
(e.g. `/usr/local/bin` or `~/.local/bin`).

**From source**:

```bash
cargo build --release   # Rust 1.82+
# Binary at target/release/torrentx
```

System packages needed when building: `libxkbcommon-dev libasound2-dev
libxcb-*-dev libssl-dev pkg-config` (Debian/Ubuntu names). No GTK, appindicator
or xdotool system libraries are required — the tray is published over D-Bus.

**Tray on Wayland**: a Wayland client cannot hide or raise its own window
(`set_visible` and `focus_window` are no-ops, and un-minimizing is ignored), so
the tray item reads **Minimize** there and only minimizes — restoring is then a
compositor action (click the taskbar / panel entry). On X11 the item is a real
**Show / Hide** toggle.

## Quick start

TorrentX is a frontend for [Jackett](https://github.com/Jackett/Jackett) — it
needs a running Jackett instance:

1. Install and start Jackett (`http://localhost:9117` by default)
2. Add your trackers/indexers in the Jackett dashboard
3. Launch TorrentX → click **Settings** (⚙)
4. Paste your **Jackett URL** and **API Key** (copy it from the Jackett
   dashboard, top-right)
5. Search

TorrentX fetches your configured indexers automatically — pick **All** or one
specific indexer from the dropdown next to the search bar.

## Searching

- Type a query, press **Enter** or click **Search**
- Results stream back from every enabled indexer; the status bar shows the
  count and elapsed time
- **Category chips** above the results filter by category (Movies, TV, …);
  click again to clear
- Re-run the last search anytime with **Ctrl+R**

### Filters (apply within results, no new request)

| Filter | Meaning |
|--------|---------|
| Text | matches title, tracker, or category |
| Seeds ≥ | minimum seeders |
| Max GB | maximum size in GiB |
| Year ≥ | earliest publish year |
| Tracker | substring match on tracker name |
| Health | HOT (>500 seeds) · GOOD (101–500) · SLOW (11–100) · DEAD (≤10) |

Sort by Name, Tracker, Size, Seeds, Leechers, Ratio, or Date: click a column
header to sort by it, click the same header again to flip DESC/ASC. The header is
the only sort control (the filter band no longer duplicates it).

## Batch actions

1. Enable the **Select** checkbox in the filter bar (or press **Ctrl+A** to
   select all visible results)
2. Click rows (or their checkboxes) to toggle selection
3. **Copy N magnets** puts one magnet URI per line on your clipboard — paste
   straight into your torrent client

## Favorites

- Press **F** on a selected row, click the ★ action, or save from the detail
  panel / RSS items
- Favorites persist in the config file and get a saved-at timestamp
- The Favorites tab has its own text filter

## RSS feeds

The RSS tab watches Torznab feeds served by Jackett:

1. Add a feed: name it, optionally pin an indexer, query, and category
2. Enable **auto-refresh** for background polling (interval set globally in
   Settings, default 10 min, 0 disables)
3. Items are deduplicated by feed `<guid>` (falls back to the same content-marker
   key the search results use)

## Direct play

In the detail panel, **Play** streams a result straight to a video player — no
waiting for the download to finish, and seeking works.

1. Install the streaming helper once: `npm install -g webtorrent-cli`
2. Open a result's detail panel and press **Play** (or select a row and press **P**)
3. TorrentX streams the torrent's **largest file** to the player chosen in
   **Settings → Play with** (mpv or vlc)

**Season packs / multi-file torrents:** press **Choose episode/file…** to list the
torrent's files (with sizes), filter the list, and pick one — **Play** then
streams that file. **Largest file (default)** restores the default. The list is
fetched from peers, so it needs a reachable swarm; the choice stays until you
pick another file or another result.

Only one stream runs at a time; pressing Play again replaces it, **Stop stream**
ends it, and quitting TorrentX stops the stream. The player also ends the stream
when you close it. If `webtorrent` is not on `PATH`, Play says so instead of
failing silently.

## Settings reference

Settings opens as a floating window over the app (close it with the X or the
Settings button); the search bar, filters and table keep their place while it is
open.

| Setting | Values | Notes |
|---------|--------|-------|
| Jackett URL | URL | must start with http:// or https:// |
| API Key | string | from the Jackett dashboard |
| Timeout | 5–120 s | per search request |
| RSS refresh | seconds | 0 = never auto-refresh |
| Rows | Compact / Normal / Roomy | row height |
| Font | S / M / L | 12 / 14 / 16 px base |
| Page | 25 / 50 / 100 / All | results per page |
| Dedupe | on/off | merge near-duplicate titles across trackers |
| Updates | on/off | startup check for newer releases |
| Cat bar | on/off | category chips above results |
| Columns | per-column toggles | Tracker…Date; the order is fixed (Tracker, Name, Size, Seeds, Leech, Ratio, Health, Date) |
| Accent | color | overrides the theme's accent |
| Play with | mpv / vlc | player for direct play |

Changes apply immediately; press **Save** to persist connection changes.

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| ↑ / ↓ | Move selection (opens detail panel) |
| Enter | Open magnet for selected row |
| D | Toggle detail panel |
| F | Favorite selected row |
| M | Open magnet for selected row |
| P | Stream selected row (direct play) |
| Esc | Close detail panel / clear query |
| Ctrl+F | Focus search bar |
| Ctrl+A | Select all visible results |
| Ctrl+C | Copy magnet (detail panel open) |
| Ctrl+R | Re-run last search |

Shortcuts don't fire while you're typing in an input field.

## Config file reference

Default location: `~/.config/torrentx/config.json` (mode 0600 — it contains
your API key). All fields are managed by the UI; the notable ones:

| Field | Purpose |
|-------|---------|
| `jackett_url`, `api_key` | connection |
| `history` | last 20 searches |
| `favorites` | saved torrents |
| `theme`, `accent` | appearance |
| `col_order` | visible-column order (Name always first slot) |
| `rss_feeds`, `rss_refresh_secs` | RSS tab |
| `check_updates` | startup release check |
| `last_tab`, `win_size` | session restore |
| `install_id` | anonymous UUID for bug reports (never sent anywhere) |

## UI standards

The look is a system, not a set of one-off numbers. `src/ui/tokens.rs` is the
single source of truth:

| Scale | Tokens |
|-------|--------|
| Spacing | `SP1` 2 · `SP2` 4 · `SP3` 8 · `SP4` 12 · `SP5` 16 · `SP6` 24 · `SP7` 32 (4 px grid) |
| Radii | `R_SM` 4 · `R_MD` 8 · `R_LG` 12 |
| Icons | `ICON_XS` 10 · `ICON_SM` 12 · `ICON_MD` 14 · `ICON_LG` 16 |
| Table | `CELL_PAD_X` 10 px cell inset |
| Motion | `MOTION_MS` 120 ms hover/selection transition |

Rules:

- Spacing comes from `SP1..SP7`, radii from `R_SM/R_MD/R_LG`, icon sizes from
  `ICON_*` — never a literal.
- New colours go in `src/themes.rs`, never inline in a screen.
- All three text tiers (`text`, `sub`, `dim`) must reach 4.5:1 contrast on every
  one of the 19 themes; `Pal::from` clamps `sub`/`dim` toward `text` to enforce
  it, and `cargo test contrast_tests` proves it.
- `./scripts/ui-standards.sh` must print `ui-standards: ok` (CI runs it).

## CLI flags

```
torrentx [--config <path>] [-h] [-V]
```

`--config <path>` runs a fully separate profile (own config file) — handy for
pointing two instances at different Jackett servers.

## Known limitations

Deliberate, not bugs — each line says what to do instead.

- **CSV exports the whole filtered view**, every page, ticks or no ticks
  ("Export CSV" next to the result count). Ticking rows only feeds *Copy
  magnets*; there is no "export selection only".
- **One stream at a time** — starting a second play stops the first
  (`start_play` calls `stop_play`), so two players never fight for bandwidth.
- **Right-click menu is on the title column** — the other columns belong to the
  table's own cell hit areas, so the menu opens over the result name.
- **Column order is fixed** — Settings toggles which columns are shown, not
  their order.
- **No download manager** — "Download .torrent" and "Open magnet in browser"
  hand off to your torrent client; TorrentX never queues or seeds.
- **Direct play needs `webtorrent-cli` on `PATH`** and outbound BitTorrent. On a
  network that blocks tracker/DHT ports the picker reports
  "the torrent's peers are unreachable" instead of hanging.
- **Windows**: the `.exe` compiles in CI (`cargo build --target
  x86_64-pc-windows-gnu`) but is not published and has never been run on
  Windows; releases ship Linux only.

## Troubleshooting

**"Cannot reach Jackett"**
Jackett isn't running or is on another port. Try `curl localhost:9117`. On
systemd: `sudo systemctl start jackett`.

**"Invalid API key" (HTTP 401)**
Re-copy the key from the Jackett dashboard into Settings.

**Search times out**
Some indexers are slow. Raise **Timeout** in Settings (up to 120 s).

**Black / blank window** (VMs, some NVIDIA+Wayland setups)
TorrentX automatically retries with Mesa software GL if GPU init fails. You
can force it beforehand: `WGPU_BACKEND=opengl LIBGL_ALWAYS_SOFTWARE=1 torrentx`.

**Play does nothing / "needs webtorrent-cli"**
Install it once: `npm install -g webtorrent-cli` (check with `webtorrent --version`).
A desktop-launched app may not see `~/.local/bin`; run it from a terminal, or
install with a prefix already on `PATH`.

**Play opens no window / nothing plays**
Direct play needs peers. `~/.cache/torrentx/webtorrent.log` holds the streamer's
own output — a line like `Peers: 0/0` means the torrent's trackers and DHT are
unreachable, which is what many ISPs and corporate networks do (BitTorrent uses
UDP and non-standard ports). The same command in a terminal shows the same:
`webtorrent '<magnet>' --mpv`.

**Wrong indexer list**
The list is fetched at startup and whenever URL/key changes; if Jackett was
still booting, TorrentX retries automatically within a minute.

## Privacy

- No telemetry. The only outbound calls are to *your* Jackett server and
  (optionally) the GitHub releases API
- Disable the release check via **Settings → Updates**
- The random Install ID shown in About exists so bug reports can be told
  apart — it stays in your config unless you share it yourself

---

Found a bug? [Open an issue](https://github.com/chethan62/torrentx/issues/new)
— including your Install ID helps a lot.
