//! The CLI surface: `--version` / `--help` / `--minimized`.
//!
//! These flags return before any GUI or config work, so the process exits in a
//! few milliseconds — a spawned-binary test is fast and needs no display. It is
//! also the only automated check of the CLI on Linux: the `print_cli` fix for
//! Windows release builds (GUI subsystem, no console) is proven by the
//! windows-native workflow, but nothing here would notice `--version` printing
//! the wrong thing or panicking on this side.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_torrentx");

#[test]
fn version_prints_the_crate_version() {
    let out = Command::new(BIN).arg("--version").output().expect("spawn");
    assert!(out.status.success(), "--version exited {}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        text.trim(),
        concat!("TorrentX ", env!("CARGO_PKG_VERSION")),
        "--version output changed: {text:?}"
    );
}

#[test]
fn help_lists_every_flag() {
    let out = Command::new(BIN).arg("--help").output().expect("spawn");
    assert!(out.status.success(), "--help exited {}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    for flag in ["--config", "--minimized", "--help", "--version"] {
        assert!(
            text.contains(flag),
            "--help does not mention {flag}: {text}"
        );
    }
}
