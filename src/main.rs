#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The desktop entry point. On Android the platform loads `libtorrentx.so` and
// calls `android_main` inside it, so this binary is never started there.

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    torrentx::run_gui()
}

#[cfg(target_os = "android")]
fn main() {}
