# Contributing to TorrentX

Bug reports and PRs are welcome. Keep changes small, and keep the UI
consistent — the rules below are enforced, not suggestions.

## Before you open a PR

```bash
cargo fmt --all --check
cargo build --all-targets
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
./scripts/ui-standards.sh          # must print: ui-standards: ok
```

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

## Docs go through a PR

README, GUIDE and CHANGELOG edits take the same route as code: branch, PR, green run, merge. A doc
commit pushed straight to `main` cannot be reviewed afterwards — giving it a review record then costs a
revert on `main` plus a re-land branch, which is exactly what happened once already. Prose deserves the
review more than most code does: the bugs here are invisible in the diff and obvious in the rendered
page (clipped code blocks, a table that scrolls, a stale install line).

## Commit style

Conventional Commits, one logical change per commit
(`fix(theme): …`, `refactor(ui): …`, `polish(ui): …`, `docs: …`).
