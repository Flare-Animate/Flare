# The Flare Rust Rewrite

How Flare's C++/Qt codebase is being transformed into Rust, the rules that govern the migration, and the external projects whose code is being absorbed into the port. This document describes architecture, process, and policy rather than a delivery schedule.

## Why

Flare is a fork of OpenToonz. OpenToonz is a large, mature C++/Qt application containing decades of production-tested animation tooling. Throwing that work away and building an entirely new editor would be wasteful, so the rewrite preserves the upstream architecture while replacing implementations with Rust.

The objective is not merely to write new Rust modules that resemble the existing code. The objective is to produce a Rust implementation of the OpenToonz/Tahoma2D codebase while preserving the structure, layout, naming, responsibilities, and behaviour of the original project.

As a general rule, every C++ source file should eventually have a Rust equivalent occupying the same role within the application's architecture. Exceptions are allowed only where retaining a non-Rust implementation is demonstrably safer, more practical, or technically superior for its specific use case, such as selected scripting, tooling, or build-integration components.

The port is therefore **structural, incremental, and fidelity-driven**:

- Directory layouts remain aligned with OpenToonz/Tahoma2D.
- Module boundaries remain aligned with OpenToonz/Tahoma2D.
- File responsibilities remain aligned with OpenToonz/Tahoma2D.
- Public behaviour remains aligned with OpenToonz/Tahoma2D.
- Implementations are rewritten in Rust, replacing the original files, and match those files down to each line of code.
- Once changes are stable, the original script can be completely removed to the codebase, provided everything is ported over from it and it hasn't been subject to newer changes.

Where practical, a Rust port should mirror the source file it replaces as closely as possible, preserving intent, logic flow, naming, variables, line content, and behaviour so that the relationship between the original implementation and its replacement remains obvious.

The goal is a single application, not multiple competing products.

## The central rule: a ported module lives where the original is/was

There is no separate Rust codebase.

Every Rust file belongs alongside the subsystem it replaces:

```text
flare/sources/common/<subsystem>/rust/<name>.rs
```

A Rust port is considered an implementation replacement for an existing subsystem, not the foundation of a new parallel architecture.

Concretely, the modules currently registered in:

```text
flare/sources/rust/flare_formats.rs
```

already follow this pattern:

| Path | Ports |
|--------|--------|
| `flare/sources/common/flash/rust/` (`swf`, `xfl`, `fcp`, `framecount`, `xflrender`, `symlib`) | Flash-related readers, formats and rendering |
| `flare/sources/common/moho/rust/moho.rs` | Lost Marble / Moho support |
| `flare/sources/common/tapptools/rust/tenv.rs` | Environment helpers |
| `flare/sources/common/tsystem/rust/tfilepath.rs` | Path and filesystem helpers |

`flare/sources/common/tgame/` currently remains C++ only and has not yet been ported.

Two rules follow directly from this structure:

### 1. Every Rust file must be registered

Every ported module must be registered in:

```rust
flare/sources/rust/flare_formats.rs
```

using the appropriate `#[path]` declaration.

Unregistered modules compile successfully yet remain unused, making this the most common form of silent failure during migration.

### 2. Flare remains one workspace

The project manifest is:

```text
flare/sources/Cargo.toml
```

There are no independent Rust projects nested within the tree.

Tests are executed through:

```text
python tests/run_rust.py
```

---

## Language split

| Layer | Language | Rationale |
|---------|---------|---------|
| Core engine, backend, rendering, document model | **Rust** | Primary implementation language moving forward. |
| Tooling, web-facing utilities, importer/exporter workflows | **TypeScript** | Based largely on Next2Flash tooling patterns. |
| Small automation and orchestration tasks | **Python** | Existing tooling and CI infrastructure already depend on it. |

New engine-level work defaults to Rust unless a clear technical reason exists for another language.

## Code absorption and redistribution

The rewrite is not solely a language migration. It is also a consolidation effort.

Three projects serve as major sources of functionality, ideas, implementations, or workflows:

- AnimationCraft
- Next2Flash
- NFlare

Their code should not exist as isolated imported subsystems.

Instead, equivalent functionality should be mapped onto the Rust implementations of the OpenToonz/Tahoma2D architecture.

### AnimationCraft

AnimationCraft's codebase is treated as a source of Rust-native animation technology.

Rather than embedding AnimationCraft as a standalone component, its systems should be decomposed and redistributed throughout Flare's Rust architecture, with functionality mapped into the corresponding editor, rendering, animation, timeline, document, asset, and playback systems.

The objective is that no useful AnimationCraft functionality is discarded simply because its original project structure differs from OpenToonz.

### Next2Flash

Next2Flash serves as the primary reference for TypeScript-based Flash tooling.

Importer, exporter, workflow, asset-processing, and web-adjacent functionality should be integrated into the corresponding Flare subsystems rather than preserved as a separate product inside the repository.

### NFlare

NFlare remains a behavioural and architectural reference.

Its concepts, workflows, and implementation ideas can guide equivalent systems within Flare's Rust architecture.

Where licensing permits, functionality should be integrated into the corresponding subsystem rather than duplicated in parallel.

The overall objective is a unified codebase in which AnimationCraft, Next2Flash, NFlare, and OpenToonz contributions converge into a single coherent application instead of remaining as distinct projects.

## One application

The Qt application:

```text
flare/sources/flare/
```

is the product.

The Studio workspace:

```text
flare/sources/flare/rust/studio/
```

began life as a separate executable but is being merged into the main application rather than being shipped as a second editor.

When Studio systems migrate, they should move into the subsystems they implement rather than remaining in a dedicated tree.

Studio also acts as the reference for many target UX features, including:

- Animate-style workflows
- Timeline interaction
- Layer management
- Docking systems
- Workspace management
- Theming and customization

The long-term direction is convergence toward a single integrated editor experience.

## Upstream sync must accelerate the port

Flare continues tracking upstream OpenToonz and Tahoma2D.

The sync system:

```text
tools/sync/upstream_agent.py
```

must do more than replay upstream commits.

When an upstream change affects code that has already been ported to Rust, the sync process should automatically generate a draft pull request that attempts to map the upstream change onto the Rust implementation of the affected subsystem.

The purpose is not to fully automate porting.

The purpose is to provide maintainers with:

- Change detection.
- Candidate Rust-side modifications.
- Affected-file mapping.
- Draft implementation suggestions.
- Context for manual review.

This creates a head start whenever upstream functionality changes.

The ideal workflow is:

1. Upstream modifies a C++ file.
2. Sync detects the corresponding Rust replacement.
3. Sync analyses the change.
4. Sync proposes a Rust-side equivalent.
5. Sync opens a draft PR for developer review.
6. Developers finalize the port manually.

The upstream sync therefore becomes a migration assistant rather than merely a source mirror.

### Rebrand protection

Two rule sets remain critical:

- Shared rebrand rules rewrite includes and branding.
- CMake-specific rebrand rules rewrite build-system identifiers.

These rules are enforced by tests because broken rebranding can render every supported platform unbuildable.

`tools/sync/tests/test_upstream_agent.py` pins these transformations to prevent regressions.

## Licensing rules for absorbed work

Absorbing functionality is not the same as copying source verbatim.

Flare remains BSD-3-Clause.

### Permissive licences

MIT, BSD, and Apache licensed projects may be incorporated, provided attribution and licence requirements are respected.

AnimationCraft falls into this category and may be used as a source for redistributed subsystem implementations.

### Copyleft licences

GPL, LGPL, and AGPL code may not simply be copied into BSD-licensed Flare components.

Projects such as NFlare may be referenced for behaviour, architecture, workflows, and implementation guidance, but licence obligations must always be respected.

### Missing licences

Projects lacking an explicit licence grant should be treated as unavailable for direct inclusion until permission is clarified.

### Origin tracking

Every absorbed source must be recorded in:

```text
ORIGIN.md
```

including upstream repository and commit information.

## Keeping the documentation honest

`LOGIC-ATLAS.md` remains the authoritative architecture map.

`scripts/atlas_check.py` validates:

- Paths
- Symbols
- Constants
- Structural references

against the actual source tree.

Whenever a subsystem changes:

1. The code changes.
2. The atlas changes.
3. Validation passes.

Documentation should never silently drift away from implementation reality.

## What "done" means for a module

A module is considered fully ported when:

1. A Rust implementation exists under:

   ```text
   common/<subsystem>/rust/<name>.rs
   ```

2. The module is registered in:

   ```text
   flare_formats.rs
   ```

3. The production code path uses the Rust implementation.
4. The old C++ implementation is removed or retired.
5. Behaviour is covered by Rust tests.
6. Historical edge cases are covered.
7. `LOGIC-ATLAS.md` is updated.
8. `python scripts/atlas_check.py` succeeds.
9. Upstream-sync mapping exists for the subsystem where applicable.

A compiled-but-unused Rust implementation is not ported.

A Rust implementation running alongside an active C++ equivalent is not ported.

A module is only finished when the Rust version becomes the authoritative implementation.

## Reference

| Source | Role |
|----------|----------|
| AnimationCraft (`KMSDax`) | Primary Rust-native animation, document, rendering and timeline functionality source |
| Next2Flash (`SSF2-Mods-Official`) | TypeScript tooling and Flash workflow reference |
| NFlare | Architectural and behavioural reference |
| OpenToonz / Tahoma2D | Upstream structure, module layout, and subsystem architecture |

In short, Flare's end state is a unified Rust implementation of the OpenToonz/Tahoma2D architecture, augmented by functionality derived from AnimationCraft, Next2Flash, and NFlare, while preserving upstream compatibility, subsystem structure, and long-term maintainability.


## Placement correction (supersedes the `rust/` subfolder rule above)

Ported files now live **beside** the C++ they replace (e.g. `tnzbase/permissionsmanager.rs`), with no `rust/` subfolder. Existing `common/*/rust/` files migrate to their parent dir as they are touched. C++ files are deleted once ported; headers stay as thin C-ABI forwarders only while C++ callers remain.

### Ported so far (branch `Conversion-ft-animationCraft`)
- `tnzbase/permissionsmanager.rs` (cpp deleted; header forwards via C ABI). Tests: `cargo test --lib permissions`.
