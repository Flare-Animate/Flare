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

An Interface switcher sits at the top right, left of the gear, where Animate has its workspace switcher. It will switch between whole interfaces; for now it is a dimmed button, and Window > Interfaces is a dimmed entry.

Publish (the button beside Play, and File > Publish) opens a popup in the middle of the window, like Preferences. It lists the formats (Flash .swf, HTML5 Canvas .html, animated GIF .gif, and MP4 .mp4 dimmed, because Flare has no video encoder yet), the Publish button, which then asks where to save and writes each format ticked side by side, and Share: Twitter, YouTube, Newgrounds and Facebook. A share button opens that site's own page for adding a file in the browser, and sends nothing itself; Flare has no account on any of them and nothing is uploaded. Twitter is called Twitter in the interface and opens x.com, as artists still call it. File > Publish Settings shows the same format choices on their own.

Theme: the General category also has Theme, Dark (the default) or Light. The palette is in `flare-ui`'s `theme.rs` as two sets of colors read by name through functions (`theme::text()`, `theme::panel()`), and the choice is kept with the preferences. Colors drawn on the document itself (the stage, guides, selection handles, the timeline's span colors) are the same in both. Both palettes keep text at least 7:1 against every surface (a test checks this); nobody has looked at the light one on a screen yet, so expect rough edges in places that draw their own colors.

Language: the General category of Preferences has a Language list, "System locale (English)" first and selected by default (the name in brackets is the language the system's locale maps to, English when Flare has none), then English, and then the others in the alphabetical order of their English names, each written in itself: 简体中文, Deutsch, 日本語, 한국어, Português (Brasil), Español. The words are Fluent files, one per language and section, in `localizations/<language>/<section>.ftl` (`en`, `zh-CN`, `de`, `ja`, `ko`, `pt-BR`, `es`), compiled in; a message a language lacks is shown in English. Sections are moved over one at a time: so far Preferences, Publish and the shared Close are translated, and the rest of the interface is English in every language. The translations were written by the maintainers' tooling and want a native reader's review. Japanese and Chinese need a font with their characters, and the interface does not carry one: it uses one the system has (PingFang or Hiragino on macOS, Yu Gothic or Microsoft YaHei on Windows, Noto Sans CJK on Linux) and shows boxes where there is none.

In the Timeline a layer is renamed by double clicking its name, which becomes a text box (Enter keeps the name, Escape drops it, an empty name is refused). A right click on a layer's name opens its menu: Rename, Hide or Show, Lock or Unlock, Mask, Motion guide, Move up and down, New layer, New folder and Delete. There is no rename box in the timeline's toolbar.

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

## Menu bar

The menus are Animate's, in its order: File, Edit, View, Insert, Modify, Text, Commands, Control,
Window, Help (Debug is for ActionScript and is left out). A command sits where Animate keeps it, taken
from Adobe's Help pages: File > Import and Export, Publish Settings and Publish; Edit > Timeline
(Remove, Copy and Paste Frames); Insert > Timeline (Layer, Layer Folder, Frame, Keyframe, Blank
Keyframe) with Create Motion Tween and Create Shape Tween beside it; Modify > Timeline (the layer
commands, Clear Keyframe, Convert to Keyframes) and Modify > Transform (Flip, Rotate, Scale, and Flare's
Mirror Copy); Control > Test. There is no Tools, Layer or Timeline menu, because Animate has none: the
tools are picked from the tool strip or by their keys (listed in Help > Keyboard Shortcuts), and
Home is the house in the top bar. Planned commands stay in their Animate places, dimmed. New
commands are put where Adobe puts the nearest one, not in a menu of Flare's own.

## Tweens

Insert > Create Motion Tween works on a selected instance, which moves, scales and turns toward the
next keyframe's instance of the same symbol (a motion guide above it steers the path). Insert >
Create Shape Tween works on a selected shape, which turns into the next keyframe's shape: the
outlines are matched contour by contour (the shorter is split until the counts agree, a closed one is
turned and flipped to the nearest start), and colors, gradients, stroke widths and fills are
blended; a contour with no partner shrinks to its middle or grows from it; a stroke that only one
shape has fades. A tween that cannot be built (a closed outline against an open one, or more than
256 contours or 4096 segments) holds the first shape. Each command removes its tween when chosen
again. The Properties panel has the same two switches and an **Ease** slider from -100 (starts
slowly) to 100 (ends slowly); the curve is a blend of a straight line and a quadratic, exact at the
ends and at both extremes but not checked against Animate's. A tweened span is drawn blue (motion)
or green (shape) with an arrow in the timeline.

## Panel rail

A narrow bar at the right edge, like Animate's collapsed panel strip, has an icon for each panel
of the right-hand stack: Properties, Library, Color, Align, Transform, History and Info. Clicking one
brings that panel to the front of its stack; the one showing is highlighted. It is a shortcut to what
the tabs and Window menu already do, so no panel moves or closes. It is not shown on Home.

## Tool strip

The tools are in two columns, in the order Animate's toolbar has them, divided by a line into
categories: Selection, Free Transform, Lasso; Pencil, Brush, Eraser, Rectangle, Line; Pen, Text,
Paint Bucket, Eyedropper, Asset Warp; Hand, Zoom and the "…" button. As in Animate a cell is a slot
that holds related tools and shows the one in use, with a small triangle in its corner: Selection
holds Subselection, Free Transform holds Gradient Transform, Lasso holds Polygon and Magic Wand,
Rectangle holds Oval, Rectangle Primitive, Oval Primitive and PolyStar, Pen holds Add, Delete and
Convert Anchor Point, Pencil holds Paint Brush, Paint Bucket holds Ink Bottle, and Hand holds Rotation and Time scrub. Press and hold a
slot, or right-click it, to open its list; picking a tool from the list makes it the one the cell
shows, and keys that pick a tool (A for Subselection, O for Oval) change what its cell shows too.
The "…" button (More tools) opens a list of the tools that are not in the strip:
Flare's own Mirror and Ruler, and 3D Rotation, 3D Translation, Deco, Bone, Bind, Width and Camera, which
are not built. Tools that are not built are dimmed with a note, like the menu's planned commands,
and so is Asset Warp, a whole cell. Escape, a pick or a click elsewhere closes a list. The strip scrolls
when it is taller than its panel.
Any tool in the dots list can be dragged out of it onto the strip, where it becomes a cell of its own just above the dots (the tools not built too, dimmed); a right click on it offers Remove from toolbar, and it is in the list again. The tools put there are kept with the preferences, and Restore defaults empties them. Escape, or letting go away from the strip, drops the tool being dragged.
Below them the fill and stroke colors are two overlapping squares, the stroke in front as a frame
around an empty middle (smaller in a strip one tool wide). Two small buttons keep their old places:
swap at the top right of the squares and black and white at the bottom left (stacked beneath them in
a strip one tool wide). An outline shows only while the pointer is over a square. Click either square to choose it and open its color
picker, which starts with a No color button (an empty square is drawn white with a red slash): that
is how a shape is drawn with no stroke or no fill. The strip is one tool wide by default and runs
the whole height of the window, from the top bar to the bottom, with the timeline beside it, not
under it; it has two columns only when it is made at least 72 px wide. It is a shade lighter than
the page around it, with a thin dark line down its right edge. A layout saved with the old
arrangement opens as the new default.
The segmented control inside panels (Properties' Tool, Object,
Frame and Doc) marks the chosen segment by raising it, with no underline.

## Home

A plain launch opens on the Home page, laid out like Animate's: a sidebar with the app name and
version, Home (and Learn, dimmed: not built), and the buttons New File, Open and Import; in the main
area **Create new** (cards for Character Animation 1920 x 1080 at 24 fps, Social 1080 x 1080 at 30,
Portrait 1080 x 1920 at 30, Web 1280 x 720 at 24, Classic 550 x 400 at 24, each drawn with the
proportions of its stage), then **Recent** (up to twelve project cards, newest first, each with the
folder, when the file was last changed and its size, a search box over the names and folders). There is no button on Home to clear the list: it is **Clear recent projects** in Preferences, away from the search box and the cards, because one slip there forgets every project. The time reads as how long ago ("Modified 2 hours ago", then days, months and years), never a date, so it is the same in every time zone; a file that
is gone says "Not found". The files are read on a thread of their own, so a slow drive never
stalls the page, and a card says "Reading…" until the answer is in. The presets are Flare's own, not Animate's. Choosing a card, New File,
Open or Import leaves Home; a new project over an untouched document replaces it instead of adding a tab.

The top bar's right end holds, from the left, Publish (an arrow up out of a tray, as Animate has a Publish button there), Play, Undo, Redo and the settings gear (Preferences), which is the rightmost button of the top bar, in the same corner on Home and in the editor. Preferences opens as a dialog in the middle of the window over a dimmed backdrop, like PaintCraft's. It has an X at the top right and Close at the bottom right, and Escape or a click outside closes it; Restore defaults, red, is at the bottom left. Its categories (General, Stage, Timeline and drawing, Recent projects) each have a bold heading with a picture, and the controls sit in a box inset below the dialog's own color, so a category reads as a block; they are not tabs.

Home is a view, not a document, and none of the document's controls show on it: the top bar has
no undo, redo, play or document name, there is no status bar, and the window is titled just
"Flare" (a document's title is its saved name, or Untitled, with `*` when unsaved). Keys and commands
that act on the document are off while it is up. The house and "Flare" at the left of the top bar
toggles it; choosing a tab, even the current one, goes to that tab's document. A
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
The same tool offers Copy left-right and Copy top-bottom (also Modify > Transform > Mirror Copy), which put a
reflected copy of the selection across the axes, and Flip.

Pencil and Brush also have **Straight**: with it on, or while Shift is held, a stroke is the
straight line between where the drag began and where it ended.

## Info panel

Size and position of the selection, and under Pointer the stage position and the color of the
topmost shape there (its fill, the start of a gradient, or its line when it has no fill), found by
hit-testing the frame rather than reading pixels. Layouts saved before the panel existed get it
added next to History when they are opened.
