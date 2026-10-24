# Changelog

All notable changes to TorrentX are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [SemVer](https://semver.org/).

## [Unreleased]

## [18.6.20] — 2026-10-24

### Fixed
- **The search bar is on one grid.** Row 1: the field and the Search button are the
  same box (y=64–99, 36 px). Row 2: Category, its 120 px dropdown, Indexer, its 120 px
  dropdown, and `Indexers 9/9` are all 28 px tall on one baseline, with 4 px from a
  label to its control and 12–16 px between groups. `item_spacing.x` is zeroed in both
  rows because egui added its own spacing around every `add_space`, so the same token
  rendered as 12 px in one place and 20 px in another. The combos used to be 115 and
  130 px wide, which pushed every downstream edge out of line.
- **The Search button no longer jumps to a second line.** The search field's width
  came from `available - 505`, a number that had to be re-measured every time a
  control was added beside it; the indexer chip pushed the button onto its own row,
  where it rendered at x=5. The row is now split: the field and a **fixed-width**
  (132 px) Search button on line one, and Category / Indexer / the indexer button on
  line two. Field width is derived from the button, not guessed. Measured: field fill
  spans x=24–1768, the button x=1784–1916 at the same y, window 1920 wide.

### Changed
- **The indexer list is cached on disk for a day.** Every launch fetched Jackett's
  whole `t=indexers` document — 644 entries, 2.34 MB — just to learn which nine are
  configured. It is now read from `~/.cache/torrentx/indexers-<hash>.xml` (hash of
  URL+key, so another Jackett or key cannot inherit a stale list, and the key never
  appears in a path) and refetched when older than 24 h or unparseable. "Refresh
  list" in the indexer window always bypasses it. Measured against a stub server
  serving the real 2.34 MB body: launch 1 = one request + cache written; launch 2 =
  zero requests, chip still `9/9`; "Refresh list" = one request and the cache
  rewritten; a cache aged two days = one request, timestamp refreshed.

## [18.6.19] — 2026-10-24

### Added
- **Indexer catalogue in the chip window.** The app already downloads Jackett's full
  indexer list for `t=indexers` (644 ids, 2.3 MB, ~0.09 s), so the unconfigured ones
  are now listed too — 635 on this install — behind a filter box. Clicking one copies
  its id and opens Jackett's dashboard, which is the only place an indexer can be
  added (every write on `/api/v2.0/indexers*` is behind Jackett's cookie session).
- **The indexer control is a labelled button.** It read `9/9`, which said nothing
  about what it did; it is now `Indexers  9/9`, bordered, tinted when some are off,
  with a tooltip naming enable/disable, add-in-Jackett and the catalogue. Escape
  closes its window.
- **The chip opens a small window, not a menu.** A menu popup closes on the first
  click, which made the checkboxes and the filter field unusable; the window keeps
  its state and its position.
- **The Indexers chip also gets you to Jackett's own dashboard.** Adding an indexer
  needs Jackett's web UI: `/api/v2.0/indexers/*` is UI-session gated (a POST or even
  a DELETE there redirects to `/UI/Login` with a valid API key; the only API-key route
  is the Torznab search/list endpoints), and each indexer wants its own credentials,
  cookies or FlareSolverr. So the popup carries "Add indexer in Jackett…" (opens the
  dashboard) and "Refresh list" (re-reads Jackett's configured indexers without a
  restart). Indexer credentials stay in Jackett, not in TorrentX's config.
- **Indexers can be switched off.** The `N/M` chip next to the Indexer picker opens a
  checklist of the configured indexers; an unticked one is skipped by the "All"
  fan-out (and by the `n/m` counter), so a laggard or a broken tracker stops costing
  every search. The choice is saved immediately to `disabled_indexers` and survives
  restarts. Never fans out over nothing: if the list empties, the first configured
  indexer is kept.

### Fixed
- **Column resizing behaves like a spreadsheet.** The results table used to squeeze
  the `Name` column to absorb a resize (and had no horizontal scroll at all, so wide
  columns were simply clipped). Widths are now owned by the app: dragging a divider in
  the header changes exactly that column and everything to its right shifts, and when
  the columns outgrow the window a horizontal scrollbar appears instead of the trailing
  columns disappearing. Measured: separators `[407, 477, 557, …]` → `[407, 477, 597,
  667, 735, …]` after a +40 drag of the third divider; scrollbar pixel row 813 appears
  only when the sum exceeds the viewport, and dragging it moved the header labels
  +65 px, revealing the scrolled-out `Tracker` column.
- **Column widths are remembered, per column.** Widths are stored keyed by column
  name (`{"name": 754, "size": 152, …}`), so hiding a column in Settings or reordering
  them keeps the rest, and a config written before the field existed simply has no
  keys. Saved once per drag end (`save_cfg` fsyncs — not per frame), clamped to
  60–900 px, NaN discarded.
- **The table still fills a fresh window.** With fixed widths the table ended where
  the columns ended and left a dead gap; `Name` now absorbs the leftover until the
  user resizes it, after which the widths are exactly what was dragged (Excel-like).
- Column dividers are drawn by the app now (egui_extras only paints them on a
  resizable table), so the header keeps its grid lines.



### Changed
- **Dedupe is on by default.** The same release routinely comes back from two
  indexers (TheRARBG and The Pirate Bay, for instance) and both rows took a slot in
  the table. The merge keeps the best-seeded copy; existing configs that pin
  `dedupe: false` keep their choice, and a config without the field now reads as on.

## [18.6.18] — 2026-10-23

### Fixed
- **Dragging a column separator resizes the column again.** Every table cell —
  header and body — puts a full-rect click layer on top (row selection, sort,
  right-click menu). egui_extras registers its resize handle *before* the cell
  content, so that layer covered the divider: the drag selected a row or re-sorted
  the column and the width never changed. The hit rect now stops at the handle band
  (`cell_rect`), which gives the drag back to the table. `Name` stays the column that
  absorbs a resize (it is the table's only remainder), so widening any other column
  makes it narrower by the same amount.

### Changed
- **egui 0.36 → 0.37** (eframe and egui_extras with it). One source change: `Id::new`
  is now an alias for `Id::unique`. `clipboard-win`, pulled in by egui-winit, had to
  move off a yanked release (5.4.1 → 5.4.3). Column resizing is unchanged: 0.37 still
  builds the table without horizontal scrolling, and double-click-to-autosize already
  existed.

## [18.6.17] — 2026-10-23

### Fixed
- **Streamed data no longer accumulates.** webtorrent keeps every piece it
  downloads in `--out`, and nothing ever removed it: one season-pack session left
  **3.1 GB** in `~/.cache/torrentx/stream`, and the next Play then stalled
  re-verifying it. The directory belongs to the single stream the app runs, so it is
  now cleared when a stream stops (off-thread — it can be gigabytes) and once at
  startup, which also cleans up after a crash or a kill.

## [18.6.16] — 2026-10-23

### Fixed
- **The episode/file picker actually lists files.** `webtorrent <src> --select` with no
  index used to print the list and exit; webtorrent-cli 6 ignores it and starts
  DOWNLOADING (measured: a local pack printed "torrent downloaded successfully", and a
  real 546-seeder magnet sat at "verifying existing torrent data" until the 30 s
  deadline — the picker was silently pulling the whole torrent). The list now comes from
  `webtorrent info <torrent>`, which prints JSON and exits.
- **Magnet-only results can be listed at all.** With no Jackett `.torrent` (most
  private/public indexers redirect to a magnet) the app now runs
  `webtorrent downloadmeta` into its own cache and reads that — 14 s on the first
  request, instant afterwards.
- **The picker appears on its own.** The listing thread never woke the UI loop, which
  parks when idle, so the picker only opened once the mouse moved.
- **Play streams into the cache partition, not `/tmp`.** `/tmp` here is a 7.8 GB tmpfs;
  a season pack either filled RAM or stalled re-verifying gigabytes of sparse files.
- **Play passes a free `--port`.** webtorrent's default 8000 was held by a leftover
  WebTorrent, and a squatter there hangs the stream with no player and no error.
- **The cached `.torrent` is keyed on the infohash everywhere**, so Play reuses the file
  the picker fetched instead of paying the metadata round trip again.

### Measured (real swarm, 411-seed season pack, 8 files)
- picker: 8 entries with real names and sizes (cold 14 s, warm 6 s); picking
  "S01E01 - Pilot" showed `…Silence).mkv (1.56 GB)` in the panel; Play opened **mpv in
  0.66 s** from the cached `.torrent` over a free port, and webtorrent reported data
  arriving from 7/10 peers.

## [18.6.15] — 2026-10-23

### Changed
- **One slow tracker no longer defines the search.** The "All" fan-out joins every
  indexer before reporting Done, so a laggard *is* the search duration — and with a
  raised timeout the worst case was two minutes. Each indexer now gets at most 20 s
  in an All search (a single-indexer search still honours the Timeout setting). The
  status line also names the wait: `Searching "x" — 36 so far  2.5s · 8/9 indexers`.
- Default **Timeout** is 30 s (was 45); the field is still 5–120 and now says the
  All-search cap in its tooltip.

### Measured
- Live Jackett, 9 configured indexers, cold queries: 8 answer in ≤1.2 s, `anisource`
  takes 12.3–17.6 s and returns nothing for TV/movie queries. A fresh query showed
  `8/9 indexers` at 2.5 s and finished at 12.65 s with 36 rows — previously that wait
  read as an unexplained `Scanning…`.

## [18.6.14] — 2026-10-23

### Changed
- **The smallest text grew a step.** Settings section headings (CONNECTION /
  DISPLAY / COLUMNS / PLAY WITH), the Category and Indexer labels, the version
  badge and the status-bar key hints were 10–10.5 px — the last tier that still read
  as fine print. They are 11–11.5 px now, and the headings moved out of the dimmest
  colour tier.
- **Settings › DISPLAY is two lines.** Seven groups (Rows, Scale, Page, Updates, At
  login, Cat bar, Accent) on one row read as a dense wall; the toggles now sit on
  their own line.
- The category sub-label under a result name went from `fs − 2.5` to `fs − 1.5` —
  11.5 px at the default size, for a value people actually read.

### Fixed
- **The Search button wrapped onto a second line.** The row's field is
  `available − 505`, and the constants had drifted: after the labels grew, the total
  (`field + chrome`) overflowed by a few pixels, which is all egui needs to wrap the
  button. Re-measured; verified one line with the button ending 15 px inside the
  right padding.

## [18.6.13] — 2026-10-23

### Added
- **A Scale control, so the whole interface can be made bigger.** Settings › DISPLAY
  gets 80 / 90 / 100 / 110 / 125 / 150 % presets, and **Ctrl +** / **Ctrl −** step by
  5 % (Ctrl 0 resets). It is applied as pixels-per-point, so text, rows, icons and
  spacing grow together — the old Font S/M/L buttons only moved body text and left
  the 10 px section labels, chips and toolbar at their fixed sizes. Verified at 160 %
  with results on screen: no clipped tracker names, badges or action icons, and the
  search row still fits on one line.

### Changed
- The Font S/M/L buttons are gone (Scale supersedes them); a saved text size stays
  as the base the scale multiplies.

### Fixed
- Zoom keys count key events rather than `key_pressed`, so several presses in one
  frame (a fast double tap, a held key) no longer collapse into a single step.

## [18.6.12] — 2026-10-23

### Fixed
- **Clicking a past search did nothing.** The recent-searches popup is drawn at the
  end of the search row, but the field's focus-loss handler ran first and closed it
  — and clicking the popup IS a focus loss, so the popup was never drawn in that
  frame and the click never landed. It now stays up when the pointer is inside the
  rect it reported last frame, and still closes on any other focus loss so a click
  meant for a filter cannot silently re-run an entry. The entry sets the query,
  fills the search box and runs the search.
- **The clear button sits inside the field and clears the search.** It used to come
  after the Search button in a wrapping row and fell onto a second line; and it
  cleared only the text, leaving the table and the status line still describing the
  old query. It is now the usual in-field × at the field's right edge, and it drops
  the results, returns to the welcome screen and invalidates any search still in
  flight. Esc uses the same reset.
- Removed-history buttons were laid out on the line below each entry, so every row
  of the popup was double height and misaligned.

## [18.6.11] — 2026-10-23

### Fixed
- **A failed RSS fetch no longer passes for a healthy, empty feed.** The Torznab body
  is now checked before parsing: an HTML login wall or a truncated `<item>` used to
  read to EOF as zero items, so the pill showed a green "ok · 0 items" — indistinguishable
  from a feed that is simply empty. Non-feed bodies report "Jackett did not return an
  RSS feed" (or "malformed"), while a genuinely empty `<channel>` stays a success.
- **A failing feed backs off instead of being retried on every tick.** Each consecutive
  failure doubles the wait before the next attempt (×1, ×2, ×4, ×8, ceiling 30 minutes)
  and a success resets it, so a dead indexer costs a handful of requests a day instead
  of one per interval. At a 2 s interval the pill also oscillated Loading→Error on
  every tick; it now settles on the error.
- **"Choose episode / file…" looked like it did nothing.** The listing can take the
  full 30 s deadline and its only progress line lives in the detail panel, which the
  context-menu item did not open. It now opens the panel, and the line reads
  "Listing files — up to 30 s" beside a spinner instead of "Listing files…" in the
  dimmest text tier.
- **`--version` no longer panics on a Windows release build.** Those binaries are
  GUI-subsystem, so they start with no console and `println!` aborted with
  "failed printing to stdout: The pipe is being closed" (os error 232, exit 101)
  whenever a shell captured the output. `print_cli()` attaches to the parent
  console (kernel32 `AttachConsole`, declared inline — no new dependency) and
  ignores the write error: the version prints in a terminal and exits 0 when
  nothing is listening.
- **A config save cannot come back empty after a crash.** The temp file is written
  through a `File` and `sync_all()`ed, then renamed, then the directory is synced —
  temp+rename alone stops a torn file but not a zero-byte one on ext4
  `data=writeback`.
- **`--minimized` is a round trip, not a one-way door.** The tray's Show/Hide read
  focus as the visibility source, and a hidden window keeps its last focus state, so
  "Show" hid it again. Visibility is tracked explicitly, and the flag is ignored
  entirely when the tray could not be created.

### Added
- **One instance per profile.** Two copies shared `config.json` and each save
  rewrites the whole file, so the second silently clobbered the first one's
  favourites and history (and both polled RSS). A Unix socket beside the config is
  the lock: launching again asks the running copy to show its window and exits
  with a message. A socket left by a crash is reclaimed on the next start, and
  `--config <path>` keeps its own instance.
- **`--minimized`** — start with no window (tray icon and RSS polling only). The
  At login entry now uses it, so logging in no longer opens a window over
  whatever you were doing. On Wayland it starts minimised, since hiding a window
  is impossible there.
- **`windows-native` workflow** — builds the `.exe` natively on a GitHub-hosted
  Windows VM, checks `--version`, and asserts a window titled TorrentX actually
  appears (wgpu adapter, embedded fonts, `%APPDATA%` config). Dispatch-only,
  because it costs ~8 minutes and proves a claim rather than gating every push.
- `tests/cli.rs` spawns the binary and pins `--version` (exact string) and
  `--help` (every flag listed) — the CLI had no automated check on Linux.

### Changed
- **Edition 2024** (rustc 1.97), with the let-chains it unlocks. `gen` is a reserved
  keyword now (the search epoch guard uses `generation`), and `env::set_var` is unsafe.

## [18.6.10] — 2026-10-23

### Fixed
- **The "At login" pill is Linux-only now.** It was compiled on every target but
  writes an XDG autostart file, so on Windows/macOS it was a control that silently
  did nothing. Hidden there rather than left lying (no build ships for either).

## [18.6.9] — 2026-10-23

### Fixed
- **The Jackett API key could appear on screen.** A failed decode printed reqwest's
  error verbatim, which appends ` for url (…&apikey=<KEY>…)` — an HTML login wall
  or a schema change therefore showed the key in the results area, and in any
  screenshot or pasted bug report. Decode errors now use `without_url()` and name
  the likely cause instead, with a test that answers with HTML and asserts the
  message carries neither `apikey` nor the key.
- "At login" is compiled only on Linux (XDG autostart); elsewhere it is absent
  rather than a control that writes a file nothing reads.

### Verified
- Fault-injection pass against the stub (500 / 401 / HTML / 30 s / empty /
  garbage / all-500): every other path already reported a plain-language message.

## [18.6.8] — 2026-10-23

### Added
- **Settings → At login** writes `~/.config/autostart/torrentx.desktop` so the app
  starts with your session and RSS feeds keep polling (no daemon exists, so a
  closed window never polls). The entry names the AppImage, not the temporary
  mount path inside it, and quotes `Exec` for paths with spaces.

### Changed
- CI gains a "Lock convention" step: `if let Ok(..) = X.lock()` and
  `.lock().map(..)` fail the build — both silently drop an update on a poisoned
  mutex.

## [18.6.7] — 2026-10-23

### Fixed
- **A poisoned state lock no longer strands the UI.** 21 sites used `if let Ok(...)`
  / `.map(..).unwrap_or(0)`, which silently dropped the update: the status stayed
  "Searching" after an error, the result counter read 0 over a populated table,
  and Stop Stream found no child. Every lock now goes through one crate-root
  `lock_or_recover()`; no raw `lock()` remains outside tests.
- Test poisons the search-state mutex and asserts the error still lands.

## [18.6.6] — 2026-10-23

### Fixed
- **Right-click menu works on every column.** The tracking cell was the only
  cell with a full-rect hit layer; the other data cells had one too but never
  handed their response to the menu, so right-clicking Seeds, Size, Health or
  Date did nothing. Actions keeps its own buttons.
- **A panicking worker thread no longer kills the app** — the state mutexes in
  `jackett.rs` and the Settings spinner are locked through `lock_or_recover()`,
  which takes the data out of a poisoned lock instead of panicking again on the
  next search.

### Changed
- GUIDE: named two more limits — Jackett-only endpoints (Prowlarr will not
  connect) and one request per indexer per search (the page buttons slice the
  returned set, they do not fetch more).

## [18.6.5] — 2026-10-22

### Changed
- **Release builds use thin LTO.** Measured on this 8-core machine: an
  edit-rebuild drops from 311.7 s to 262.1 s (−49.6 s, −16 %) for +1.2 MB of
  AppImage (17.3 MB). Time to first painted frame is unchanged (85/97 ms fat vs
  74/100 ms thin, three runs each). `lto = true` goes back if download size ever
  becomes binding. Measurements, method and reproduce commands:
  [`docs/build-performance.md`](docs/build-performance.md).
- **The two 1000-line UI screens are split by concern** — `ui/results.rs`
  1019 → 503 lines and `ui/search.rs` 885 → 483, with the welcome screen,
  detail panel, filter band, recent-searches dropdown and pagination bar in
  their own modules. Pure moves; no behaviour change.

### Added
- **The view logic is tested** — filtering (seed floor, max size, minimum year
  in both ISO and RFC 2822 form, tracker, free text, health bands), sorting
  (every column, both directions, parsed dates, ratio with zero leechers) and
  paging (`All`, partial last page, stale page index, empty set) are now pure
  functions with tests, and `heal_col_order` is covered too. 58 → 62 unit tests.

### Fixed
- **Dual licence shipped properly.** MIT *and* Apache-2.0 texts both travel in
  the AppImage next to the font and icon notices; the licence section is
  readable secondary text at 2.0× and selectable, so it can be copied.
- **Settings is one pill with a wide enough dialog**, and the idle screen says
  when Jackett is unreachable instead of looking empty.
- **The result count, the badge and the table agree** — deduplication runs once
  when results are finalised instead of per frame only.
- **The build script never calls `sudo`** and treats the Windows bundle as
  best-effort, so a Linux build cannot be derailed by a missing cross toolchain.

### Security
- **`deny.toml` licence allow-list wired into CI.** `cargo deny` gates both
  advisories and licences on every push, alongside `cargo audit`.

## [18.6.4] — 2026-10-22

### Added
- **Settings are a floating dialog** — Preferences opens over the workspace and
  no longer reflows it, so the search results stay where you left them.
- **Labelled search pickers** — the category and indexer combos carry visible
  `Category` / `Indexer` labels instead of relying on their contents.

### Changed
- **The table header is the only sort control** — the filter band's duplicate
  "Sort: …" strip is gone; click a header to sort, click again to flip ASC/DESC.
- **Batch selection moved to the right end of the filter row** — Select with
  Copy/All/Clear sits by the results, Health chips keep the left edge.
- **The search row stays on one line**, with the button pinned to the window
  edge, and idle-state recent-search pills are centred.

### Fixed
- **9 RUSTSEC advisories cleared** by lockfile-only dependency bumps: `rustls`
  0.23.46, `rustls-webpki` 0.103.16, `quinn-proto` 0.11.18, `webpki-root-certs`
  1.0.10, and `chacha20` 0.10.2 (0.10.1 was yanked). `cargo audit` now reports
  0 vulnerabilities. Four unmaintained-crate warnings remain (`ttf-parser` via
  `egui`; `zbus_names`/`zvariant`/`zvariant_derive` via `accesskit`) — no patched
  release exists, so clearing them needs an upstream major bump.

## [18.6.3] — 2026-10-22

### Added
- **Design tokens module** (`src/ui/tokens.rs`) — one source of truth for spacing,
  radii, icon sizes and the selected-row accent, replacing literals scattered
  across the widgets.
- **`scripts/ui-standards.sh`** — a CI gate that fails the build when a widget
  hardcodes a spacing, radius or icon-size value instead of using a token.
- **Filled star for saved results** — a saved row is now visibly distinct at a
  glance instead of sharing the outline icon with unsaved ones.
- **Dedicated sort arrows** — `SortAsc`/`SortDesc` replace the recycled
  ArrowUp/ArrowDown glyphs, so a sort indicator no longer reads as navigation.
- **Accent focus ring** — keyboard focus is drawn as a ring in the accent colour.
- **Selected-row accent bar** — the active row carries a bar on its leading edge.

### Changed
- **Spacing, radii and icon sizes unified** — a 4 px grid, 3 radii and 4 icon
  sizes; cell insets follow the grid instead of per-widget padding.
- **Header tabs are one clickable pill** (icon included) rather than an icon and
  a separate label hit target.
- **Row hover is instant and pointer-driven** — it follows the pointer instead of
  fading in behind it.
- **Icons repinned to Lucide 1.61** and the `.torrent`/Play path from 18.6.2 is
  unchanged.

### Fixed
- **WCAG AA contrast in all 19 themes.** Dim/secondary text measured 1.9–3.4:1
  against its background (One Dark's `sub` was 3.98:1) — all below the 4.5:1 AA
  floor for body text. `src/themes.rs` now clamps every theme's muted tones to AA
  at load time, so a theme added later cannot regress below it.

## [18.6.2] — 2026-10-22

### Changed
- **Play starts from the result's `.torrent`, not the magnet.** Jackett already
  caches the torrent file for results that carry a link, so webtorrent gets the
  file names immediately instead of waiting for a metadata round trip: mpv
  opened in 1.0 s vs 3.0 s on a real swarm, and the episode picker now lists
  files on swarms that never hand out metadata.

## [18.6.1] — 2026-10-22

### Fixed
- **Tracker column no longer truncates** — 84 px -> 104 px, so names like
  "The Pirate Bay" fit instead of rendering as "The Pirat…".
- **"Actions" header right-aligned** over its icon buttons (it was stranded on
  the left edge of a right-aligned column).
- **Health filter chips are colour-coded** (Hot/Good green, Slow/Dying yellow,
  Dead red) instead of uniformly grey.

### Changed
- **Search results arrive as each indexer answers.** A search used to be one
  aggregate Jackett request that waited for the slowest tracker before showing
  anything. It now fans out one request per configured indexer and appends each
  answer as it lands, so the first page is usable while the rest is still
  scanning. One chosen indexer is still a single request; a tracker that fails
  does not hide the others (the error only shows if every one failed).
- **One status line while scanning** — the status bar now reads
  `Searching "q" — 127 so far  2.4s`; the extra banner row above the table is
  gone, so the layout no longer jumps when the last indexer lands.
- **Icon stroke weight unified** at 2.0 across all 21 SVGs (8 were 2.4).

## [18.6.0] — 2026-10-22

### Added
- **Episode picker for multi-file torrents.** Direct play streamed a pack's largest
  file and nothing else, so a season pack gave you whichever episode happened to be
  biggest. The detail panel now lists the torrent's files (index, name, size) in a
  filterable chooser, and Play passes `--select <index>`. "Largest file (default)"
  restores webtorrent's behaviour; the list needs peers (it comes from torrent
  metadata) and reports that when the swarm is unreachable.
- **Stop stream** in the detail panel while a stream is running; the button only
  appears when a streamer we started is still alive.

## [18.5.1] — 2026-10-22

### Fixed
- **A silent Play gave no way to diagnose it.** The streamer's output went to
  `/dev/null`, so when no player window appeared the user saw nothing — and the one
  line that explains it (webtorrent's `Peers: 0/0`) was discarded. Output now goes to
  `~/.cache/torrentx/webtorrent.log`, GUIDE's troubleshooting covers "nothing plays",
  and the spawn test asserts the redirect (reverting to `Stdio::null()` fails it).

## [18.5.0] — 2026-10-22

### Fixed
- **Korean titles rendered as □□□□.** The embedded font was Noto Sans CJK **JP**,
  which has Han and kana but no Hangul, while the README promised "CJK torrent titles".
  A Hangul subset (11,172 syllables + jamo, 1.1 MB) is now embedded next to it and
  registered as a second per-character fallback, and a unit test asserts every promised
  script resolves to a real glyph (a wrong face fails the test instead of a screenshot).
- **The font notice raised a licence doubt it could not settle.** It instructed the
  reader to rename the subset "if Google's notice for this face reserves 'Noto'". The
  face declares no Reserved Font Name (its copyright string is only
  "© 2014-2021 Adobe"), and the upstream Noto Sans CJK licence is the plain OFL with no
  reserved name either — so OFL condition 3 does not apply and the family name stays.
  The notice now records that evidence and the exact pyftsubset commands.
- **RSS dates rendered as "Tue, 07 Ma".** Jackett's torznab `pubDate` is RFC 2822
  (`Tue, 07 May 2024 10:20:30 +0000`), but `time_ago` only parsed RFC 3339, so every
  RSS row (and the item detail panel) fell back to chopping the first 10 bytes of the
  raw string. RFC 2822 is now parsed.
- **RSS items wrapped in CDATA disappeared.** quick-xml reports `<![CDATA[…]]>` as
  `Event::CData`, not `Event::Text`; the feed parser ignored it, so such items parsed
  with an empty title and were dropped as invalid. CDATA content is now accumulated
  like text.
- **RSS deduplication merged different episodes.** The feed list still keyed on
  `normalize(title)` (first four words), the exact heuristic fixed for search results
  in 18.4.1 — so episodes 1177/1178 or two films sharing an opening collapsed into
  one row. RSS now uses the same content-marker `dedupe_key` (guid when present) and
  keeps the best-seeded copy in feed order.

### Added
- **Direct play.** A **Play** button in the detail panel (and `P`) streams a result's
  largest file to mpv or vlc through `webtorrent-cli`, without waiting for the whole
  torrent — seeking works because webtorrent serves the file over localhost HTTP with
  range requests. The player is chosen in **Settings → Play with**; a second Play
  replaces the first stream and quitting the app stops it. Missing `webtorrent-cli` is
  reported as an actionable toast, not a silent failure.

### Changed
- **Embedded CJK font data shrank by 3.2 MB** (9.23 MB → 6.03 MB: JP 4.90 MB +
  KR 1.13 MB). The subsets drop vertical-layout tables and hinting, which egui never
  uses, so the shipped bytes track what the app renders.

## [18.4.1] — 2026-09-15

### Fixed
- **Deduplication was hiding real results.** The key was `normalize(title)` — the first
  four words once quality noise is stripped — so different releases sharing an opening
  collapsed into one row and only the best-seeded survived. Measured against live
  Jackett output:
  - `One Piece Episode of East Blue (2017)`, `… Sabo (2015)` and `… Skypiea (2018)` —
    three different films, one visible row;
  - episodes **1177 and 1178** merged; episodes **227-259, 260-292 and 293-325** merged;
  - `ubuntu 22.04.3` merged with `ubuntu 23.10`.

  That sample had 12 such merges out of 382 results, and 10 out of 141 for `ubuntu`.
  The key is now the same normalised title **plus content markers** — episode numbers in
  any of their usual forms (`s02e07`, `ep1178`, `1x02`, bare `1177`), years, and other
  numbers that are not encoding — so those cases separate (0 wrong merges in both
  samples) while the same release in another encoding still collapses.

  Resolutions and codec numbers (`1080`, `720`, `264`, `265`, …) are explicitly
  excluded from the markers, because they differ between two copies of the *same*
  release and must not split them.

## [18.4.0] — 2026-09-15

### Added
- **Settings can start a local Jackett for you.** No more installing and running one
  by hand: if nothing answers at the configured URL, the button starts the Jackett
  copy already on your machine, or — with a single click — downloads the official
  build once into `~/.local/share/torrentx`. It waits for it to come up, fills in the
  URL, and reads the generated API key from Jackett's own `ServerConfig.json`. It
  binds localhost only, reuses your existing Jackett config when there is one (so
  your indexers come with it), and is stopped when the app exits — the app only ever
  kills the instance it started itself.
  Jackett is deliberately **not** bundled: 48MB packed / 116MB unpacked and GPL-2.0
  against this MIT app, and an AppImage payload is a read-only squashfs that Jackett
  could never update itself from.
- An **Open Jackett** button beside it, for adding indexers in Jackett's own page.

### Changed
- The "cannot reach Jackett" message now points at Settings → Start Jackett before
  falling back to the systemd command.

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

[18.4.1]: https://github.com/chethan62/torrentx/releases/tag/v18.4.1
[18.4.0]: https://github.com/chethan62/torrentx/releases/tag/v18.4.0
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
