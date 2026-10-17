# Sprint 1 (2026-10-15): unify AC+MM, port batch 4, JS->TS

Goal: advance the Rust rewrite on Conversion-ft-animationCraft with every unit independently reviewed before push.

## Backlog (owner tier)
| # | Item | Tier | State |
|---|---|---|---|
| 1 | Batch 3 ports hsvutil, igs_perlin_noise | sonnet | done; reviewed APPROVE; golden tests added |
| 2 | minnamate wave 2 import (cd2afd9) | sonnet | done; reviewed CHANGES -> 3 security findings fixed (path traversal, decompression bombs, audio alloc) |
| 3 | cpp2rust draft workflow | sonnet | reviewed OK (no injection); not runnable until on default branch |
| 4 | Unification plan, step 0 + 3 (mm_bridge) | sonnet | done; reviewed CHANGES -> silent-loss/dup-symbol findings fixed |
| 5 | html5_runtime.js -> TypeScript | sonnet | done; reviewed OK; CI drift check added |
| 6 | Batch 4 C++ ports (metnum, predict3d, CIL, CallCircle) | sonnet | done; reviewed APPROVE; lows fixed |
| 7 | macOS smoke-test abort (SIGABRT, exit 134) | haiku triage, sonnet fix | cause: host LC_RPATH -> 2 copies of libomp/vtk/ffmpeg/opencv (OMP Error #15). Fix: strip non-@ rpaths from all Mach-O + dyld image check; reviewed APPROVE; VERIFIED green on macOS+Windows+Linux (98bd964, b9e7fae) |
| 8 | Merge plan step 2 (single html5 runtime) | sonnet | STOPPED by design: contracts incompatible, see UNIFICATION_PLAN.md s5; needs mm->AC_MOVIE lowering |
| 9 | jsfl.js -> TypeScript | haiku | done; reviewed APPROVE; CI drift check added |

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

## Verification before push
flare_formats 254, minnamate-core all suites green, html5 runtime --check up to date. CI on the pushed head is the remaining gate (macOS smoke abort still open).

## Close
Branch renamed Conversion-ft-animationCraft -> Rust-Rewrite (PR #122 closed by the rename). Lesson: build* in .gitignore hid new build scripts; reviewers must read from git, not the working tree. Loop stopped at user request; next sprint awaits user instructions.
