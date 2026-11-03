#!/bin/sh
# Fixture test for scripts/rerun-stalled-releases.sh — the watchdog that re-runs a
# release whose job a dead runner killed.
#
# No network: a stub `gh` in PATH serves a fixed set of runs and jobs. What is
# pinned is the *decision*, because the cost of getting it wrong is asymmetric —
# re-running a real build failure turns a red release green by luck, and never
# re-running a stalled one leaves the release page half-published.
#
#   ok1  cancelled, attempt 1, a job with ZERO steps      -> re-run
#   ok2  cancelled, attempt 1, one of two jobs stalled    -> re-run
#   no1  cancelled, but the cancelled job DID run steps   -> no
#   no2  failure with steps (a real build error)          -> no
#   no3  cancelled, attempt 2 (already retried once)      -> no
#   no4  success                                          -> no
#   no5  right shape but 5 days old                       -> no
#   no6  still running                                    -> no
set -eu
cd "$(dirname "$0")/.."

STUB=$(mktemp -d)
trap 'rm -rf "$STUB"' EXIT INT TERM

cat > "$STUB/gh" <<'STUBEOF'
#!/usr/bin/env python3
import json, os, sys
from datetime import datetime, timedelta, timezone

now = datetime.now(timezone.utc)
recent = (now - timedelta(hours=3)).strftime("%Y-%m-%dT%H:%M:%SZ")
old = (now - timedelta(days=5)).strftime("%Y-%m-%dT%H:%M:%SZ")

RUNS = [
    {"id": 1001, "status": "completed", "conclusion": "cancelled", "run_attempt": 1, "created_at": recent},
    {"id": 1002, "status": "completed", "conclusion": "cancelled", "run_attempt": 1, "created_at": recent},
    {"id": 2001, "status": "completed", "conclusion": "cancelled", "run_attempt": 1, "created_at": recent},
    {"id": 2002, "status": "completed", "conclusion": "failure",   "run_attempt": 1, "created_at": recent},
    {"id": 2003, "status": "completed", "conclusion": "cancelled", "run_attempt": 2, "created_at": recent},
    {"id": 2004, "status": "completed", "conclusion": "success",   "run_attempt": 1, "created_at": recent},
    {"id": 2005, "status": "completed", "conclusion": "cancelled", "run_attempt": 1, "created_at": old},
    {"id": 3001, "status": "in_progress", "conclusion": None,      "run_attempt": 1, "created_at": recent},
]
STALLED = {"name": "macOS app + .dmg", "conclusion": "cancelled", "steps": []}
RAN = {"name": "build", "conclusion": "cancelled", "steps": [{"name": "checkout"}]}
JOBS = {
    1001: [{"name": "Linux", "conclusion": "success", "steps": [{"name": "build"}]}, STALLED],
    1002: [{"name": "Linux", "conclusion": "success", "steps": [{"name": "build"}]}, STALLED],
    2001: [RAN],
    2002: [{"name": "build", "conclusion": "failure", "steps": [{"name": "cargo build"}]}],
    2003: [STALLED],
    2004: [{"name": "build", "conclusion": "success", "steps": [{"name": "cargo build"}]}],
    2005: [STALLED],
}

# FIXTURE=quiet: nothing stalled at all, one real failure and one success.
if os.environ.get("FIXTURE") == "quiet":
    RUNS = [r for r in RUNS if r["id"] in (2002, 2004)]

args = sys.argv[1:]
if args[:2] == ["api", "repos/chethan62/torrentx/actions/workflows/release.yml/runs?per_page=20"]:
    print(json.dumps({"workflow_runs": RUNS}))
elif args and args[0] == "api" and "/actions/runs/" in args[1]:
    rid = int(args[1].split("/actions/runs/")[1].split("/")[0])
    print(json.dumps({"jobs": JOBS[rid]}))
elif args[:2] == ["run", "rerun"]:
    print(f"RERUN {args[2]}")
else:
    sys.exit(f"stub gh: unhandled {args}")
STUBEOF
chmod +x "$STUB/gh"

run() { PATH="$STUB:$PATH" "$@"; }

# 1. Dry run lists the two stalled runs and nothing else.
out=$(run env DRY_RUN=1 ./scripts/rerun-stalled-releases.sh)
[ "$(printf '%s\n' "$out" | grep -c 'would re-run')" = 2 ] \
    || { echo "watchdog: dry run found $(printf '%s\n' "$out" | grep -c 'would re-run') candidates, expected 2" >&2; echo "$out" >&2; exit 1; }
printf '%s\n' "$out" | grep -q 'would re-run 1001' \
    || { echo "watchdog: 1001 (cancelled, zero steps) not chosen" >&2; exit 1; }
printf '%s\n' "$out" | grep -q 'would re-run 1002' \
    || { echo "watchdog: 1002 (one stalled job of two) not chosen" >&2; exit 1; }
for id in 2001 2002 2003 2004 2005 3001; do
    ! printf '%s\n' "$out" | grep -q "would re-run $id" \
        || { echo "watchdog: $id must not be re-run" >&2; exit 1; }
done

# 2. The real path re-runs exactly those two, in order.
out=$(run ./scripts/rerun-stalled-releases.sh)
[ "$(printf '%s\n' "$out" | grep '^RERUN' | tr '\n' ' ')" = "RERUN 1001 RERUN 1002 " ] \
    || { echo "watchdog: unexpected re-runs: $(printf '%s\n' "$out" | grep '^RERUN' | tr '\n' ' ')" >&2; exit 1; }

# 3. A quiet day says so instead of succeeding silently.
out=$(run env FIXTURE=quiet ./scripts/rerun-stalled-releases.sh)
printf '%s\n' "$out" | grep -q 'nothing stalled' \
    || { echo "watchdog: quiet day did not report 'nothing stalled'" >&2; echo "$out" >&2; exit 1; }
! printf '%s\n' "$out" | grep -q '^RERUN' \
    || { echo "watchdog: re-ran something on a quiet day" >&2; exit 1; }

echo "watchdog: ok — 2 of 8 stalling shapes re-run, a real failure never is"
