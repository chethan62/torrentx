#!/bin/bash
# TorrentX AppImage Builder — improved
set -e

echo "==> Building TorrentX AppImage..."

PROJECT_DIR="$(pwd)"
APP_NAME="TorrentX"
BINARY_NAME="torrentx"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
ARCH="x86_64"
APPDIR="$PROJECT_DIR/AppDir"

# ── 1. Build release binary ──────────────────────────────────────────────────
echo "==> Compiling release binary..."
cargo build --release
BINARY="$PROJECT_DIR/target/release/$BINARY_NAME"

# ── 2. Download tools ────────────────────────────────────────────────────────
echo "==> Downloading tools..."

# appimagetool — packs the AppDir into an AppImage
if [ ! -f "/tmp/appimagetool" ]; then
  wget -q --show-progress \
    "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" \
    -O /tmp/appimagetool
  chmod +x /tmp/appimagetool
fi

# linuxdeploy — properly bundles shared libs (much more reliable than manual ldconfig grep)
if [ ! -f "/tmp/linuxdeploy" ]; then
  wget -q --show-progress \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage" \
    -O /tmp/linuxdeploy
  chmod +x /tmp/linuxdeploy
fi

# ── 3. Create AppDir structure ───────────────────────────────────────────────
echo "==> Creating AppDir..."
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"

cp "$BINARY" "$APPDIR/usr/bin/$BINARY_NAME"

# ── 4. Desktop entry ─────────────────────────────────────────────────────────
cat > "$APPDIR/usr/share/applications/$BINARY_NAME.desktop" << EOF
[Desktop Entry]
Name=$APP_NAME
Comment=Torrent search GUI powered by Jackett
Exec=$BINARY_NAME
Icon=$BINARY_NAME
Type=Application
Categories=Network;FileTransfer;
Keywords=torrent;search;jackett;download;
StartupNotify=true
EOF
ln -sf "usr/share/applications/$BINARY_NAME.desktop" "$APPDIR/$BINARY_NAME.desktop"

# ── 5. Icon ──────────────────────────────────────────────────────────────────
ICON_PATH="$APPDIR/usr/share/icons/hicolor/256x256/apps/$BINARY_NAME.png"

# Use existing icon if present in project
if [ -f "$PROJECT_DIR/assets/icon.png" ]; then
  cp "$PROJECT_DIR/assets/icon.png" "$ICON_PATH"
elif command -v convert &>/dev/null; then
  convert -size 256x256 xc:'#020817' \
    -fill '#06b6d4' -pointsize 180 -gravity center \
    -font DejaVu-Sans -annotate 0 '↓' \
    "$ICON_PATH" 2>/dev/null || \
  # Fallback: solid color block
  convert -size 256x256 xc:'#020817' "$ICON_PATH"
elif command -v rsvg-convert &>/dev/null; then
  cat > /tmp/torrentx_icon.svg << 'SVGEOF'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256">
  <rect width="256" height="256" rx="48" fill="#020817"/>
  <text x="128" y="185" font-size="160" text-anchor="middle"
        fill="#06b6d4" font-family="monospace" font-weight="bold">⬇</text>
</svg>
SVGEOF
  rsvg-convert -w 256 -h 256 /tmp/torrentx_icon.svg -o "$ICON_PATH"
else
  # Absolute fallback: minimal valid PNG (1×1 dark pixel, scales fine for AppImage)
  printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc\x10\x14\x10\x00\x00\x00\xf8\x00\x01\x8a\xc2\xa8i\x00\x00\x00\x00IEND\xaeB`\x82' \
    > "$ICON_PATH"
fi

ln -sf "usr/share/icons/hicolor/256x256/apps/$BINARY_NAME.png" "$APPDIR/$BINARY_NAME.png"

# ── 6. Bundle libraries with linuxdeploy ─────────────────────────────────────
# linuxdeploy walks ldd output and copies all non-system libs automatically.
# This is far more reliable than manually grepping ldconfig.
echo "==> Bundling libraries (linuxdeploy)..."

# Suppress FUSE warning — we run linuxdeploy extracted
export APPIMAGE_EXTRACT_AND_RUN=1

/tmp/linuxdeploy \
  --appdir "$APPDIR" \
  --executable "$APPDIR/usr/bin/$BINARY_NAME" \
  --desktop-file "$APPDIR/usr/share/applications/$BINARY_NAME.desktop" \
  --icon-file "$ICON_PATH" 2>/dev/null || {
    echo "  (linuxdeploy warning — continuing; libraries may need to exist on target)"
  }

# ── 7. AppRun ────────────────────────────────────────────────────────────────
cat > "$APPDIR/AppRun" << 'RUNEOF'
#!/bin/bash
HERE="$(dirname "$(readlink -f "${0}")")"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${HERE}/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH}"
# Wayland / X11 fallback
export GDK_BACKEND="${GDK_BACKEND:-x11}"
exec "${HERE}/usr/bin/torrentx" "$@"
RUNEOF
chmod +x "$APPDIR/AppRun"

# ── 8. Pack AppImage ─────────────────────────────────────────────────────────
echo "==> Packing AppImage..."
OUTPUT="${APP_NAME}-${VERSION}-${ARCH}.AppImage"

ARCH=x86_64 /tmp/appimagetool --no-appstream "$APPDIR" "$OUTPUT" 2>/dev/null

if [ ! -f "$OUTPUT" ]; then
  echo "❌ AppImage build failed — trying without FUSE (extract-and-run)..."
  APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 /tmp/appimagetool --no-appstream "$APPDIR" "$OUTPUT"
fi

chmod +x "$OUTPUT"

SIZE=$(du -sh "$OUTPUT" | cut -f1)
echo ""
echo "✅ Done!  $OUTPUT  ($SIZE)"
echo ""
echo "   Run directly:        ./$OUTPUT"
echo "   Install system-wide: sudo cp $OUTPUT /usr/local/bin/torrentx"
