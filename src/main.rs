#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The desktop entry point. On Android the platform loads `libtorrentx.so` and
// calls `android_main` inside it, so this binary is never started there.

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    torrentx::install_panic_log();
    let argv: Vec<String> = std::env::args().collect();
    // `--search` runs the same sources with no window: the only scriptable way
    // in, since the egui window can't be driven on Wayland.
    // Only the search flags: --help/--version keep their existing single
    // implementation in `run()`, which the CLI test pins.
    if argv
        .iter()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--search" | "-s" | "--sources" | "--json"))
    {
        match torrentx::run_cli(argv) {
            Ok(()) => return Ok(()),
            Err(e) => {
                eprintln!("torrentx: {e}");
                std::process::exit(2);
            }
        }
    }
    torrentx::run_gui()
}

#[cfg(target_os = "android")]
fn main() {}
