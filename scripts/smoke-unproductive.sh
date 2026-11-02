#!/bin/sh
# Slow-and-empty demotion, end to end: a source that spends the whole timeout and
# returns nothing must stop costing the wall time after three consecutive runs.
#
# The rule itself is unit-tested (a_slow_empty_source_earns_the_same_streak_as_a
# _dead_one). What this pins is the glue the unit test cannot see: the CLI seeds
# the streak from the config, persists it back, disables on the third crossing,
# and then skips the source — measured, because "three in a row" across separate
# processes is exactly what broke once (each run started from zero).
#
# Costs ~28 s: three 9-second runs. That is the point — the shape being tested is
# slowness.
set -eu
cd "$(dirname "$0")/.."

BIN="${BIN:-target/debug/torrentx}"
[ -x "$BIN" ] || { echo "unproductive: no binary at $BIN (cargo build first)" >&2; exit 1; }

TMP=$(mktemp -d)
MOCK_PID=""
cleanup() {
    [ -n "$MOCK_PID" ] && kill "$MOCK_PID" 2>/dev/null || true
    rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')

cat > "$TMP/mock.py" <<'PY'
import json, sys, time
from http.server import BaseHTTPRequestHandler, HTTPServer

CATALOGUE = """<?xml version="1.0" encoding="UTF-8"?>
<indexers>
  <indexer id="fast" configured="true"><title>Fast</title></indexer>
  <indexer id="slow" configured="true"><title>Slow</title></indexer>
</indexers>"""


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        path = self.path.split("?")[0]
        if path.endswith("/torznab/api"):
            body, kind = CATALOGUE, "application/xml"
        elif "/indexers/slow/" in path:
            time.sleep(9)  # long, and answers nothing: the shape seen for "ubuntu"
            body, kind = json.dumps({"Results": [], "Indexers": []}), "application/json"
        else:
            body, kind = json.dumps({"Results": [{
                "Title": "Dune Part Two 1080p", "Tracker": "fast", "Size": 2000000000,
                "Seeders": 50, "Peers": 5, "MagnetUri": "magnet:?xt=urn:btih:aaaa1111"}],
                "Indexers": []}), "application/json"
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

i=0
while [ "$i" -lt 60 ]; do
    python3 -c "import socket,sys;s=socket.socket();sys.exit(0 if s.connect_ex(('127.0.0.1',$PORT))==0 else 1)" && break
    i=$((i + 1))
    sleep 0.1
done

printf '{"jackett_url": "http://127.0.0.1:%s", "api_key": "slow", "timeout_secs": 20}\n' "$PORT" \
    > "$TMP/config.json"

run() {
    start=$(date +%s%N)
    "$BIN" --config "$TMP/config.json" --search dune --json > "$TMP/rows.json" 2> "$TMP/err.txt"
    end=$(date +%s%N)
    echo $(( (end - start) / 1000000 ))
}

# Three slow runs: each one pays the nine seconds and says so.
n=1
while [ "$n" -le 3 ]; do
    ms=$(run)
    [ "$ms" -ge 8000 ] || { echo "unproductive: run $n took ${ms}ms, expected >= 8000 (the slow source must be in the fan-out)" >&2; exit 1; }
    grep -q '^slow: 0 row(s)' "$TMP/err.txt" \
        || { echo "unproductive: run $n did not list the slow source" >&2; cat "$TMP/err.txt" >&2; exit 1; }
    n=$((n + 1))
done

# The third crossing switches it off, and says why.
grep -q 'disabled slow:' "$TMP/err.txt" \
    || { echo "unproductive: no demotion message after three slow runs" >&2; cat "$TMP/err.txt" >&2; exit 1; }
grep -q '"slow"' "$TMP/config.json" \
    || { echo "unproductive: the demotion was not persisted to the config" >&2; cat "$TMP/config.json" >&2; exit 1; }

# …and the wall time drops, which is the whole reason any of this exists.
ms=$(run)
[ "$ms" -lt 3000 ] || { echo "unproductive: after demotion the run still took ${ms}ms" >&2; exit 1; }
! grep -q '^slow:' "$TMP/err.txt" \
    || { echo "unproductive: the demoted source was still queried" >&2; cat "$TMP/err.txt" >&2; exit 1; }

echo "unproductive: ok — 3 runs at ~9 s, demoted, next run ${ms}ms without it"
