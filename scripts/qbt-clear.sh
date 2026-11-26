#!/usr/bin/env bash
# Delete every torrent from the local qBittorrent, and prove it happened.
#
# The Web API answers **200 to a short hash and deletes nothing**, so a cleanup
# that trusts the status code leaves torrents behind. This always sends the full
# 40-char hashes and then re-reads `torrents/info` — the only honest check.
#
#   ./scripts/qbt-clear.sh            # local qBittorrent, default config
#   QBT_URL=http://host:8080 QBT_CONF=/path/qBittorrent.conf ./scripts/qbt-clear.sh
set -euo pipefail

CONF=${QBT_CONF:-$HOME/.config/qBittorrent/qBittorrent.conf}
BASE=${QBT_URL:-http://localhost:8080}
KEY=$(grep -oP '^APIKey=\K.*' "$CONF" 2>/dev/null || true)
[ -n "$KEY" ] || { echo "no WebUI APIKey in $CONF" >&2; exit 1; }

hashes=$(curl -sf -H "Authorization: Bearer $KEY" "$BASE/api/v2/torrents/info" |
    python3 -c 'import json,sys; print("|".join(t["hash"] for t in json.load(sys.stdin)))')
if [ -z "$hashes" ]; then
    echo "already empty"
    exit 0
fi

curl -sf -o /dev/null -X POST "$BASE/api/v2/torrents/delete" \
    -H "Authorization: Bearer $KEY" \
    --data-urlencode "hashes=$hashes" --data 'deleteFiles=true'

left=$(curl -sf -H "Authorization: Bearer $KEY" "$BASE/api/v2/torrents/info" |
    python3 -c 'import json,sys; print(len(json.load(sys.stdin)))')
[ "$left" = 0 ] || { echo "still $left torrent(s) after delete" >&2; exit 1; }
echo "cleared"
