<!-- desloppify-begin -->
<!-- desloppify-skill-version: 2 -->
---
name: desloppify
description: >
  Codebase health scanner and technical debt tracker. Use when the user asks
  about code quality, technical debt, dead code, large files, god classes,
  duplicate functions, code smells, naming issues, import cycles, or coupling
  problems. Also use when asked for a health score, what to fix next, or to
  create a cleanup plan. Supports 28 languages.
allowed-tools: Bash(desloppify *)
---

# Desloppify

## 1. Your Job

Improve code quality by maximising the **strict score** honestly.

**The main thing you do is run `desloppify next`** — it tells you exactly what to fix and how. Fix it, resolve it, run `next` again. Keep going.

Follow the scan output's **INSTRUCTIONS FOR AGENTS** — don't substitute your own analysis.

## 2. The Workflow

Two loops. The **outer loop** rescans periodically to measure progress.
The **inner loop** is where you spend most of your time: fixing issues one by one.

### Outer loop — scan and check

```bash
desloppify scan --path .       # analyse the codebase
desloppify status              # check scores — are we at target?
```
If not at target, work the inner loop. Rescan periodically — especially after clearing a cluster or batch of related fixes. Issues cascade-resolve and new ones may surface.

### Inner loop — fix issues

Repeat until the queue is clear:

```
1. desloppify next              ← tells you exactly what to fix next
2. Fix the issue in code
3. Resolve it (next shows you the exact command including required attestation)
```

Score may temporarily drop after fixes — cascade effects are normal, keep going.
If `next` suggests an auto-fixer, run `desloppify fix <fixer> --dry-run` to preview, then apply.

**To be strategic**, use `plan` to shape what `next` gives you:
```bash
desloppify plan                        # see the full ordered queue
desloppify plan move <pat> top         # reorder — what unblocks the most?
desloppify plan cluster create <name>  # group related issues to batch-fix
desloppify plan focus <cluster>        # scope next to one cluster
desloppify plan defer <pat>            # push low-value items aside
desloppify plan skip <pat>             # hide from next
desloppify plan done <pat>             # mark complete
desloppify plan reopen <pat>           # reopen
```

### Subjective reviews

The scan will prompt you when a subjective review is needed — just follow its instructions.
If you need to trigger one manually:
```bash
desloppify review --run-batches --runner codex --parallel --scan-after-import
```

### Other useful commands

```bash
desloppify next --count 5                         # top 5 priorities
desloppify next --cluster <name>                  # drill into a cluster
desloppify show <pattern>                         # filter by file/detector/ID
desloppify show --status open                     # all open findings
desloppify plan skip --permanent "<id>" --note "reason" # accept debt (lowers strict score)
desloppify scan --path . --reset-subjective       # reset subjective baseline to 0
```

## 3. Reference

### How scoring works

Overall score = **40% mechanical** + **60% subjective**.

- **Mechanical (40%)**: auto-detected issues — duplication, dead code, smells, unused imports, security. Fixed by changing code and rescanning.
- **Subjective (60%)**: design quality review — naming, error handling, abstractions, clarity. Starts at **0%** until reviewed. The scan will prompt you when a review is needed.
- **Strict score** is the north star: wontfix items count as open. The gap between overall and strict is your wontfix debt.
- **Score types**: overall (lenient), strict (wontfix counts), objective (mechanical only), verified (confirmed fixes only).

### Subjective reviews in detail

- **Preferred**: `desloppify review --run-batches --runner codex --parallel --scan-after-import` — does everything in one command.
- **Manual path**: `desloppify review --prepare` → review per dimension → `desloppify review --import file.json`.
- Import first, fix after — import creates tracked state entries for correlation.
- Integrity: reviewers score from evidence only. Scores hitting exact targets trigger auto-reset.
- Even moderate scores (60-80) dramatically improve overall health.
- Stale dimensions auto-surface in `next` — just follow the queue.

### Key concepts

- **Tiers**: T1 auto-fix → T2 quick manual → T3 judgment call → T4 major refactor.
- **Auto-clusters**: related findings are auto-grouped in `next`. Drill in with `next --cluster <name>`.
- **Zones**: production/script (scored), test/config/generated/vendor (not scored). Fix with `zone set`.
- **Wontfix cost**: widens the lenient↔strict gap. Challenge past decisions when the gap grows.
- Score can temporarily drop after fixes (cascade effects are normal).

## 4. Escalate Tool Issues Upstream

When desloppify itself appears wrong or inconsistent:

1. Capture a minimal repro (`command`, `path`, `expected`, `actual`).
2. Open a GitHub issue in `peteromallet/desloppify`.
3. If you can fix it safely, open a PR linked to that issue.
4. If unsure whether it is tool bug vs user workflow, issue first, PR second.

## Prerequisite

`command -v desloppify >/dev/null 2>&1 && echo "desloppify: installed" || echo "NOT INSTALLED — run: pip install --upgrade git+https://github.com/peteromallet/desloppify.git"`

<!-- desloppify-end -->

## VS Code Copilot Overlay

VS Code Copilot supports native subagents via `.github/agents/` definitions.
Use them for context-isolated subjective reviews.

### Subjective review

1. **Preferred**: `desloppify review --run-batches --runner codex --parallel --scan-after-import`.
2. **Copilot/cloud path**: `desloppify review --external-start --external-runner claude` → use generated prompt/template → run printed `--external-submit` command.
3. **Manual path**: define a reviewer agent, split dimensions, merge, import.

For the manual path, define a reviewer in `.github/agents/desloppify-reviewer.md`:

```yaml
---
name: desloppify-reviewer
tools: ['read', 'search']
---
You are a code quality reviewer. You will be given a codebase path, a set of
dimensions to score, and what each dimension means. Read the code, score each
dimension 0-100 from evidence only, and return JSON in the required format.
Do not anchor to target thresholds. When evidence is mixed, score lower and
explain uncertainty.
```

And an orchestrator in `.github/agents/desloppify-review-orchestrator.md`:

```yaml
---
name: desloppify-review-orchestrator
tools: ['agent', 'read', 'search']
agents: ['desloppify-reviewer']
---
```

Split dimensions across `desloppify-reviewer` calls (Copilot runs them concurrently), merge assessments (average overlaps) and findings, then import.

### Review integrity

1. Do not use prior chat context, score history, or target-threshold anchoring while scoring.
2. Score from evidence only; when mixed, score lower and explain uncertainty.
3. Return JSON matching the format in the base skill doc. For `--external-submit`, include `session` from the generated template.
4. `findings` MUST match `query.system_prompt` exactly. Use `"findings": []` when no defects found.
5. Import is fail-closed: invalid findings abort unless `--allow-partial` is passed.

<!-- desloppify-overlay: copilot -->
<!-- desloppify-end -->

---

## Flare Project — Copilot Agent Instructions

### Repository layout

```
flare/sources/flare/          – main app sources (menus, UI, command handlers)
flare/sources/flare_legacy/   – the shipped executable (extends flare/ with legacy glue)
flare/sources/common/flash/   – XFLReader.h/.cpp, all platform-shared Flash parsing code
flare/sources/tnzcore/        – core engine library (compiled as shared lib / DLL)
flare/sources/include/        – public headers (use "flare/" prefix when including)
stuff/profiles/layouts/rooms/ – room preset directories (each needs menubar_template.xml)
.github/workflows/            – CI/CD workflows for Linux, macOS, Windows
```

### Build system quick facts

- **Primary executable**: `flare_legacy` target (CMake).  Files in `flare/sources/flare_legacy/CMakeLists.txt`.
- **Shared engine**: `tnzcore` library.  `XFLReader.cpp` lives there — **do NOT add it to `flare_legacy` SOURCES** (would cause duplicate symbol LNK2005/multiple definition errors).
- **Include paths for flare_legacy**: headers in `sources/flare/` use bare names (`#include "menubarcommandids.h"`); headers in `sources/include/flare/` use the `flare/` prefix (`#include "flare/txsheet.h"`).
- **MOC_HEADERS**: every `.h` with `Q_OBJECT` must be listed in `MOC_HEADERS` in its owning `CMakeLists.txt` or the link will fail with missing `staticMetaObject`/vtable symbols.

### Flash / FLA support conventions

- `.fla` files are ZIP archives containing an XFL directory (`DOMDocument.xml` + `LIBRARY/`).
- `XFL::Reader` (in `XFLReader.cpp`) parses `DOMDocument.xml` into an `XFL::Document` struct.
- Import entry point: `ImportFlashVectorCommand::execute()` in `flare_legacy/flashimport.cpp`.
- Export entry point: `FlashExportCommand::execute()` in `flare_legacy/flashexport.cpp`.
- Flash guide dialog: `FlashGuideCommand::execute()` in `flare/flashguidedialog.cpp`.
- **Never add duplicate command registrations** — each `MI_*` ID must appear in exactly one `MenuItemHandler` subclass across both files.
- TNZ format (`.tnz`) is Flare's native project format and must be preserved alongside FLA support.

### Command IDs

New menu commands need to be declared in **both**:
1. `flare/sources/flare/menubarcommandids.h`
2. `flare/sources/flare_legacy/menubarcommandids.h`

And wired into the menu in:
- `flare/sources/flare_legacy/menubar.cpp`
- `flare/sources/flare_legacy/mainwindow.cpp`
- `stuff/profiles/layouts/rooms/AdobeAnimate/menubar_template.xml`
- `stuff/profiles/layouts/rooms/Default/menubar_template.xml`

### CI workflow notes

- Linux/macOS build: `ninja -j2` inside `flare/build/` (limited to avoid memory exhaustion)
- Windows build: `cmake --build build --config RelWithDebInfo`
- macOS: must `brew unlink qtbase qtdeclarative qt` before installing `qt@5` (pre-installed Qt 6 conflicts)
- `upload-artifact` version must be `@v4` across all workflows (no v5/v6)
- Workflow runs from bot commits may show `action_required` — this needs manual approval from a maintainer

### PR checklist

Before pushing a fix:
1. Check that no symbol is defined in both `flashguidedialog.cpp` **and** `flashexport.cpp`.
2. Check that `XFLReader.cpp` is NOT in `flare_legacy/CMakeLists.txt`.
3. All headers with `Q_OBJECT` are in `MOC_HEADERS`.
4. `stuff/profiles/layouts/rooms/*/menubar_template.xml` contains any new menu items.
