# Working on Flare

Flare is a familiar, free animation suite written in Rust. Start with [docs/README.md](docs/README.md): it
says what each document covers. This file is only the rules for how to work here. How things
are built, and why, is in `docs/` and is kept up to date with the code.

## Read first

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): the crates, their layers, the document model.
- [docs/ENGINEERING.md](docs/ENGINEERING.md): toolchain, conventions, verification, reference code policy.
- [docs/PERFORMANCE.md](docs/PERFORMANCE.md): the rules every feature follows for responsiveness.
- [docs/UI.md](docs/UI.md): the interface, icons, type, and how unbuilt features are shown.
- [docs/ROADMAP.md](docs/ROADMAP.md): what is done and what is next.
- `.codesight/`: a generated, uncommitted map of the code; refresh it with `npx codesight --wiki --watch`. A reading aid only; the code and docs win.

## Structure

- Crates live in layers under `crates/` (foundation, editing, io, adapters, services, presentation);
  binaries live in `apps/`. Adapters integrate the Flash ecosystem (Ruffle, Flash file detection); file formats are io; text and sound are foundation; the operating
  system is `flare-platform`, so an app is only its window or its protocol. A crate depends only on layers above it. Put new code where its
  responsibility belongs; add a crate only when it has code and tests.
- `original/` is an archive of the old code. Never depend on it, link it or copy from it.
- The root `Cargo.toml` owns versions, paths and lints; members inherit them.
- Markdown documents are named in capitals (`docs/ROADMAP.md`, `docs/decisions/UI-STACK.md`), as the ones in the
  root are. Exceptions: `docs/releases/v<version>.md`, which the release workflow looks up by tag, and the README
  translations, named by language code.
- Keep every Rust file under 500 lines. Split by responsibility into modules, and keep tests in
  their own files.

## Every surface

- A feature is not finished until it works in the desktop app, the browser build, `flare-cli` and
  `flare-mcp`, as far as that surface can have it at all (a command line has no stage to drag on,
  but it can create, set, export and inspect). The change that adds a feature adds it to each. Where
  one cannot yet, that surface says so: dimmed with the usual hint in the interface, a clear error
  naming the feature in the command line and the MCP server. Record the gap and its reason in
  [docs/SURFACES.md](docs/SURFACES.md); every row there is a debt, looked at before a release.
- Behaviour lives in the libraries, never in an app. An app is a window or a protocol over them, so
  parity is a few lines and not a second implementation.
- A surface is tested: either its own test drives it, or the library's test covers the behaviour and
  the surface's covers the wiring.

## Code

- Safe Rust, typed errors in libraries, `anyhow` only in binaries. No `unwrap`, `expect` or panics on
  user data (test modules may allow `expect`).
- All input is untrusted and everything that grows has a bound.
- Documents change only through validated command batches: a failed batch changes nothing, and
  every change is one undo step.
- Nothing slow runs on the UI thread. Follow docs/PERFORMANCE.md, measure before optimizing, and
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
  later; see docs/UI.md.
- App icons are trimmed when packaging starts, not before.

## Git

- Small conventional commits (`feat(scope): ...`, `fix`, `refactor`, `docs`, `chore`, `test`).
- Work is committed straight to `main` in the Flare-Animate/Flare repository; there are no stacked
  branches or PRs for the maintainers. Outside contributors use a fork and a pull request.
- Never create or push a version tag, or publish a release, without the maintainer's explicit
  permission: a `v*` tag builds and publishes.
- Never commit generated or local files (`target/`, `docs/temp/`, tool caches).
- Change the docs in the same commit as the behaviour they describe. Committed docs never refer to
  `docs/temp`.
- The README is translated into every language the interface has (`Language::ALL` in `flare-i18n`; a
  test checks it), so adding an interface language adds a README translation, and the other way round.
- `README.md` and its translations in `docs/translations/` change together: an edit to the English
  README is made in every translation in the same commit, with the same structure, links and
  commands. Command lines, file names and code stay as they are; only the prose is translated. If a
  translation cannot be done well, say so in the commit instead of leaving the file behind.
