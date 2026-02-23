# TorrentX — Rust + egui Torrent Search GUI

A fast, native torrent search GUI powered by your local Jackett instance.

## Requirements

- Rust (install via rustup)
- Jackett running locally
- A working internet connection for dependencies

---

## Install Rust (if not installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

## Install system dependencies (Arch Linux)

```bash
sudo pacman -S base-devel pkg-config openssl gtk3
```

---

## Build & Run

```bash
# Clone or copy this project folder, then:
cd torrentx

# Debug build (faster compile, slower runtime)
cargo run

# Release build (optimized, ~13MB binary)
cargo build --release
./target/release/torrentx
```

First build downloads dependencies and compiles — takes 1-3 minutes.
After that, rebuilds are fast.

---

## Usage

1. Start Jackett:
   ```bash
   systemctl start jackett --user
   ```

2. Open TorrentX — it launches a native window

3. Enter your Jackett API key in the config bar at the top
   - Get it from: http://localhost:9117 (top-right corner)

4. Type a search query, pick a category, hit SEARCH or press Enter

5. Click 🧲 to open magnet in qBittorrent, ↓ to download .torrent, ℹ for details

---

## Features

- Native Rust GUI (no browser, no Electron)
- Connects directly to Jackett API
- Real results from all your configured indexers
- Sort by Seeders / Size / Date
- Filter by category
- Magnet links open in your torrent client automatically
- Health indicator (HOT / OK / DEAD)
- Dark cyberpunk theme

---

## Install as system app (optional)

```bash
cargo build --release
sudo cp target/release/torrentx /usr/local/bin/
```
