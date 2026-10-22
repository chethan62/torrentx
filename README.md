# TorrentX

**Native Rust desktop torrent search app** — query all your [Jackett](https://github.com/Jackett/Jackett) indexers from one blazing-fast GUI.

[![Release](https://img.shields.io/github/v/release/chethan62/torrentx?label=latest)](https://github.com/chethan62/torrentx/releases/latest)
[![CI](https://img.shields.io/github/actions/workflow/status/chethan62/torrentx/ci.yml?branch=main&label=CI)](https://github.com/chethan62/torrentx/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/chethan62/torrentx)](LICENSE)
[![Guide](https://img.shields.io/badge/docs-user%20guide-blue)](GUIDE.md)

## Screenshots

| Dark (Tokyo Night) | Light (Catppuccin Latte) |
|--------------------|--------------------------|
| ![Dark theme — Tokyo Night](docs/screenshots/dark.png) | ![Light theme — Catppuccin Latte](docs/screenshots/light.png) |

*19 themes total — 16 dark + 3 light, instant switching.*

## Features

- **19 themes** — 16 dark + 3 light, instant switching
- **All Jackett indexers** — search 100+ trackers simultaneously, or pick one
- **Multi-column results** — Name, Tracker, Size, Seeds, Leechers, Ratio, Health, Date (toggle any, reorder via Settings)
- **Row density** — Compact / Normal / Roomy
- **Filters** — text search, min seeds, size range, year, tracker, health status, category chips
- **Sort** — by Name, Tracker, Size, Seeds, Leechers, Ratio, Date
- **Favorites** — save torrents with timestamps, search filter, persistent storage
- **Detail panel** — seeder/leecher ratio bar, magnet copy/open, .torrent download
- **Direct play** — stream a result to mpv/vlc without downloading the whole torrent;
  multi-file packs get an episode picker (filterable file list) and a Stop control
  (needs [webtorrent-cli](https://github.com/webtorrent/webtorrent-cli): `npm install -g webtorrent-cli`;
  its output, including the peer count, goes to `~/.cache/torrentx/webtorrent.log`)
- **Batch actions** — select mode, select-all, copy N magnets at once
- **Keyboard shortcuts** — ↑↓ Enter D F M P Ctrl+F Ctrl+R Esc
- **CSV export** — export filtered results
- **Pagination** — 25/50/100/All
- **Deduplication** — across trackers (main search + RSS)
- **Search history** — with per-item delete
- **RSS feeds** — Jackett Torznab indexer feeds with auto-refresh (10 min), deduped
- **Custom accent color** — override any theme's accent
- **System tray** — show/hide + quit from the tray menu (on Wayland, where a
  window cannot be restored by the app itself, the item minimizes instead)
- **Desktop notifications** — alerted when a search completes
- **`--config <path>`** — portable config for multiple profiles/installs
- **Update checker** — notifies when a new release is out (opt-out in Settings)
- **Toast notifications** — animated, per-theme colored
- **egui 0.36** — modern rendering, GPU-accelerated

## Download

Grab the latest build from the [releases page](https://github.com/chethan62/torrentx/releases/latest):

| Platform | File | Size |
|----------|------|------|
| Linux (AppImage) | `TorrentX-<version>-x86_64.AppImage` | ~16 MB |
| Linux (binary) | `torrentx` | ~32 MB (stripped, LTO) |

## Build from source

```bash
# Prerequisites: Rust 1.82+, cargo
cargo build --release
# Binary: target/release/torrentx
```

## Setup

1. Launch TorrentX
2. Click ⚙ **Settings**
3. Enter your Jackett URL (default: `http://localhost:9117`) and API Key
4. Start searching

📖 **Full documentation**: [GUIDE.md](GUIDE.md) — installation, filters,
RSS, settings reference, config file, troubleshooting, and privacy notes.

## Tech

- **GUI:** egui 0.36 + eframe (GPU-accelerated via wgpu, software-GL fallback)
- **HTTP:** reqwest (blocking)
- **Config:** `~/.config/torrentx/config.json`
- **Binary size:** ~32 MB stripped + LTO; ~16 MB packed as AppImage (measured)

## License

MIT © [chethan62](https://github.com/chethan62)

### Bundled fonts

Torrent titles are often Japanese, Chinese or Korean, so two font subsets are
embedded in every build (`include_bytes!` in `src/app.rs`) and used as per-character
fallbacks after egui's Latin font:

| File | Covers | Size |
|------|--------|------|
| `assets/NotoSansCJKjp-subset.otf` | Han (CJK Unified Ideographs), hiragana, katakana, CJK/fullwidth punctuation | 4.9 MB |
| `assets/NotoSansKR-subset.otf` | Hangul syllables and jamo | 1.1 MB |

They are single-face `.otf` files because egui's `ab_glyph` cannot select a face out of
a system `.ttc` collection — the app therefore needs no CJK fonts installed. Together
they are 6.0 MB of the binary, down from 9.2 MB for the previous JP-only subset, which
also left Korean titles as □□□□ (the JP face contains no Hangul).

Both are subsets of **Noto Sans CJK 2.004** (Adobe/Google, derivative of Source Han
Sans) under the **SIL Open Font License 1.1** — © 2014-2021 Adobe; "Noto" is a
trademark of Google Inc. A subset is a *modified* version, and the copyright notice,
the licence, the modification evidence (the upstream faces declare **no** Reserved
Font Name) and the exact `pyftsubset` commands ship at
[`assets/NotoSansCJKjp-subset.LICENSE.txt`](assets/NotoSansCJKjp-subset.LICENSE.txt).
A unit test asserts the promised scripts resolve to real glyphs, and the same notice is
viewable in **About → Bundled font licence** (the release artifacts are a bare binary and
an AppImage, so the text ships inside both).
