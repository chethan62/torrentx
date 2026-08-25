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
- [Settings reference](#settings-reference)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Config file reference](#config-file-reference)
- [CLI flags](#cli-flags)
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

System packages needed when building: `libxkbcommon-dev libgtk-3-dev
libasound2-dev libxcb-*-dev libssl-dev pkg-config` (Debian/Ubuntu names).

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

Sort by Name, Tracker, Size, Seeds, Leechers, Ratio, or Date — click column
headers or use the Sort picker. Toggle direction DESC/ASC next to it.

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
3. Items are deduplicated by feed `<guid>` (falls back to normalized title)

## Settings reference

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
| Columns | per-column toggles | Tracker…Date; reorder via drag in Settings order list |
| Accent | color | overrides the theme's accent |

Changes apply immediately; press **Save** to persist connection changes.

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| ↑ / ↓ | Move selection (opens detail panel) |
| Enter | Open magnet for selected row |
| D | Toggle detail panel |
| F | Favorite selected row |
| M | Open magnet for selected row |
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

## CLI flags

```
torrentx [--config <path>] [-h] [-V]
```

`--config <path>` runs a fully separate profile (own config file) — handy for
pointing two instances at different Jackett servers.

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
