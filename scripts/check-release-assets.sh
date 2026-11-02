#!/bin/sh
# A tag can publish an incomplete release: on 18.9.1 the macOS job was cancelled
# with zero steps executed (runner never started), so the page went live without
# the .dmg, `bump the cask` was merely *skipped*, and only a manual rerun fixed
# it. Nothing failed loudly. This does.
#
#   scripts/check-release-assets.sh v18.9.1
#
# Every artefact the release promises must be on the release, or exit 1.
set -eu

tag="${1:-${GITHUB_REF_NAME:?usage: check-release-assets.sh <tag>}}"
repo="${REPO:-${GITHUB_REPOSITORY:-chethan62/torrentx}}"
v="${tag#v}"

want="TorrentX-$v-x86_64.AppImage
torrentx-v$v-x86_64-linux.tar.gz
torrentx_${v}_amd64.deb
torrentx-$v-1.x86_64.rpm
TorrentX-$v-windows-x64.zip
TorrentX-$v-arm64-mac.dmg
sha256sums.txt
sha256sums-macos.txt
sha256sums-windows.txt"

have=$(gh release view "$tag" -R "$repo" --json assets -q '.assets[].name' 2>/dev/null || true)
# A tag that published nothing at all is just as broken as a partial one, and
# more likely to be missed by eye.
[ -n "$have" ] || { echo "check-release-assets: no published release for $tag" >&2; exit 1; }

missing=""
for a in $want; do
    printf '%s\n' "$have" | grep -qx -- "$a" || missing="$missing $a"
done
if [ -n "$missing" ]; then
    echo "check-release-assets: $tag is missing:$missing" >&2
    echo "check-release-assets: on the release:$have" >&2
    exit 1
fi

echo "check-release-assets: ok — $(printf '%s\n' "$want" | wc -l | tr -d ' ') artefacts on $tag"
