# Project Cairn Log

Reverse-chronological, newest first. Summary and pointer only; conclusions live in `cairn/<topic>.md` and `docs/`.

## 2026-10-15 · Sprint 1 started under strict workflow
- macOS exit 134 root cause: absolute LC_RPATH left in bundle (two libomp). Fix in ci-scripts/osx/bundle-homebrew-dylibs.sh. jsfl.ts added. Runtime merge deferred (UNIFICATION_PLAN s5).
- 2026-10-15 Sprint 1 batch pushed after independent reviews (batch3/4 APPROVE, wave2+bridge fixed). See docs/sprints/SPRINT-1.md.

- Cairn initialised (defaults: track, en, no graduation provider, start_fresh). AGENTS.md and CLAUDE.md untouched (project-owned).
- Sprint plan, DoD, budget and retro: `docs/sprints/SPRINT-1.md`.
- Lesson: `build*` in .gitignore hid scripts/build-jsfl.sh and flare/build.rs (CI exit 127). Check `git status --ignored` before push; reviewers must read from git, not working tree.
- Lesson: pushes cancel in-progress CI; batch agent work into one push per cycle.
- Lesson: independent review before push is mandatory (DoD), not optional.
