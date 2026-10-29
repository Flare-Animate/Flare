# Interface

The workspace follows the familiar animation-tool layout: a tool strip on the left, the stage in
the middle with a bar above it (scene, zoom, view toggles) and rulers along its edges, panels on
the right (Properties and Library; Color, Align, Transform and History), the timeline below, and a
status bar. Panels dock, tab, resize and can be rearranged; `View > Reset Workspace Layout`
restores the default.

## Look

`flare-ui/src/theme.rs` holds the one dark palette, spacing, rounding and type sizes; colours are
named there and used by name. `chrome.rs` holds the shared widgets (icon buttons, section headers,
label/control rows) so every panel looks alike.

## Icons

Icons are SVGs in [`assets/ui/icons`](../assets/ui/icons), drawn on a 24 x 24 grid with a 1.6 px
round stroke and a translucent fill for the body of a glyph. They are white on purpose: the app
tints them (dim, normal, hover, active, disabled), so one file serves every state and theme.
They are original drawings in a consistent duotone style, not copies of any other product's icons.

To add an icon, put an SVG in that folder. `build.rs` embeds every file, named after the file, and
the tests check that each one parses, draws something, is 24 x 24 and uses only white. A name used
in the code that has no file panics in debug builds and fails the tests.

## Planned features are visible

Controls for features that do not exist yet are drawn at full fidelity but dimmed, and say so on
hover ("Not available yet"). They are built with the `mock_*` helpers in `chrome.rs`, and menu
commands that are planned are `MenuEntry::Mock` entries in `MENU_LAYOUT`. Finding them is a search
for `mock_` and `Mock(`; building one replaces it with the real control.

Planned, in the order they will be built:

- Tools: Subselection, Free Transform, Lasso, PolyStar, Pencil, Brush, Paint Bucket, Ink Bottle,
  Eyedropper, Eraser, Hand, Zoom.
- Properties: editable position and size, stroke cap and join, text style, rectangle corner
  radius, frame rate.
- Library: search, new symbol, folders, symbol properties, delete, preview.
- Color: hex entry, gradient editor, eyedropper, no color, swap and default colors.
- Align panel: align, distribute, arrange, group.
- Transform panel: numeric scale, rotate and skew; break apart.
- History panel: the list of edits and jumping to one.
- Timeline: loop playback, onion-skin outlines, layer folders, motion guides, the outline column.
- Stage bar: grid, rulers toggle, snapping.
- Menus: the commands listed as dimmed entries.

## Top bar

The window has no system title bar on macOS: the top bar (38 px) takes its place, with room at the
left for the window buttons, the app mark, the in-window menus where there is no native menu bar,
the document name in the middle, and play, undo and redo at the right. Empty space in the bar
moves the window and a double click zooms it; the workspace asks and the desktop app does it
(`WindowRequest`), so the UI crate stays free of window code.
