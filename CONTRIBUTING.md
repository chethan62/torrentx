# Contributing to TorrentX

Bug reports and PRs are welcome. Keep changes small, and keep the UI
consistent — the rules below are enforced, not suggestions.

## Before you open a PR

```bash
cargo fmt --all --check
cargo build --all-targets
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
./scripts/ui-standards.sh                              # must print: ui-standards: ok
cargo deny --all-features check licenses advisories bans sources
cargo audit
```

`cargo deny` and `cargo audit` are not interchangeable: deny treats `unsound` as a
warn-level advisory kind, so it prints `advisories ok` for an advisory that audit
reports. Run both.

## UI rules

- Spacing only from `SP1..SP7` (`src/ui/tokens.rs`), radii only
  `R_SM`/`R_MD`/`R_LG`, icon sizes only `ICON_XS`/`ICON_SM`/`ICON_MD`/`ICON_LG`.
  No literals: `scripts/ui-standards.sh` fails the build on them.
- New colours go in `src/themes.rs`, never inline in a screen.
- All three text tiers must pass WCAG AA (4.5:1) in all 19 themes —
  `cargo test contrast_tests`. `Pal::from` clamps `sub`/`dim` toward `text`,
  so a new theme cannot silently ship unreadable hint text.
- Motion is one duration: `MOTION_MS` (120 ms).
- Icons are Lucide SVGs in `assets/icons/` (stroke 2, `currentColor`); see
  `assets/icons/README.md` before adding one.
- Screenshots are evidence: include before/after images in the PR body for any
  visual change.

## Start with a good first issue

Issues labelled [`good first issue`](https://github.com/chethan62/torrentx/labels/good%20first%20issue)
are scoped to one file or one workflow step and say what "done" looks like —
comment on one to claim it. If nothing is there, a bug report with a reproduction
is just as welcome as a patch.

## Everything goes through a pull request

`main` is protected by a ruleset that requires a pull request: a direct `git push` is
rejected with `GH013 … Changes must be made through a pull request`. Branch, open a PR,
let the run go green, merge.

This started as a rule for docs only — a README commit pushed straight to `main` cannot be reviewed
afterwards, and giving it a review record later costs a revert plus a re-land. GitHub cannot scope a
ruleset to file paths on a public repository (that is a *push* ruleset, Team plan, private/internal
only), so the rule applies to the whole branch. It is a small tax on code changes and it buys a review
trail for the changes where a diff hides the bug.

## Commit style

Conventional Commits, one logical change per commit
(`fix(theme): …`, `refactor(ui): …`, `polish(ui): …`, `docs: …`).
