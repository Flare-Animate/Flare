# Decision: UI stack

Status: accepted as the starting stack; docking and input gates are open.

## Stack

egui 0.37 for widgets, egui-winit for input, egui-wgpu for painting, winit 0.30 for the
window, wgpu 30 for the GPU. The shell integrates winit and egui-wgpu directly (one event
loop, no framework wrapper). egui is a widget foundation, not a requirement to look stock;
the stage and timeline are custom-drawn.

Presentation logic lives in `flare-ui` with no window or GPU code, so it can be tested by
running real egui passes with synthetic input. `flare-desktop` only owns the window, the
surface, native dialogs and the native menu.

## Menus and shortcuts

Menu behaviour is defined once as `MenuAction` in `flare-ui`: whether an action is enabled,
its current label, and how to perform it. Every menu surface uses that, so behaviour cannot
drift. On macOS the menu bar is native (muda) and the in-window menu is hidden; elsewhere
the in-window menu is used. See `crates/presentation/flare-ui/src/actions.rs` and
`apps/flare-desktop/src/menu.rs`.

Shortcuts follow the familiar Animate profile: F5 insert frame, Shift+F5 remove frame, F6
insert keyframe, Shift+F6 clear keyframe, F7 insert blank keyframe, F8 convert to symbol,
plus the usual Cmd/Ctrl+Z, Shift+Z, S. Modifiers are read from the key event itself. Single
letter accelerators are avoided so typing into text fields is never hijacked.

## Open

- Docking: choose between `egui_tiles` and `egui_dock` after a prototype that tests keyboard
  use, persistence, multi-monitor and panel lifecycle. Pick one, not both.
- Pen pressure and tilt, IME, and accessibility need hardware evidence before this stack is
  considered final. AccessKit is not enabled yet.
