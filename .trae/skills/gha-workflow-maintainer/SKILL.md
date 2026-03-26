---
name: "gha-workflow-maintainer"
description: "Scrapes PR build logs and diagnoses GitHub Actions issues; invoke when CI fails or user asks to fix workflows or maintain CI health."
---

# GitHub Actions Workflow Maintainer

This skill continuously inspects GitHub Actions runs for a given PR or branch, scrapes build logs, diagnoses common CI failures, and proposes code changes to workflow files to stabilize builds across Linux, macOS, and Windows. It also configures log artifacts and PR comments so failures are visible and actionable.

## When to Invoke

- CI builds are failing or flaky on any platform.
- Logs/artifacts are missing in PRs or Actions runs.
- You want standardized checkout, caching, and reproducible builds.
- You need path-based triggers or concurrency to reduce redundant runs.
- You want an automated agent to summarize build diagnostics on PRs.

## Capabilities

- Scrape latest Actions run logs for a PR/branch via GitHub API.
- Detect missing artifacts/log uploads and add steps to ensure they exist.
- Standardize actions/checkout options (fetch-depth, submodules, credentials).
- Add concurrency groups to cancel in-progress duplicates.
- Add smoke tests for binary launch and dependency checks per platform.
- Create or update workflows for linting (actionlint) and CI health audits.
- Post summarized diagnostics to PR comments.

## Inputs

- PR URL or repo/branch reference.
- Desired platforms and build strategy (matrix, Ninja/MSVC/Xcode).

## Outputs

- Updated workflow files under .github/workflows/.
- Uploaded logs and smoke-test artifacts visible in Actions.
- PR comments with truncated logs and actionable notes.

## Usage

1. Provide a PR URL or branch to audit.
2. The skill will fetch recent runs, identify gaps, and propose workflow edits.
3. Review proposed patches; accept to apply.
4. On next push/PR event, new workflows will upload logs and post comments.
