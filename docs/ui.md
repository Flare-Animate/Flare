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

Left to right: the tool strip, the stage, and one stack of tabs (Properties, Library, Color, Align, Transform, History, Info). Underneath, spanning the window, are the Timeline and Output tabs. Output keeps every message the status bar showed, so a refusal that scrolled past can be read again. The Window menu brings any panel to the front. Any panel can be dragged to another stack or out on its own, and View > Reset Layout restores this arrangement.

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

a Color panel with a fill-type dropdown, hex
field and HSB/RGB modes; a Library with a preview, search and a linkage column; a zoom dropdown
and a close button on the document tab. The default arrangement stays as it is.

## Guides

Drag from the top or left ruler to pull a guide onto the stage; drag a guide to move it, or back
onto a ruler to remove it. View > Hide Guides hides them and Clear Guides removes them all. With
Snap on, points placed by the pen and the shape tools snap to a guide within six screen pixels.
Guides belong to the view of one document tab and are not saved in the file (at most 200).
Points placed by the shape, pen, Ruler and Mirror tools are pulled onto a guide whether or not the
grid is on.

## Home

A plain launch opens on the Home page, laid out like Animate's: a sidebar with the app name and
version, Home (and Learn, dimmed: not built), and the buttons New File, Open and Import; in the main
area **Create new** (cards for Character Animation 1920 x 1080 at 24 fps, Social 1080 x 1080 at 30,
Portrait 1080 x 1920 at 30, Web 1280 x 720 at 24, Classic 550 x 400 at 24, each drawn with the
proportions of its stage), then **Recent** (up to twelve project cards, newest first, with a
Clear the list button). The presets are Flare's own, not Animate's. Choosing a card, New File,
Open or Import leaves Home; a new project over an untouched document replaces it instead of adding a tab.

Home is a view, not a document, and none of the document's controls show on it: the top bar has
no undo, redo, play or document name, there is no status bar, and the window is titled just
"Flare" (a document's title is its saved name, or Untitled, with `*` when unsaved). Keys and commands
that act on the document are off while it is up. The house and "Flare" at the left of the top bar
and Window > Home toggle it; choosing a tab, even the current one, goes to that tab's document. A
project named on the command line or recovered after a crash opens straight to its document.

## Spacing

Docked panels (everything but the stage and the timeline, which use their whole area) keep their
content 10 px from the sides and 8 px from the top and bottom (`theme::PANEL_PADDING`); the gutter
between panels and around the dock is 4 px (`theme::GUTTER`). Between widgets: 6 by 4 px, with
8 by 3 px of padding inside buttons and 22 px rows. These are set in one place, `theme::apply`.

## Ruler and Mirror

The **Ruler** tool (M) measures: drag between two points and the stage shows the distance, the
angle (counter-clockwise from the right, up positive) and the offsets. The reading stays until the
next drag and is repeated in the tool's options.

The **Mirror** tool (X) holds the symmetry settings. Symmetry is Off, Left-right, Top-bottom or
Four-way; click the stage, or type x and y, to put the axes (the middle of the stage until set).
While Pencil, Brush or Mirror is in hand the axes are drawn in pink. With symmetry on, each
Pencil or Brush stroke is drawn with its reflections, each on its own layer, as one undo step.
The same tool offers Copy left-right and Copy top-bottom (also Modify > Mirror Copy), which put a
reflected copy of the selection across the axes, and Flip.

Pencil and Brush also have **Straight**: with it on, or while Shift is held, a stroke is the
straight line between where the drag began and where it ended.

## Info panel

Size and position of the selection, and under Pointer the stage position and the color of the
topmost shape there (its fill, the start of a gradient, or its line when it has no fill), found by
hit-testing the frame rather than reading pixels. Layouts saved before the panel existed get it
added next to History when they are opened.
