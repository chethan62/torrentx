#!/bin/sh
# Headless fan-out smoke test: `torrentx --search` against a mock Jackett.
#
# Two mock indexers answer a Jackett-shaped API on localhost:
#   alpha  → two rows, one of which shares an infohash with beta (dedupe + also_on)
#   beta   → one row, seeded higher
#   broken → HTTP 200 whose body carries an indexer error (the real "Cloudflare
#            looked like no results" shape)
#
# Proves the whole search path — fan-out, per-source counts, dedupe, the
# partial-failure note, JSON output — with no network and no display. The GUI
# cannot be scripted on Wayland, which is why this exists.
set -eu
cd "$(dirname "$0")/.."

BIN="${BIN:-target/debug/torrentx}"
[ -x "$BIN" ] || { echo "smoke: no binary at $BIN (cargo build first)" >&2; exit 1; }

TMP=$(mktemp -d)
MOCK_PID=""
cleanup() {
    [ -n "$MOCK_PID" ] && kill "$MOCK_PID" 2>/dev/null || true
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')

cat > "$TMP/mock.py" <<'PY'
import json, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

ROWS = {
    "alpha": [
        {"Title": "Dune Part Two 1080p", "Tracker": "alpha", "Size": 2000000000,
         "Seeders": 50, "Peers": 5, "MagnetUri": "magnet:?xt=urn:btih:aaaa1111"},
        # Deliberately a different title: `dedupe_best_seeded` also merges
        # near-identical names, and this row must survive to prove that.
        {"Title": "Dune Prophecy S01E01 1080p", "Tracker": "alpha", "Size": 4000000000,
         "Seeders": 9, "Peers": 1, "MagnetUri": "magnet:?xt=urn:btih:bbbb2222"},
    ],
    "beta": [
        {"Title": "Dune Part Two 1080p", "Tracker": "beta", "Size": 2000000000,
         "Seeders": 400, "Peers": 20, "MagnetUri": "magnet:?xt=urn:btih:aaaa1111"},
    ],
}
CATALOGUE = """<?xml version="1.0" encoding="UTF-8"?>
<indexers>
  <indexer id="alpha" configured="true"><title>Alpha</title></indexer>
  <indexer id="beta" configured="true"><title>Beta</title></indexer>
  <indexer id="broken" configured="true"><title>Broken</title></indexer>
</indexers>"""


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        path = self.path.split("?")[0]
        if path.endswith("/torznab/api"):
            body, kind = CATALOGUE, "application/xml"
        elif "/indexers/broken/" in path:
            # 200 with the failure inside the body — Jackett's aggregate shape.
            body = json.dumps({"Results": [], "Indexers": [
                {"ID": "broken", "Error": "System.Net.Http.HttpRequestException: "
                                          "Response status code does not indicate success: 403"}]})
            kind = "application/json"
        else:
            idx = "beta" if "/indexers/beta/" in path else "alpha"
            body, kind = json.dumps({"Results": ROWS[idx], "Indexers": []}), "application/json"
        raw = body.encode()
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)


HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
PY

python3 "$TMP/mock.py" "$PORT" &
MOCK_PID=$!

# Wait for the port rather than sleeping a guessed amount.
i=0
while [ "$i" -lt 50 ]; do
    python3 -c "import socket,sys;s=socket.socket();sys.exit(0 if s.connect_ex(('127.0.0.1',$PORT))==0 else 1)" && break
    i=$((i + 1))
    sleep 0.1
done

cat > "$TMP/config.json" <<JSON
{"jackett_url": "http://127.0.0.1:$PORT", "api_key": "smoke", "timeout_secs": 5}
JSON

"$BIN" --config "$TMP/config.json" --search dune >"$TMP/out.json" 2>"$TMP/err.txt"
rc=$?
[ "$rc" -eq 0 ] || { echo "smoke: CLI exited $rc" >&2; cat "$TMP/err.txt" >&2; exit 1; }

# --limit trims AFTER dedupe, and says so.
"$BIN" --config "$TMP/config.json" --search dune --limit 1 >"$TMP/lim.json" 2>"$TMP/lim.err"
[ "$(python3 -c 'import json,sys;print(len(json.load(open(sys.argv[1]))))' "$TMP/lim.json")" = 1 ] \
    || { echo "smoke: --limit 1 did not trim to one row" >&2; exit 1; }
grep -q 'limited to 1 row' "$TMP/lim.err" \
    || { echo "smoke: --limit gave no notice" >&2; cat "$TMP/lim.err" >&2; exit 1; }

python3 - "$TMP/out.json" "$TMP/err.txt" <<'PY'
import json, sys

rows = json.load(open(sys.argv[1]))
err = open(sys.argv[2]).read()

# 1. Rows from both mock indexers arrived (3 fetched, 1 merged: same infohash).
assert len(rows) == 2, f"expected 2 deduped rows, got {len(rows)}: {rows}"

# 2. Dedupe kept the better-seeded copy and recorded the other tracker.
best = next(r for r in rows if r["seeders"] == 400)
assert best["tracker"] == "beta", f"best-seeded row should be beta: {best}"
assert best["also_on"] == ["alpha"], f"also_on lost the merge: {best}"

# 3. Per-source counts are on stderr, biggest first (this is the status line).
for frag in ("alpha 2", "beta 1"):
    assert frag in err, f"per-source counts missing {frag!r} in {err!r}"

# 4. The indexer that errored inside a 200 is named, and the search still exits 0.
assert "broken" in err and "failed" in err, f"failure note missing: {err!r}"

# 4b. The summary line is truncated for a status bar, and the FULL reason is on
#     its own stderr line — that split is the whole point of this output.
summary = [l for l in err.splitlines() if l.strip().endswith(")") or "…" in l]
assert any("…" in l for l in summary), f"summary not shortened: {err!r}"
full = [l for l in err.splitlines() if l.startswith("failed broken:")]
assert full, f"no full-reason line: {err!r}"
assert "403" in full[0], f"full reason truncated: {full[0]!r}"
assert "…" not in full[0], f"full reason still shortened: {full[0]!r}"

# 5. Every row carries a magnet — nothing is reported that cannot be played.
assert all(r.get("magnet") for r in rows), f"row without a magnet: {rows}"

print(f"smoke: ok — {len(rows)} rows, {err.strip()}")
PY
