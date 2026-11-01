# Decision: UI stack

Status: accepted as the starting stack; docking and input gates are open.

## Stack

egui 0.37 for widgets, egui-winit for input, egui-wgpu for painting, winit 0.30 for the
window, wgpu 30 for the GPU. The shell integrates winit and egui-wgpu directly (one event
loop, no framework wrapper). egui is a widget foundation, not a requirement to look stock;
the stage and timeline are custom-drawn.

Presentation logic lives in `flare-animate` (on the shared widgets of `flare-ui`) with no window or GPU code, so it can be tested by
running real egui passes with synthetic input. `flare-desktop` only owns the window, the
surface, native dialogs and the native menu.

## Menus and shortcuts

Menu behaviour is defined once as `MenuAction` in `flare-animate`: whether an action is enabled,
its current label, and how to perform it. Every menu surface uses that, so behaviour cannot
drift. On macOS the menu bar is native (muda) and the in-window menu is hidden; elsewhere
the in-window menu is used. See `crates/presentation/flare-animate/src/actions/` and
`apps/flare-desktop/src/menu.rs`.

Shortcuts follow the familiar Animate profile: F5 insert frame, Shift+F5 remove frame, F6
insert keyframe, Shift+F6 clear keyframe, F7 insert blank keyframe, F8 convert to symbol,
plus the usual Cmd/Ctrl+Z, Shift+Z, S. Modifiers are read from the key event itself. Single
letter accelerators are avoided so typing into text fields is never hijacked.

## Docking: egui_tiles

The workspace is a dock of four panels (Tools, Stage, Library, Timeline) built on
`egui_tiles` 0.18. Panels have stable identities, not translated titles. A saved layout is
untrusted: it is size-capped and must contain every panel exactly once, otherwise it is
refused and the default is used, so a stale file can never leave the app without a panel.
The layout is remembered in the per-user data directory on a confirmed exit, and View >
Reset Workspace Layout restores the default without touching the document.

Why `egui_tiles` over `egui_dock`: both build against egui 0.37 and both serialize layouts.
`egui_tiles` fit the model better (tabs and splits in one tree, a small trait to implement,
simple validation of the pane set). `egui_dock` has floating windows, which would matter
for multi-monitor use. This choice came from API fit and a working prototype with tests; it
has not had hands-on UX testing. Revisit it if keyboard navigation, multi-monitor use or
panel detaching turn out to matter more than the tree model.

## Open
- Pen pressure and tilt, IME, and accessibility need hardware evidence before this stack is
  considered final. AccessKit is not enabled yet.
