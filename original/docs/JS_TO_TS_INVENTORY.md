# First-party JavaScript inventory and TS port feasibility

Scope: `/home/user/Flare`, excluding `thirdparty/`, `target/` (incl. `flare/sources/target/tmp/*` generated copies), `node_modules/`, `.git/`. Read-only survey; no code changed.

## Table

| Path | Lines | Loaded by | Runtime | TS port feasible? |
|---|---|---|---|---|
| `flare/sources/common/tvrender/html5_runtime.js` | 857 | Rust `include_str!` in `ac_html5.rs:19`; emitted into exported HTML5 (inline `<script>` or `flare-runtime.js`) | Browser (exported HTML5 movie) | Yes. Plain IIFE, no imports. Highest value. |
| `flare/sources/common/tvrender/mm_html5_runtime.js` | 307 | Rust `include_str!` in `mm_html5.rs:22`, written out as `runtime.js` | Browser (minnamate HTML5 player) | Yes. Same pattern. Check whether it duplicates `html5_runtime.js` before porting. |
| `flare/sources/common/expressions/jsfl.js` | 319 | Rust `include_str!` in `ac_script.rs:292`; eval'd as a string prefix in `run_jsfl` | Flare's embedded script engine (not browser, not node) | Yes, but output must stay a classic script string. Medium value. |
| `flare/sources/common/tvrender/tests/runtime_stages.js` | 80 | Spawned by `html5_runtime_stages.rs` via `node` (skipped if node missing) | Node test | Optional. Node 24 can strip types from `.ts`. |
| `flare/sources/flare/Resources/init.js` | 1 (1 byte) | Qt resource `flare.qrc:694`; loader in `scriptconsolepanel.cpp:149-156` is commented out | Qt (dead) | No. Looks like dead code, a removal candidate. Not changed here. |
| `flare/sources/flare/examples/scripts/frame-script-loop.js` | 7 | User-facing example, pasted into Flare | Flare script engine | No. Keep as JS; it is shipped to end users as-is. |
| `flare/sources/flare/examples/scripts/grid.js` | 13 | User-facing example, run via Commands > Run Script | Flare script engine | No. Keep as JS. |
| `.claude/workflows/flare-feature-pass.js` | 24 | Workflow harness (ESM `export const meta`) | Harness, not browser or CI | Not worth it. Keep JS unless the harness accepts TS. |
| `tools/icon/extract-ico.mjs` | 143 | Manual run; referenced in `toonzqt/icon.rs:16` comment; `verify-ico.mjs` imports it | Node, manual | Yes. Use `.mts` with erasable syntax only, or tsc. |
| `tools/icon/reencode.mjs` | 55 | Manual verification only | Node, manual | Yes, low value. |
| `tools/icon/verify-ico.mjs` | 64 | Manual; imports `extract-ico.mjs` | Node, manual | Yes, low value. |
| `tools/themes/check-themes.mjs` | 126 | Manual (`node tools/themes/check-themes.mjs`); not in any workflow file | Node, manual | Yes, low value. |

Inline JS in first-party `.html`: none. No `<script>` appears in any repo `.html` (`stuff/doc/*.html` included). Generated pages embed the runtime in Rust string literals (`ac_html5.rs:817-827`), so they are not separate HTML files.

Qt-loaded JS: only the dead `init.js` resource above. No Qt WebEngine JS was found.

CI JS: none. `pages.yml` runs `tools/flash/webapp/build.ts` with node 24 (already TypeScript, no build step). `html5-import.ts` and `app.ts` in `tools/flash/` are existing TS precedents. `check-themes.mjs` and the icon tools are not wired into any workflow.

## Build and embedding options

- Option A (recommended): author `html5_runtime.ts` and commit the tsc output under the same name, e.g. `html5_runtime.js`. Compile with `tsc --strict --target ES2017 --module none --noEmitOnError`. `include_str!` stays unchanged, and `--module none` guarantees no `import`/`export` in the output. Add a CI step that runs tsc and `git diff --exit-code`, plus the existing `node --check` test in `ac_html5.rs:970`.
- Option B: a `build.rs` that runs tsc. Not recommended. It would make node a hard build dependency for the Rust crates.

## Recommendation

1. Port `html5_runtime.js` first, using Option A. It is the largest browser-side file and is shipped in every exported movie. Also verify `mm_html5_runtime.js` against it, since the two may share code.
2. Port `jsfl.js` second. It is a string prelude, so the committed output must remain a single script.
3. Leave the examples, the workflow file and `init.js` as they are. Delete `init.js` in a separate change if the dead Qt loader is confirmed unused.
4. Tools (`tools/icon`, `tools/themes`) are optional and low value.

Assumptions: I did not read the script engine behind `run_js`, so the runtime of `jsfl.js` is inferred from its use. I did not diff the two runtimes.
