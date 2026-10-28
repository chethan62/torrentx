# Licences and notices

TorrentX is **MIT OR Apache-2.0** (© [chethan62](https://github.com/chethan62)) — take whichever
suits you; Apache-2.0 adds an explicit patent grant.

Every licence the binary carries ships with it: `usr/share/licenses/torrentx/` in the AppImage and
the tarball holds `MIT.txt`, `Apache-2.0.txt`, `NotoSansCJK-OFL.txt` and `Lucide-ISC.txt`.
`cargo deny` gates the dependency tree against the allow-list in
[`deny.toml`](../deny.toml) in CI.

Paths below are relative to the repo root, and to the same directory inside the AppImage.

| Part | Licence | Notice file |
| --- | --- | --- |
| TorrentX source and binary | MIT OR Apache-2.0 | `LICENSE-MIT`, `LICENSE-APACHE` |
| `NotoSansCJK*-subset.otf` (modified Noto CJK subsets) | OFL-1.1 | `NotoSansCJKjp-subset.LICENSE.txt` |
| `icons/*.svg` (Lucide 1.61.0) | ISC | `Lucide.ISC.LICENSE.txt` |

## Bundled icons

`assets/icons/*.svg` are [Lucide](https://lucide.dev) glyphs (ISC licence, © Lucide Contributors),
pinned to lucide-static 1.61.0, `stroke="currentColor"` so the palette tints them.
`assets/icons/update-icons.sh <version>` refreshes them from upstream; see
`assets/icons/README.md`.

## Bundled fonts

Torrent titles are often Japanese, Chinese or Korean, so two subsets are embedded in every build
(`include_bytes!` in `src/app.rs`) as per-character fallbacks after egui's Latin face:

| File | Covers | Size |
|------|--------|------|
| `assets/NotoSansCJKjp-subset.otf` | Han (CJK Unified Ideographs), hiragana, katakana, CJK/fullwidth punctuation | 4.9 MB |
| `assets/NotoSansKR-subset.otf` | Hangul syllables and jamo | 1.1 MB |

They are single-face `.otf` files because egui's `ab_glyph` cannot select a face out of a system
`.ttc` collection — the app therefore needs no CJK fonts installed. Together they are 6.0 MB of the
binary.

Both are subsets of **Noto Sans CJK 2.004** (Adobe/Google, a derivative of Source Han Sans) under the
**SIL Open Font License 1.1** — © 2014-2021 Adobe; "Noto" is a trademark of Google Inc. A subset is a
*modified* version, so the copyright notice, the licence, the modification evidence (the upstream
faces declare **no** Reserved Font Name) and the exact `pyftsubset` commands ship in
[`assets/NotoSansCJKjp-subset.LICENSE.txt`](../assets/NotoSansCJKjp-subset.LICENSE.txt).
A unit test asserts the promised scripts resolve to real glyphs, and the same notice is readable in
**About → Bundled font licence** (release artefacts are a bare binary and an AppImage, so the text
travels inside both).
