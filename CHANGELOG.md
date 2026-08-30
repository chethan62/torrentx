# Changelog

All notable changes to TorrentX are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [SemVer](https://semver.org/).

## [18.2.2] — 2026-08-30

### Fixed
- **Actions column clipped off-screen** (search + RSS tabs): at narrow window
  widths the fixed columns plus the Actions icon buttons exceeded the table
  viewport. egui_extras hardcodes horizontal scroll OFF inside tables, so the
  buttons were pushed past the right edge — unreachable. The search results
  table is now wrapped in a both-axis scroll area (narrow windows scroll
  horizontally), and the RSS Title remainder column is non-resizable so it
  re-fills after resize (regression-tested offscreen).

## [18.2.1] — 2026-08-30

### Fixed
- **Results table dead space**: after a window resize/maximize, the table
  ended early and left a large empty band after the Actions column. The Name
  remainder column is now non-resizable, so egui re-computes it every frame
  to fill the window (regression-tested via offscreen render).
- **Settings Save button**: right-aligned on its own row — no more orphaned
  button floating mid-panel on wide windows.

## [18.2.0] — 2026-08-25

### Fixed
- **Search race**: a slow older search could overwrite a newer one's results;
  stale threads are now discarded via a per-search epoch
- **Ctrl+A / Ctrl+C hijack**: shortcuts no longer fire while a text field has
  focus (select-all works in inputs again; clipboard isn't clobbered)
- **Software-GL fallback**: the retry now actually lands on Mesa software GL
  (`WGPU_BACKEND=opengl` + low power pref — eframe 0.36 defaults to wgpu)
- **Indexer list**: re-fetches when Jackett URL/key changes; retries 60 s
  after a failed attempt instead of never (e.g. Jackett still booting)
- **CSV injection**: exported cells beginning with `= + - @` tab/CR are
  neutralized so spreadsheet apps don't execute them
- **URL safety**: indexer-supplied links/details are scheme-checked
  (http/https/magnet only) before opening
- **RSS titles**: HTML entities (`&ldquo;` …) decode correctly under
  quick-xml 0.41's split-event model; numeric refs resolve, unknown refs
  stay literal
- CI: install `libxdo-dev` (lld hard-fails without it); rustfmt gate is
  now blocking

### Added
- Window-size persistence (restored on launch, throttled writes)
- Last-active tab restored across restarts
- Update-check opt-out in Settings
- Anonymous per-install ID (UUID v4) in About — click to copy, for bug reports
- Credits, platform/build info, and link buttons in the About tab
- CSV export feedback toast (filename on success, error on failure)
- User guide (`GUIDE.md`), bug-report & feature templates, dependabot,
  automated release workflow (binary + AppImage + sha256sums per tag)

### Changed
- Performance: one filter/sort pass per frame shared by all consumers;
  results shared via `Arc` (refcount bumps instead of deep copies);
  per-frame `Config` clone removed from the table hot path
- Dependencies: reqwest 0.13, quick-xml 0.41, tray-icon 0.24

## [18.1.5] — 2026-08-22

- SVG-only controls and data typography finished

## [18.1.4] — 2026-08-22

- Replaced all remaining font glyphs with Lucide SVG icons

## [18.1.3] — 2026-08-21

- HTML entity decoding in titles + design-review fixes

## [18.1.2] — 2026-08-21

- Full-row clickability on favorites; headless click-registration tests

## [18.1.1] — 2026-08-21

- First public release: Jackett/Torznab search, 19 themes, filters, sorting,
  favorites, RSS feeds, batch magnets, CSV export, tray, update checker

[18.2.2]: https://github.com/chethan62/torrentx/releases/tag/v18.2.2
[18.2.1]: https://github.com/chethan62/torrentx/releases/tag/v18.2.1
[18.2.0]: https://github.com/chethan62/torrentx/releases/tag/v18.2.0
[18.1.5]: https://github.com/chethan62/torrentx/releases/tag/v18.1.5
[18.1.4]: https://github.com/chethan62/torrentx/releases/tag/v18.1.4
[18.1.3]: https://github.com/chethan62/torrentx/releases/tag/v18.1.3
[18.1.2]: https://github.com/chethan62/torrentx/releases/tag/v18.1.2
[18.1.1]: https://github.com/chethan62/torrentx/releases/tag/v18.1.1
