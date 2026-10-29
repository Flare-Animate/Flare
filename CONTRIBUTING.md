# Contributing to Flare

Thanks for helping. Start with [docs/README.md](docs/README.md) for how things are built and
[AGENTS.md](AGENTS.md) for the rules we work by; they apply to people and tools alike.

## Setup

Rust is pinned in `rust-toolchain.toml`; `rustup` installs it on the first build. On Linux you need
`pkg-config libasound2-dev libfontconfig1-dev libwayland-dev libxkbcommon-dev libx11-dev libxcursor-dev libxi-dev
libxrandr-dev libxcb1-dev`.

```
cargo run -p flare-desktop      # the app
cargo run -p flare-cli -- --help
```

## Before you commit

All three pass, and failures are reported as they are:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

- Every behaviour has a test, including the failure paths. A test must be able to fail; never
  change an expected output to make a failure go away. See [docs/testing.md](docs/testing.md).
- No Rust file over 500 lines; put code in the crate and layer that owns the responsibility.
- Safe Rust, typed errors, no panics on user data, bounds on everything that grows.
- Change the docs in the same commit as the behaviour they describe.
- Do not copy from `original/` (the archived C++ code) and do not copy another product's artwork.

## Workflow

Work happens on `main`. Small commits with conventional messages
(`feat(scope): ...`, `fix`, `refactor`, `docs`, `chore`, `test`). Outside contributors open a pull
request against `main` from a fork; keep it focused on one thing.

## Reporting problems

Use the issue templates. For anything security related follow [SECURITY.md](SECURITY.md) and do not
open a public issue. Importing a file that comes out wrong? Use the "Import problem" template and
attach the file if you can.
