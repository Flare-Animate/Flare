# Open-issue map

Every open issue at the time of writing, what in this tree answers it, and what
still does not. Generated claims live in `tests/check_issue_fixes.py`, which
probes the tree for each row below and prints "addressed / not addressed" —
so this table can be re-verified with one command rather than trusted:

```
python tests/check_issue_fixes.py
```

A row only says "addressed" when the probe found the code. Where the answer is
"not addressed", the reason is stated rather than left blank.

## Answered by code (9 of 13)

| Issue | Title | What answers it | Where |
|---|---|---|---|
| #11 | Custom toolbar | Draggable dock panes, persisted as room `.ini` files, so the toolbar is user-customisable and a profile can be submitted by PR | `flare/sources/flareqt/docklayout.cpp`, `flare/sources/flare/mainwindow.h`, `doc/LAYOUT_CUSTOMIZATION.md` |
| #15 | Migrate features from Tahoma | Moho project import (layers, groups, switch layers, bones, keyframe tracks) | `flare/sources/common/moho/MohoReader.h`, `flare/sources/common/moho/moho.rs` |
| #25 | Pencil tool -> CSP vector brush | Vector/schematic brush node and its rasterisation | `flare/sources/flareqt/schematicnode.cpp` |
| #47 | Opening FLA files does not work + Animate room layout | FLA (zip) / XFL readers, DOMDocument + LIBRARY parsing, plus the `Animate` room profile | `flare/sources/flare/flashimport.cpp`, `flare/sources/common/flash/XFLReader.cpp`, `stuff/profiles/layouts/rooms/Animate/` |
| #49 | OpenToonz rigging -> Harmony rigging | Moho bone and switch-layer structures mapped onto xsheet columns | `flare/sources/common/moho/MohoReader.h` (`struct Bone`, `struct Switch`), `flare/sources/common/moho/MohoPlan.h` |
| #50 | Logo rebrand | Flare icon and app branding | `flare/sources/flare/icons/Flare.png` |
| #52 | JSFL / ZXP / MXP integration | ZXP/MXP/ZXP-aware dispatch in the import command | `flare/sources/flare/flashimport.cpp` |
| #56 | Linux build kernel panic | Documented Linux build path with dependencies | `doc/how_to_build_linux.md` |
| #66 | App is incomplete | The shipped reader set, including the ISO-BMFF and legacy OLE2 FLA paths | `flare/sources/common/flash/SWFAssets.h` |

## Startup / import crashes (Oct 2026 pass)

Statuses are what could be established from code reading plus unit tests. None
of these has been confirmed by the reporter; "needs repro" says what is missing.

| Issue | Status | Root cause found | Fix | Still needs |
|---|---|---|---|---|
| #112 AppImage `FLAREROOT` error | fixed (unit-tested, not user-confirmed) | `TEnv` on Linux read the root only from `~/.config/Flare/SystemVar.ini`; the `FLAREROOT` environment variable was never consulted. | Discovery moved to Rust (`common/tapptools/tenv.rs`, `resolve_stuff_dir`, called from `tenv.cpp` via `flare_tenv_resolve_stuff_dir`): command line, env (relative paths resolved against cwd), portable dirs, macOS bundle Resources, `SystemVar.ini`, `$APPDIR` and installed `share/flare/stuff` layouts, missing dir falls through with a trace instead of aborting. | A run of a fresh AppImage. |
| #114, #116 AppImage SIGABRT on start | mitigated | `EnvGlobals` is first constructed before `QApplication` exists, so the "portablestuff next to the executable" check (added for #112) silently never ran and the AppImage (which ships `usr/bin/portablestuff`, no `share/flare/stuff`) ended with no stuff dir; an exception then escaped `main()` and hit `std::terminate`. The `libtnzcore +0xc4358` frame is the throw site; no symbols, so the exact throw is not identified. | Stuff dir is re-resolved after `QApplication` exists (also in `initUserStuffDir`); `main()` is wrapped (`flareMain`) so any escaping exception becomes a message plus a log entry; `AppRun` fixed (launched the nonexistent `Tahoma2D`); uninitialised `m_dllRelativeDir` pointer in `EnvGlobals` fixed (undefined behaviour). A reporter said the later nightly starts. | Confirmation on a post-fix AppImage; if it still fails, `~/.local/share/Flare/Flare/startup.log` now says where. |
| #110 Windows 11 startup crash | mitigated, needs repro | WinDbg shows an unhandled C++ exception on the main thread after the style editor pages are built, only when `portablestuff` is present. The throwing call is not identifiable from the log. The `AA_UseDesktopOpenGL` line is a warning, not the crash. | Same top-level handler and `startup.log` (cwd, env, plugin paths, stuff dir trace, GL result); every Rust FFI entry point is wrapped in `catch_unwind`. | A `startup.log` or a symbolised stack from a build with PDBs. |
| #117 FLA/SWF import crash (Windows) | mitigated, needs repro | The report says Flare's own project files crash at ~1% too, so this is probably not format specific. Real defects found in the readers: odd CFBF directory-name length indexed past a chunk (panic), unbounded zlib/deflate inflation, `u32` overflow in frame spans, unbounded `setCell` loop on a corrupt span. | Fixed all of those (`framecount.rs`, `xflrender.rs`, `ac_xfl.rs`, `flashimport.cpp`, `XFLReader.cpp`); import command wrapped in try/catch; Rust FFI panics can no longer abort. Fuzz-ish tests (truncated and mutated SWF/CWS/FLA zip/XFL/CFBF built in-test) in `flare_formats.rs` and `common/flash/ac_xfl_fuzz_tests.rs`. | The reporter's FLA, or `%TEMP%\flare_import.log` plus `startup.log`; a non-import crash (CPU, GPU driver) is not excluded. |
| #98 macOS missing FFmpeg dylib | fixed in CI script (not run on a Mac here) | A bundled `libavdevice` still referenced a versioned Homebrew Cellar path that only exists on the build machine. | `ci-scripts/osx/bundle-homebrew-dylibs.sh` rewritten: resolves via the install name, `LC_RPATH`, `brew --prefix <formula>`, `brew --prefix`/lib and the Cellar; rewrites to `@rpath`, adds an `@executable_path/../Frameworks` rpath, and fails the job if anything non-portable remains. | A nightly run on `macos-15-intel`, then `otool -L` on the bundle. |
| #71 macOS error, no logs | needs repro | The report has screenshots only. A prohibited-sign icon on an Apple Silicon Mac is what macOS shows for an app whose architecture it cannot run: the nightly is Intel-only, so Rosetta must be installed (`softwareupdate --install-rosetta`). The old nightly also copied `portablestuff` to the bundle root, breaking the code signature. | Signature problem fixed (see #66). The startup log now exists for launches that get as far as `main()`. | The screenshot text; whether Rosetta is installed; an arm64 build is not done. |
| #66 nightly macOS has no executable | mitigated | Bundle root contained an unsealed `portablestuff` copy (invalid signature -> "damaged"); no check that the executable exists. | `nightly.yml` places `portablestuff` in `Contents/MacOS`, sets `+x`, fails the build if the executable is missing, prints `lipo -info`. | A downloaded nightly on a Mac. |

## Not answered (4 of 13)

| Issue | Title | Why | What would close it |
|---|---|---|---|
| #51 | Title mix-up | Needs confirmation of which strings are wrong. Nothing in the tree identifies the intended title text, so guessing would risk changing correct strings. | A one-line statement of the expected window/file title, or a screenshot. |
| #53 | Studio pipeline readiness | A process and policy question, not a code change: versioning, review, release cadence, versioning of project files. | A decision from the maintainers on release cadence and compatibility policy; the code side is the `.flw`/version handling that already exists. |
| #55 | Finding more developers | Recruitment, not code. Related work that does help: `CONTRIBUTING.md` now labels and a beginner-friendly issue template (`docs/ISSUE_MAP.md` is generated from the same issue list). | Maintainer decision on where to advertise. |
| #71 | macOS error | The error text was never attached (screenshots only); probable cause is an Intel-only build without Rosetta, see the table above. Note this is distinct from the macOS **build** failures on Actions, which are fixed in CI. | Paste the full error output and the macOS version. |

## Closed since the map was written

Issues previously tracked here that are now closed: #60 (AppImage `FLAREROOT`
not booting), #65 (`FLAREROOT` unset on Puppy Linux), #67 (crash with unknown
cause), and the earlier #5/#6/#27/#28/#30/#45 PR branches, all merged or
archived under `archive/*` tags.

## How to keep this honest

`tests/check_issue_fixes.py` is the source for the "answered by code" column.
If an issue is fixed or a new one is opened, run the script and update this
table; it prints the count at the end so drift is visible in a review diff.