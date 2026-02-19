---
name: "Agentic — Auto-fix Build"
on:
  pull_request:
    types: [opened, synchronize, reopened, labeled]
  workflow_dispatch: {}
permissions:
  contents: write
  pull-requests: write
  issues: write
concurrency:
  group: agentic-auto-fix
  cancel-in-progress: true
inputs:
  require_manual_approval:
    description: "If 'true', agent must create a PR and wait for human approval before pushing fixes"
    required: false
    default: "false"
---

# Agentic: Auto-fix Build

Purpose
- Run the full configure + build on PR updates; when build failures are detected, attempt safe, incremental automated fixes (CMake/source/workflow fixes), re-run the build, and open a follow-up PR or push fixes directly depending on policy.

Agent instructions (concise)
1. Check out the PR branch and run:
   - `cmake -S . -B build -G "%MSVCVERSION%" -A x64` (use repo defaults) and
   - `cmake --build build --config RelWithDebInfo --parallel 4`
2. If build succeeds: post a short comment on the PR with the pass summary and exit.
3. If build fails: analyze `build/build.log` and `build/cmake_configure.log` to identify the minimal fix. Try fixes in this order (each followed by configure+build):
   a. Detect and replace Git-LFS *pointer* placeholders (tiny files < 2KB) with either vcpkg/system libs or generate import `.lib` from available `.dll`/`.def`.
   b. Add CMake fallbacks for thirdparty libs (prefer vcpkg/system) and fix path detection logic.
   c. Repair missing Qt resources referenced by `.qrc` (add the file or remove the unused reference).
   d. Fix simple compile errors (missing includes, incorrect casts, typos) with minimal edits and unit/rebuild verification.
   e. If a fix requires a binary that cannot be auto-generated, create a GitHub Issue describing the needed binary and propose a human-approved replacement.
4. When a fix is applied and the build passes:
   - If `inputs.require_manual_approval == 'true'`: push the fix to a feature branch and open a PR back to the original branch, then post a PR comment asking reviewers to approve/merge.
   - Otherwise: commit directly to the PR branch (small source/CMake/workflow-only changes) and post a comment summarizing the change and test results.
5. For anything ambiguous or risky (large binary files, license concerns, unknown thirdparty), create an Issue and leave the PR unchanged.

Safety & constraints
- Never introduce stub implementations for missing libs — only real binaries or generated import-libs from present DLLs, or vcpkg/system fallbacks.
- Minimal permissions only: modify source, CMake, and workflow files; do not add large binary assets without explicit human approval.
- Always include tests (rebuild) after any change. Report full `cmake_configure.log` and `build.log` truncated in PR comments.

Outputs
- The agent should either: push a fix commit to the PR branch, open a follow-up PR, or open an Issue with detailed diagnostics and suggested fixes.

Notes for maintainers
- Compile this agentic workflow with `gh aw compile` to produce the runnable `*.lock.yml` before enabling automated runs.
