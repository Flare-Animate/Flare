# Interface

The workspace follows the familiar animation-tool layout: a tool strip on the left, the stage in
the middle with a bar above it (scene, zoom, view toggles) and rulers along its edges, panels on
the right (Properties, Library and Layers; Color, Align, Transform and History), the timeline below, and a
status bar. Panels dock, tab, resize and can be rearranged; `View > Reset Workspace Layout`
restores the default.

## Look

`flare-ui/src/theme.rs` (shared widgets and the icon set are in `flare-ui`, the workspace in `flare-animate`) holds the palettes, spacing, rounding and type sizes; colours are
named there and used by name. The default dark palette is flat greys with no tint, as animation
tools use, so the colors on the stage are judged against nothing colored; the blue-tinted dark it
replaced is the Blue theme. Every palette has three tones of ground, so neighbouring areas never
merge into one: the gaps between panels and the pasteboard darkest (`#1d1d1d` in Dark), the bars a
step up (`theme::chrome()`, `#282828`: the menu bar, the dock's tab rows and unchosen tabs, the
document tabs, the panel rail and the status bar), and the panels themselves lightest (`#323232`,
which the chosen tab shares so it reads as part of its panel). A test keeps each step at least
1.08:1 in every palette. Fields are a shade darker than the panel, labels a quiet grey (`#9a9a9a`)
and values near white (`#e3e3e3`). Text is 13 px (11 px small, 14 px headings), and a stack's tabs are 12 px in a
22 px row, so a panel's own content reads before its name. `chrome.rs` holds the shared widgets (icon buttons, section headers,
label/control rows) so every panel looks alike.

## Brand

The app icon is the flame on a rounded warm tile (`assets/flare-icon*.png`, `.icns`, `.ico`). The
interface shows it where it names the program: beside the name and version at the top of the Home
sidebar and in Help > About (`flare_ui::icons::app_icon`). The splash and its campfire use the
flame cut out of it (`icons::flame`). The desktop window carries it for the task bar and window
switcher on Linux and Windows (`flare-shell`'s `window_icon.rs`). A packaged macOS app takes its
name and icon from its bundle; a program run as built (`cargo run`) has neither, so it gives the
Dock the icon at start when it is not inside a bundle (`flare_platform::app_icon`), and the
desktop binary carries a small Info.plist of its own (`apps/flare-desktop/build.rs`, a
`__TEXT,__info_plist` section with `CFBundleName` Flare and no bundle identifier) so the menu bar
says Flare rather than the file's name.

## Icons

Icons are SVGs in [`assets/ui/icons`](../assets/ui/icons), drawn on a 24 x 24 grid with a 1.6 px
round stroke and a translucent fill for the body of a glyph. They are white on purpose: the app
tints them (dim, normal, hover, active, disabled), so one file serves every state and theme.
They are original drawings in a consistent duotone style, not copies of any other product's icons.

To add an icon, put an SVG in that folder. `build.rs` embeds every file, named after the file, and
the tests check that each one parses, draws something, is 24 x 24 and uses only white. A name used
in the code that has no file panics in debug builds and fails the tests.

## Magic Wand

The Magic Wand works on bitmaps. A click on one picks the pixels whose color is within the
Threshold (0 to 200, 20 to start, as in Animate) of the one clicked: each of red, green, blue and
alpha may differ by that much, compared premultiplied, so every clear pixel counts as the same
color. With Contiguous on (the default, as Animate's wand) only pixels joined to the clicked one
are picked; off, every like pixel in the picture. Shift-click adds to what is picked and Alt-click
takes away; a plain click on another bitmap starts again there. The picked pixels are striped in
the accent color over the bitmap, and the tool's options say how many there are. Delete (the key, Edit >
Delete, or Clear pixels in the options) makes them transparent as one undo step, which is how a
background is taken off a picture; Invert picks the others instead, to keep only what was
picked. Smoothing sets how soft the edge left behind is: Pixels keeps it exact, and Rough, Normal
and Smooth fade the kept pixels next to it over one, two and three pixels. Escape, Deselect All,
another tool, or an edit that takes the bitmap off the stage lets go of what is picked. A bitmap
in a group or a symbol is reached by editing it in place. Animate's wand works on a bitmap broken
apart into a fill; Flare's works on the bitmap itself, and a clear gives that one element a new
bitmap (the old one stays in the library while anything else shows it), so other frames showing
the same picture keep it whole. Picking and clearing run on the pixels worker
([PERFORMANCE.md](PERFORMANCE.md)); while one is out, further clicks wait for it.

## Blur and Smudge

Blur and Smudge work on bitmaps the way Clip Studio Paint's Blur and Finger tip do. Drag over a
bitmap: Blur softens what the stroke passes over, and Smudge picks up the colors under it and
pushes them along, as a finger through wet paint. Both work on the topmost bitmap under the start
of the drag (on a layer that is shown and unlocked, as the Magic Wand finds it), and the stage
shows the stroke as it is drawn. The whole drag is one undo step, Escape takes it back, and a click
blurs one spot (a Smudge click has picked up nothing to lay down). A stroke is a row of round dabs
whose width follows the pen's pressure. Its strength is a ceiling: going over the same place again
in one stroke does not blur or smear it further, so lift the pen and go again for more.

The options are Size (stage units, 1 to 1000; Blur starts at 80 and Smudge at 30, Clip Studio's
values), Strength, Hardness (how sharp the edge of a dab is; Blur starts soft, Smudge half way),
Pressure sets size, and for Smudge, Color stretch (0 to 100, 70 to start): how far a picked-up
color is carried along the stroke; at 0 it mixes in place. Each tool keeps its own settings.
Clear pixels blur and smudge too, so a picture's edge can be softened into the background. As
with a wand clear, a stroke gives that one element a new bitmap, so other frames showing the same
picture keep it as it was. Strokes are not mirrored by Symmetry or straightened by Shift. The
dabs and the new bitmap are made on the pixels worker; a large bitmap shows the stroke a few
times a second instead of every frame ([PERFORMANCE.md](PERFORMANCE.md)).

## Splash

On a plain launch (desktop with no file named, and the browser build) a splash shows for 2.5 seconds or until a click or key. It is laid out as Animate's: a card with rounded corners and a soft shadow in the middle of the window (which a plain sheet hides meanwhile), light on the left with the mark (the flame on a dark tile), the name beside it, the version, the tagline and a line on who makes Flare, and "Loading…" at the bottom; on the right, set in with a white margin and its own rounded corners, Flare's own picture: a campfire at night under a moon and stars, between pines, with embers rising (`assets/ui/splash.svg`, no text in it); the fire on its logs is the app's mark, painted there by `flare-ui`'s `splash::paint`. The words are set by `workspace/splash.rs`, so they are translated like the rest. In a small window the card scales down to fit. Neither Animate's nor the old program's artwork is used.

## Planned features are visible

Controls for features that do not exist yet are drawn at full fidelity but dimmed, and say so on
hover ("Not available yet"). They are built with the `mock_*` helpers in `chrome.rs`, and menu
commands that are planned are `MenuEntry::Mock` entries in `MENU_LAYOUT`. Finding them is a search
for `mock_` and `Mock(`; building one replaces it with the real control.

Still planned (everything else in the menus and panels is built):

- Library: folders.
- Edit, Modify, Text, Commands and Help entries still listed dimmed in the menus (Window and the Help windows are real now: panels come to the front, About and a read-only shortcut list).

## Top bar

The window has no system title bar, so the document's name is not written twice: the top bar takes its place. It is Animate's own bar: the home mark, the in-window menus where there is no native menu bar (File, Edit, View, Insert, Modify, Text, Commands, Control, Debug, Window and Help; Debug is dimmed, as Flare has no scripts to debug), and at the right Play, Publish and the workspace switcher, then the window buttons. The document's name is on its tab, undo and redo are in the Edit menu and on their keys, and Preferences are in the Edit menu. Empty space in the bar moves the window and a double click zooms it; the workspace asks and the desktop app does it (`WindowRequest`), so the UI crate stays free of window code.

The corner is laid out first, from the bar's right end, with its last button centred over the rail's icons below it, and the menus get what is left; a window too narrow for every menu keeps the last ones in a "»" menu after the others rather than letting them run under the corner (`workspace/top_bar.rs`).

On macOS the bar is 28 px high with room at the left for the window's own buttons. On Linux and Windows the window opens without the system's frame and the same bar carries its own minimize, maximize or restore, and close buttons at the right (close asks about unsaved work like Quit does); the window is resized from its edges and corners by the system's own resize (`drag_resize_window`), and `FLARE_SYSTEM_TITLEBAR=1` brings back the system's title bar for a window manager that does not get on with this. Not done: Windows 11's Snap Layouts flyout over the maximize button, which needs a hit-test hook that Flare's no-`unsafe` rule keeps out for now, and the shadow and rounded corners a compositor gives a framed window. This has been built and its logic tested on macOS only; it has not been run on Linux or Windows.

### Small windows

The desktop window is never smaller than 960 by 600 points (`MIN_WINDOW_SIZE`), the least the workspace is laid out for. When the window changes size the panels keep their width and the stage gives way, as in Animate: a split whose size changed keeps each side that does not hold the stage at its size in points, and the stage's side takes the rest, down to 240 points before the panels give way too (`flare_dock::keep_panel_sizes`, run before the dock is drawn). The tool strip is never narrower than one tool. The timeline's toolbar takes a second row when the timeline is too narrow for it, rather than losing its last buttons, and the status bar's message is cut short rather than run under the document's facts. `workspace/tests/narrow_window.rs` draws the workspace at the smallest size and at usual ones and checks that nothing is cut off at the window's edge or past the end of a bar.

## Screen readers

On macOS, Windows and Linux the desktop app gives screen readers (VoiceOver, Narrator, Orca) the
interface through AccessKit, which egui builds into its widgets; Animate's interface gives them
nothing. The tree is made only while a reader asks for it (`flare-shell` turns it on when the
reader's first request arrives and off when it lets go), so nobody else pays for it. The window
opens hidden for the moment it takes to attach AccessKit, which must exist before the window is
shown. Flare's own drawn widgets say what they are: an icon button (`chrome::icon_button`, which
the tool strip, the top bar and the panel toolbars use) is a button named by its hint, read as
pressed when it is on and unavailable when it is dimmed; the strip's icons are buttons named for
their panels; a swatch is a button named by its hex value. Not yet named: the stage and the
timeline's frame grid, which are drawn as one surface each. The tree is tested; it has not been
tried with a real screen reader yet.

## Type

Interface text is Source Sans 3 Regular (`assets/ui/fonts`, SIL Open Font License, licence text beside it), embedded in the program and placed ahead of egui's built-in fonts, which still cover the glyphs it lacks. On macOS the top bar is 28 px tall, the height of the title bar it replaces, so the window buttons the system draws sit centred in it.

## Default layout

Left to right: the tool strip, the stage with the Timeline and Output tabs under it, one stack of tabs (Properties, Library and Assets, as Animate's Essentials has them), and the icon strip at the right edge. As in Animate, the stack runs the window's whole height and the timeline sits under the stage alone, so a long Properties panel is not cut short. Output keeps every message the status bar showed, so a refusal that scrolled past can be read again. The Window menu brings any panel to the front, or opens it beside the strip when it is kept there. Any docked panel can be dragged to another stack or out on its own, and View > Reset Layout restores this arrangement. A layout saved with the timeline running under the stack opens as this default; one saved before the strip existed opens with every panel docked where it was.

## Workspaces

The workspace switcher sits at the top right, left of the gear, where Animate has its own, and Window > Workspaces lists the built-in ones. They are modelled on Animate's: **Essentials** (the default above), **Animator** (the timeline above the stage, Properties, Library and Assets in one stack on the right and Color, Swatches, Align, Transform and Info in a second under it), **Designer** (the timeline on top, Properties left of the stage, and Library and Assets, Align, Info and Transform, and Color and Swatches in three stacks on the right), **Classic** (the timeline on top, Properties and Assets on the right, everything else in the strip) and **Small Screen** (every panel in one stack on the right and nothing in the strip). Panels a workspace does not dock are in its icon strip.

Each workspace keeps the changes made to it while it was in use, as Animate's do: switching away and back finds it as it was left. View > Reset Layout, or the arrow beside the one in use in the switcher, takes it back to how it was built in or saved. The switcher's New workspace field saves the arrangement on screen under a name (up to 40 characters, at most 20 of them; the same name in any case replaces the one saved) and makes it the one in use; the cross beside one of the person's own forgets it, and Essentials comes back when it was in use. A refusal says why in the status line. The workspaces are kept between sessions in the layout file with the arrangement on screen (`flare_dock::Workspaces`, at most 1 MiB); a file from before workspaces opens as Essentials arranged as it says, and a damaged one is refused and changes nothing. The panels and their arrangement belong to the window: the document is never touched.

Publish (the button beside Play, and File > Publish) opens a popup in the middle of the window, like Preferences. It lists the formats (Flash .swf, HTML5 Canvas .html, animated GIF .gif, MP4 .mp4, the picture as H.264 and the cued sounds as AAC, and QuickTime .mov, the same picture and sound branded for QuickTime, both written by Flare itself with nothing to install; the browser build does not make either yet) and the Publish button, which then asks where to save, starting from the document's own folder and name (`scene.fla` publishes `scene.swf`, `scene.mp4` and the rest beside it; a new document is `Untitled`), and writes each format ticked side by side under the name chosen. There is no Share section: buttons that only opened YouTube or Twitter, with the file still to add by hand, were taken out until sharing can upload (see the roadmap). File > Publish Settings shows the same format choices on their own. File > Export has Image, Movie (an animated GIF, an MP4 or a QuickTime movie, chosen in the save dialog by the name's ending; a name with none is a GIF), Flash Document (a `.fla`, which Animate opens) and SVG (the frame on show, as a picture; animated SVG is not built).

The command palette opens from the search button at the top right, or Ctrl+K (also with a native menu bar): a search box over every command and tool, matched by the letters of the query appearing in the name, run with a click or Enter, stepped through with the arrow keys, closed with Escape. It is AniCraft's (storytold/anicraft), re-implemented for Flare's command registry. A phone or tablet gets the touch form of the workspace: the panel rail, the dock's tab bar and every widget grow to finger sizes (`Workspace::set_touch`); the web build detects one and the Android build always uses it.

Theme: the General category also has Theme, Dark (the default) or Light. The palette is in `flare-ui`'s `theme.rs` as two sets of colors read by name through functions (`theme::text()`, `theme::panel()`), and the choice is kept with the preferences. Colors drawn on the document itself (the stage, guides, selection handles, the timeline's span colors) are the same in both. Both palettes keep text at least 7:1 against every surface (a test checks this); nobody has looked at the light one on a screen yet, so expect rough edges in places that draw their own colors.

Language: the General category of Preferences has a Language list, "System locale (English)" first and selected by default (the name in brackets is the language the system's locale maps to, English when Flare has none), then English, then the languages written in Latin letters alphabetically by their own names (Deutsch, Español, Français, Português (Brasil)), then the others by language tag (日本語, 한국어, 简体中文); each is written in itself. The code keeps the same order (`Language::ALL`, and a test checks it). The words are Fluent files, one per language and section, in `localizations/<language>/<section>.ftl` (`en`, `de`, `es`, `fr`, `pt-BR`, `ja`, `ko`, `zh-CN`), compiled in; a message a language lacks is shown in English. Everything the interface says is a message (menus, panels, tools and their hints, dialogs, status text, the names a new document gives its scene and layer, how long ago a project was opened); only "Flare", units, product and format names and what the person typed stay as they are. A file added to `localizations/en/` is a section with no registration, and every other language must have the same file and keys (the build stops, and tests check the keys, the `{ $placeholders }`, that no key is in two files, and that a count takes its language's plural form). Code asks for a message with `tr!(workspace, "key")`, whose key is checked when the program is built; a test draws every tool, panel and window in a pseudo language that puts each message between ⟦ and ⟧ and fails on a word outside the marks, and another reads the source for plain English given to a widget. Not yet translated: the text of errors that come from the libraries (a file that cannot be read, a document the editor refuses), which is shown in English after a translated sentence, and the command line and MCP server, which speak English. The translations were written by the maintainers' tooling and want a native reader's review. Japanese, Korean and Chinese need fonts with their characters, so the interface carries Noto Sans CJK cut down to the national character set of each (about 3.5 MB together, `scripts/fonts/subset_cjk.py`) and a test checks every character in a translation is in its font; the language list draws each name in its own font, so it reads right whatever the interface speaks.

A selected Graphic instance has a Looping section in Properties, as Animate's does: Options (Loop, Play Once, Single Frame, Loop Reverse or Play Once Reverse; the last two count down from the First frame, wrapping or holding the symbol's first frame) and the First frame it starts on, counted from 1. Each change is one undo step. A Movie Clip, a Button, artwork or several selected things show no such section, because only a Graphic follows the setting (a Movie Clip loops from where its instance starts; a Button shows its Up state).

The Timeline's frames are small fixed cells (10 points at the normal zoom), not the panel's width shared among them: the scene's frames are followed by empty cells to the panel's edge, and on to the end of the longest sound, so a sound longer than the scene can be scrolled to whole and its frames clicked. The layer names stay at the left while the frames scroll sideways, and the ruler stays at the top while the layers scroll down. The frame rate beside the frame field is a number field, as in Animate: drag it or click and type a rate from 0.01 to 240 frames a second (29.97 and 12.5 too), and it applies when let go or on Enter, as one undo step; the same field is in Properties > Doc, and both are dimmed while a symbol is being edited, since the rate belongs to the whole document. The edge between the names and the frames drags sideways to make the names column wider or narrower (120 to 600 points, 230 to start), and the width is kept with the preferences. The timeline's toolbar starts with what is done to layers, over their names as in Animate: New Layer, New Folder, the Folder menu, Motion Guide, Mask and Delete; the playback buttons, the frame field, the frame rate and the rest start over the frames, or just after the layer buttons when the column is narrower than they are. A sound's bar shows its waveform, and moving the paused playhead plays the stretch of sound under it. The zoom (Ctrl/Cmd and the wheel, or the buttons) widens or narrows them from 3 to 40 points, and Fit sets the zoom at which the scene fills the panel. Onion skin is the first of the two onion buttons in the timeline's toolbar; the second shows the ghosts as outlines, the two fields beside them are how many frames before and after are shown (up to 12 each), and the arrow after them opens its settings: Frames before and Frames after as sliders, Outlines, Tinted, the colors of earlier and later frames, and Reset colors. Tinted, as Animate's onion skin is by default, draws earlier frames in blue and later ones in green (lines in the color, fills in a pale shade of it, so the drawing still reads); off, each ghost is a faded copy in its own colors. While onion skin is on, the ruler shows its range as a band from the earliest ghost to the latest with a handle at each end in that side's color, and dragging a handle sets that side's range without moving the playhead. The range, outlines, tint and colors are kept with the preferences; whether onion skin is on is not, so a session starts with it off. Onion skin is a view of the stage and is never exported. While Loop playback is on, a bar along the top of the ruler shows the loop range, as Animate's does: faint over the whole scene until one of its handles is dragged, then strong over the frames that play round and round, so one stretch of a long scene can be watched while it is worked on. A playhead before the range plays on into it, and one after it goes to its start. A double click on the bar loops the whole scene again. The range belongs to the open document's tab, is cut to the scene if the scene gets shorter, and is never saved (`flare_editor::loop_range`). Edit Multiple Frames (the button after the onion buttons, View > Edit Multiple Frames, Alt+Shift+E as in Animate 2024) shows every keyframe in a range at full strength in its own colors and lets them be edited together, so a whole animation can be moved, scaled or turned and stay the same from frame to frame. Its bracket runs along the bottom of the ruler, over the whole scene until an end is dragged; a double click on it takes in the whole scene again. While it is on, a click picks what is under the pointer on the playhead's frame and then in the other keyframes shown (nearest first), and Select All and the Lasso take from all of them, on shown, unlocked layers; Move, the arrow keys and Free Transform then change them all as one undo step. Onion skin rests meanwhile, its markers hidden. At most 120 keyframes are drawn at once (`flare_tools::multi_frame`). The Selection tool has no drag-a-box marquee yet, so Select All or the Lasso takes everything. The Selection and Free Transform tools pick a picture (a bitmap) anywhere in its rectangle when no shape is under the pointer, so it can be moved, scaled and turned; the tools that paint, erase or edit points keep to shapes.

A click on any cell, an empty one past the layer's end too, makes its layer the active one, puts
the playhead on it and shows it selected in Animate's blue (as are frames selected with Shift); the
frame field in the toolbar follows the playhead there rather than pulling it back into the scene. The timeline's toolbar has the frame commands used most, after the frame number and rate: Insert Keyframe (F6), Insert Blank Keyframe (F7), Insert Frame (F5) and Remove Frames (Shift+F5). They are the commands of the Insert menu and of the frames' own menu, act on the active layer at the playhead (Insert Frame at an empty cell past a layer's end holds its last keyframe out to it; on a layer with no frames yet, such as a sound's, it makes blank frames up to the playhead, as Animate does), say their name and keys when pointed at, and are dimmed on a locked layer. In the Timeline each frame is a cell: a line between frames, every fifth frame (5, 10, 15) shaded a tone apart on empty cells and on keyframes alike so they can be counted, a dot on the first frame of a keyframe (hollow when the keyframe holds nothing) and a box on the last frame of a held keyframe, as Animate draws them. The ruler numbers frame 1 and then every fifth frame, every tenth or further apart when the frames are too narrow for that, with a taller tick on each fifth frame. A drawing's keyframes are a light grey, an instance's a cool grey, a picture's a warm one, a motion tween soft blue and a shape tween soft green, so the black marks read on either theme; the active layer's row is tinted only faintly over its frames. Dragging a keyframe by its body moves it, along its row or onto another layer's row, and dropping it over frames that hold something replaces them: a keyframe it covers goes, one it cuts keeps the frames either side, and the frames it left are empty, as one undo step; dragging its right edge lengthens or shortens it, and a length that would run into the next keyframe is refused. A locked layer neither gives nor takes one. Dragging a layer by its name moves it up or down the stack: a line shows where it will land, a folder takes everything in it along, and a drop just under an open folder puts the layer in it while a drop under a folder's last layer takes it out, as one undo step. A right click on the stage picks the shape under the pointer (unless it is already selected) and opens a menu with Cut, Copy, Paste, Paste in Place, Duplicate, Delete, Group, Ungroup, Break Apart, Convert to Symbol, Select All, Deselect All, Undo, Redo and the zoom commands; on empty stage it clears the selection first. In the Timeline a layer is renamed by double clicking its name, which becomes a text box (Enter keeps the name, Escape drops it, an empty name is refused). A right click on a layer's name opens its menu: Rename, Hide or Show, Lock or Unlock, Transparent and an Opacity slider, Show All, Lock Others, Hide Others, Duplicate Layer, Convert Layers to Symbol, Mask, Motion guide, Move up and down, New layer, New folder and Delete. Lock Others and Hide Others leave the layer, the folders it is in and (for a folder) what it holds as they need to be to work on it, and each is one undo step, or none when nothing changes; Duplicate Layer puts a copy (`Layer 1 copy`, then `copy 2`) with its own elements just above and makes it the active layer, a folder with everything in it; the camera's layer and a layer of bones are not duplicated (`flare_editor::layer_ops`). Convert Layers to Symbol moves the layer (a folder with everything in it), frames, tweens and folders as they are, into a new Graphic symbol named after it (`Layer 1`, or `Layer 1 2` when that is taken; rename it in the Library), and one layer of that name takes its place with an instance of the symbol from the first frame to the layers' last, so nothing moves on the stage; their sounds stay in the scene on that layer (`flare_editor::layers_to_symbol`). To convert several layers at once, put them in a folder first: the timeline selects one layer at a time. A transparent layer, as in Animate's Layer Properties, is one the stage draws see-through so a rough under it can be drawn over: Shift with a click on its eye, or Transparent in its menu, sets it to 50%, and the menu's Opacity slider sets any amount (applied when let go, one undo step). Its eye is drawn half bright. The layer is faded as one sheet (its strokes do not show its own fills through them), a transparent folder fades what it holds, and a symbol inside fades with it; exports, Test Movie and published files always draw it whole. It is kept in a `.flare` project; XFL has no place for it, so a `.fla` save says so and the layer opens opaque (`Layer::view_opacity`, `flare_render::render_stage_viewport`). There is no rename box in the timeline's toolbar. A right click on the frames picks the frame under it (unless it is inside the frames already selected) and opens the menu for frames: Insert Frame, Insert Keyframe, Insert Blank Keyframe, Clear Keyframe, Remove Frames, Create Motion Tween, Create Shape Tween, Cut, Copy, Paste, Select All and Reverse Frames. Cut Frames takes the frames out, so what follows moves left; Select All Frames selects the active layer's frames (the selection holds one layer); Reverse Frames needs whole keyframes selected.

## Documents and tabs

A window holds several documents as tabs above the stage; when there are more than fit the strip scrolls (the wheel turns it, and the tab you switch to or make is brought into view) and the new-document button stays beside it. One is edited; the others are parked whole and swapped in when their tab is chosen, so each keeps its own history, selection and view. The person's tool, colors, settings and panel layout stay with the window, not the document. Open (which takes a Flare project, a `.fla`, an `.xfl` or an XFL folder's `DOMDocument.xml`, and a `.swf`, as Animate's does for its own files: Animate's Import is for pictures, sound and video) and New open in a tab of their own, except that a blank untouched document gives way to what is opened. Revert reloads in the same tab, from the file the document lives in (the `.fla` too). Closing a tab with unsaved work asks first, in a box over the window: Save (writes the document in its own format, asking where for an untitled one, and closes the tab once it is on disk; if the place is not chosen the tab stays), Don't Save (closes it and drops its recovery copy) or Cancel (Escape does the same). Closing the window asks about every tab. Autosave and crash recovery cover only the document on show for now.

Save formats, as in Animate: an animator can work without ever making a `.flare` and send the file to somebody who uses Animate. **File > Save As** offers Flash Document (`.fla`, first and the default), uncompressed Flash Document (`.xfl`) and Flare project (`.flare`); the ending of the name decides the format, in any case, and only the last ending counts (`a.flare.fla` is a `.fla`). A name with no ending, or an ending that is none of the three (`drawing.v2`, `movie.swf`), gets `.fla` added. The dialog starts from the document's own folder and name, or `Untitled.fla`. An `.xfl` is a folder, as in Animate: saving `dir/a.xfl` writes the folder `dir/a/` holding `DOMDocument.xml`, `LIBRARY/` and the stub `a.xfl`, and the document is then `dir/a/a.xfl` (a folder that came with another stub name keeps it); a folder that is already an XFL document is replaced in place, and any other existing folder of that name is refused. **File > Save** writes the document back to the file it lives in: a document opened from `x.fla`, `x.xfl` (or its folder, or its `DOMDocument.xml`, which all name the same stub) is saved back to it with no `.flare` made; one opened from `x.flare` stays a project; a new document asks Save As. Opening a `.fla` or `.xfl` does not make the document unsaved; the window title and tab show the file's real name and ending (`x.fla`), with `*` when unsaved, and the Recent list holds `.fla` and `.xfl` paths, which reopen through the normal Open. The file is replaced only by a whole new one written beside it (a read-only file, a full disk or a folder that cannot be written leaves the old file as it was, says why in the status line and keeps the document unsaved), and the work is done off the UI thread. A movie (`.swf`) or an Animate file Flare could not keep whole (a pre-CS5 binary `.fla`, one over the size cap) opens as an unsaved project and asks Save As; a pre-CS5 `.fla` is never overwritten. Autosave writes the same recovery snapshot (a `.flare`-format file in the recovery folder) for any document, and a recovered document is unsaved work with no file until it is saved. After a save to `.fla` or `.xfl` the status line says what was written, and the Output panel lists it: `Saved x.fla (unchanged original)`, or `Saved x.fla (3 of 14 files rebuilt, the rest carried over as they were)` with the names of the rebuilt files, plus, when the format cannot hold something Flare has (cubic curves approximated, sounds not written, a translucent background written opaque), the count of such notes and each one. In the browser build Save and Save As are a download with a name prompt (the last name offered again), and Publish downloads each format under the name of the file last opened or saved; `.fla` is the default, `.flare` is a choice, and `.xfl` is saved as a `.fla`, which holds the same files.

## Pasteboard

As in Animate, the stage is only the part of the scene that is published. Around it is the
pasteboard, where artwork can be drawn, moved, selected and picked from like any other: a shape
that crosses the stage edge shows whole, and one wholly off the stage stays in the scene. The
editor renders everything on screen over nothing, and paints the stage color under that picture
on the stage alone, so the pasteboard keeps the panel's own grey (`workspace/render.rs`). The
pasteboard reaches twice the stage's longer side past each edge (`flare_ui::view::pasteboard`);
the pointer stops there, so nothing is drawn where it could never be seen. Publishing, export,
Test Movie and thumbnails still show the stage only. View > Hide Pasteboard (Animate's View >
Pasteboard, Cmd/Ctrl+Shift+W, and the dashed-frame button in the stage bar) cuts the view to the
stage to show what will publish; looking through the camera does the same, since the camera sees
only its frame. The Eyedropper takes the stage color only on the stage; on bare pasteboard it takes
nothing.

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
and a close button on the document tab.

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

The Rotation tool (under the Hand, as in Animate; key D) turns the view of the stage, never the
document: drag on the stage to turn it about the panel's center (Shift: in steps of 15 degrees),
double-click to set it upright. Tapping Alt alone (Option on a Mac) picks it and shows its dial at
the foot of the stage: drag the needle to point where the top of the stage should be, step 15
degrees either way, type an angle, or set it upright; a second tap, Escape or another tool puts
the dial away and the tool held before back in hand. A turn is a quick look, as with Animate's
spring-loaded tools: once a drag on the stage is let go, or a double-click has set it upright, the
tool held before the Rotation tool is back in hand (the dial closes with it); a click that turns
nothing leaves the Rotation tool in hand. A tap is a press and release within half a
second with nothing else pressed (`flare_ui::tap::ModifierTap`), so Alt-click, Alt-drag and Alt
chords keep their meaning. While the view is turned the stage bar shows the angle (a click sets it
upright), the grid and guides turn with the stage, the rulers stay along the screen without
numbers that would no longer line up, and guides are not dragged out of or along them; the text
being typed turns with the stage while its invisible field stays upright.

Help > Keyboard Shortcuts has a search field over the list: words find commands by their name in
the interface language or their id (`save as`), and keys find what they run (`cmd+s`, `⌘⇧S`,
`shift+f5`, or a modifier's name such as `option`). The matching is `flare_menubar::ShortcutQuery`,
which the command line and MCP server use too. Keys another command already has are not refused:
Change (or Reset, for the set's own keys) takes them from that command, which is left with no
shortcut until it is given one, and the editor says which command lost them
(`ShortcutMap::bind_taking`, `reset_taking`). So the map still never holds two commands on one key.
Help > Find a Command (Cmd/Ctrl+Shift+P, a key Animate leaves free) is a command palette, which
Animate does not have: a field near the top of the window that finds any menu command, with keys
or without, by its name in the interface's language, its id or its keys, as the shortcut list's
search does (`ShortcutMap::find_commands`). Up to 12 are listed with their keys; the arrows choose
one and Enter or a click runs it. A command that cannot run now is listed dimmed and does nothing.
Escape or a click outside closes it. It only reaches commands the menus have.

Import from Animate… reads Animate's own shortcut file (`Shortcuts/<set>.xml` in Animate's
settings folder, at most 512 KiB) onto Animate's set: each binding of a command Flare has is
taken over, taking the keys from any command that had them, and the rest (a command Flare does
not have, a second key for one command, a key code not yet matched against Animate's menus, a
modifier not understood) is listed in Output with the reason. A file that is not XML, declares a
document type, nests too deeply or holds more than 4096 bindings changes nothing.

Over the stage the pointer is the tool in hand (`workspace/tools/stage_pointer.rs`). The Brush, the
Pencil and the rubbing Eraser show only their tip: the outline of what they lay down, at its size,
never smaller than a few screen points. A tool whose icon has a point where it acts (Paint Bucket, Ink
Bottle, Eyedropper, Pen, Subselection, Hand, Zoom) is drawn as the pointer with that point on the
spot. Every other tool shows a fine cross on the spot with its icon beside it, so the icon never
covers where a click lands. The Selection tool keeps the system arrow, which is its icon. A pointer
the stage chose for what a press would do (a guide's grip, a Free Transform handle) wins over the
tool's.

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

## Icon strip

The narrow bar at the right edge is Animate's icon panel strip. It holds an icon for each panel
kept there, by default Layers, Color, Swatches, Align, Transform, Info, History, Motion Editor, Find and
Replace and Mannequin. A click opens that panel beside the strip, 300 px wide (its left edge drags
it wider or narrower) and level with its icon, over the stage, so Properties stays in view while a color is picked; a second click on the icon, Escape, or
a press on the stage or another panel puts it away (a list or picker the panel opened does not).
Its title row has two buttons: dock it, which moves it into the stack on the right, in front, and
collapse it. A docked panel goes the other way from the collapse button at the right of its
stack's tabs, or Collapse to icon on its tab's right-click menu; it is added at the end of the
strip. The tool strip, the stage, the timeline and Output always stay docked. Where each panel is
(docked or in the strip, and the strip's order) is saved with the layout (`flare_dock::Layout`),
and a layout file that has a panel twice, the stage in the strip, or no stage, timeline, output or
tool strip is refused; a panel added to Flare after the file was saved is put at the end of its
strip. The
strip is not shown on Home.

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
holds Subselection, Free Transform holds Gradient Transform, Lasso holds Polygon (click the corners of a loop; a double click, Enter or a click on the first corner closes it and selects what it encloses) and Magic Wand (see Magic Wand above),
Rectangle holds Oval, Rectangle Primitive, Oval Primitive and PolyStar, Pen holds Add, Delete and
Convert Anchor Point, Pencil holds Paint Brush, Paint Bucket holds Ink Bottle, and Hand holds Rotation and Time scrub. Press and hold a
slot, or right-click it, to open its list; picking a tool from the list makes it the one the cell
shows, and keys that pick a tool (A for Subselection, O for Oval) change what its cell shows too.
The "…" button (More tools) opens a list of the tools that are not in the strip:
Flare's own Mirror and Ruler, Smudge and Blur (see Blur and Smudge above), the Camera and the Bone tool, and 3D Rotation, 3D Translation, Deco, Bind and
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

The Properties panel follows the selection, as Animate's does, and has Animate's four tabs over it:
Tool, Object, Frame and Doc. The tab it chose is raised; a click on another shows that one until the
selection or the tool changes, and Object is dimmed with nothing selected. A tool's amounts (brush
size, smoothing, stabilization, tapers, angle, flatness, pressure response, light touch, stroke
width, corner radius, sides and depth) are Animate's slider rows: the name at the left and the value
at the right on one line, a slider the panel's width below it; the value is also a number field to
type or drag, and sizes slide on a logarithmic track so the small ones get most of it. The Tool tab shows only the paint a tool uses: the Paint Bucket its fill (and gradient) with Gap size, Animate's Don't close gaps, Close small gaps, Close medium gaps and Close large gaps (10, 24 and 48 screen points, so the same-looking gap closes at any zoom; small is the default); the Ink Bottle its stroke color, width, cap, join and style; a shape tool both. Something selected on the stage: a shape shows Position and size and its Fill and stroke
(fill color, outline on or off, stroke color, width, cap and join), which are changed on the shape,
a color when the picker is let go and a number when it is, each one undo step; a symbol instance
shows its symbol and type, Edit Symbol, Position and size, Looping for a Graphic, and Color Effect,
Display and Filters dimmed until they exist; several things show their shared Position and size.
Nothing selected but frames: the frame, with its tween and Ease. Nothing at all selected: the
settings of the tool in hand if it has any (the drawing tools), else the document (size, frame
rate, background, Publish Settings). Modify > Document asks for the document until the selection or
the tool changes.

## Dialogs

Every dialog is drawn with `flare_ui::dialog`: a card in the middle of the window over a dimmed
backdrop, with 24 points of room at the sides and 20 above and below (egui's own popup frame is a
menu's, too tight for a question), a bold title, labelled rows whose controls line up in one
column, and the buttons at the bottom right, at least 88 by 28, the one that does the thing filled
in the accent colour and rightmost. A choice that gives work up stands apart at the bottom left: the
question a close of unsaved work asks says what is lost ("Your changes will be lost if you don't
save them."), with Don't Save at the left and Cancel and Save at the right; Return saves, Escape
cancels.

**Insert > New Symbol** (Ctrl+F8), the Library's New Symbol button, **Modify > Convert to Symbol**
(F8) and the stage menu's Convert to Symbol ask first, in Animate's Create New Symbol / Convert to
Symbol dialog: Name (the first free "Symbol n", selected so typing replaces it), Type as three
cards with their icons (Movie Clip, Button, Graphic, in Animate's order), Folder (Library root,
dimmed until the library has folders) and, when converting, Registration, Animate's grid of nine
points: the symbol's own origin goes on that point of the artwork's box and the instance stands
there, so nothing moves on the stage. A name that is blank, too long or already in the library is
explained under the field and dims OK. OK or Return makes the symbol as one undo step; a new symbol
then opens for drawing in it, as in Animate, and a converted one stays selected on the stage. The
next dialog starts on the type and registration chosen last; a dialog given up keeps nothing.

## New Document

File > New (Cmd/Ctrl+N) and Home's New File open the New Document dialog, laid out as Animate's: the
kinds of project as tabs across the top, each an icon over its name (Character Animation, Social,
Game, Education, Ads, Web and Advanced), the chosen kind's sizes as cards on the left under
"Presets (n)", each drawn with its stage's proportions and its size and rate, and Details on the
right: Width, Height, Units (pixels, the only unit a stage has, so the list is dimmed), Frame rate
(up to 240) and Background color, then Create, which makes the project (in a tab of its own when the
open document has work in it), and Cancel. It is drawn as Flare's other dialogs are (Preferences,
Publish): in the middle of the window over a dimmed backdrop, with its title and an X at the top, and
the sizes are Home's own cards under Home's headings. Escape, the X and a click outside cancel too. Choosing a card fills
the details; editing them leaves the card. The sizes are the usual ones of each medium, Flare's own
list. The + by the tabs still makes a blank tab at once.

## Home

A plain launch opens on the Home page, laid out like Animate's: a sidebar with the app icon, name and
version, Home (and Learn, dimmed: not built), and the buttons New File, Open and Import; in the main
area **Create new** (cards for Character Animation 1920 x 1080 at 24 fps, Social 1080 x 1080 at 30,
Portrait 1080 x 1920 at 30, Web 1280 x 720 at 24, Classic 550 x 400 at 24, each drawn as a grey frame with the
proportions of its stage (the New Document dialog draws its presets the same way), and **Custom Size**, a plus card that opens the New Document dialog on
Advanced with a size of the person's own; it goes away if a tab is chosen over Home), then **Recent**: a list, as Animate's, of up to twelve projects
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
between panels and around the dock is 3 px (`theme::GUTTER`). Between widgets: 6 by 4 px, with
8 by 3 px of padding inside buttons and 22 px rows. These are set in one place, `theme::apply`.

## Eraser

The **Eraser** (E) rubs out what it is dragged over as it goes: fills lose the area it covered and
lines lose the stretches it crossed, on every shown, unlocked layer at the current frame, while the
pointer moves, and the whole drag is one undo step. Escape (or anything else that cancels the drag)
puts back what it rubbed out. A click rubs out a dot. **Size** (1 to 400 px) is how wide it rubs;
a round outline of that size follows the pointer. A straightened drag (Straight, or Shift held when
it starts) shows the area it will rub out see-through and rubs it out on release. Pen pressure
does not change it. With **Faucet** on, a click deletes the whole shape under it instead, which is
what the Eraser did before it could rub. Symmetry and Straight (Shift) apply as they do to the
Brush. Symbols, bitmaps and text are left whole.

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

## Swatches

The Swatches panel (Window > Swatches, in the icon strip by default) is Animate's: a row of
twelve greys from black to white, the 216 web-safe colors in the classic twelve columns (the red
steps in pairs side by side, green down the rows, blue across; `flare_core::swatches`), and Your
swatches. A click on a cell makes it the color the Color panel is setting (fill, stroke or the
gradient's end), and pointing at one shows its hex value. The + keeps the color being set as one
of the person's own (each color once, at most 120; a full list says so), and a right click on one
of them deletes it. They are kept with the preferences, not in the document. The web-safe colors
are the web's standard palette, not Animate's artwork.

## Mannequin

The Mannequin panel (Window > Mannequin, in the icon strip by default) is a 3D figure to pose and
draw from, as Clip Studio's 3D drawing figures are: a smooth male or female body, pale grey and
lit from the upper left, with guide lines round and along it (the lines down the middle of the
front and back in orange), an outline, and its shadow on a floor grid. Posing works as it does
with such a figure. A drag on any part pulls it and the body follows: a hand, forearm, foot or
shin brings the arm or leg with it, an upper arm, thigh, the head or the chest swings about its
joint, and the hips move the whole body. Pinned hands and feet stay where they are (both feet
start pinned, so the hips dragged down crouch over them). A right click on a hand or foot, or the
Pinned toggles under the view, pins or frees it. A click picks a part, tints it, and shows three
rings round its joint (bend red, twist green, side blue, the half toward the viewer bold); a drag
along a ring turns the joint about that axis alone. With Alt a drag turns a part about the line of
sight. A drag anywhere else turns the view round the figure, a right or middle drag (or Shift)
moves it across, and scrolling zooms. Every joint stops where a body does: a knee bends one way, a
neck turns so far. Above the view: Reset view, guide lines on or off, pin both feet, and at the
right the expand button, which makes the panel wide and the view tall (again to put it back). A
grip under the view drags it taller or shorter (a double click puts it back). Under the view: the
body, the built-in poses (Stand, T-Pose, A-Pose, Walk, Run, Sit, Crouch, Kneel, Jump, Reach Up,
Wave), Mirror and Reset; Pinned; Bend, Twist and Side sliders for the picked joint (and Raise for
the whole body); Proportions (head, shoulders, hips, arms, legs, torso and build); and View (turn,
tilt, perspective, zoom, and Front, Side and Back).

On the Stage shows the figure over the stage, see-through, at a place and height of its own (the
middle of the stage, four fifths of its height, to start); it is only drawn on the screen and is
never in the document or an export. Place on a Layer puts it, shaded as the view shows it, as a
picture where Place Into says, as one undo step: New layer, a layer named Mannequin at the top of
the scene from the playhead to the end, drawn at half opacity while editing; This keyframe, the
active layer's keyframe at the playhead beside what it holds, so several figures share the same
frames; or New keyframe here, a keyframe of its own on the active layer from the playhead (the one
shown there is cut short), so each stretch of frames of one layer can hold another pose. Where the
layer shows nothing at the playhead either of the last two starts a keyframe up to the layer's
next one; a locked or hidden layer, a folder, mask, guide or camera layer is refused with the
reason. The same pose placed again shows the picture the library already has. The placed figure
is selected with Free Transform: drag it to move it and its handles to scale, turn or skew it, as
any picture (the Selection tool picks it again anywhere in its rectangle; lock the layer to draw
over it without picking it). It is artwork like any other, so it is hidden or deleted before
export. The figure is kept with the preferences, not in the document.

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
