# Sprint 1 (2026-10-15): unify AC+MM, port batch 4, JS->TS

Goal: advance the Rust rewrite on Conversion-ft-animationCraft with every unit independently reviewed before push.

## Backlog (owner tier)
| # | Item | Tier | State |
|---|---|---|---|
| 1 | Batch 3 ports hsvutil, igs_perlin_noise | sonnet | done, REVIEW PENDING |
| 2 | minnamate wave 2 import (cd2afd9) | sonnet | done, REVIEW PENDING |
| 3 | cpp2rust draft workflow | sonnet | written, not runnable (default-branch rule); review pending |
| 4 | Unification plan, step 0 + 3 (mm_bridge) | sonnet | in progress |
| 5 | html5_runtime.js -> TypeScript | sonnet | in progress |
| 6 | Batch 4 C++ ports | sonnet | in progress |
| 7 | macOS smoke-test abort (SIGABRT, exit 134) | haiku triage, sonnet fix | waiting for diagnostics run on a413c66 |
| 8 | Merge plan step 2 (single html5 runtime) | sonnet | blocked on 5 |

## Definition of Done (per item)
1. Builds and tests pass for touched crates (CI green on Linux, Windows, macOS where applicable).
2. Independent read-only reviewer (not the author) signs off; no open high findings.
3. Differential test vs original C++ for every ported file.
4. Committed, pushed once per CI cycle; no cancelled-run churn.
5. Docs/PORTING.md and this file updated.

## Contracts
- mm_* files stay out of ac crates' mod lists (see UNIFICATION_PLAN.md).
- html5_runtime.js path unchanged (include_str!); TS compiled output committed + drift check.

## Token budget (approx, this session)
Main thread large (long CI-notification churn). Agents: ~85-140k tokens each, 14 spawned. Next sprint: batch notification reads, cap agents at one build at a time (disk).

## Retro (one line)
CI notification noise and repeated pushes burned the most; hold pushes until a reviewed batch is ready.
