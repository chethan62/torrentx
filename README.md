# TorrentX

A lightweight Linux torrent search app built with [Rust](https://www.rust-lang.org/), [egui](https://www.egui.rs/) and [Jackett](https://github.com/Jackett/Jackett).

Search multiple torrent trackers in one place, get download magnet links directly.

## Features

- Search across 100+ trackers via Jackett
- Sort by seeders, size, date
- Copy magnet link to clipboard
- Open magnet link in default client
- Rust backend, tiny binary (~5 MB AppImage)

## Building

```bash
cargo build --release
# Binary at: target/release/torrentx
```

Or use the prebuilt AppImage (see [Releases](https://github.com/chethan62/torrentx/releases)).

## Setup

1. Run `./torrentx`
2. Go to Settings → enter your Jackett URL + API Key
3. Search

## Dependencies

- [Rust](https://rustup.rs/) 1.70+
- [Jackett](https://github.com/Jackett/Jackett) running somewhere

## License

MIT
