# ac-render in Flare

Imported from `Flare-Animate/RustConversion-AnimateCraft` @ `f636b05`, `crates/ac-render`.
Licence: `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES.md` at `flare/sources/`).

Layout adaptation only: the crate manifest and root live in this OpenToonz-matching
folder (root `ac_render.rs`); files carry an `ac_` prefix and are wired with `#[path]` so `crate::x`
paths are unchanged. Package name `ac-render` is kept. Source text is otherwise verbatim
except relative asset / fixture paths.

Adaptation: `ac_fonts.rs` test `fallback_fonts_cover_fullwidth_when_installed` only asserts when fc-list reports a family from the preferred UI list for U+FF02 (it failed on hosts with only unifont/wqy-zenhei).
