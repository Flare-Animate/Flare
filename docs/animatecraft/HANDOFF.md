# Flare: handoff and how to continue

Last updated **Oct 15 2026**, at commit `e2a6aa3` (P2 group 1 done). Use this to pick up the
work where it stopped. Read it together with:

| File | What it holds |
|---|---|
| `CLAUDE.md` | Project brief: feature map (✅/🟡/⬜), architecture, roadmap, rules |
| `docs/ANIMATE-GAP-ANALYSIS.md` | Feature-by-feature comparison with Animate, with statuses and priorities. **This is the to-do list.** |
| `log/devlog.md` | What was done in each task, in order, with the tests and checks used |
| `CONTRIBUTING.md` | Clean-room policy (must read before your first change) |
| `docs/FLARE-REVIEW-PHASE5.md`, `docs/OPENTOONZ-REVIEW.md` | Reviews of the sources we port from |
| `SCRIPTING.md` | Scripting and command reference |

---

## 1. Where things stand

- **Repo:** `github.com/Flare-Animate/RustConversion-AnimateCraft`, branch `main`.
- **Tests:** 250 passing (`cargo test --workspace`), clippy clean.
- **Windows installer:** `sh tools/build-windows.sh` builds `Flare-<version>-setup.exe`. The last
  one was built from `e2a6aa3`.
- **Phases 1–4** (usable for real projects, animation depth, Merge Drawing engine,
  interchange/publish) are done, along with their gap-closing passes.
- **Phase 5** (workspace polish) is under way.
- **P1 gaps** are all done.
- **P2 gaps** are being worked through in **groups of five** (section 3). Group 1 is done;
  **Group 2 is next**.

### What has been done, in order
(Details for each step are in `log/devlog.md`; commit hashes are in `git log`.)

1. **Phases 1–4:** model, tweens (classic, shape, motion), Motion Editor, Merge Drawing, every
   tool, filters and blend modes, masks and guides, sounds and lip-sync, .fla/XFL read and write,
   HTML5 Canvas/SVG/GIF/video/Lottie/sprite-sheet export, the scripting engines and the JSFL shim,
   and the `.acx` package.
2. **Flare merge and the OpenToonz ports:**
   - Flare's core and Studio behaviour were merged in.
   - Ported from OpenToonz: stroke styles, deformation tools, drawing assistants, expressions,
     MyPaint brushes, IK, Asset Warp, XDTS, particles, and centreline trace.
   - EffectCraft effects come in as a git dependency.
3. **Rename to Flare.** Also added: the startup window, tool icons, and fixes for real .fla files.
4. **Studio-style workspace:**
   - Movable docks (`dock.rs`): grips, drop targets, Workspace menu, Reset Layout.
   - Flare logo at the top left (opens About).
   - Edit bar with zoom presets, rulers and clip toggles.
   - Swatches column, Navigator, command search (Ctrl+Shift+P), Tool Options bar.
5. **Gaps closed using our referenced sources:**
   - Gradient focal points.
   - .fla art brushes.
   - Per-document and gradient swatches; .aco/.ase/.grd through PhotoCraft's `photocraft-psd`.
   - History panel.
   - Convert Lines to Fills / Expand Fill / Soften Fill Edges.
   - Test Movie.
6. **P1 gaps:**
   - Alt-drag copy.
   - Registration point on Convert to Symbol.
   - Onion Keyframes Only and Next/Previous Keyframe.
   - Document Settings (units, anchor, scale content, scale spans, Match Contents).
   - Preferences.
   - Library folders, linkage and item Properties.
   - Color panel.
   - Text paragraphs (wrap, justify, spacing, indent, margins, super/subscript).
   - Dynamic and Input text in the editor, .fla and HTML5.
7. **P2 group 1 (timeline):**
   - Loop range (Ctrl+Alt+L).
   - Frames/seconds display.
   - Frame widths and Ctrl+wheel timeline zoom.
   - Synchronize Symbols.
   - Duplicate Symbol and swap.

---

## 2. How we work (the step process)

Each task, and each **group of five** P2 items, goes through these steps:

1. **Pick the items** from the gap analysis and say which five are in the group before starting.
2. **Find the right place in the code.** Use `rg` or Grep. The core logic goes in `ac-core`
   (no UI, pure functions where possible). The UI goes in `ac-app`. File-format work goes in
   `ac-xfl` and `ac-render`.
3. **Implement.**
   - Follow the existing patterns: every document change goes through `Editor`, as **one undo
     step** (`checkpoint`).
   - Add a command-search entry in `ac-app/src/palette.rs` (`COMMANDS`).
   - Add menu items and shortcuts in `main.rs`, in the same style as their neighbours.
4. **Tests:**
   - Add an integration test file in `crates/<crate>/tests/` (e.g. `ac-core/tests/p2_timeline.rs`).
   - For pure UI helpers, use a `#[cfg(test)] mod tests` in that file.
   - Run:
     ```sh
     CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --workspace
     cargo clippy --workspace --all-targets      # must be warning-free
     ```
   - To test against your own .fla files, add `AC_FLA_CORPUS=<folder of .fla files>` to the test
     command. Only use files you made or have permission to use.
5. **Check it in the app** (section 5). Take screenshots and look at them. For HTML5 changes,
   open the published page in Chromium (Playwright).
6. **Update the docs:**
   - Add a devlog entry to `log/devlog.md` (what changed, design choices, test count, what was
     checked in the app).
   - Update the rows in `docs/ANIMATE-GAP-ANALYSIS.md`, and the summary table if coverage changed.
   - Update the feature map in `CLAUDE.md`.
   - Update this file when the plan changes.
7. **Commit** with a message describing the group.
8. **Report** to the project lead before starting the next group. **Push only after they
   confirm.** Build an installer when they ask for one.

The groups are kept to five so each one gets checked properly. Don't merge groups together.

---

## 3. P2 plan: groups of five

| # | Group | Items | Status |
|---|---|---|---|
| 1 | Timeline | Loop range, time display (frames/seconds), frame widths / timeline zoom, Synchronize Symbols, Duplicate Symbol and swap | ✅ `e2a6aa3` |
| 2 | Symbols & motion | Copy/Paste Motion, motion presets, Cache as Bitmap / Export as Bitmap / Visible flag, 9-slice scaling, layer effects (filters on layers) | **next** |
| 3 | Drawing & rigging | Hairline strokes + stroke scaling (normal/horizontal/vertical/none), Auto Keyframe toggle, 3D global/local toggle, Asset Warp mesh density option, bone springiness/damping | |
| 4 | Commands & panels | Code Snippets panel, saved Commands (Run Command / Manage Saved Commands), History replay / Save as Command, Find and Replace, Publish profiles | |
| 5 | Documents & media | Multiple open documents (tabs), rich text runs, bitmap compression settings, .ai import, motion objects in .fla export (instead of baking frame by frame) | |
| last | Needs the egui upgrade | Named workspaces + floating panels/tab groups, keyboard shortcut editor, tablet pressure/tilt | |

### Notes for group 2
- **Copy/Paste Motion:**
  - Copy a classic or motion tween's per-frame transforms, or its `Keyframe.motion` and ease
    curve, from a frame selection.
  - Paste it onto another selected instance as a new tween.
  - The frame clipboard code is in `frames.rs::copy_frames` and `Editor::copy_frames`.
  - "Copy Motion as JS" can produce our scripting API calls (see `SCRIPTING.md`).
- **Motion presets:**
  - A panel with built-in presets of our own design (bounce, fly-in, fade, pulse, etc.). These are
    stored as JSON motion data.
  - Users can save their own from a motion tween.
  - Don't copy Adobe's preset files (clean-room rule).
- **Cache as Bitmap / Export as Bitmap / Visible:**
  - These are new fields on `Element` (`model.rs`), with `serde(default)`.
  - The renderer (`ac-render`) skips invisible instances.
  - Cache as Bitmap can just be a flag that is honoured on export.
  - Read and write them in XFL (`ac-xfl` read.rs/write.rs; DOMSymbolInstance attributes
    `cacheAsBitmap`, `isVisible`, `exportAsBitmap`).
- **9-slice scaling:**
  - A `Symbol` field for the grid rectangle.
  - The edit-symbol stage draws dashed guides that you can drag.
  - When rendering a scaled instance, shapes are split into nine regions; only the middle
    stretches, the corners keep their size.
  - XFL has `scaleGridLeft/Right/Top/Bottom` on DOMSymbolItem.
- **Layer effects:**
  - A `Vec<Filter>` on `Layer`, applied to the layer's composited content in `ac-render`, in the
    same way as element filters.
  - Edit them from the Layer Properties dialog.

---

## 4. Open gaps

### Gaps to close (bigger items, recorded separately)
1. **Floating panels and tab groups:**
   - Any panel can float, dock on any side, or join another panel as a tab. Panels collapse to
     icons. Named workspaces can be saved and switched.
   - This needs **egui 0.29 → 0.36** first.
   - PhotoCraft's `panel_docking.rs` (egui 0.36, MIT/Apache) is the reference. Our grips and
     drop targets in `dock.rs` become the drag source for its tree.
2. **Pattern brushes** (.fla `PatternBrush` definitions; currently drawn as a solid line with a
   note). Also: drawing with art brushes in the editor, a brush library panel, and writing paint
   brushes back to .fla.
3. **Animate `.clr` colour sets.** This is an undocumented binary format; we have no spec or
   sample to work from.

### Remaining P2 (outside the groups above)
- Shared/external libraries and the Assets panel.
- Layer depth (2.5D with camera); camera attach-layers.
- Fluid Brush pressure.
- Paint Brush art/pattern modes (tied to Gaps to close #2).
- Bone translation constraints.
- Asset Warp: tween the warped poses of bitmaps/symbols.
- PSD editable text/vector options.
- The rest of the CreateJS display API in HTML5.
- Output/Compiler Errors panels.

### P3 (low priority; see the gap analysis for the full list)
- **Animation and drawing:** custom sound envelopes, onion skin colour settings, the Update
  library item command, linear-RGB gradients, named width profiles.
- **Test Movie, preferences and scripting:** a player window for Test Movie and Test Scene,
  smart-guide preferences, stroke-style parameters and hinting, Sync/Code Editor preferences,
  the rest of JSFL, a visual frame picker.
- **Text and fonts:** web fonts, vertical text, spell check.
- **Interface and project:** UI themes, localisation, accessibility panel, Movie Explorer,
  components, templates.
- **Import and export:** SWF import/export, video layers, legacy binary .fla, WebGL/VR.

### Known small issues
- Escape doesn't close Document Settings (Cancel does).
- The colour panel's hex field was widened in code and hasn't been checked on screen.
- Synchronize Symbols adds an (empty) undo step when nothing needed changing.
- The playhead tag in the timeline header can overlap the frame numbers at the narrowest widths.

---

## 5. Environment and checks

```sh
# build and run
cargo run -p ac-app                                   # the editor ("flare")
cargo run -p ac-cli -- --help                         # headless exports
sh tools/build-windows.sh                             # Windows exes + NSIS installer (MinGW + NSIS)

# UI check under Xvfb (Linux)
Xvfb :99 -screen 0 1600x900x24 &
cargo build --release -p ac-app
sh tools/restart.sh                                   # starts target/release/flare on :99, settings in out/cfg
python3 tools/ui_drive.py click:337:655 shiftclick:447:655 combo:ctrl+alt+l shot:check
#   ops: click rclick dclick shiftclick drag path mv down up key type combo hold release
#        scroll:x:y:n (n<0 = up)  sleep:secs  shot:name (→ out/name.png)
#   Ctrl+wheel: hold:ctrl scroll:600:680:-3 release:ctrl
AC_SCREENSHOT=out.png cargo run -p ac-app            # one-shot screenshot without driving
```

The window opens at 1280 × 800 on the 1600 × 900 screen.

### Timeline coordinates at default settings
- Frame cells start at x ≈ 238, 11 px per frame.
- Layer rows start at y ≈ 607, 24 px each.

### Gotchas we've hit
- **Shortcut order.** egui's `consume_key` also matches when extra modifiers are held. Register
  the longer combination first: Ctrl+Alt+L before Ctrl+L, Ctrl+Alt+B before Ctrl+B.
- **Colour conversion.** `Color32::to_array` gives premultiplied values. Use
  `to_srgba_unmultiplied` when converting to document colours.
- **Missing glyphs.** Some glyphs (▾, ⠿, 📏, ✂) render as boxes in the bundled fonts. Use words
  or icons we draw ourselves.
- **Disk and build time.** Large debug builds use a lot of disk. Use
  `CARGO_PROFILE_DEV_DEBUG=0` (or `line-tables-only`).
- **Dependencies.** EffectCraft and `photocraft-psd` are **git dependencies**, not copied code.
  Ported code names its source in the file header, in `THIRD-PARTY-NOTICES.md` and in the devlog.
- **Look and naming.** Behaviour can follow Animate; the look must not copy Adobe's UI. Never use
  "Adobe" or "Animate" in the product name.
