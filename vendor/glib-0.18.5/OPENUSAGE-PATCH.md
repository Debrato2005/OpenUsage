# GLib iterator safety backport

This directory contains the crates.io `glib 0.18.5` source archive with the two-line fix from upstream commit [`b5a4071e439bef2b5eea76c3aa25e5ae84839e34`](https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34).

The fix addresses [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html). `VariantStrIter::impl_get` now passes a mutable out-pointer to `g_variant_get_child`. The upstream version passes an immutable reference to memory that C changes. Optimized builds can then read a null pointer.

GTK3 requires the 0.18 API. The root Cargo patch selects this local copy for every consumer without changing the package version. A complete migration to glib 0.20 requires compatible GTK and Tauri dependencies. No advisory suppression or version disguise is used.

## Provenance

- Archive: `https://static.crates.io/crates/glib/glib-0.18.5.crate`
- Archive SHA-256: `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`
- Original source revision: `42b9caf98e03ded086362d9653ca58fe94dc8658`, from the archive's `.cargo_vcs_info.json`.
- Patched iterator SHA-256: `a0f5ee8acb8faa089bcdfbc9a57372609fce7654026ccef7d9a224d05a654ccc`
- License: MIT. The original `LICENSE` and source headers remain included.

All 121 archive files remain present. Only `src/variant_iter.rs` differs from the archive: `let p` becomes `let mut p`, and `&p` becomes `&mut p`. This note is an added file. Cargo's local cache and checksum metadata are not copied.

## Regression

`apps/desktop/src-tauri/tests/glib_variant.rs` exercises forward, backward, indexed and mixed iteration, empty arrays, Unicode, exhaustion and type refusal. The workspace test profile compiles glib with optimization level 2, where the original defect reproduces.

On October 24, 2026, an isolated Linux probe with this test aborted on the unpatched source. The same probe passed after the fix. Both runs used the shared Cargo target and synthetic strings. This is iterator evidence, not native desktop acceptance.

Remove this patch after the dependency chain supports a fixed upstream version. Recheck the lockfile, notices, optimized regression and native acceptance at that time.
