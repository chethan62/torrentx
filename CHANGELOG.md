# Changelog

All notable changes to TorrentX are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [SemVer](https://semver.org/).

## [18.3.1] — 2026-09-15

### Fixed
- **Scroll-list rows gave their clicks to the wrong row — 18.3.0's fix for this was
  incomplete.** `ui.max_rect()` is the *available* rect, not the row's rect: inside a
  Frame inside a ScrollArea it reaches the bottom of the list, so **every** row's
  click layer covered every later row as well. egui settles a hit on the last layer
  registered, so:
  - the Favorites list still opened the **last** favourite's magnet when you clicked
    any row (18.3.0 only moved the layer inside the frame, which shortened it but did
    not make the layers disjoint), and its Magnet/Download/Remove buttons only worked
    on the last row;
  - the RSS feed list selected the **last** feed on any click, and the selected
    feed's Refresh/Edit/Delete buttons never fired unless it happened to be the last
    feed.

  Rows now allocate their own band up front and draw into it, so the layers are
  disjoint *and* the row's own buttons still win their clicks. Regression-tested:
  `tests/row_hit_bands.rs` asserts disjoint bands, correct routing and that an inner
  button receives its click — and asserts that the previous pattern *fails* those
  same expectations, so the test cannot silently stop covering the bug.

## [18.3.0] — 2026-09-14

### Changed
- **The app no longer uses GTK at all.** The tray now uses `tray-icon`'s `ksni`
  backend, which publishes the StatusNotifierItem straight over D-Bus. 45
  packages left the dependency graph (all 20 gtk-family crates gone), there is no
  `libgtk`/`libappindicator` runtime requirement, and the security advisory
  against `glib 0.18.5` is gone with it. Cost: +1.19 MB in the binary.
- On Wayland the tray item reads **Minimize**, not "Show / Hide": a Wayland client
  cannot un-minimize itself, so the old label promised something the platform
  refuses to do. Documented in README/GUIDE.
- `quick-xml` 0.41 → 0.42 (parser API migration).

### Fixed
- **The tray item had a different identity on every launch.** Its SNI `Id` defaulted
  to `"<pid>-<n>"`, but the spec says `Id` is how the host identifies an item across
  sessions — it is what per-item placement state is keyed on. It is now the stable
  `"torrentx"`.
- **The tray icon was effectively invisible on a dark panel.** It was a
  near-black tile with thin strokes; measured in the panel it rendered as dim
  slate with no trace of the icon colour, because the tile merged into the panel
  once Plasma scaled 32px down to tray size. It is now a transparent-background
  glyph with bold, bright strokes.
- **Tray menu actions did nothing until you clicked the window.** The app only
  repainted while a search was running, so menu events — including Quit — sat
  unprocessed while idle. Tray events now wake the UI.
- **RSS feeds never auto-refreshed while the app was idle.** The refresh timer was
  polled from the UI loop without scheduling a repaint, so a 5-second interval
  produced zero fetches in 35 seconds; feeds only updated after some input event.
  Now 8 fetches in 35 seconds, exactly on schedule.
- **Favorites: clicking a row opened the wrong torrent.** The row click layer
  covered the entire list viewport, so egui delivered every click to the last row:
  clicking the first favorite opened the *last* one's magnet, and the per-row
  Download/Remove buttons were dead on every row but the last.
- Row selection survived sorting, filtering and paging, so the highlight — and
  F/Enter/M plus "Copy N magnets" — could act on a different torrent than the one
  you picked. Selection is now dropped when the view changes.
- Escape cleared the search query while an RSS panel was open, and fired while
  typing in a filter field.
- **The Jackett API key appeared in error messages.** reqwest's error text appends
  the request URL, so an unreachable Jackett printed your key on screen.
- **An unreadable config was silently replaced by defaults**, destroying
  favorites, history and the API key. It is now copied to
  `config.json.corrupt-<timestamp>` beside the config before defaults are used,
  with a message in the UI, and saves are atomic (temp file + rename).
- Settings: numeric fields mangled typed values (typing `10` landed on 50) and the
  Save button reported "Settings saved" even when the write failed.
- Settings: a padded Jackett URL passed validation and then failed every request;
  a custom accent colour was discarded unless you clicked Done.
- Magnet links with `dn=` before `xt=urn:btih:` were rejected, hiding the
  copy/open buttons on valid links; BitTorrent v2 (`urn:btmh:`) magnets are
  recognised now too.
- URL validation accepted `http://`, `http://:9117` and `http:///api`.
- **Deduplication kept whichever tracker answered first** rather than the
  best-seeded copy, so a 1-seeder row could survive while a 900-seeder copy was
  discarded. It now keeps the best-seeded copy per title.
- Date sorting compared raw strings, which is wrong even within one format
  ("07 May" sorts before "12 Apr"); dates are parsed now.
- The health filter chip **DEAD** also selected rows badged **DYING**; there is a
  separate DYING chip, and a test pins the chips to the row badges.
- A partial column order hid columns permanently — their Settings toggles flipped
  and nothing appeared.
- The search-history dropdown listed 10 of the 20 stored entries.
- "Search complete" notifications fired even while the window was focused.
- Deleting an RSS feed left the item panel showing a different feed's item.
- Category-bar toggle appeared twice, so the second click undid the first.

### Internal
- CI: `Cargo.lock` is committed and used for the cargo cache key, the workflow
  has read-only permissions and concurrency cancellation, and dependabot no longer
  has to be told to ignore gtk.
- Real-feed parser tests added against live Torznab output (175 items, HTML
  entities, attribute coverage).

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

[18.3.1]: https://github.com/chethan62/torrentx/releases/tag/v18.3.1
[18.3.0]: https://github.com/chethan62/torrentx/releases/tag/v18.3.0
[18.2.2]: https://github.com/chethan62/torrentx/releases/tag/v18.2.2
[18.2.1]: https://github.com/chethan62/torrentx/releases/tag/v18.2.1
[18.2.0]: https://github.com/chethan62/torrentx/releases/tag/v18.2.0
[18.1.5]: https://github.com/chethan62/torrentx/releases/tag/v18.1.5
[18.1.4]: https://github.com/chethan62/torrentx/releases/tag/v18.1.4
[18.1.3]: https://github.com/chethan62/torrentx/releases/tag/v18.1.3
[18.1.2]: https://github.com/chethan62/torrentx/releases/tag/v18.1.2
[18.1.1]: https://github.com/chethan62/torrentx/releases/tag/v18.1.1
