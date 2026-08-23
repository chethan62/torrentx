# Design System — TorrentX

Cross-session design decisions for the torrentx Rust/egui app. Load and apply before any UI edit.

## Direction

**Personality:** Precision & Density — a technical tool for power users. Dense data tables, no decorative chrome.
**Foundation:** Cool/dark slate (theme-dependent hue, all dark themes)
**Depth:** Borders-only — no shadows anywhere. Separation via low-alpha borders + surface lightness steps + row tinting.
**Signature:** Per-row fixed SVG action icons (Lucide, tinted to theme), always visible, right-aligned; full-row/cell click layers; seed/leech counts color-coded (green/red).

## Tokens

### Spacing
Base: 4px (egui points). Used as multiples: 4, 6, 8, 10, 12, 14, 16, 20.
Rule: leading cell text is never flush-left — `ui.add_space(6.0)` before name, `ui.add_space(4.0)` before data cells. Panel margin X = 12. Row inner margin = symmetric(16, 10) in Favorites; RSS feed rows symmetric(10, 7).

### Row heights
- Compact: 32.0 · Normal (default): 44.0 · Roomy: 56.0
- User-selectable in Settings → Display → Rows.

### Font sizes (user-selectable)
- S: 12 · M (default): 14 · L: 16
- Derived sizes: `fs - 1.5` sub-meta, `fs - 2.0` dim meta, `fs - 3.0` badge counts; headers +1–3; tab labels 14.

### Colors (dark themes; all share this shape)
```
bg/surface/surface2/hdr  — 4-step elevation, same hue, lightness only
accent  — theme accent (TokyoNight #7AA2F7, Cyberpunk #06B6D4, …)
text    — primary        sub  — secondary
dim     — muted          border — surface2 value
green   — seeds/positive  red — leechers/destructive  yellow — favorites
row_odd / row_even — zebra, 1 lightness step
row_sel — accent @ ~22% alpha   row_hov — accent @ ~6-7% alpha
```

### Radius
PANEL_RADIUS = 8.0. Small controls 4–6, buttons 5, panels 8. No large-radius decoration.

### Icons
**Lucide SVG only** (MIT, embedded via `bytes://` image loader + `egui::Image::from_bytes(...).tint(color)`). NEVER font glyphs (emoji, ✕, ⟳, ▶ — they tofu). Pre-tint: `svg.replace("currentColor", "#fff")` so resvg black doesn't defeat the tint. Sizes: 16px in buttons, 12–15 for inline.
- Registered inline set includes ArrowUp/Down, ChevronLeft/Right, Circle/CircleDot, Check, Info, Settings, Refresh, Close, Copy, plus row-action icons.
- Icon+text controls use a **single native button surface** (`Button::image_and_text` / `icon_text_btn`) so the icon and label are one hover/focus/click target — never an adjacent decorative icon plus text-only button.

## Patterns

### Button — svg_btn (icon action)
- Size: 32×28 · Radius: 5 · Fill: tint(color,14) idle / tint(color,26) hover · Stroke: 1px tint(color,70)
- Usage: row actions (Magnet/Copy/Download/Star/Close) — FIXED visible, never hover-reveal
- Behavior: full-rect `allocate_exact_size` + `Sense::click()` — the whole button, not just the icon

### Button — act_btn (text action)
- Height: 25 · Radius: 5 · Fill: tint(color,18) · Stroke: 1px tint(color,70) · Font: 11.5

### Button — wide_icon_btn (detail-panel action)
- Full-width, icon + label, accent-tinted. Used in detail panels (Open Magnet / Copy Magnet / Download / Save to Favorites).

### Rows — click contract (CRITICAL)
- **Interact-first:** `ui.interact(ui.max_rect(), Id::new(("row", i)), Sense::click())` BEFORE drawing any content. Clicks on text pixels fall through hover-only labels to this layer (egui 0.36 hit-test: click goes to topmost click-sensing widget; hover-only widgets never block).
- Buttons drawn after win over the row layer automatically (real Buttons sense click).
- Hover: `CursorIcon::PointingHand` on row hover.
- Verify with headless tests (`tests/click_repro.rs`): press/release as separate frames via `ctx.run_ui` + `end_pass` + `textures_delta.clear()`.

### Table cells (search results, RSS)
- Every data column gets its own interact layer (Tracker/Size/Seeds/Date…) → click selects row / opens detail.
- Zebra rows row_odd/row_even; selected row = row_sel fill; hover = row_hov fill.
- Numbers tabular; seed count colored via `seed_col()`; size via `fmt_size()`.

### Status dots (RSS feeds / Jackett)
- Circle SVG green=Ok · Refresh SVG accent=Loading · Close SVG red=Error · CircleDot SVG dim=Idle.
- Status pills use `status_icon_pill`: SVG + label on one tinted surface. No glyph exceptions.

### Toasts
- `self.toast(msg, color)` — bottom transient, 2.5s. Success green, info accent.
- Slide in from right (30px, ~150ms ease-out cubic), fade out last 0.4s. TTL 2.5s.

### Motion (animation system, added 2026-08)
- All easing centralized in `VisualTokens` (themes.rs): `easing` = ease-out cubic for entrances (`1-(1-t)³`), `easing_hover` = smoothstep for bidirectional hover.
- Animated surfaces: search state transitions (fade, 200ms), table content on page/filter/sort change (overlay dissolve, 250ms), detail panel open/close (fade + downward settle 8px, 200ms open / 150ms close, cached `detail_row` so close animates), toasts (slide+fade).
- Row hover: eased color lerp between base row and `row_hov` (100ms, smoothstep).
- Press feedback: `svg_btn` shrinks to 94% while pointer-down (tactile confirmation).
- Rule: entrances NEVER ease-in; hover/exit use symmetric ease; <300ms for everything.

## Decisions

| Decision | Rationale | Date |
|----------|-----------|------|
| Borders-only depth | Dense data tool; shadows add weight without information | 2026-08 |
| Lucide SVG icons, never glyphs | Glyphs tofu when font stack breaks; SVG is vector-crisp, theme-tintable | 2026-08 |
| Fixed always-visible row actions | User rejected hover-reveal ("dont make hover-reveal make fixed") | 2026-08-21 |
| Full-row/cell click layers | User: "text not clickable" — every cell must register | 2026-08-22 |
| Leading cell padding (6/4) | User: "dont align text to full left edge" | 2026-08 |
| Interact-first, headless-verified | egui 0.36 hit-test proof in tests/click_repro.rs | 2026-08-22 |
| Ease-out entrances, <300ms | Interface-design motion rules; fast start never feels slow | 2026-08-23 |
| Press feedback on icon buttons | Tactile confirmation (scale 0.94 while held) | 2026-08-23 |
