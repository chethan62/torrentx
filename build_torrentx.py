#!/usr/bin/env python3
"""
TorrentX v7 — Build Script
Builds: Linux AppImage + Windows .exe
Run from anywhere, finds torrentx project automatically.
"""

import os, sys, shutil, subprocess, urllib.request, stat
from pathlib import Path

# ── Config ────────────────────────────────────────────────────────────────
PROJECT = Path.home() / "softwears" / "torrentx"
BINARY  = "torrentx"
VERSION = "7.0.0"
WIN_TARGET = "x86_64-pc-windows-gnu"
APPIMAGE_TOOL_URL = (
    "https://github.com/AppImage/AppImageKit/releases/download/continuous/"
    "appimagetool-x86_64.AppImage"
)

# ── Helpers ───────────────────────────────────────────────────────────────
def run(cmd, cwd=None, check=True):
    print(f"\n$ {cmd}")
    result = subprocess.run(cmd, shell=True, cwd=cwd or PROJECT, check=check)
    return result.returncode

def header(msg):
    print(f"\n{'─'*60}")
    print(f"  {msg}")
    print('─'*60)

def check_tool(name):
    if shutil.which(name):
        print(f"  ✓ {name} found")
        return True
    print(f"  ✗ {name} not found")
    return False

# ── Step 1: Build Linux release binary ───────────────────────────────────
def build_linux():
    header("Building Linux release binary")
    run("cargo build --release")
    binary = PROJECT / "target" / "release" / BINARY
    if not binary.exists():
        print("✗ Binary not found after build!")
        sys.exit(1)
    print(f"✓ Built: {binary}")
    return binary

# ── Step 2: Build Windows .exe ────────────────────────────────────────────
def build_windows():
    header("Building Windows .exe")

    # Check mingw
    if not check_tool("x86_64-w64-mingw32-gcc"):
        print("  Installing mingw-w64...")
        # Try apt, then pacman
        if shutil.which("apt"):
            run("sudo apt install -y gcc-mingw-w64-x86-64", cwd="/")
        elif shutil.which("pacman"):
            run("sudo pacman -S --noconfirm mingw-w64-gcc", cwd="/")
        else:
            print("  ✗ Cannot auto-install mingw. Install manually:")
            print("    Ubuntu/Debian: sudo apt install gcc-mingw-w64-x86-64")
            print("    Arch:          sudo pacman -S mingw-w64-gcc")
            return None

    # Add Rust Windows target
    run("rustup target add " + WIN_TARGET)

    # Write .cargo/config.toml
    cargo_config = PROJECT / ".cargo" / "config.toml"
    cargo_config.parent.mkdir(exist_ok=True)
    config_content = f"""[target.{WIN_TARGET}]
linker = "x86_64-w64-mingw32-gcc"

[target.{WIN_TARGET}.env]
WINAPI_NO_BUNDLED_LIBRARIES = "1"
"""
    cargo_config.write_text(config_content)
    print(f"✓ Written: {cargo_config}")

    ret = run(f"cargo build --release --target {WIN_TARGET}", check=False)
    exe = PROJECT / "target" / WIN_TARGET / "release" / f"{BINARY}.exe"
    if exe.exists():
        print(f"✓ Built: {exe}")
        return exe
    else:
        print(f"✗ Windows build failed (exit {ret})")
        print("  Tip: egui requires a windowing backend — cross-compiling egui to Windows")
        print("  sometimes needs extra libs. The Linux AppImage will still work.")
        return None

# ── Step 3: Package AppImage ──────────────────────────────────────────────
def build_appimage(linux_binary: Path):
    header("Packaging Linux AppImage")

    appdir = PROJECT / "AppDir"
    if appdir.exists():
        shutil.rmtree(appdir)

    # Create directory structure
    (appdir / "usr" / "bin").mkdir(parents=True)
    (appdir / "usr" / "share" / "applications").mkdir(parents=True)
    (appdir / "usr" / "share" / "icons" / "hicolor" / "256x256" / "apps").mkdir(parents=True)

    # Copy binary
    dest_bin = appdir / "usr" / "bin" / BINARY
    shutil.copy2(linux_binary, dest_bin)
    dest_bin.chmod(dest_bin.stat().st_mode | stat.S_IEXEC)

    # Desktop entry
    desktop = f"""[Desktop Entry]
Name=TorrentX
Comment=Torrent search via Jackett
Exec=torrentx
Icon=torrentx
Type=Application
Categories=Network;P2P;
Keywords=torrent;jackett;search;
"""
    desktop_path = appdir / "torrentx.desktop"
    desktop_path.write_text(desktop)
    shutil.copy2(desktop_path, appdir / "usr" / "share" / "applications" / "torrentx.desktop")

    # Icon — create a simple SVG if no PNG exists
    icon_dest = appdir / "usr" / "share" / "icons" / "hicolor" / "256x256" / "apps" / "torrentx.png"
    svg_icon  = appdir / "torrentx.svg"
    # Check if user has an icon
    user_icon = PROJECT / "assets" / "icon.png"
    if user_icon.exists():
        shutil.copy2(user_icon, icon_dest)
        shutil.copy2(user_icon, appdir / "torrentx.png")
        print(f"✓ Using icon: {user_icon}")
    else:
        # Write SVG fallback
        svg_icon.write_text("""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256">
  <rect width="256" height="256" rx="32" fill="#1a1b26"/>
  <text x="128" y="160" font-size="140" text-anchor="middle" fill="#7aa2f7" font-family="sans-serif" font-weight="bold">T</text>
</svg>""")
        # Try to convert SVG→PNG with rsvg-convert or inkscape
        converted = False
        for tool, cmd in [
            ("rsvg-convert", f"rsvg-convert -w 256 -h 256 {svg_icon} -o {icon_dest}"),
            ("inkscape",     f"inkscape {svg_icon} -w 256 -h 256 -o {icon_dest}"),
            ("convert",      f"convert -size 256x256 {svg_icon} {icon_dest}"),
        ]:
            if shutil.which(tool):
                if run(cmd, check=False) == 0:
                    shutil.copy2(icon_dest, appdir / "torrentx.png")
                    print(f"✓ Icon converted with {tool}")
                    converted = True
                    break
        if not converted:
            # Just touch the file so appimagetool doesn't crash
            icon_dest.touch()
            (appdir / "torrentx.png").touch()
            print("  ⚠ No icon converter found — using blank icon")

    # AppRun symlink
    apprun = appdir / "AppRun"
    apprun.write_text(f"#!/bin/sh\nexec \"$(dirname \"$0\")/usr/bin/{BINARY}\" \"$@\"\n")
    apprun.chmod(apprun.stat().st_mode | stat.S_IEXEC)

    # Download appimagetool if needed
    tool_path = Path.home() / ".local" / "bin" / "appimagetool"
    tool_path.parent.mkdir(parents=True, exist_ok=True)
    if not tool_path.exists():
        print(f"  Downloading appimagetool...")
        try:
            urllib.request.urlretrieve(APPIMAGE_TOOL_URL, tool_path)
            tool_path.chmod(tool_path.stat().st_mode | stat.S_IEXEC)
            print(f"✓ Downloaded: {tool_path}")
        except Exception as e:
            print(f"✗ Could not download appimagetool: {e}")
            print(f"  Download manually from: {APPIMAGE_TOOL_URL}")
            print(f"  Place at: {tool_path}")
            return None
    else:
        print(f"✓ appimagetool already at {tool_path}")

    # Build AppImage
    out = PROJECT / f"TorrentX-v{VERSION}-x86_64.AppImage"
    env = os.environ.copy()
    env["ARCH"] = "x86_64"
    ret = subprocess.run(
        f"{tool_path} {appdir} {out}",
        shell=True, cwd=PROJECT, env=env, check=False
    ).returncode

    if out.exists():
        size_mb = out.stat().st_size / 1024 / 1024
        print(f"✓ AppImage: {out}  ({size_mb:.1f} MB)")
        out.chmod(out.stat().st_mode | stat.S_IEXEC)
        return out
    else:
        print(f"✗ AppImage build failed (exit {ret})")
        return None

# ── Step 4: Copy outputs ──────────────────────────────────────────────────
def collect_outputs(appimage, exe):
    header("Output summary")
    dist = PROJECT / "dist"
    dist.mkdir(exist_ok=True)

    if appimage and appimage.exists():
        dest = dist / appimage.name
        shutil.copy2(appimage, dest)
        print(f"  📦 AppImage : {dest}")

    if exe and exe.exists():
        dest = dist / exe.name
        shutil.copy2(exe, dest)
        print(f"  🪟 Windows  : {dest}")

    print(f"\n  All outputs in: {dist}")

# ── Main ──────────────────────────────────────────────────────────────────
if __name__ == "__main__":
    if not PROJECT.exists():
        print(f"✗ Project not found at {PROJECT}")
        sys.exit(1)

    print(f"TorrentX v{VERSION} Build Script")
    print(f"Project: {PROJECT}")

    linux_bin  = build_linux()
    appimage   = build_appimage(linux_bin)
    exe        = build_windows()
    collect_outputs(appimage, exe)

    print("\n✓ Build complete!")
