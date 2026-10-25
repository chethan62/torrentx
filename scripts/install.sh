#!/bin/sh
# Install the latest TorrentX AppImage in one command:
#
#   curl -fsSL https://raw.githubusercontent.com/chethan62/torrentx/main/scripts/install.sh | sh
#
# INSTALL_DIR overrides the destination (default ~/Applications). The download is
# checked against the release's sha256sums.txt before it is moved into place.
set -eu

REPO=chethan62/torrentx
DIR=${INSTALL_DIR:-$HOME/Applications}
API="https://api.github.com/repos/$REPO/releases/latest"

command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "sha256sum is required" >&2; exit 1; }

# Under sudo, HOME is /root, so the default destination quietly becomes
# /root/Applications — a copy nobody will ever launch. Say so before downloading.
if [ "$(id -u)" -eq 0 ] && [ -z "${INSTALL_DIR:-}" ]; then
    echo "warning: running as root without INSTALL_DIR — this installs to $DIR," >&2
    echo "         which is not on anyone's PATH. Use INSTALL_DIR=/opt/tx for a system-wide copy." >&2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

echo "fetching the latest release..."
curl -fsSL "$API" -o "$tmp/release.json" || { echo "cannot reach the GitHub API" >&2; exit 1; }

app_url=$(sed -n 's/.*"browser_download_url": "\([^"]*x86_64\.AppImage\)".*/\1/p' "$tmp/release.json" | head -1)
sum_url=$(sed -n 's/.*"browser_download_url": "\([^"]*sha256sums\.txt\)".*/\1/p' "$tmp/release.json" | head -1)
[ -n "$app_url" ] || { echo "the latest release has no AppImage" >&2; exit 1; }
[ -n "$sum_url" ] || { echo "the latest release has no sha256sums.txt" >&2; exit 1; }

asset=$(basename "$app_url")
echo "downloading $asset"
curl -fL --progress-bar "$app_url" -o "$tmp/$asset" || { echo "download failed" >&2; exit 1; }
curl -fsSL "$sum_url" -o "$tmp/sha256sums.txt"

# Verify before installing: a truncated or swapped AppImage must never land.
sum=$(grep -E "[[:space:]]$asset\$" "$tmp/sha256sums.txt" | head -1)
[ -n "$sum" ] || { echo "$asset is missing from sha256sums.txt" >&2; exit 1; }
( cd "$tmp" && printf '%s\n' "$sum" | sha256sum -c - >/dev/null ) || {
    echo "checksum mismatch for $asset — not installing" >&2
    exit 1
}

mkdir -p "$DIR"
target="$DIR/TorrentX.AppImage"
mv -f "$tmp/$asset" "$target"
chmod +x "$target"

echo "installed: $target (checksum verified)"
"$target" --version 2>/dev/null || true
