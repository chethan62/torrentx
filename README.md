```markdown
# TorrentX — Rust + egui Torrent Search GUI

A fast, native torrent search GUI powered by your local Jackett instance.

---

## ✨ Features

- Native Rust GUI (no browser, no Electron) — runs as a single executable
- Connects directly to Jackett API
- Real results from all your configured indexers
- Sort by Seeders / Size / Date
- Filter by category, health, tracker, and more
- Clickable category chips
- Magnet links open in your torrent client automatically
- Health indicator (HOT / GOOD / SLOW / DEAD)
- 19 built‑in themes (16 dark, 3 light)
- Keyboard navigation (↑↓ Enter D F M Ctrl+F Ctrl+R Esc)
- Favorites list with search & timestamps
- Export results to CSV
- Pagination, adjustable row height, font size
- History dropdown with per‑item delete
- Detail side panel with seeder/leecher ratio bar
- Deduplication across trackers
- Cross‑platform: **Linux**, **Windows**, **macOS** (tested on Linux & Windows)

---

## 🖼️ Screenshots

*[Add your screenshots here]*

---

## 📦 Download Pre‑built Binaries

You can download ready‑to‑run executables for:

| Platform | Format | How to run |
|----------|--------|------------|
| **Linux** | `.AppImage` | `chmod +x TorrentX-*.AppImage && ./TorrentX-*.AppImage` |
| **Linux** | plain binary | `./torrentx` |
| **Windows** | `.exe` | double‑click or run from terminal |
| **Windows** | portable `.exe` | same as above |
| **macOS** | `.dmg` / `.app` | *(coming soon)* |

Pre‑built binaries are available from the [Releases](https://github.com/chethan62/torrentx/releases) page.  
If you prefer to build from source, see instructions below.

---

## 🛠️ Build from Source

### Requirements

- Rust (install via [rustup](https://rustup.rs/))
- Jackett running locally
- Internet connection for dependencies

### Install Rust (if not installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Install system dependencies

**Arch Linux**
```bash
sudo pacman -S base-devel pkg-config openssl gtk3
```

**Ubuntu / Debian**
```bash
sudo apt install build-essential pkg-config libssl-dev libgtk-3-dev
```

**Fedora**
```bash
sudo dnf install gcc pkg-config openssl-devel gtk3-devel
```

**Windows**  
- Install [Rust for Windows](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe)  
- Visual Studio C++ build tools are required – see [Rust setup guide](https://rust-lang.github.io/rustup/installation/windows-msvc.html)

### Build & Run

```bash
# Clone or copy the project folder
cd torrentx

# Debug build (faster compile, slower runtime)
cargo run

# Release build (optimized, ~13MB binary)
cargo build --release

# Run the release binary
./target/release/torrentx        # Linux / macOS
.\target\release\torrentx.exe    # Windows
```

First build downloads dependencies and compiles — takes 1‑3 minutes.  
After that, rebuilds are fast.

---

## 🚀 Usage

1. **Start Jackett**  
   - If installed as a service:  
     `systemctl start jackett --user` (Linux)  
   - Or run Jackett manually from its installation folder.

2. **Open TorrentX** — a native window appears.

3. **Enter your Jackett API key**  
   - Click the ⚙ **Settings** bar at the top.  
   - Paste your API key (you can find it at [http://localhost:9117](http://localhost:9117) in the top‑right corner).  
   - Adjust any other settings (timeout, columns, etc.) – they are saved automatically.

4. **Search**  
   - Type a query, pick a category, and press **Enter** or click **Search**.  
   - Results appear in a sortable, filterable table.

5. **Interact with results**  
   - Click any row to select it, then:  
     - **Enter** / **M** → open magnet  
     - **F** → add to Favorites  
     - **D** → toggle detail panel  
   - Use the action buttons in each row:  
     - 🧲 **Mag** – open magnet in your torrent client  
     - 📋 **Copy** – copy magnet link to clipboard  
     - ↓ **DL** – download `.torrent` file  
     - ★ **Fav** – save to favorites  
     - ℹ **Info** – show/hide detail panel  
     - 🌐 **Web** – open details page in browser

---

## 🤖 Acknowledgements

TorrentX was built with the assistance of **Claude AI** and **DeepSeek AI** – they helped shape the UI, fix bugs, and polish the final product.  

Many thanks to the open‑source community behind:
- [egui](https://github.com/emilk/egui) – immediate mode GUI library
- [reqwest](https://github.com/seanmonstar/reqwest) – HTTP client
- [Jackett](https://github.com/Jackett/Jackett) – API provider

---

## 📄 License

MIT – do whatever you want with it, but please give credit where it's due.

---

## 💬 Support / Feedback

Open an issue on [GitHub](https://github.com/chethan62/torrentx) or reach out directly.  
Pull requests are welcome!
```

Save this content as `README.md` in your project root.
