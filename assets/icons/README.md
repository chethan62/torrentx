# Icons

All 20 SVGs are **Lucide** (<https://lucide.dev>), pinned to **lucide-static
1.61.0**, ISC licence, 24x24 grid, stroke-width 2, `stroke="currentColor"` so
`egui_extras` tints them with the active theme's palette.

Lucide is ISC-licensed (© Lucide Contributors); the full text ships at
<https://github.com/lucide-icons/lucide/blob/main/LICENSE>.

`web.svg` is Lucide's `external-link` glyph under the name the app already
uses (`SvgIcon::Web`, "open in browser"). Rename the variant if you prefer the
upstream name.

Regenerate after a Lucide bump:

```sh
./update-icons.sh 1.61.0
```

The script rewrites every SVG in this directory (Lucide markup only, no
edits) and reports any icon that 404s.
