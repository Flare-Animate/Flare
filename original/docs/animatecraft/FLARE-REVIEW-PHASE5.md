# Flare's own code vs AnimationCraft and OpenToonz, and the Phase 5 plan

> **Note (2026-10-14):** written when this code base was still called AnimationCraft. It has since
> become the base of Flare's Rust conversion and is named Flare; "AnimationCraft" below means this
> repository, "Flare" the C++ Flare app.

Reviewed 2026-10-14 at Flare `a27136a`. This covers only the code Flare wrote
itself. What Flare inherits from OpenToonz is in `docs/OPENTOONZ-REVIEW.md`.

## 1. Where Flare is now

Flare changed direction. Its active target is no longer the Qt/OpenToonz app.
It is a **native Rust "Studio"** (egui 0.36, ~14k lines) built on **a copy of
our `ac-core`**, taken at our commit `e3aadad` (Oct 11). It also pins our
`ac-render` at that commit. The Qt app stays as the Flash-import and OpenToonz
host. Their `docs/FLASH_CC_GAPS.md` targets Flash Professional CC 2015 and plans
the Studio by pairing modules with `AC/…` functions (ours), VectorCraft and
PhotoCraft.

That copy is now **11 of our commits behind**. It is before our Phase 4 work
(.fla/XFL read and write, sound, HTML5, .acx) and before all of today's
gap-closing. Their older `ANIMATIONCRAFT_ABSORPTION.md` ("absorb into OpenToonz
subsystems, don't import the model or renderer") no longer matches what they
build. FLASH_CC_GAPS says so itself.

| Part | Lines | What it is |
|---|---|---|
| `flare/rust/studio` | 14k Rust | egui app on our core: timeline/layer editing, docking grips, themes, en/fr Fluent i18n, AccessKit, egui_kittest UI tests |
| `flare/rust/core` | our ac-core + ~1.1k | Our core plus patches (§2) |
| `common/flash` | 18k C++ | XFL census and shape reader (1.2k), SWF shapes and assets (2.1k), legacy FLA bitmap carving, the old Macromedia SWF SDK writer (~14k, 1999) |
| `common/moho` | 1k | Moho project structure reader (no rendering) |
| `common/tgame` | 0.8k | ECS game scaffold, not wired to the UI |
| `tools/flash/next2flash` | Python | Optional AS3 decompile / patch / compile sidecar (Flex SDK) |

## 2. Their patches to our core

After normalising formatting, the patches touch only `model.rs`, `frames.rs` and
`editor.rs`, and add `tween_edit.rs`. The other modules are unchanged.

| Patch | What it does | Ours | Recommendation |
|---|---|---|---|
| **TweenBoundary** (`Keyframe.tween_boundary`, `tween_edit.rs`) | When a tween is split (F6 inside it, partial copy, reverse, retime), each piece keeps the *original* curve over a source-time interval, instead of a new tween from the sampled pose. Edits inside a piece are stored as sparse pose corrections. | We sample the pose at the split and make two tweens, as Animate does. Splitting an eased tween therefore changes the motion, which is Animate's behaviour too. | **Don't adopt.** It moves away from Animate's model and has no XFL representation, so it can't round-trip through .fla. Their own notes call it incomplete ("editable tween-boundary parity" open). It costs complexity everywhere: every element-removal path now has to remap pose edits. |
| **Graphic playback clock** (`playback_offset`, `playback_reversed`) | Inserting a keyframe in a span showing a Graphic symbol keeps the symbol's animation running instead of restarting it. Reverse Frames runs nested graphics backwards. | Our new keyframe restarts the graphic at its First Frame. | **Check Animate's behaviour, then fix our way.** If Animate continues the animation, set the instance's `first_frame` in the new keyframe (it maps to XFL `firstFrame`) rather than adding a field. Reverse-running nested graphics maps to our existing Play Reverse loop modes. |
| First-frame overflow (`u64` sum) | `first_frame + local` can overflow on imported files with huge offsets | Same code | **Adopt** (one line). |
| Layer row height (`Layer.row_height`, %) | Taller timeline rows | Missing | **Adopt** as Animate's Layer Properties ▸ Layer height (100/200/300%), with XFL mapping. |
| Library "Uses" counts bitmap fills; deleting a bitmap clears fills that use it | Still open here: `Editor::bitmap_usage` counts bitmap elements only, and deleting a bitmap leaves fills pointing at it | **Fix** (small, in 5.9). |
| Paste Frames vs **Paste and Overwrite Frames** | Two paste modes | One mode (replace) | **Adopt** (Animate has both). |
| Copy/Cut/Paste **Layers** (with folders, across scenes/symbols/documents, assets copied) | Studio + core | Duplicate Layer only | **Adopt** in Phase 5 with multiple documents. |
| Timeline Frame View **Cells / Preview / Preview in Context** | Thumbnails in spans | One size, no preview | **Adopt** (gap row "Frame view sizes, preview thumbnails"). |

Licence: their core copy keeps our MIT/Apache licence, and Studio is BSD-3, so the
ideas above are free to use. We'd write our own versions in any case, because
their code is shaped around TweenBoundary.

## 3. What they do differently

**Process.** Their gap document sets acceptance rules we could borrow. A feature is
"done" only when it works through menus, shortcuts and direct manipulation, with
one undo per transaction, cancel on Escape and focus loss, locked-layer
rejection, symbol context, and save/reopen. They test this with **egui_kittest**
(headless egui driven through AccessKit labels) in English, French, an
"expanded-text" pseudo-locale and an RTL preview, in both themes. Our UI checks
are screenshots under Xvfb plus manual inspection. Kittest would make many of
them automatic and repeatable.

**i18n.** Every UI string is a Fluent message (`locales/en-US.ftl`, `fr.ftl`).
Numbers parse comma decimals in French. Ours: English strings in code.

**Workspace.** Grips move Properties/Library and Tools to either side and the
Timeline above or below the stage. Changes are persisted, with Reset Layout.
There are no floating windows or tab regrouping yet. Light and dark themes,
custom window chrome, command search.

**Import breadth vs depth.** Flare reads many containers. It detects formats by
content (SWF FWS/CWS/ZWS, ZIP-XFL, OLE2, ISO-BMFF), salvages bitmaps from legacy
binary FLA streams, and extracts SWF bitmaps, sounds and shapes (as SVG). Their
XFL reader is a structure and shape reader, not a full document loader: their
own notes say Studio has no Flash import yet. We go deeper on fewer formats:
`ac-xfl` reads and writes full documents (tweens, filters, sounds, motion XML).
Worth taking as ideas: **content sniffing** on Open, and **SWF asset salvage**
via Ruffle's `swf` crate (MIT/Apache).

**Moho, game engine, AS3 sidecar, Android, P2P.** These are mostly scaffolds; Flare
marks them as such. None are on our roadmap.

## 4. What we must not take

- **`theme.rs` and `widgets.rs` (Studio):** their colours and tab and button
  materials come from a **Ghidra study of Flash Professional's own binary**. The
  code comments cite Ghidra symbol names; the study itself is gitignored. Ghidra is a
  disassembler/decompiler. CONTRIBUTING.md forbids decompiling or disassembling
  Adobe software and anything derived from it, so these two files and any
  colour values from them are off-limits. Our themes are designed from scratch.
- **`common/flash/XFLReader.cpp`, `SWFShape.cpp`, `SWFAssets.cpp`:** their
  comments cite JPEXS (GPL-3.0) as the model reference and an AGPL project's
  "approach". We don't port these. For SWF we use Ruffle's crate and the public
  spec.
- **The Macromedia SWF SDK** (`F*.cpp`, 1999): unclear licence; skip.

## 5. Phase 5: Workspace polish — plan

Phase 5 in the gap analysis lists docking/floating panels and workspaces,
preferences, a shortcut editor, the History panel, Swatches/Color panels, Find
and Replace, multiple documents, themes and accessibility. The better lip sync
was deferred here too. Proposed order, each step a usable slice:

| # | Slice | Contents | Notes |
|---|---|---|---|
| 5.0 | **egui upgrade + UI test harness** | eframe 0.29 → 0.36, AccessKit on, egui_kittest tests for the main workflows (draw, F6/F7, tween, convert to symbol, save/reopen); Xvfb stays for visual checks | Needed by everything below (egui_dock 0.21 needs egui 0.36). Expect API churn in ac-app. |
| 5.1 | **Docking and workspaces** | egui_dock (MIT): every panel is a tab; drag to dock, split, stack or float; collapse to icons; workspaces Essentials / Animator / Designer / Developer / Small Screen; New / Save / Reset Workspace; persisted | Replaces the fixed layout and the Library dock-left special case |
| 5.2 | **Preferences + themes** | Preferences dialog (General, Drawing, Text, Timeline (span-based selection), Script, Interface); Dark / Light / custom accent; UI scale | Our own palette |
| 5.3 | **Shortcut editor** | Every menu item and tool becomes a registered action with an id; shortcut sets (Animate defaults = built-in), conflict check, export as HTML | Also feeds command search and the History panel |
| 5.4 | **History panel** | Undo steps list, click to go back, replay selected steps, "Save as Command" / copy as script for steps that came through the command registry | Needs a command log next to the snapshot undo stack |
| 5.5 | **Color + Swatches panels** | Color panel (solid/linear/radial/bitmap, HSB/RGB/hex, alpha, flow); Swatches (document palette, load/save .gpl, .aco/.act from public specs, Web 216); optional linked swatches (OpenToonz idea) | |
| 5.6 | **Multiple documents** | Document tabs, each with its own Editor, undo, autosave/recovery; cross-document copy/paste of frames, layers and objects with symbols/bitmaps carried across | Copy/Paste Layers lands here |
| 5.7 | **Find and Replace** | Text, fonts, colours, symbols, sounds, bitmaps; scope: current scene / document | |
| 5.8 | **Accessibility + localisation** | Keyboard focus order, AccessKit names on custom widgets, Fluent string catalogue (English first), expanded-text and RTL pseudo-locales in tests | Flare's approach, our code |
| 5.9 | **Timeline polish (from this review)** | Frame view sizes + Preview / Preview in Context thumbnails, layer height %, Paste and Overwrite Frames, graphic clock continuity on F6 (after checking Animate), first-frame overflow fix, bitmap-fill usage count + delete | |
| 5.10 | **Better lip sync** | Optional Rhubarb Lip Sync (MIT, separate program) for phoneme-accurate mouth shapes; the FFT heuristic stays as the built-in fallback; same Mouth-shapes mapping | Deferred from Phase 4 |

Sizes, roughly: 5.0 and 5.1 are the large ones (1–2 weeks each); 5.6 is medium;
the rest are days each.

## 6. Status (14 Oct 2026, later the same day)

The direction changed after this review: AnimationCraft and Flare are being
folded into one Rust application, so Flare is ported as a whole (see the devlog),
except the two items in §4. Done so far:

- **Core merged**: TweenBoundary, pose corrections, playback clock, row height,
  bitmap-fill usage; Flare's 21 core tests pass on our engine. Animate's
  behaviour stays the default; Flare's is a switch (Keep Tween Curves When
  Splitting). .fla export flattens both.
- **Studio features**: Paste Frames / Paste and Overwrite, Copy/Cut/Paste Layers,
  layer height, Frame View previews.
- **OpenToonz list**: Trace Bitmap, Shift & Trace, XDTS export, PSD import,
  Rhubarb lip sync (5.10). Effects/particles (#12, #13) now come from EffectCraft.
- **Launcher**: icon, Windows exe without console, 64-bit installer, CI workflow.

Next, in order: 5.0 egui upgrade + egui_kittest tests (also unblocks Flare's
Studio tests and AccessKit), 5.1 docking, then the remaining OpenToonz ports
(stroke styles, vector deformation tools, drawing assistants/symmetry, curve
editor keyframe types, IK limits, MyPaint raster brushes, centerline tracing)
and Flare's own subsystems (content sniffing on Open, SWF asset salvage via
Ruffle's crate, Moho structure import).
