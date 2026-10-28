# TorrentX User Guide

Everything you need to run and use TorrentX effectively.

## Contents

- [Installation](#installation)
- [Quick start](#quick-start)
- [Searching](#searching)
- [Your own indexers](#your-own-indexers)
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
cargo build --release   # Rust 1.85+ (edition 2024)
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
- **Right-click a result** — on any column except Actions — for its menu: play,
  choose episode, open details, favourite, copy magnet/name, download .torrent
- **Category chips** above the results filter by category (Movies, TV, …);
  click again to clear
- Re-run the last search anytime with **Ctrl+R**

## Your own indexers

TorrentX searches three kinds of source: the two built-in APIs (`apibay`,
`knaben`), any JSON indexer you add as a file, and Jackett's indexers if you run
Jackett. The first two need no Jackett at all.

A definition is one JSON file in `~/.config/torrentx/indexers/<id>.json`
(under `$XDG_CONFIG_HOME/torrentx/indexers` when that variable is set):

```json
{
  "id": "tpb-books",
  "name": "TPB Books",
  "category": "Books",
  "url": "https://apibay.org/q.php?cat=601&q={q}",
  "fields": {
    "title": "name", "hash": "info_hash", "size": "size",
    "seeders": "seeders", "leechers": "leechers", "date": "added"
  }
}
```

- `id` — letters, digits, `-` and `_`, at most 32 characters; it becomes the
  picker entry. A file whose id matches a built-in (`apibay`, `knaben`) replaces it.
- `url` — `{q}` is replaced with the URL-encoded query.
- `method` + `body` — for an API that wants a POST (knaben, most JSON APIs):
  `"method": "POST"` and a `body` template. `{q}` is **JSON-escaped** inside the
  body, not URL-encoded, so a title containing a quote cannot break the request.
  `content_type` defaults to `application/json`. Without `body`, a definition is a GET.
- `rows` — dotted path to the array of results: `"torrents"`, `"data.movies"`,
  `"items.0.list"`. Leave it out when the body is the array itself, or is a
  single result.
- `fields` — dotted paths inside one row. `title` is required. `magnet` can be
  left out when the site returns only an infohash (a magnet is then synthesised
  with public trackers). Counts and sizes may be numbers or numeric strings;
  `date` may be unix seconds or a date string.
- `category` — the bucket it fills: `Movies`, `TV`, `Music`, `PC Games`,
  `Software`, `Anime`, `Books`, `XXX`. Without it the rows only appear under **All**.
- `category_field` — instead of one fixed bucket, a dotted path to the row's own
  category (`"category"`), for a meta search that returns films, books and
  software in one answer. It uses the same matching as the built-in `knaben`, so
  "Movies / HD" lands in Movies and "PC / Unix" in Software.

### Keys and other headers

`headers` sends a fixed set of headers with every request — where an API key goes.
A value containing `${VAR}` is read from the **environment** when the request is
made, so the key never has to be written into the file. A variable that is not
set is an error naming the variable (never its value), instead of a request that
goes out with a literal `${VAR}` and fails as a mystery 401. `${VAR}` also works
inside `url`, for the sites that take the key as a query parameter.

```json
{ "id": "keyed-site", "url": "https://api.example/v1/search?q={q}",
  "headers": { "Authorization": "Bearer ${EXAMPLE_API_KEY}" }, "rows": "results",
  "fields": { "title": "name", "magnet": "magnet", "seeders": "seeders" } }
```

Start the app with the key in its environment (e.g. `EXAMPLE_API_KEY=… torrentx`,
or set it in your shell profile). Keys are read per request, so no restart is
needed after a rotation.

A POST definition for knaben — the same source the app ships natively, written as
a file, as the worked example of the format:

```json
{
  "id": "knaben-api", "name": "Knaben (definition)", "category_field": "category",
  "url": "https://api.knaben.org/v1", "method": "POST",
  "body": "{\"query\":\"{q}\",\"searchType\":\"100%\",\"orderBy\":\"seeders\",\"orderDirection\":\"desc\",\"from\":0,\"size\":100,\"hideUnsafe\":false}",
  "rows": "hits",
  "fields": { "title": "title", "magnet": "magnetUrl", "hash": "hash", "size": "bytes",
              "seeders": "seeders", "leechers": "peers", "date": "date" }
}
```

Restart TorrentX to see the file in the picker (the list is built at launch and
when Jackett's catalogue refreshes). An unreadable or invalid file is skipped,
never guessed at. `eztv-api` ships with the app as a worked example of the same
format — copy it as a starting point.

This covers sites whose search is already JSON. An HTML site needs Jackett's
Cardigann definitions instead (see [Known limitations](#known-limitations)).

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
| Timeout | 5–120 s | per request; an **All** search caps each indexer at 20 s |
| RSS refresh | seconds | 0 = never auto-refresh |
| Rows | Compact / Normal / Roomy | row height |
| Scale | 80% / 90% / 100% / 110% / 125% / 150% | scales text, rows and icons together (Ctrl +/- steps 5%) |
| Page | 25 / 50 / 100 / All | results per page |
| Indexer | All / one | search one indexer instead of the fan-out |
| Indexers chip | `8/9` | tick/untick which indexers the "All" search uses (saved at once); lists Jackett's 600+ catalogue entries, click one to copy its name and open Jackett |

| Dedupe | on (default) / off | merge near-duplicate titles across trackers, keeping the best-seeded copy |
| Updates | on/off | startup check for newer releases |
| At login | on/off | Linux only: writes `~/.config/autostart/torrentx.desktop` (launched with `--minimized`, tray only) so RSS keeps polling |
| Cat bar | on/off | category chips above results |
| Columns | per-column toggles + ◀ ▶ | Tracker…Date; the arrows move a column, Name and Seeds stay pinned |
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

Drag a column separator anywhere down its line to resize that column — the line lights up and
a guide follows the pointer. Widths persist in `config.json`. When the columns are wider than
the window a scrollbar appears along the bottom of the table; the divider grab band stops above
it, so the bar keeps its drags.
| P | Stream selected row (direct play) |
| Esc | Close detail panel / clear query |
| Ctrl+F | Focus search bar |
| Ctrl+A | Select all visible results |
| Ctrl+C | Copy magnet (detail panel open) |
| Ctrl+R | Re-run last search |
| Ctrl + / Ctrl − | Scale the interface up / down (5% steps) |
| Ctrl+0 | Reset the interface scale to 100% |

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
| `qbittorrent_url`, `qbittorrent_user`, `qbittorrent_pass` | qBittorrent Web UI root (e.g. `http://localhost:8080`), user and password — empty URL hides Send to qBittorrent |
| `android_inset_top`, `android_inset_bottom` | Android only: status-bar / gesture-strip height in points, used when the platform reports no safe-area inset (default 40 and 16, measured at density 480) |
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
torrentx [--config <path>] [--minimized] [-h] [-V]
```

`--config <path>` runs a fully separate profile (own config file) — handy for
pointing two instances at different Jackett servers.

**One instance per profile.** Launching TorrentX again (from the menu, or from a
second copy of the AppImage) asks the running one to show its window and exits —
two copies would otherwise both poll RSS and rewrite the same `config.json`. The
lock is a `torrentx.sock` beside the config file; a socket left behind by a crash
is reclaimed on the next start, and `--config <path>` gets its own instance.

`--minimized` starts with no window: the app runs for its tray icon and RSS
polling only (the tray's **Show / Hide** brings the window back; on Wayland it
starts minimised instead, and the taskbar restores it). The **At login** autostart
entry uses it, so logging in does not open a window in front of you. If the tray
cannot be created (no system-tray host, missing appindicator), the flag is ignored
and the window opens — a hidden window with no way back would be a dead end.

## Known limitations

Deliberate, not bugs — each line says what to do instead.

- **CSV exports what you ticked**, or the whole filtered view (every page) when
  nothing is ticked ("Export CSV" next to the result count). The same ticks feed
  *Copy magnets* and the export, so a selection is one idea, not two.
- **RSS polls only while TorrentX runs** — there is no background daemon, so an
  enabled feed refreshes only with the window open. **Settings → At login** starts
  the app at login; closing the window stops the polling.
- **Jackett is optional, Prowlarr is not supported** — without Jackett the app
  searches the built-in `apibay` and `knaben` APIs plus any JSON indexers you add
  as files (`~/.config/torrentx/indexers/`). Jackett's own catalogue and its
  indexers still come from Jackett's `/api/v2.0/indexers/…` endpoints, so Prowlarr
  (or a bare Torznab URL) will not connect. The definition mechanism is JSON-only:
  an HTML site needs Jackett.
- **No deeper paging exists** — a search asks each indexer once and Jackett returns
  what it has (≤100 per indexer, newest first). Jackett ignores `Limit`/`Offset` on
  its results API (measured: identical rows with and without them) and its Torznab
  endpoint honours `limit` but returns nothing for any `offset`, so the page buttons
  can only slice the set that came back.
- **One stream at a time** — starting a second play stops the first
  (`start_play` calls `stop_play`), so two players never fight for bandwidth.
- **Column order is reorderable, but Name and Seeds stay put** — Settings
  toggles which columns are shown; the ◀ ▶ arrows beside each chip move it left
  or right. Name owns the row's full-cell click layer and Seeds anchors the sort
  arrow, so those two are pinned.
- **No download manager** — "Download .torrent" and "Open magnet in browser"
  hand off to your torrent client; TorrentX never queues or seeds.
- **Direct play needs `webtorrent-cli` on `PATH`** and outbound BitTorrent. On a
  network that blocks tracker/DHT ports the picker reports
  "the torrent's peers are unreachable" instead of hanging.
- **Windows**: published since 18.7.0 as `TorrentX-<version>-windows-x64.zip`
  (static CRT). It is compiled, packaged and launched on a real Windows runner in
  CI (`.github/workflows/windows-native.yml` — a window titled TorrentX appears,
  the config lands in `%APPDATA%`), but no interactive pass (menus, playback,
  pickers) has been driven by hand there, so treat the exe as tested by CI only.

## Troubleshooting

**"Cannot reach Jackett"**
Jackett isn't running or is on another port. Try `curl localhost:9117`. On
systemd: `sudo systemctl start jackett`.

**"Invalid API key" (HTTP 401)**
Re-copy the key from the Jackett dashboard into Settings.

**Search times out**
Some indexers are slow. In an **All** search each indexer gets at most 20 s (the status
line shows `3/9 indexers` while the stragglers are still out), and raising **Timeout**
in Settings to 120 s only affects a single-indexer search. If one tracker is always
last and never returns rows, disable it in Jackett instead.

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
