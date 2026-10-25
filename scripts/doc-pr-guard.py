#!/usr/bin/env python3
"""Fail a push to main whose commits are documentation-only and have no PR.

CONTRIBUTING.md promises that README/GUIDE/CHANGELOG edits arrive through a
pull request: a doc commit pushed straight to main cannot be reviewed
afterwards, and giving it a review record later costs a revert plus a re-land.
This is the machine check for that promise.

A commit counts as "documentation-only" when every file it touches is one of
README/GUIDE/CHANGELOG/CONTRIBUTING at the repo root, anything under docs/, or
an asset README. Such a commit must be associated with a merged pull request;
commits that a rebase-merge lands on main are associated, a `git push` is not.

Env: GITHUB_TOKEN, REPO (owner/name), COMMITS (space separated sha list).
Exit 0 = fine, 1 = at least one doc commit arrived without a PR.
"""
import json
import os
import re
import sys
import urllib.error
import urllib.request

DOC = re.compile(r"^(README|GUIDE|CHANGELOG|CONTRIBUTING)\.md$|^docs/|/README\.md$")
API = "https://api.github.com"


def api(path):
    req = urllib.request.Request(
        f"{API}{path}",
        headers={
            "Authorization": f"Bearer {os.environ['GITHUB_TOKEN']}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def main():
    repo = os.environ["REPO"]
    commits = os.environ.get("COMMITS", "").split()
    if not commits:
        print("no commits in this push (nothing to check)")
        return 0

    bad = []
    for sha in commits:
        files = [f["filename"] for f in api(f"/repos/{repo}/commits/{sha}")["files"]]
        if not files or not all(DOC.search(f) for f in files):
            continue
        prs = api(f"/repos/{repo}/commits/{sha}/pulls")
        if prs:
            print(f"ok    {sha[:7]}  doc change, PR #{prs[0]['number']}")
        else:
            bad.append((sha, files))

    for sha, files in bad:
        print(f"FAIL  {sha[:7]}  documentation-only commit with no pull request")
        for f in files:
            print(f"        {f}")
    if bad:
        print(
            "\nDoc changes must go through a PR (CONTRIBUTING.md, 'Docs go through a PR').\n"
            "Fix: move the commit onto a docs/<topic> branch, open a PR, merge it once green."
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
