# Build performance: sccache and LTO (measured 2026-10-22)

All numbers below were measured on the dev box (8 cores, `sccache 0.18.0`,
`mold 3.0.0`, Rust stable 1.97) with `/usr/bin/time` around real builds, not
estimated. Reproduce with the commands at the end.

## TL;DR

| Decision | Evidence |
|---|---|
| Keep `sccache` as `rustc-wrapper` | 369 of 371 cache hits are **C/C++ + assembler** (`aws-lc-sys`, `ring`) — every clean rebuild skips that compile |
| Keep incremental **on** for dev | dev edit-rebuild 3.28 s incremental vs 7.55 s with `CARGO_INCREMENTAL=0` |
| **Use `lto = "thin"` for release** | release edit-rebuild **311.7 s → 262.1 s (−16 %)**; binary 30.8 → 35.8 MB, AppImage 16.1 → 17.3 MB |

## What the "0.26 % sccache hit rate" actually was

A cold snapshot taken right after `sccache` had just started. Steady state:

```
Cache hits 371   Cache hits (Assembler) 111   Cache hits (C/C++) 258   Cache hits (Rust) 2
Cache misses 350 (all Rust)                  Non-cacheable calls 93
Cache hits rate 51–57 % overall · C/C++ 100 % · Assembler 100 % · Rust 0.57 %
```

Rust crates are structurally non-cacheable here, for two reasons sccache
reports itself (`sccache --show-stats`):

1. **`crate-type: bin`** — the final `torrentx` crate is an executable, and
   sccache cannot verify a binary's link inputs, so it never caches it.
2. **incremental compilation** — dev builds pass `-C incremental`, which sccache
   refuses to cache.

So a Rust hit rate near zero is expected behaviour, not a misconfiguration. The
cache still earns its keep: the expensive native dependencies (BoringSSL via
`aws-lc-sys`, `ring`) come back at 100 % on every clean build.

## Build-time measurements

| Scenario | Time | Cache effect |
|---|---:|---|
| dev edit-rebuild, incremental (default) | **3.28 s** | none needed |
| dev edit-rebuild, `CARGO_INCREMENTAL=0` | 7.55 s | none (bin crate) |
| release edit-rebuild, `lto = true` (fat) | **311.67 s** | — |
| release edit-rebuild, `lto = "thin"` | **262.11 s** | — |
| release full rebuild after `cargo clean` | ~300 s | C/C++ + asm reused |
| `cargo build` after the C/C++ deps are in cache | seconds | 111 asm + 258 C/C++ hits |

Fat LTO dominates a release rebuild: one rustc process does the whole-program
import pass single-threaded (~5 min at 100 % CPU on this box), twice (the bin and
its rlib). Thin LTO keeps per-CGU codegen parallel across 8 cores, which is where
the 16 % comes from.

## The tradeoff we accepted

Thin LTO optimises across crate boundaries per codegen unit, so it dedupes less:

| Artefact | fat LTO | thin LTO | Δ |
|---|---:|---:|---:|
| stripped binary | 30.8 MB | 35.8 MB | +16 % |
| AppImage | 16.1 MB | 17.3 MB | +1.2 MB (+7.4 %) |

For a desktop app whose download is a 17 MB AppImage, 1.2 MB is cheap; 49 s off
every release rebuild is not. **Switch back to `lto = true`** if download size
ever becomes the binding constraint (e.g. a bandwidth-capped mirror), and accept
the slower release builds.

## Not done, and why

- **`codegen-units = 1`** with thin LTO: would recover some of the size but
  serialises codegen again — the opposite of what we want.
- **`panic = "abort"`**: smaller and faster, but a panic then aborts with no
  unwinding, and this app reports fatal I/O errors through panics-free paths
  today; not worth changing for ~1 MB.
- **Tuning sccache's Rust hit rate**: nothing to tune — the two blockers above
  are by design. A CI job could set `CARGO_INCREMENTAL=0` and cache `~/.cache/sccache`
  across runs to get Rust hits for release builds; that is a CI-time optimisation,
  not a local one, and is not wired up yet.

## Reproduce

```sh
# cache behaviour
sccache --show-stats

# dev edit-rebuild, incremental vs not
touch src/main.rs && time cargo build
touch src/main.rs && time env CARGO_INCREMENTAL=0 cargo build

# release edit-rebuild, fat vs thin (no file edits needed)
touch src/main.rs && time cargo build --release
touch src/main.rs && time env CARGO_PROFILE_RELEASE_LTO=thin cargo build --release
```
