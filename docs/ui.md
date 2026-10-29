# Interface

The workspace follows the familiar animation-tool layout: a tool strip on the left, the stage in
the middle with a bar above it (scene, zoom, view toggles) and rulers along its edges, panels on
the right (Properties and Library; Color, Align, Transform and History), the timeline below, and a
status bar. Panels dock, tab, resize and can be rearranged; `View > Reset Workspace Layout`
restores the default.

## Look

`flare-ui/src/theme.rs` (shared widgets and the icon set are in `flare-ui`, the workspace in `flare-animate`) holds the one dark palette, spacing, rounding and type sizes; colours are
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

Still planned (everything else in the menus and panels is built):

- Library: folders.
- View: guides.
- Edit, Modify, Text, Commands and Help entries still listed dimmed in the menus (Window and the Help windows are real now: panels come to the front, About and a read-only shortcut list).

## Top bar

The window has no system title bar on macOS: the top bar (38 px) takes its place, with room at the
left for the window buttons, the app mark, the in-window menus where there is no native menu bar,
the document name in the middle, and play, undo and redo at the right. Empty space in the bar
moves the window and a double click zooms it; the workspace asks and the desktop app does it
(`WindowRequest`), so the UI crate stays free of window code.

## Type

Interface text is Source Sans 3 Regular (`assets/ui/fonts`, SIL Open Font License, licence text beside it), embedded in the program and placed ahead of egui's built-in fonts, which still cover the glyphs it lacks. On macOS the top bar is 28 px tall, the height of the title bar it replaces, so the window buttons the system draws sit centred in it.

## Default layout

Left to right: the tool strip, the stage, and one stack of tabs (Properties, Library, Color, Align, Transform, History). Underneath, spanning the window, are the Timeline and Output tabs. Output keeps every message the status bar showed, so a refusal that scrolled past can be read again. The Window menu brings any panel to the front. Any panel can be dragged to another stack or out on its own, and View > Reset Layout restores this arrangement.

## Documents and tabs

A window holds several documents as tabs above the stage. One is edited; the others are parked whole and swapped in when their tab is chosen, so each keeps its own history, selection and view. The person's tool, colors, settings and panel layout stay with the window, not the document. Open, Import and New open in a tab of their own, except that a blank untouched document gives way to what is opened. Revert reloads in the same tab. Closing a tab with unsaved work is refused, and closing the window asks about every tab. Autosave and crash recovery cover only the document on show for now.

## Outline colors and Outline Mode

Every layer has a color chip at the right edge of its label (the outline column): its own color if
it was given one, otherwise one of eight by position. Clicking the chip cycles the palette as one
undo step. The color is kept in `.flare` files and in XFL (`DOMLayer color`). View > Outline Mode
draws every shape as a one-pixel line in its layer's color with no fills and ignores masks, so
everything can be found; the evaluated frame carries each shape's layer color for this
(`DrawnShape::outline`). A mask layer itself is not drawn, so it has no outline yet.

## Still to match from Animate 2024 (Linux reference)

Properties with Tool, Object, Frame and Doc tabs and Publish Settings inside Doc; an Info panel
(size, position, and the color under the pointer); a Color panel with a fill-type dropdown, hex
field and HSB/RGB modes; a Library with a preview, search and a linkage column; a zoom dropdown
and a close button on the document tab. The default arrangement stays as it is.
