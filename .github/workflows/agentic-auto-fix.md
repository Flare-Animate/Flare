# Agentic: Auto-fix Build

Purpose
- Run the same gate CI runs on a pull request that carries the `agentic-auto-fix` label;
  when it fails, attempt safe, incremental automated fixes, re-run the gate, and report
  in the pull request what was changed and why.

The gate
1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
3. `cargo test --workspace --locked`

Agent instructions (concise)
1. Read the gate log posted to the pull request (or the `agentic-auto-fix-gate-log`
   artifact) and name the first failing check.
2. Fix in this order, re-running the failed check after each fix and nothing more:
   a. Formatting: the failing files only, with `cargo fmt`.
   b. Lints: the smallest change that satisfies the lint without weakening the code.
      An `#[allow]` needs a comment saying why the lint is wrong here.
   c. Tests: fix the behaviour the test pins, not the test — a test is only changed
      when it pinned the wrong behaviour, and the reason goes in the commit message.
   d. If the fix moves behaviour, the docs that describe it move in the same commit
     (docs/ROADMAP.md, docs/SURFACES.md, the crate's docs/ file).
3. Commit in small conventional commits (`fix`, `test`, `refactor`, `docs`) and push to
   the pull request's branch; the gate runs again on the push.
4. If a failure needs a decision (a dependency change, a licence question, a redesign,
   anything touching third_party/), stop and write the question into the pull request
   instead of guessing.

Safety and constraints
- Minimal diffs; no new dependencies; no edits under `original/` (it is an archive) and
  none under `third_party/` (it is pinned upstream code).
- Never weaken a check to make it pass: no `unwrap` on user data, no removed test, no
  reworded assertion, no deleted clippy lint without the comment above.
- Every fix must be verifiable by the gate; "it should pass now" is not a report.

Outputs
- Pushed fix commits on the pull request branch, each with the check it satisfies in the
  message, or a comment asking the question that blocks a safe fix.

Notes for maintainers
- The workflow runs on the label; add `agentic-auto-fix` to a pull request to arm it,
  and remove it to disarm. Nothing runs on pull requests without it.
