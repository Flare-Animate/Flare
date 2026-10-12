# Flare Studio — native Rust workspace

Run from the Flare repository with `./scripts/run-studio.ps1`. The script selects
Vulkan, shared with the snapshot renderer, to avoid the observed Windows backend
startup/teardown faults. Override with `-Backend dx12` or `-Backend gl` when
testing another adapter. The executable is built under the Cargo workspace target
directory. Rust and the Windows MSVC build tools are required.

This is the native workspace, a separate executable alongside
the existing Qt application. PhotoCraft informed the immediate-mode Rust panel,
theme, and command organization. AnimationCraft's local ac-core (see `../core/ORIGIN.md`) and pinned ac-render
crates own the editable document, history, timeline evaluation, and rasterization.
The existing Flare SWF/XFL/Moho import pipeline is not connected to this executable.

## Working features

The module work list is [Flash CC gaps and implementation pairings](../../../../../docs/FLASH_CC_GAPS.md).

- Selection/move, rectangles, ellipses, hand pan, wheel zoom, anchored Zoom tool, zoom presets, fit stage.
- Timeline with pinned layer names, visibility/locking, adjustable frame widths, keyframe context menus, classic tween assignment and playback.
- One-frame blank documents; future-cell targeting, frame insertion/removal and keyframe clearing with undo and persistence.
- Contiguous and disjoint multi-layer frame selection, span-selection preference, distinct replace/overwrite paste, clear/reverse/bake and move/copy gestures. Entire ranges and destinations are checked for locks and limits before mutation.
- Layer/folder duplication, deletion, sibling reorder, drag into folders, indent/outdent, collapse/expand, bulk show/lock and Alt-click isolation; hierarchy and animation-parent references survive reorder.
- Shift/Ctrl layer selection, nonadjacent group moves, layer/folder copy/cut/paste across scene/symbol timelines and document replacement, with hierarchy remapping and transitive symbol/bitmap transfer.
- Masked/Guided layer properties with batch linking/unlinking and hierarchy remapping; Cells/content Preview/Preview in Context timeline display modes.
- Direct layer rename (double-click or F2), selected-layer visibility/lock, outline/color, type and persistent row-height properties, and flag-column drag painting with cancellation and one undo per edit.
- Frame labels and last-span duration fields, duration dragging with Escape cancellation, one undo transaction per committed gesture.
- Symbol conversion, searchable Library, symbol timeline editing, return to scene.
- Stage dimensions, frame rate, background, selected position/opacity/fill.
- Validated AnimationCraft JSON open/save with atomic replacement; PNG frame export.
- Unsaved-change prompts, undo/redo, English/French, live Light/Dark preferences.
- New Document presets with live aspect preview, resizable Properties/Library and Timeline/Output docks.
- Tool options strip, status bar, stage rulers, swatches, Navigator with click-to-fit, and localized command search (Ctrl+K).

The toolbar exposes the five implemented tools. Text, pen/brush tools, full dock
rearrangement, ActionScript, publishing, and Adobe file compatibility remain work
for later passes. A white empty stage is intentional. File → Open example animation
loads original demo artwork. Preferences are under Window or the top-right icon.

Shortcuts: Ctrl+N/O/S, Ctrl+Shift+S, Ctrl+Z/Shift+Z, Delete, F5/F6/F7,
Shift+F5 (remove frame), Shift+F6 (clear keyframe), Space; V/R/O/H/Z
select tools. Ctrl+K opens command search; Enter executes the first enabled result and Escape closes it. Shortcuts are suppressed while editing text or using a modal dialog.

Timeline: Shift-click/drag selects a rectangle; Ctrl-click toggles disjoint cells.
Double-click selects a span. With span selection enabled, click selects a span,
Shift-click adds spans, and Ctrl-click selects individual cells. Drag
an existing selection to move it, Alt-drag to copy it. Drag the end marker to set
the last span's duration; Ctrl-drag a static span edge to resize it. The numbered
ruler scrubs independently of selection. Frame/layer drags autoscroll at the edges.
Ctrl+Alt+C/X/V copy/cut/paste frames (Ctrl+C/X/V also work with frame-grid focus);
Alt+Left/Right moves the selected range by one frame. Arrow keys navigate and
Shift+arrows extend selection. Right-click cells or layer names for editing commands.
Paste Frames replaces the selected duration and shifts later frames. Paste and
Overwrite Frames writes exactly the copied duration without shifting later frames.
Disjoint copied pieces on one row retain their offsets and do not clear the gaps.

Layer headers: Shift selects a range; Ctrl toggles individual rows. Ctrl+C/X/V/A
copy/cut/paste/select layers while header selection is active. Paste inserts above
the active row or inside a selected folder. Required linked parents and masks are
included and counted in the clipboard status; cut removes only selected subtrees.
Both clipboards survive document replacement within the running application.
ID collisions are remapped, repeated identical assets are reused, and imported
assets undo with pasted content. They are not system clipboard/file formats.
Paste rejects missing dependencies, recursive symbol references, invalid containers,
locked destination folders and layer-limit overflow before changing the document.
Double-click a name or press F2 to rename; Enter/focus loss commits and Escape
cancels. Drag down the eye/lock columns to paint a value; Escape or release outside
the layer strip cancels. Folder drop edges insert siblings; the center inserts children. Properties applies
visibility/locking, outline/color, type and row height to selected rows.

Collapsed folder headers also support Copy/Cut Frames and Paste Frames. The
contained layers and their dependencies transfer through the frame clipboard.
Drag a selected layer/folder to the transport trash button to delete its subtree;
Escape cancels. Outline-column clicks toggle one/all layers; Alt-click toggles
other layers. PNG export ignores these authoring outlines.

Whole and partial copied tweens retain interpolation, including clips beginning
inside a span. Retained source intervals preserve easing, rotation turns, paths
and shape hints through repeated copy, splitting, duration changes and reversal.
Clearing compatible split keys rejoins their source intervals. Partial bake,
blank/clear/remove and paste keep the exterior curves in the tested workflows.
Inspector pose edits of interior/reversed intervals still materialize editable
frame poses with a localized notice; this does not complete curve authoring parity.
Copied, split and baked graphic instances retain their nested animation phase,
including after cross-document asset transfer, repeated copying and save/reopen.
Static and tween reversal reverse graphic playback, including the endpoint's
distinct clock; movie clips retain their global clock. Baking samples the original
curve before splitting the selected boundaries. Subsequent pose/property editing
without sampling, endpoint ownership after authoring edits, and broader Flash
conformance remain explicit P1 acceptance gaps.
See the phase register for the remaining acceptance work.

## Visual evidence

The target is the user's Flash Professional CC 2015 screenshots, build 15.0.0.173,
in both dark and light modes. The locally installed Essentials workspace XML
specifies a 282-unit Properties/Library dock, a 35-unit auxiliary strip, a narrow
right tools dock, and a 200-unit bottom Timeline/Output dock. These are preset
dimensions, not live screenshot measurements. Flare uses a 40-unit tools dock and auxiliary strip, a 282-unit inspector and
200-unit timeline. Tool options and the status bar are optional in Window;
rulers start hidden. These preferences affect only the workspace view.

The [archived Adobe CC 2015 manual](https://helpx.adobe.com/archive/kr/flash/cc/2015/flash_reference.pdf)
provides additional workspace behavior context (printed pages 146–154).
The Korean archive supplies the version baseline; the supplementary English
2015 archive (February 2016) informs the ongoing behavior checklist.
The screenshots and local XML establish the particular Essentials layout.
No Adobe binaries, artwork, logos, icons, or resource files are bundled in Flare.
This pass approximates the workspace; it is not a pixel-identical reproduction.

## Design system

PhotoCraft's `theme.rs`, `widgets.rs`, `icons.rs` and `panels.rs` informed the
semantic surface palette, measured tab strips, shared field styling, vector icon
weights and panel spacing. Flare retains the Flash-style workspace arrangement.
The user's font preference is Segoe UI: Windows loads its installed regular and
semibold faces at runtime. These proprietary font files are not redistributed.
Other platforms fall back to egui's bundled proportional fonts. JetBrains Mono is
reserved for dense numeric readouts. Ordinary labels, fields, tabs and inspector
section titles share 12-logical-pixel proportional text (15 device pixels at 125% scaling). Light mode has a darker Flare
wordmark. Both themes share component geometry and behavior.

`theme.rs` owns colors, typography and SVG icons; `widgets.rs` owns shared tabs,
fields, section headers and swatches; `chrome.rs` owns tool/status bars, zoom,
Navigator and command search; `timeline.rs` owns the timeline surface.
Bundled font/icon licenses and origins are listed in `assets/ATTRIBUTION.md`.
The [UI interaction rules](../../../../../docs/UI_INTERACTION_RULES.md) require
stable geometry on hover and focus. Use `widgets::selectable` for selection rows;
the reserved frame keeps inactive and hovered controls the same size.

The native Light/Dark chrome colors and bevel behavior are documented in the
[Ghidra appearance study](../../../../../docs/UI_MATERIALS.md). It records
appearance-role observations only; Flare paints its own gradients and edges.

## Translation contract

All interface copy belongs in `locales/en-US.ftl` and locale catalogs such as
`locales/fr.ftl`. Use stable message IDs, named parameters and Fluent plural
selectors. Do not construct sentences by joining translated fragments. Document,
layer, and symbol names are authored content and must not change with the language.

Add a locale to `Locale`, load its catalog in `Localizer`, and update locale
detection and number formatting. English is the fallback. Number-entry controls
accept comma decimals; French output uses comma decimals. Existing catalogs cover
Latin scripts; other scripts need suitable licensed font fallbacks and shaping
validation. The mirrored test locale is not a production RTL implementation.
Widget IDs, commands, serialized preferences, and document data remain independent
of translated labels. Expanded text and mirrored layout modes live in Preferences.

## Verification

From the Cargo workspace (`flare/sources`):

```powershell
cargo test --locked -p flare-studio --lib
cargo check --locked -p flare-studio --all-targets
$env:WGPU_BACKEND = 'vulkan'
cargo run --locked -p flare-studio --example studio_snapshot -- target/studio-light.png en 1440 920 blank light
```

Snapshot arguments: output, locale (`en`, `fr`, `expanded`, `rtl`), width, height,
content (`blank`, `demo`, `new`), theme (`dark`, `light`), optional hover message ID
(for example `preset-hd`, or `none`), optional frame display (`preview`, `context`,
or `cells`). It renders the production
UI offscreen. Tests cover catalog resolution, locale/theme independence, live
theme interaction, New Document interaction, menu hover tracking, localized command
search, zoom/ruler controls, history, save/reopen, and rendering. Geometry checks
cover intermediate hover frames, preset selection and keyboard focus, with both
themes, expanded translations and 125% scaling for the New Document dialog.

Snapshot comparison: `studio_snapshot output.png en 1920 1030 blank light none cells 1.25`.
Use `rulers` in place of `blank` to capture its selected state. The Windows
native application and snapshot helper both default to Vulkan;
`WGPU_BACKEND` explicitly overrides enumeration for that process.

Windows uses one combined menu/caption bar with native minimize, maximize/restore,
drag, double-click maximize and edge-resize commands. Close retains the save guard.
The initialized root is shown once to avoid eframe 0.36's hidden-first-render cycle.
Debug builds also run without a separate console window.

Striped grips move Properties/Library and tools to either side, and timeline
above/below the stage. Release on a highlighted drop target to commit; Escape or
an outside drop cancels. Positions persist in preferences; Reset Layout restores
defaults. Floating windows, arbitrary tab regrouping and docking trees remain P9.
See `docs/UI_COMPARISON.md` at the repository root for reproducible paired crops.
