#!/bin/sh
# Rewrite assets/icons/*.svg from a pinned lucide-static release.
# usage: ./update-icons.sh [version]   (default 1.61.0)
set -eu
ver=${1:-1.61.0}
cd "$(dirname "$0")"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fail=0
for f in *.svg; do
    n=${f%.svg}
    case $n in
    *-filled) echo "skip  $n (derived locally from its base icon)"; continue ;;  # no lucide equivalent
    esac
    up=$n
    # app name -> lucide name
    [ "$n" = web ] && up=external-link
    [ "$n" = arrow-down-narrow-wide ] && up=arrow-down-wide-narrow
    if ! curl -fsSL --max-time 20 "https://unpkg.com/lucide-static@$ver/icons/$up.svg" -o "$tmp/$n"; then
        echo "FAIL  $n (lucide: $up)"; fail=1; continue
    fi
    body=$(sed -e 's/<!--.*//' "$tmp/$n" | tr '\n' ' ' | sed -e 's/.*<svg[^>]*>//' -e 's#</svg>.*##' -e 's/  */ /g')
    printf '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">%s</svg>\n' "$body" > "$f"
    echo "ok    $n"
done
exit $fail
