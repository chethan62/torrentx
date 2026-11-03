#!/bin/sh
# A release run sometimes loses its runner: the job is assigned, never executes a
# single step, and is cancelled at 15 minutes. That happened three times on
# 2026-11-02, and each time the fix was a person typing `gh run rerun --failed`.
#
# `check-release` already refuses to call such a release done, and `bump-cask`
# waits for it — so the only missing piece is somebody pressing the button. This
# presses it once, automatically.
#
# Scope is deliberately narrow:
#   * only release.yml runs,
#   * only a job that was CANCELLED WITH ZERO STEPS (a real build failure is not
#     a flake and must not be retried into looking green),
#   * only the run's first attempt, so a flake on the retry stops here and shouts
#     instead of looping,
#   * only runs from the last two days, so history is not churned.
#
#   DRY_RUN=1 scripts/rerun-stalled-releases.sh
set -eu

repo="${REPO:-${GITHUB_REPOSITORY:-chethan62/torrentx}}"
workflow="${WORKFLOW:-release.yml}"
dry="${DRY_RUN:-0}"

runs=$(gh api "repos/$repo/actions/workflows/$workflow/runs?per_page=20")

# Candidates: finished, not green, first attempt, recent.
candidates=$(printf '%s' "$runs" | python3 -c '
import json, sys
from datetime import datetime, timedelta, timezone

cutoff = datetime.now(timezone.utc) - timedelta(hours=48)
for r in json.load(sys.stdin)["workflow_runs"]:
    if r["status"] != "completed":
        continue
    if r["conclusion"] not in ("failure", "cancelled"):
        continue
    if r["run_attempt"] != 1:
        continue
    created = datetime.fromisoformat(r["created_at"].replace("Z", "+00:00"))
    if created < cutoff:
        continue
    print(r["id"])
')

found=0
for id in $candidates; do
    stalled=$(gh api "repos/$repo/actions/runs/$id/jobs" | python3 -c '
import json, sys
jobs = json.load(sys.stdin)["jobs"]
print(1 if any(j["conclusion"] == "cancelled" and not (j.get("steps") or []) for j in jobs) else 0)
')
    [ "$stalled" = 1 ] || continue
    found=$((found + 1))
    if [ "$dry" = 1 ]; then
        echo "would re-run $id — a job was cancelled with zero steps executed"
        continue
    fi
    echo "re-running $id — a job was cancelled with zero steps executed"
    gh run rerun "$id" -R "$repo" --failed
done

if [ "$found" = 0 ]; then
    echo "rerun-stalled-releases: nothing stalled"
fi
