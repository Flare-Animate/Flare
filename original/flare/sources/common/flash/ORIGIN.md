# ac-xfl in Flare

Imported from `Flare-Animate/RustConversion-AnimateCraft` @ `f636b05`, `crates/ac-xfl`.
Licence: `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES.md` at `flare/sources/`).

Layout adaptation only: the crate manifest and root live in this OpenToonz-matching
folder (root `ac_xfl.rs`); files carry an `ac_` prefix and are wired with `#[path]` so `crate::x`
paths are unchanged. Package name `ac-xfl` is kept. Source text is otherwise verbatim
except relative asset / fixture paths.

## Absorbed: `lso.rs` (Flash Local Shared Object)

`common/flash/lso.rs` (a `flare_formats` module) decodes and encodes `.sol`
files: the SOL container header plus an AMF0 body. Absorbed from
[`ruffle-rs/rust-flash-lso`](https://github.com/ruffle-rs/rust-flash-lso)
(`flash-lso` crate, version 0.7.0), licence **MIT** (`LICENSE-MIT` upstream).

The container constants and the AMF0 type handling follow upstream
`flash-lso/src/{read,write}.rs` and `flash-lso/src/amf0/{read,write}.rs`. The
upstream parser is built on `nom`; this port keeps the same format semantics
with a bounds-checked cursor and no external parser dependency, so it can be
reused by `ac-xfl`. AMF3 bodies are recognised but not decoded.
