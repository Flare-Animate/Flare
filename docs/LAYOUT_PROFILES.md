# Layout profiles

A layout profile (a "rooms" directory) decides **where panels sit and which menus you see**. It is
independent of the colour theme (`stuff/config/qss/<Theme>/`); any theme works with any profile
(`scripts/check_theme_layout_separation.py` keeps the two namespaces apart).

Pick one in Preferences > Interface > Rooms. Profiles live in `stuff/profiles/layouts/rooms/<Name>/`.

## The rule every profile obeys

> A profile may hide features, it may never remove them.

Every profile exposes **all** UI options: each room's menubar contains every command the app
registers. What a profile's philosophy would leave out of its hand-made menus is listed under the last
menu, **All Commands**, grouped by category (Files and Project, Tool Modes, Keyframes, Columns, ...).
Every panel type has its `MI_Open*` command, so every panel is reachable from a menu, and every
shortcut preset defines every shortcut key. `scripts/check_layout_profiles.py` enforces this (CI,
no Qt needed) and must print `RESULT: OK` with 100.0 for every profile.

## Profiles

| Profile | Rooms | For | Origin |
|---|---|---|---|
| **Animate** (default) | 8: Draw, Animate, Timing, Rigging, Compose, Cleanup, Sound, Library | Everyday work. Few panels per room, OpenToonz depth one click away. | Designed deliberately, see below. |
| **AnimateLegacy** | 4: Drawing, Animation, Rigging, Compositing | The previous default, kept untouched. | The profile that was called `Default`, then `Animate`, before the redesign (see history). |
| **OpenToonz** | 9: Drawing, Timeline, Cleanup, Palette, InknPaint, Composite, Xsheet, Tasks, Browser | The classic OpenToonz workspace, unchanged. | Byte-identical to upstream `stuff/profiles/layouts/rooms/Default/` (rooms and menubar). |
| **FlipaClip** | 1: FlipaClip | One-room, frame-by-frame sketch/animation in the style of FlipaClip. | Shipped as is. |
| **StudioGhibli** | 6: Cleanup, PltEdit, InknPaint, Xsheet, Batches, Browser | Task-per-room production workflow with a menubar per room. | Upstream OpenToonz `StudioGhibli`, with the quoting fix below. |

"Preserved" profiles (AnimateLegacy, OpenToonz, FlipaClip, StudioGhibli) are locked by
`tests/layout_profile_manifest.json`: `tests/test_layout_profiles.py` fails if their room files or
hand-made menus change. The only additions are the generated **All Commands** menu and, for
StudioGhibli, removal of stray quote characters around command ids in `room5/6_menubar.xml`
(`"MI_RunScript"` never resolved, so those commands were silently missing).

### How to restore a profile

* Never edited it: nothing to do, it is the file on disk.
* Your personal copy diverged: Windows > Workspace > **Reset Room Layout** deletes
  `<profile folder>/layouts/personal/<Profile>.<user>/` for the **current** profile, so the stock
  rooms and menubars are copied again at the next start. Other profiles' personal copies are untouched.
* To get back the old default: choose **AnimateLegacy**. To get the classic OpenToonz UI: choose
  **OpenToonz** (and Shortcuts > preset **OpenToonz** for the classic keys).
* From git: `git checkout <commit> -- stuff/profiles/layouts/rooms/<Profile>`.

Your selected profile is stored in the preference `CurrentRoomChoice` and is never changed by an
update; if that profile no longer exists the loader falls back to `Animate`. Because user copies
override the stock files, **existing users keep their old Animate rooms until they use Reset Room
Layout once** (or pick another profile and back after deleting the personal folder).

## Design of the Animate profile

Goal (from the product owner): *keep the depth of OpenToonz with the simplicity of the Rust UI*,
deliberately, not as an accidental merge.

**Simplicity is taken from the Rust editor** (`flare/ac_app/`): one stage, a tool strip on the left
(`toolbar.rs`: sections of slots, groups behind flyouts), a properties column on the right
(`panels.rs`, `dock.rs` module `Properties`), the timeline along the bottom (`timeline.rs`), the
library as a secondary panel (`dock.rs` module `Library`). Rules that follow:

1. **Stage first.** In the drawing rooms (Draw, Animate, Rigging, Cleanup) the stage takes more than half the width; Timing, Compose and Library trade stage width for the panel they exist for.
2. **At most 7 panels per room**, usually 5-6. OpenToonz rooms have 8-9.
3. **Same skeleton in every room**: command bar on top, tools left, stage centre, properties right,
   timeline bottom. A room changes one slot, it does not rearrange the others.
4. **Secondary panels are one click away, not on screen**: Xsheet, Schematic, FX settings, function
   editor, browser live in their own rooms or in Windows menu, not in Draw.
5. **Menus are short and task based**: File, Edit, Draw, Timeline, Scene, View, Tools, Windows, Help
   (9 menus; OpenToonz has 11), each with at most 18 direct entries (OpenToonz: up to 35); the rest is
   in submenus.

**Depth is taken from OpenToonz**: nothing was dropped.

* The nine OpenToonz workflows exist as rooms: Draw (Drawing / InknPaint / Palette), Animate and
  Timing (Timeline / Xsheet), Cleanup, Rigging and Compose (Composite), Library (Browser), plus Sound.
* Every one of the 467 registered commands and 32 tools is in a menu: the hand-made menus hold
  270 of them (including all panels, tools, import/export, rigging, cleanup, lip sync), and
  **All Commands** holds the rest by category.
* Tool modes (fill/erase/tape/geometric sub-modes) are shown by the Tool Options strip when a tool is
  active, so there is no tool-options clutter in Draw; the same commands are under All Commands > Tool Modes.
* The command bar editor (Windows > Workspace) can put any command on the top bar; presets in
  `stuff/profiles/layouts/settings/presets/commandbar_*.xml` can be loaded with *Import Layout...*.

### Room by room

Geometry is for a 1920x1020 work area; the dock redistributes on other sizes. `hierarchy`: first
`-1` = nothing maximised, then `1` = the root stacks top-to-bottom, orientation alternates per
nesting level (`flareqt/docklayout.cpp`). `scripts/check_layout_profiles.py` verifies geometry and
hierarchy agree.

| # | Room | Panels | Arrangement | Why |
|---|---|---|---|---|
| 1 | **Draw** | CommandBar, ToolOptions, ToolBar, SceneViewer, StyleEditor, LevelPalette, Timeline | top bars; tools 40 px left; stage 1536 px; style editor over palette on the right (336 px); timeline 216 px bottom | Drawing and ink and paint. The Rust default layout plus the colour panels a drawing artist needs. `-1 1 [ 0 1 [ 2 3 [ 4 5 ] ] 6 ]` |
| 2 | **Animate** | CommandBar, ToolOptions, ToolBar, SceneViewer, FlashInstancePanel, Timeline | stage 1536 px; instance properties right; tall timeline (276 px) | Flash/Animate-style tweening: pick a symbol instance, edit its properties, work on the timeline. `-1 1 [ 0 1 [ 2 3 4 ] 5 ]` |
| 3 | **Timing** | CommandBar, ComboViewer, Xsheet, FunctionEditor | viewer 760 / xsheet 520 / graph editor 632 | The OpenToonz timing workflow: dope sheet and curves next to a preview. `-1 1 [ 0 [ 1 2 3 ] ]` |
| 4 | **Rigging** | CommandBar, ToolOptions, ToolBar, SceneViewer, Schematic, Timeline | stage 1056, schematic 816 | Plastic/skeleton/IK tools with the stage schematic beside them. `-1 1 [ 0 1 [ 2 3 4 ] 5 ]` |
| 5 | **Compose** | CommandBar, SceneViewer, Schematic, FxSettings, Timeline | stage 820, FX schematic 760, FX settings 332 | Compositing: nodes, effect parameters, preview. `-1 1 [ 0 [ 1 2 3 ] 4 ]` |
| 6 | **Cleanup** | CommandBar, ToolBar, CleanupSettings, SceneViewer, Xsheet | settings 376, viewer 1076, xsheet 416 | Scan and cleanup, as in OpenToonz room 3. `-1 1 [ 0 [ 1 2 3 4 ] ]` |
| 7 | **Sound** | CommandBar, ComboViewer, Browser, Timeline | viewer 1200, file browser 716, timeline 280 | Sound and lip sync: audio files from the browser onto the timeline's sound columns. `-1 1 [ 0 [ 1 2 ] 3 ]` |
| 8 | **Library** | CommandBar, Browser, SceneCast, StudioPalette | three columns | Assets: files, scene cast, shared palettes. `-1 1 [ 0 [ 1 2 3 ] ]` |

All eight rooms share one menubar (`menubar_template.xml`); there are no per-room menubars. Panels
not in any room (Tasks, BatchServers, History, Locator, FilmStrip, ScriptConsole, StopMotion, Guided
drawing, ...) open as floating panels from **Windows**.

### Not verified here

The profile was built and checked without a GUI. Collapsing of secondary panels relies on the
existing dock separators; there is no hide/show state in the room files. The integrator should open each
room once (see the checklist in the commit message / PR).

## History (what the git archaeology found)

* The `rooms/` tree enters history in one root commit, `b0d94e1` (2026-10-10), already containing
  `Default` (4 custom rooms), `OpenToonz` (9 rooms), `FlipaClip`, `StudioGhibli`. No earlier layout
  history exists; upstream OpenToonz (`stuff/profiles/layouts/rooms/Default`, 9 rooms) is the only
  pristine reference.
* `Default` was renamed `Animate` between `b0d94e1` and `f0459e2` (git shows a pure rename, 0 content change).
* `OpenToonz` = upstream `Default`: room files identical; the menubar had six upstream commands removed
  (`MI_OpenAlignmentPanel`, `MI_OpenBrushPresetPanel`, `MI_OpenToolPropertiesPanel`, `MI_Quicklink`,
  `MI_ClearViewerContent`, `MI_NewMetaLevel`) and Flash/Discord/Website entries added (`88cd3bd`,
  `345446d`). The profile is now the pristine upstream menubar; the Flare additions are reachable
  under All Commands.
* The old `Animate` was the merge: OpenToonz's menubar flattened and trimmed (about 170 of 467
  commands), OpenToonz's Drawing room plus three hand-written rooms using Rust-UI-style positions.
  Those three rooms' pane geometry contradicted their hierarchy strings (for example the timeline
  is docked beside the viewer in the hierarchy but placed below it in the geometry), so the dock
  had to guess. It is preserved as `AnimateLegacy` exactly, and the checker reports the mismatch
  as a warning.
* `FlipaClip` has the same trimmed menubar and the same kind of geometry mismatch (kept as is).

## Adding or changing a profile

1. Copy a directory in `rooms/` to a new name (not the name of a theme).
2. Edit rooms (see LAYOUT_CUSTOMIZATION.md for the file format) and the menubar.
3. `python scripts/check_layout_profiles.py --sync-all-commands` regenerates the All Commands menu in
   every menubar; `python scripts/check_layout_profiles.py` must print `RESULT: OK`.
4. After adding a command or panel to the C++ code, run step 3 again: the check fails until every
   profile lists it.
