#!/bin/sh
# Fixture test for scripts/check-release-assets.sh — the gate that decides whether
# a release is complete enough to hand to `brew`.
#
# The script runs only on a tag, so its failure path has no other way of being
# exercised before it matters. Three shapes, one stub `gh`:
#
#   complete  all nine artefacts  -> exit 0
#   partial   missing .dmg + its sha file (the 18.9.1/18.9.2 shape) -> exit 1, both named
#   none      no published release at all -> exit 1
set -eu
cd "$(dirname "$0")/.."

STUB=$(mktemp -d)
trap 'rm -rf "$STUB"' EXIT INT TERM

cat > "$STUB/gh" <<'STUBEOF'
#!/usr/bin/env python3
import os, sys

V = "1.0.0"
ALL = [
    f"TorrentX-{V}-x86_64.AppImage",
    f"torrentx-v{V}-x86_64-linux.tar.gz",
    f"torrentx_{V}_amd64.deb",
    f"torrentx-{V}-1.x86_64.rpm",
    f"TorrentX-{V}-windows-x64.zip",
    f"TorrentX-{V}-arm64-mac.dmg",
    "sha256sums.txt",
    "sha256sums-macos.txt",
    "sha256sums-windows.txt",
]
fixture = os.environ.get("FIXTURE", "complete")

args = sys.argv[1:]
if args[:2] != ["release", "view"]:
    sys.exit(f"stub gh: unhandled {args}")

if fixture == "none":
    sys.exit(1)                      # gh exits 1 when the release does not exist
assets = ALL if fixture == "complete" else [a for a in ALL if "mac" not in a]
print("\n".join(assets))
STUBEOF
chmod +x "$STUB/gh"

run() { PATH="$STUB:$PATH" "$@"; }

# 1. Complete: passes, and says how many it checked.
out=$(run ./scripts/check-release-assets.sh v1.0.0)
printf '%s\n' "$out" | grep -q 'ok — 9 artefacts' \
    || { echo "assets: complete release was not accepted" >&2; echo "$out" >&2; exit 1; }

# 2. Missing the dmg and its checksum: fails, and names both.
if out=$(run env FIXTURE=partial ./scripts/check-release-assets.sh v1.0.0 2>&1); then
    echo "assets: a release without the .dmg was accepted" >&2; exit 1
fi
# Only the "is missing:" line, not the "on the release:" listing beside it.
missing_line=$(printf '%s\n' "$out" | grep 'is missing:')
printf '%s\n' "$missing_line" | grep -q 'arm64-mac.dmg' \
    || { echo "assets: the .dmg was not named as missing" >&2; echo "$out" >&2; exit 1; }
printf '%s\n' "$missing_line" | grep -q 'sha256sums-macos.txt' \
    || { echo "assets: the mac checksum file was not named as missing" >&2; echo "$out" >&2; exit 1; }
! printf '%s\n' "$missing_line" | grep -q 'windows-x64.zip' \
    || { echo "assets: an artefact that IS present was reported missing" >&2; echo "$out" >&2; exit 1; }

# 3. No release at all is just as broken as a partial one.
if out=$(run env FIXTURE=none ./scripts/check-release-assets.sh v1.0.0 2>&1); then
    echo "assets: a tag with no release was accepted" >&2; exit 1
fi
printf '%s\n' "$out" | grep -q 'no published release' \
    || { echo "assets: missing release was not reported" >&2; echo "$out" >&2; exit 1; }

echo "assets: ok — complete accepted, partial and missing rejected by name"
