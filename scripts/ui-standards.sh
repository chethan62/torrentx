#!/bin/sh
# Fails when a UI file hardcodes a spacing/radius/icon size instead of using
# src/ui/tokens.rs. Deliberately textual: cheap, and it names the offending line.
set -eu
cd "$(dirname "$0")/.."
status=0
check() {
    hits=$(grep -rnE "$1" src/ui src/main.rs | grep -v 'src/ui/tokens.rs' || true)
    if [ -n "$hits" ]; then
        echo "FAIL: $2"
        echo "$hits"
        status=1
    fi
}
check 'add_space\([0-9]' 'literal add_space (use SP1..SP7)'
check 'corner_radius\([0-9]|CornerRadius::same\([0-9]' 'literal radius (use R_SM/R_MD/R_LG)'
check 'svg_icon\([^)]*, (8|9|10|11|12|13|14|15|16)\.0' 'literal icon size (use ICON_*)'
check 'Margin::(symmetric|same)\([^)]*[0-9]' 'literal margin (use margin(SP*, SP*))'
# A rect shrunk past its own size inverts, and egui panics on a negative layout
# size ("Negative size: [0.0 -20.0]"). The one sanctioned shrink2 is the clamped
# helper itself; everywhere else call band_inner(band, inset).
hits=$(grep -rn 'shrink2(' src/ui src/main.rs | grep -v 'src/ui/widgets.rs' || true)
if [ -n "$hits" ]; then
    echo "FAIL: unclamped shrink2 (use band_inner(band, inset) from ui/widgets.rs)"
    echo "$hits"
    status=1
fi
[ "$status" -eq 0 ] && echo "ui-standards: ok"
exit "$status"
