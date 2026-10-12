# The Flare Rust rewrite

How Flare's C++/Qt codebase is being ported to Rust, and the rules that keep the
port honest. This is the working reference for the rewrite; it describes intent
and structure, not a schedule.

## Why

Flare is a fork of OpenToonz. That codebase is a large, mature C++/Qt
application, and the Flash/Animate work has to live alongside it rather than
around it. Rewriting everything would throw away two decades of animation
tooling, so the port is **incremental and structural**: the OpenToonz file
layout, module names and subsystem boundaries are preserved exactly, and the
implementation language changes underneath them.

The goal is a single application, not two. See "One application" below.

## The central rule: a ported module lives beside what it ports

There is no separate Rust tree. A ported module sits next to the C++ it
replaces:

```
flare/sources/common/<subsystem>/rust/<name>.rs
```

Concretely, the nine modules registered in `flare/sources/rust/flare_formats.rs`
already follow this:

| Path | Ports |
|---|---|
| `flare/sources/common/flash/rust/` (`swf`, `xfl`, `fcp`, `framecount`, `xflrender`, `symlib`) | `flare/sources/common/flash/` SWF/XFL/CFBF readers and rendering |
| `flare/sources/common/moho/rust/moho.rs` | `flare/sources/common/moho/` Lost Marble reader |
| `flare/sources/common/tapptools/rust/tenv.rs` | `flare/sources/common/tapptools/` environment helpers |
| `flare/sources/common/tsystem/rust/tfilepath.rs` | `flare/sources/common/tsystem/` path helpers |

`flare/sources/common/tgame/` is still C++ only — it has no `rust/` directory and
is not registered. It is the next candidate when the port reaches it.

Two things follow, and both are enforced rather than advisory:

1. **Every ported file must be registered** in the `#[path]` list in
   `flare/sources/rust/flare_formats.rs`. An unregistered file compiles fine and
   ships as dead code — this is the single most common way a port silently does
   nothing.
2. **The crate manifest is `flare/sources/Cargo.toml`** (one workspace), not a
   nested project. Tests run with `python tests/run_rust.py`.

### Language split

| Layer | Language | Rationale |
|---|---|---|
| Backend / core / engine | **Rust** | New work defaults here. |
| UI shell, tooling, importers | **TypeScript** | Modeled on Next2Flash; web/CLI-adjacent tooling. |
| Scripts, glue, build orchestration | **Python** (small) | Existing CI and `tools/` are Python; keep them. |

## One application

The Qt application (`flare/sources/flare/`) is the product. The native Studio
workspace (`flare/sources/flare/rust/studio/`) began as a separate executable
and is being folded into the main application rather than shipped alongside it.
When it moves, the same rule applies: the Studio code belongs under the
subsystem it implements, not in a standalone tree.

The Studio is also the reference for the target UI. Its features — the Animate
workspace, timeline, layer model, docking, theming — are the shape the ported
application converges on.

## Upstream sync must not undo the port

Flare tracks upstream OpenToonz. The sync agent (`tools/sync/upstream_agent.py`)
cherry-picks upstream commits and rebrands them. It is the mechanism most likely
to silently revert Flare-specific work, so its behaviour is pinned by tests.

Two layers matter:

- **Shared rebrand rules** rewrite `#include "toonzlib/..."`-style includes and
  brand strings, and apply to every text file.
- **CMake-scoped rules** (`CMAKE_REBRAND_RULES`) rewrite bare build-file
  identifiers — `add_subdirectory(toonzlib)`, `_find_toonz_library`,
  `$<TARGET_FILE:toonzqt>`, `toonz.qrc`. These apply **only** to `CMakeLists.txt`
  and `*.cmake`, deliberately: `toonzQt` and `ToonzLib` appear in roughly 116 C++
  comments and string literals, and rewriting a literal can change behaviour.

An earlier version had no CMake-scoped rules. A sync then replaced Flare's
build files with upstream's, every one of the identifiers above passed through
the rebrand pass unchanged, and `cmake` failed at configure time on Windows,
Linux and macOS at once. `tools/sync/tests/test_upstream_agent.py` now pins each
identifier individually, so a rule that goes missing fails a test.

## Licensing rules for absorbed work

Absorbing a project is not the same as copying it. Flare is BSD-3-Clause.

- **Permissive (MIT/Apache/BSD)** — copy, with attribution and the licence text.
  `AnimationCraft` is absorbed this way; its provenance is recorded in
  `flare/sources/flare/rust/core/ORIGIN.md` with the exact commit SHA.
- **Copyleft (GPL/LGPL/AGPL)** — do **not** copy. Reference the behaviour and
  reimplement. `Flare-Animate/nflare` is GPL-3.0 and is referenced, not vendored;
  copying it would relicense combined work and is not permitted.
- **No licence file** — assume no permission. `AnimationCraft` currently declares
  MIT/Apache-2.0 in its manifest but ships no `LICENSE` file; that discrepancy is
  worth closing upstream before more of it is vendored.
- Record every absorbed source in `ORIGIN.md`, naming the upstream and commit.

## Keeping the documentation honest

`LOGIC-ATLAS.md` is the subsystem map. `scripts/atlas_check.py` resolves every
path, symbol and constant claim in it against the tree and fails on drift. When
a subsystem changes, its atlas section and the snapshot line are updated in the
same change, so a document claim cannot quietly become false.

## What "done" means for a module

A module is ported when:

1. The Rust implementation lives at `common/<subsystem>/rust/<name>.rs`.
2. It is registered in the `#[path]` list in `flare_formats.rs`.
3. The C++ caller path is switched over, or the C++ is deleted — not left as a
   parallel implementation that both run.
4. Behaviour is covered by `cargo test`, including the edge cases the C++ had.
5. `LOGIC-ATLAS.md` is updated and `python scripts/atlas_check.py` passes.

A green build with a registered-but-uncalled module is not ported.

## Reference

| Source | Role |
|---|---|
| AnimationCraft (`KMSDax`) | Rust document/timeline/render core (`ac-core`, `ac-render`) |
| Next2Flash (`SSF2-Mods-Official`) | TypeScript tooling basis for SWF import/export |
| `Flare-Animate/nflare` | Referenced only (GPL-3.0) |
| OpenToonz / Tahoma2D | Upstream; structure is preserved, implementations are ported |