# Interface

The workspace follows the familiar animation-tool layout: a tool strip on the left, the stage in
the middle with a bar above it (scene, zoom, view toggles) and rulers along its edges, panels on
the right (Properties, Library and Layers; Color, Align, Transform and History), the timeline below, and a
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

## Splash

On a plain launch (desktop with no file named, and the browser build) a splash covers the window for 2.5 seconds or until a click or key: an original backdrop (`assets/ui/splash.svg`, no text, drawn by `flare-ui`'s `splash.rs`) with the flame, the name, the tagline and the version set over it by `workspace/splash.rs`, so the words are translated like the rest. The old program's splash artwork is not reused.

## Planned features are visible

Controls for features that do not exist yet are drawn at full fidelity but dimmed, and say so on
hover ("Not available yet"). They are built with the `mock_*` helpers in `chrome.rs`, and menu
commands that are planned are `MenuEntry::Mock` entries in `MENU_LAYOUT`. Finding them is a search
for `mock_` and `Mock(`; building one replaces it with the real control.

Still planned (everything else in the menus and panels is built):

- Library: folders.
- Edit, Modify, Text, Commands and Help entries still listed dimmed in the menus (Window and the Help windows are real now: panels come to the front, About and a read-only shortcut list).

## Top bar

The window has no system title bar, so the document's name is not written twice: the top bar takes its place. It is Animate's own bar: the home mark, the in-window menus where there is no native menu bar (File, Edit, View, Insert, Modify, Text, Commands, Control, Debug, Window and Help; Debug is dimmed, as Flare has no scripts to debug), and at the right Play, Publish and the workspace switcher (a dimmed place held for it), then the window buttons. The document's name is on its tab, undo and redo are in the Edit menu and on their keys, and Preferences are in the Edit menu. Empty space in the bar moves the window and a double click zooms it; the workspace asks and the desktop app does it (`WindowRequest`), so the UI crate stays free of window code.

On macOS the bar is 28 px high with room at the left for the window's own buttons. On Linux and Windows the window opens without the system's frame and the same bar carries its own minimize, maximize or restore, and close buttons at the right (close asks about unsaved work like Quit does); the window is resized from its edges and corners by the system's own resize (`drag_resize_window`), and `FLARE_SYSTEM_TITLEBAR=1` brings back the system's title bar for a window manager that does not get on with this. Not done: Windows 11's Snap Layouts flyout over the maximize button, which needs a hit-test hook that Flare's no-`unsafe` rule keeps out for now, and the shadow and rounded corners a compositor gives a framed window. This has been built and its logic tested on macOS only; it has not been run on Linux or Windows.

## Type

Interface text is Source Sans 3 Regular (`assets/ui/fonts`, SIL Open Font License, licence text beside it), embedded in the program and placed ahead of egui's built-in fonts, which still cover the glyphs it lacks. On macOS the top bar is 28 px tall, the height of the title bar it replaces, so the window buttons the system draws sit centred in it.

## Default layout

Left to right: the tool strip, the stage, and one stack of tabs (Properties, Library, Color, Align, Transform, History, Info). Underneath, spanning the window, are the Timeline and Output tabs. Output keeps every message the status bar showed, so a refusal that scrolled past can be read again. The Window menu brings any panel to the front. Any panel can be dragged to another stack or out on its own, and View > Reset Layout restores this arrangement.

An Interface switcher sits at the top right, left of the gear, where Animate has its workspace switcher. It will switch between whole interfaces; for now it is a dimmed button, and Window > Interfaces is a dimmed entry.

Publish (the button beside Play, and File > Publish) opens a popup in the middle of the window, like Preferences. It lists the formats (Flash .swf, HTML5 Canvas .html, animated GIF .gif, and MP4 .mp4, the picture as H.264 and the cued sounds as AAC, written by Flare itself with nothing to install; the browser build does not make MP4 yet), the Publish button, which then asks where to save and writes each format ticked side by side, and Share: Twitter, YouTube, Newgrounds and Facebook. Only Twitter's button is live: it opens x.com with the text of the post already filled in. YouTube, Newgrounds and Facebook have no address that carries anything in, so they are dimmed as not available yet until they can be filled in (YouTube through its API, once the project has credentials). Flare has no account on any of them and nothing is uploaded. Twitter is called Twitter in the interface and opens x.com, as artists still call it. File > Publish Settings shows the same format choices on their own. File > Export has Image, Animated GIF and Flash Document (a `.fla`), which Animate opens.

Theme: the General category also has Theme, Dark (the default) or Light. The palette is in `flare-ui`'s `theme.rs` as two sets of colors read by name through functions (`theme::text()`, `theme::panel()`), and the choice is kept with the preferences. Colors drawn on the document itself (the stage, guides, selection handles, the timeline's span colors) are the same in both. Both palettes keep text at least 7:1 against every surface (a test checks this); nobody has looked at the light one on a screen yet, so expect rough edges in places that draw their own colors.

Language: the General category of Preferences has a Language list, "System locale (English)" first and selected by default (the name in brackets is the language the system's locale maps to, English when Flare has none), then English, then the languages written in Latin letters alphabetically by their own names (Deutsch, Español, Français, Português (Brasil)), then the others by language tag (日本語, 한국어, 简体中文); each is written in itself. The code keeps the same order (`Language::ALL`, and a test checks it). The words are Fluent files, one per language and section, in `localizations/<language>/<section>.ftl` (`en`, `de`, `es`, `fr`, `pt-BR`, `ja`, `ko`, `zh-CN`), compiled in; a message a language lacks is shown in English. Everything the interface says is a message (menus, panels, tools and their hints, dialogs, status text, the names a new document gives its scene and layer, how long ago a project was opened); only "Flare", units, product and format names and what the person typed stay as they are. A file added to `localizations/en/` is a section with no registration, and every other language must have the same file and keys (the build stops, and tests check the keys, the `{ $placeholders }`, that no key is in two files, and that a count takes its language's plural form). Code asks for a message with `tr!(workspace, "key")`, whose key is checked when the program is built; a test draws every tool, panel and window in a pseudo language that puts each message between ⟦ and ⟧ and fails on a word outside the marks, and another reads the source for plain English given to a widget. Not yet translated: the text of errors that come from the libraries (a file that cannot be read, a document the editor refuses), which is shown in English after a translated sentence, and the command line and MCP server, which speak English. The translations were written by the maintainers' tooling and want a native reader's review. Japanese, Korean and Chinese need fonts with their characters, so the interface carries Noto Sans CJK cut down to the national character set of each (about 3.5 MB together, `scripts/fonts/subset_cjk.py`) and a test checks every character in a translation is in its font; the language list draws each name in its own font, so it reads right whatever the interface speaks.

A selected Graphic instance has a Looping section in Properties, as Animate's does: Options (Loop, Play Once or Single Frame) and the First frame it starts on, counted from 1. Each change is one undo step. A Movie Clip, a Button, artwork or several selected things show no such section, because only a Graphic follows the setting (a Movie Clip loops from where its instance starts; a Button shows its Up state).

The Timeline's frames are small fixed cells (10 points at the normal zoom), not the panel's width shared among them: the scene's frames are followed by empty cells to the panel's edge, the zoom (Ctrl/Cmd and the wheel, or the buttons) widens or narrows them from 3 to 40 points, and Fit sets the zoom at which the scene fills the panel. The timeline's toolbar has the frame commands used most, after the frame number and rate: Insert Keyframe (F6), Insert Blank Keyframe (F7), Insert Frame (F5) and Remove Frames (Shift+F5). They are the commands of the Insert menu and of the frames' own menu, act on the active layer at the playhead, say their name and keys when pointed at, and are dimmed on a locked layer. In the Timeline each frame is a cell: a line between frames (a stronger one every fifth, so they can be counted), a dot on the first frame of a keyframe (hollow when the keyframe holds nothing) and a box on the last frame of a held keyframe, as Animate draws them. A right click on the stage picks the shape under the pointer (unless it is already selected) and opens a menu with Cut, Copy, Paste, Paste in Place, Duplicate, Delete, Group, Ungroup, Break Apart, Convert to Symbol, Select All, Deselect All, Undo, Redo and the zoom commands; on empty stage it clears the selection first. In the Timeline a layer is renamed by double clicking its name, which becomes a text box (Enter keeps the name, Escape drops it, an empty name is refused). A right click on a layer's name opens its menu: Rename, Hide or Show, Lock or Unlock, Mask, Motion guide, Move up and down, New layer, New folder and Delete. There is no rename box in the timeline's toolbar. A right click on the frames picks the frame under it (unless it is inside the frames already selected) and opens the menu for frames: Insert Frame, Insert Keyframe, Insert Blank Keyframe, Clear Keyframe, Remove Frames, Create Motion Tween, Create Shape Tween, Cut, Copy, Paste, Select All and Reverse Frames. Cut Frames takes the frames out, so what follows moves left; Select All Frames selects the active layer's frames (the selection holds one layer); Reverse Frames needs whole keyframes selected.

## Documents and tabs

A window holds several documents as tabs above the stage; when there are more than fit the strip scrolls (the wheel turns it, and the tab you switch to or make is brought into view) and the new-document button stays beside it. One is edited; the others are parked whole and swapped in when their tab is chosen, so each keeps its own history, selection and view. The person's tool, colors, settings and panel layout stay with the window, not the document. Open (which takes a Flare project, a `.fla`, an `.xfl` or an XFL folder's `DOMDocument.xml`, and a `.swf`, as Animate's does for its own files: Animate's Import is for pictures, sound and video) and New open in a tab of their own, except that a blank untouched document gives way to what is opened. Revert reloads in the same tab, from the file the document lives in (the `.fla` too). Closing a tab with unsaved work asks first, in a box over the window: Save (writes the document in its own format, asking where for an untitled one, and closes the tab once it is on disk; if the place is not chosen the tab stays), Don't Save (closes it and drops its recovery copy) or Cancel (Escape does the same). Closing the window asks about every tab. Autosave and crash recovery cover only the document on show for now.

Save formats, as in Animate: an animator can work without ever making a `.flare` and send the file to somebody who uses Animate. **File > Save As** offers Flash Document (`.fla`, first and the default), uncompressed Flash Document (`.xfl`) and Flare project (`.flare`); the ending of the name decides the format, in any case, and only the last ending counts (`a.flare.fla` is a `.fla`). A name with no ending, or an ending that is none of the three (`drawing.v2`, `movie.swf`), gets `.fla` added. The dialog starts from the document's own folder and name, or `Untitled.fla`. An `.xfl` is a folder, as in Animate: saving `dir/a.xfl` writes the folder `dir/a/` holding `DOMDocument.xml`, `LIBRARY/` and the stub `a.xfl`, and the document is then `dir/a/a.xfl` (a folder that came with another stub name keeps it); a folder that is already an XFL document is replaced in place, and any other existing folder of that name is refused. **File > Save** writes the document back to the file it lives in: a document opened from `x.fla`, `x.xfl` (or its folder, or its `DOMDocument.xml`, which all name the same stub) is saved back to it with no `.flare` made; one opened from `x.flare` stays a project; a new document asks Save As. Opening a `.fla` or `.xfl` does not make the document unsaved; the window title and tab show the file's real name and ending (`x.fla`), with `*` when unsaved, and the Recent list holds `.fla` and `.xfl` paths, which reopen through the normal Open. The file is replaced only by a whole new one written beside it (a read-only file, a full disk or a folder that cannot be written leaves the old file as it was, says why in the status line and keeps the document unsaved), and the work is done off the UI thread. A movie (`.swf`) or an Animate file Flare could not keep whole (a pre-CS5 binary `.fla`, one over the size cap) opens as an unsaved project and asks Save As; a pre-CS5 `.fla` is never overwritten. Autosave writes the same recovery snapshot (a `.flare`-format file in the recovery folder) for any document, and a recovered document is unsaved work with no file until it is saved. After a save to `.fla` or `.xfl` the status line says what was written, and the Output panel lists it: `Saved x.fla (unchanged original)`, or `Saved x.fla (3 of 14 files rebuilt, the rest carried over as they were)` with the names of the rebuilt files, plus, when the format cannot hold something Flare has (cubic curves approximated, sounds not written, a translucent background written opaque), the count of such notes and each one. In the browser build Save and Save As are a download with a name prompt (the last name offered again); `.fla` is the default, `.flare` is a choice, and `.xfl` is saved as a `.fla`, which holds the same files.

## Outline colors and Outline Mode

Every layer has a color chip at the right edge of its label (the outline column): its own color if
it was given one, otherwise one of eight by position. Clicking the chip cycles the palette as one
undo step. The color is kept in `.flare` files and in XFL (`DOMLayer color`). View > Outline Mode
draws every shape as a one-pixel line in its layer's color with no fills and ignores masks, so
everything can be found; the evaluated frame carries each shape's layer color for this
(`DrawnShape::outline`). A mask layer itself is not drawn, so it has no outline yet.

## Still to make familiar from Animate 2024 (Linux reference)

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
(Remove, Cut, Copy and Paste Frames, Select All Frames); Insert > Timeline (Layer, Layer Folder, Frame, Keyframe, Blank
Keyframe) with Create Motion Tween and Create Shape Tween beside it; Modify > Timeline (the layer
commands, Clear Keyframe, Convert to Keyframes, Reverse Frames) and Modify > Transform (Flip, Rotate, Scale, and Flare's
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

## Bones

The Bone tool (in the "…" list) links symbol instances as Animate's does: drag from one instance to
another and a bone joins them, both leaving their layer for a new armature layer just above it
("Armature_1"), whose frames are poses and are drawn green in the timeline. A drag from an instance
already in an armature adds a bone below it; letting go over nothing says so. The joints go in the
middle of each instance. The bones of every visible, unlocked armature are drawn over the stage as
pale wedges with a circle at each joint, the one last touched in the accent color. The Selection
tool poses: a press on a joint, on an instance the armature carries, or on a bone (which is pulled
by its tip) drags it, the bones following the pointer as it moves, and letting go keys the pose at
the playhead. A joint is pulled by inverse kinematics, the joints above it turning within their
limits; a root moves the whole armature. Between two poses the drag makes a pose of its own, which
the one before tweens into. With the Bone tool, Properties shows the joint last touched: its Turn,
Limit rotation with Min and Max, Remove bone (that bone and the ones below it go, the instances
stay where they are) and Release armature (plain instances again, where the pose put them). Each
change is one undo step.

## Panel rail

A narrow bar at the right edge, like Animate's collapsed panel strip, has an icon for each panel
of the right-hand stack: Properties, Library, Layers, Color, Align, Transform, History and Info. Clicking one
brings that panel to the front of its stack; the one showing is highlighted. It is a shortcut to what
the tabs and Window menu already do, so no panel moves or closes. It is not shown on Home.

The Layers panel is a compact stack for the active scene: click a row to choose its layer, double
click its name to rename it, and use the eye and lock controls to change visibility and editing.
Its toolbar adds layers and folders, reorders the active layer and deletes it; the context menu
offers the same layer operations as the timeline. These changes use the same validated commands
and undo history as the timeline controls.

The Brush options include built-in Plain, Pencil, Krita-style Ink, Blender Grease Pencil,
Calligraphy, OpenToonz-style Vector and Adobe Animate-style profiles, and four inking pens: G-pen
(width follows pressure evenly), Mapping pen (fine, widens only under a firm press), Turnip pen
(widens early) and Brush pen (a hairline under a light touch that comes in late and thins when the
hand is quick). The inking pens thin to nothing at no pressure, each has its own drawn pressure
curve, and all but the Brush pen taper both ends by 20 px. Nib shape, angle, flatness,
minimum pressure width and pressure response can be edited; New keeps the settings as a custom
profile in preferences.

**Stabilization** (0 to 40, a Brush option) steadies the hand: each pen position, and its
pressure, is the average of the last that many positions, so wobble and pressure flicker are
smoothed away and the line trails the pen by about half that many samples. When the pen moves far
between two samples the window shortens (at full rate) and then grows back (at half rate), so a
quick, deliberate move lags less than a slow, careful one; the speed is judged on the screen, so
it is the same at every zoom. When the pen is let go the line catches up with where it was. The
G-pen steadies 6 samples, the Mapping pen 10, the Turnip pen 2 and the Brush pen 15; the other
profiles do not steady. **Taper start** and **Taper end** (0 to 500 px) thin the two ends of a
stroke: over that length of the line the pressure eases in from none, and out to none, along a
quarter sine, so the tip is as thin as the lightest touch. A stroke shorter than the two lengths
together tapers over all of itself. A mouse, which presses fully, gets tapered ends too. Both
settings belong to the brush in hand: choosing a profile or a custom brush sets them, editing them
edits the custom brush chosen, and they are kept with the preferences. What is drawn while dragging is
the stroke that will be kept: every frame the preview builds the committed outline itself, with the
Smoothing, the curve through the points, the nib's shape and the tapers, so nothing changes when
the pen is let go. The Pencil's preview likewise follows the smoothed curve it commits. Custom profile collections also have a bounded, versioned JSON format.
This does not read or write native `.kpp`, `.abr` or other application-specific preset
containers: those formats can contain engine-specific textures and settings that Flare's vector
brush model cannot preserve. A Blender-style sculpt/deformation tool is not implemented yet.

## Tool strip

The tools are in two columns, in the order Animate's toolbar has them, divided by a line into
categories: Selection, Free Transform, Lasso; Pencil, Brush, Eraser, Rectangle, Line; Pen, Text,
Paint Bucket, Eyedropper, Asset Warp; Hand, Zoom and the "…" button. As in Animate a cell is a slot
that holds related tools and shows the one in use, with a small triangle in its corner: Selection
holds Subselection, Free Transform holds Gradient Transform, Lasso holds Polygon (click the corners of a loop; a double click, Enter or a click on the first corner closes it and selects what it encloses) and Magic Wand,
Rectangle holds Oval, Rectangle Primitive, Oval Primitive and PolyStar, Pen holds Add, Delete and
Convert Anchor Point, Pencil holds Paint Brush, Paint Bucket holds Ink Bottle, and Hand holds Rotation and Time scrub. Press and hold a
slot, or right-click it, to open its list; picking a tool from the list makes it the one the cell
shows, and keys that pick a tool (A for Subselection, O for Oval) change what its cell shows too.
The "…" button (More tools) opens a list of the tools that are not in the strip:
Flare's own Mirror and Ruler, the Camera and the Bone tool, and 3D Rotation, 3D Translation, Deco, Bind and
Width, which are not built. Tools that are not built are dimmed with a note, like the menu's planned commands,
and so is Asset Warp, a whole cell. Escape, a pick or a click elsewhere closes a list. The strip scrolls
when it is taller than its panel.
Any tool in the dots list can be dragged out of it onto the strip, where it becomes a cell of its own just above the dots (the tools not built too, dimmed); a right click on it offers Remove from toolbar, and it is in the list again. The tools put there are kept with the preferences, and Restore defaults empties them. Escape, or letting go away from the strip, drops the tool being dragged.
Below them the fill and stroke colors are two overlapping squares, the stroke in front as a frame
around an empty middle (smaller in a strip one tool wide). Two small buttons sit side by side above
the squares, as in Animate's toolbar: black and white on the left and swap on the right. An outline shows only while the pointer is over a square. Click either square to choose it and open its color
picker, which starts with a No color button (an empty square is drawn white with a red slash): that
is how a shape is drawn with no stroke or no fill. The strip is one tool wide by default and runs
the whole height of the window, from the top bar to the bottom, with the timeline beside it, not
under it; it has two columns only when it is made at least 72 px wide. It is a shade lighter than
the page around it, with a thin dark line down its right edge. A layout saved with the old
arrangement opens as the new default.
The segmented control inside panels (Output's tabs, for one) marks the chosen segment by raising it,
with no underline.

The Properties panel is one view that follows the selection, as Animate's does, and has no tabs of
its own. Something selected on the stage: a shape shows Position and size and its Fill and stroke
(fill color, outline on or off, stroke color, width, cap and join), which are changed on the shape,
a color when the picker is let go and a number when it is, each one undo step; a symbol instance
shows its symbol and type, Edit Symbol, Position and size, Looping for a Graphic, and Color Effect,
Display and Filters dimmed until they exist; several things show their shared Position and size.
Nothing selected but frames: the frame, with its tween and Ease. Nothing at all selected: the
settings of the tool in hand if it has any (the drawing tools), else the document (size, frame
rate, background, Publish Settings). Modify > Document asks for the document until the selection or
the tool changes.

## Home

A plain launch opens on the Home page, laid out like Animate's: a sidebar with the app name and
version, Home (and Learn, dimmed: not built), and the buttons New File, Open and Import; in the main
area **Create new** (cards for Character Animation 1920 x 1080 at 24 fps, Social 1080 x 1080 at 30,
Portrait 1080 x 1920 at 30, Web 1280 x 720 at 24, Classic 550 x 400 at 24, each drawn with the
proportions of its stage, and **Custom Size**, a plus card that opens a small window for a width,
height and frame rate (up to the stage's largest side and 240 fps) with Create and Cancel; it goes
away if a tab is chosen over Home), then **Recent**: a list, as Animate's, of up to twelve projects
with a Filter box over the names and folders. The columns are Name (with its folder, dimmed), Date
modified, Size and Kind (Flare project, Animate document or Animate project). A click on a heading
orders the list by it, up, then down, then back to the order the projects were opened in; a file
whose size or time is not known yet goes last whichever way the column runs. There is no button on
Home to clear the list: it is **Clear recent projects** in Preferences, away from the filter and
the rows, because one slip there forgets every project. The time reads as how long ago ("2 hours
ago", then days, months and years), never a date, so it is the same in every time zone; a file that
is gone says "Not found". The files are read on a thread of their own, so a slow drive never
stalls the page, and a row says "Reading…" until the answer is in. The presets are Flare's own, not Animate's. Choosing a card, New File,
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

Everything drawn goes into the **active layer**, as in Animate: a new shape (Rectangle, Oval, Line, PolyStar, Pencil, Brush, Pen, Text) joins the keyframe of that layer at the playhead as one more element, drawn above the others; on a frame with no keyframe a new one starts there and lasts to the end of the scene (one frame when the layer is alone and the scene is empty) or to the layer's next keyframe; a document with no layers gets "Layer 1" first. Locked, hidden, folder, mask, guide and camera layers, and a keyframe that tweens or holds a camera, refuse a drawing and say why in the status line. A click picks one element (a stroke is picked within a few units of its path, so a hairline pencil or line is not missed), Select All and the lasso pick every element (the lasso takes a stroke when any part of its path is inside the loop), and Move, Delete, Duplicate, Group, Convert to Symbol and the Align panel act on the elements picked. Modify > Arrange (Bring to Front, Bring Forward, Send Backward, Send to Back) and the Align panel's Bring to Front and Send to Back move an element up or down among the others of its keyframe, and a keyframe that holds only that element takes its layer to the top or bottom of the layers, as before. Every tool that draws shows **Object Drawing (J)** in its options, ticked and dimmed with the note that Merge Drawing is not built yet: until it is, every shape is a separate drawing object and shapes on a layer do not cut each other.

The **Ruler** tool (M) measures: drag between two points and the stage shows the distance, the
angle (counter-clockwise from the right, up positive) and the offsets. The reading stays until the
next drag and is repeated in the tool's options.

The **Mirror** tool (X) holds the symmetry settings. Symmetry is Off, Left-right, Top-bottom or
Four-way; click the stage, or type x and y, to put the axes (the middle of the stage until set).
While Pencil, Brush or Mirror is in hand the axes are drawn in pink. With symmetry on, each
Pencil or Brush stroke is drawn with its reflections, all in the same keyframe of the active layer, as one undo step.
The same tool offers Copy left-right and Copy top-bottom (also Modify > Transform > Mirror Copy), which put a
reflected copy of the selection across the axes, and Flip.

The **Line** tool (N) draws a straight line from where the drag began to where it ended, in any
direction. While Shift is held the line keeps to the nearest of eight directions, 45 degrees apart
(level, upright and the diagonals), and its end is the point of that direction closest to the
pointer; Shift can be pressed or let go during the drag, and the preview shows what will be drawn.

Pencil and Brush also have **Straight**: with it on, or while Shift is held, a stroke is the
straight line between where the drag began and where it ended.

**Pen pressure** sets how wide a Brush stroke is. Size is the width at full pressure, and **Light
touch** (0 to 1, 0.15 by default, a tool option) is the fraction of it a stroke thins to at the
lightest touch; in between the width follows the pressure in a straight line. A mouse, a finger,
or a device that reports no pressure draws at full pressure. Pressure reaches the Brush from a pen
in the browser build (the page's pointer events on the canvas), from the window system on
Windows, from the X server on Linux when a pen is plugged in when Flare starts, and from AppKit's
events on macOS (`FLARE_TABLET=off` turns the last two off; started from a terminal, Flare says
`pen pressure from:` there once it has seen a pen, so a tablet whose driver does not mark its
events as a pen's can be told from one that does; on macOS `FLARE_TABLET=log` writes every
pointer event there, and a plain mouse event within a quarter of a second of the pen's own is
taken as the pen's motion, for drivers that send pressure and motion apart). Native Wayland does not pass pressure
yet. A Brush
stroke is saved as the outline it painted; its centre line is simplified only where neither the
line nor its width changes by more than the Smoothing allows, so a press or a taper along a
straight stroke is kept, so the width a pen gave is kept in the drawing and
playback and export draw the same width; the pressure itself is not kept, so it cannot be changed
after the stroke is made. Pencil draws a line of one width and ignores pressure. Light touch is
not kept in Preferences, so each session starts at 0.15.

## Info panel

Size and position of the selection, and under Pointer the stage position and the color of the
topmost shape there (its fill, the start of a gradient, or its line when it has no fill), found by
hit-testing the frame rather than reading pixels. Layouts saved before the panel existed get it
added next to History when they are opened.

## Camera

The Camera tool (in the "…" list) adds the scene's camera layer, above the others and drawn in orange in
the timeline, the first time it is picked; the camera then looks at the stage as it is. With the tool
in hand a frame on the stage marks what the camera sees at the current frame (turned and smaller or
larger as the camera is), and dragging moves the camera the other way, so the picture follows the
pointer; each drag, and each edit in Properties (X, Y, Zoom in percent, Rotation in degrees), keys the
camera at the current frame as one undo step. A keyframe inside another splits it, and "Move to the
next keyframe" (with an Ease) is the camera's motion tween, also available as Create Motion Tween in the
timeline. A button in the bar above the stage (shown when the scene has a camera) previews the picture
through the camera; the preview cannot be edited, and the stage is otherwise drawn and picked in the
scene's own coordinates whatever the camera does. Removing the camera removes its layer. Known gaps: a
camera keyframe does not follow frames inserted at the end of the movie (the last keyframe runs to the
end the movie had when it was set), "attach to camera" for a layer that should not move with it, and
the camera's color effects. Nobody has used it by hand yet.

## The application menu

On macOS the first menu, named Flare, holds About Flare, Preferences (Cmd+,), the system's Services,
Hide and Quit, as other Mac applications do; About and Preferences are not repeated in Edit and
Help there. Elsewhere Preferences is in Edit with Ctrl+, and About in Help. About opens Flare's
window with the version and links to GitHub and Discord.
