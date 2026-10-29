# Working on Flare

Flare is an Adobe Animate replacement written in Rust. Start with [docs/README.md](docs/README.md): it
says what each document covers. This file is only the rules for how to work here. How things
are built, and why, is in `docs/` and is kept up to date with the code.

## Read first

- [docs/architecture.md](docs/architecture.md): the crates, their layers, the document model.
- [docs/engineering.md](docs/engineering.md): toolchain, conventions, verification, reference code policy.
- [docs/performance.md](docs/performance.md): the rules every feature follows for responsiveness.
- [docs/ui.md](docs/ui.md): the interface, icons, type, and how unbuilt features are shown.
- [docs/roadmap.md](docs/roadmap.md): what is done and what is next.

## Structure

- Crates live in layers under `crates/` (foundation, editing, io, adapters, services, presentation);
  binaries live in `apps/`. Adapters integrate the Flash ecosystem (Ruffle, Flash file detection); file formats are io; text and sound are foundation; the operating
  system is `flare-platform`, so an app is only its window or its protocol. A crate depends only on layers above it. Put new code where its
  responsibility belongs; add a crate only when it has code and tests.
- `original/` is an archive of the old code. Never depend on it, link it or copy from it.
- The root `Cargo.toml` owns versions, paths and lints; members inherit them.
- Keep every Rust file under 500 lines. Split by responsibility into modules, and keep tests in
  their own files.

## Code

- Safe Rust, typed errors in libraries, `anyhow` only in binaries. No `unwrap`, `expect` or panics on
  user data (test modules may allow `expect`).
- All input is untrusted and everything that grows has a bound.
- Documents change only through validated command batches: a failed batch changes nothing, and
  every change is one undo step.
- Nothing slow runs on the UI thread. Follow docs/performance.md, measure before optimizing, and
  say what was measured.
- Comments explain why, not what. Match the surrounding style and naming.

## Verifying

Before every commit, all of these pass, and failures are reported as they are:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

- Every behaviour has a test, including the failure paths. A test must be able to fail.
- Do not drive the GUI by hand, take full-screen screenshots, or claim something works that was not
  exercised.
- Do not weaken a test or update an expected output to make a failure disappear.

## Interface work

- Icons are original SVGs in `assets/ui/icons`, white only, drawn in the house style. Add the file and
  use its name; the build embeds it. Do not copy another product's artwork.
- Features that are planned but not built are shown dimmed as mocks and replaced by the real thing
  later; see docs/ui.md.
- App icons are trimmed when packaging starts, not before.

## Git

- Small conventional commits (`feat(scope): ...`, `fix`, `refactor`, `docs`, `chore`, `test`).
- Work is a linear stack of branches (`stack/NN-topic`), each a draft PR against the one before it
  in the Flare-Animate/Flare repository. Bring a lower branch's changes forward by merging, not
  rebasing. Push only stack branches.
- Never commit generated or local files (`target/`, `docs/temp/`, tool caches).
- Change the docs in the same commit as the behaviour they describe. Committed docs never refer to
  `docs/temp`.
