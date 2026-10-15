# minnamate editor in Flare

Imported from `mirvoxtm/minnamate` @ `104f3d3`, `apps/editor`.
**Licence: upstream has NO licence file (clean-room project).** Imported with that flag kept;
obtain a licence grant from the author before any redistribution. Manifest declares MIT OR Apache-2.0
only as a workspace placeholder and `publish = false`.

Layout adaptation: manifest + root live in this OpenToonz-matching folder; depends on `minnamate-core` at `../tnztools`.
Modules are `mm_ed_<name>.rs`, wired from `minnamate_editor_main.rs` via `#[path]` (crate paths unchanged).
`assets/logo.png` is include_bytes!'d (only asset). Docs: `docs/minnamate/`. Binary: `minnamate`.
