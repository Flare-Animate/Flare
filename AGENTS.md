
## Efficiency rules (all sessions, workflows, subagents, cloud routine)
- Invoke skills `strict`, `anthropic-skills:caveman` + `anthropic-skills:ponytail`; terse output, laziest working solution.
- Haiku for tests/mechanical; opus only for hardest work. Reuse `.Codex/workflows/flare-feature-pass.js`.
- No exploration sprawl: grep before read, batch calls, reports <=6 lines. Usage is limited.
- New backend code: Rust. Mid/frontend: TypeScript (+small Python) based on Next2Flash.

## Where code lives
- Rust never gets a sidecar tree. A ported module lives at
  `flare/sources/common/<subsystem>/rust/<name>.rs`, beside the C++ it ports, and must be
  registered in the `#[path]` list in `flare/sources/rust/flare_formats.rs` (an unregistered
  file compiles and ships as dead code). Crate manifest: `flare/sources/Cargo.toml`;
  tests: `python tests/run_rust.py`.
- Before working, read `LOGIC-ATLAS.md` (subsystem map + invariants). If you change a
  subsystem its section, update it and the Snapshot line: `python scripts/atlas_check.py`
  resolves every path/symbol/constant claim against the tree and fails on drift.
