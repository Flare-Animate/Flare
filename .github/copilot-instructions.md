# Instructions for GitHub Copilot (and other coding agents)

This repository's working rules live in [AGENTS.md](../AGENTS.md), which is maintained and
authoritative; read it first, and treat what it says as what applies. What follows is the
short version, for when the rules are needed mid-task.

## What this repository is

Flare is an Adobe Animate (and Toonz-lineage) replacement written in Rust. The architecture,
the crates and their layers, and the document model are described in
[docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md); the toolchain, conventions and verification
steps in [docs/ENGINEERING.md](../docs/ENGINEERING.md).

## Rules that surprise people the most

- The `original/` directory is an archive of the old C++ code: never depend on it, link it,
  or copy from it. `third_party/` is pinned upstream code: do not edit it.
- Every Rust file stays under 500 lines; tests live in their own files.
- No `unwrap`, `expect` or panics on user data; typed errors in libraries, `anyhow` only in
  binaries. Everything that grows is bounded.
- Behaviour gets a test, including its failure paths, and a test must be able to fail.
- Docs change in the same commit as the behaviour they describe.
- Small conventional commits (`feat(scope): ...`, `fix`, `docs`, `test`), pushed to `main`.
- Never create or push a `v*` tag, or publish a release, without the maintainer's say-so.

## Verifying

Before pushing, all of these pass, and failures are reported as they are:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

A feature is not finished until it works on every surface it can (desktop, browser, command
line, MCP server); gaps are recorded in [docs/SURFACES.md](../docs/SURFACES.md).
