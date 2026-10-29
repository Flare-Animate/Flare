# Testing

What is tested, how, and where. The rules are in [engineering.md](engineering.md); this page is
the map.

| Kind | Where | What it covers |
| ---- | ----- | -------------- |
| Unit | `src/**/tests.rs` beside the code | one module's behaviour and its failure paths |
| Integration | `crates/*/tests/*.rs`, `apps/*/tests` | a crate through its public API; the CLI and MCP server as a user or agent sees them |
| Property | `crates/*/tests/properties.rs` (proptest) | invariants over random input: input smoothing, simplify/smooth/brush outlines, align and distribute, document commands and undo, evaluation, `.flare` round trips |
| Fixtures | `crates/io/flare-import/tests/fixtures`, `crates/adapters/flare-flash/tests/fixtures` | real and hand-made FLA/XFL and SWF/FLV files, truncated and stale-trailer variants |
| Structure | `scripts/tests/test_layers.py` | a crate depends only on its own layer or one below it (checked from `cargo metadata`) |
| Benchmarks | `crates/*/benches` (criterion) | the responsiveness budgets in [performance.md](performance.md); measured, not run in CI |

## Rules of thumb

- Hostile input gets a test: truncated, oversized, non-finite, wrong type. The answer is an error,
  never a panic and never a half-applied change.
- A failed command batch changes nothing; tests compare the document before and after.
- Property tests keep their `.proptest-regressions` files in git so a found failure stays a test.
- The GUI is not driven by hand in tests. Behaviour lives in the workspace and is tested without a
  window; what is left is a short manual checklist in [release.md](release.md).

## CI

`.github/workflows/ci.yml` runs on Ubuntu for every push and pull request: `cargo fmt --check`,
clippy with `-D warnings`, `cargo test --workspace --locked`, a lockfile check, a wasm32 check of
the crates the web build uses, and the doc rules (500-line limit, no references to the scratch directory).
