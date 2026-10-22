# Icons

Every SVG here is **Lucide** (<https://lucide.dev>), pinned to **lucide-static
1.61.0**, ISC licence, 24x24 grid, stroke-width 2, `stroke="currentColor"` so
`egui_extras` tints them with the active theme's palette.

Lucide is ISC-licensed (© Lucide Contributors); the full text ships at
<https://github.com/lucide-icons/lucide/blob/main/LICENSE>.

Renames and derived icons:

- `web.svg` is Lucide's `external-link` glyph under the name the app already
  uses (`SvgIcon::Web`, "open in browser"). Rename the variant if you prefer the
  upstream name.
- `arrow-down-narrow-wide.svg` is Lucide's `arrow-down-wide-narrow` glyph —
  the app names it by the sort direction it means (`SvgIcon::SortDesc`).
- `star-filled.svg` is derived locally from `star.svg` (same path, plus
  `fill="currentColor"`); it marks a result that is already in Favorites.
  `update-icons.sh` skips `*-filled` files because Lucide has no equivalent.

Regenerate after a Lucide bump:

```sh
./update-icons.sh 1.61.0
```

The script rewrites every SVG in this directory (Lucide markup only, no
edits) and reports any icon that 404s.
