# Adobe Animate UI reference for minnamate — part 2: interaction

Companion to `animate-ui.md` (part 1: layout, theme, panels). Part 2 covers what happens while you work: selection and transform feedback on the stage, timeline gestures and menus, colour controls and tool properties, and the path from the Home screen through New Document to Document Settings.

Target: Animate 2024 (v24.x) on Windows, Essentials workspace, default Dark theme, 100% UI scale. 24.0 was Animate's last feature release, so 24.x is the final UI to match (part 1). Sizes are logical px at 100%; double them at 200%.

## Build order

Ranked by how much each item affects day-to-day resemblance.

1. **Stage selection look and Selection-tool rules** (§1.1, §1.3, §1.4): white dots on raw shapes, 1 px highlight boxes per object type, no handles, click / double-click / drag behaviour.
2. **Pointer feedback** (§1.7): Selection-tool badges, drawing crosshairs, Hand and Zoom. It changes on every mouse move.
3. **Timeline frame selection and dragging** (§2.1, §2.2): click, Shift, Ctrl and drag rules, the selection tint, the playhead jump, moving, copying and stretching spans.
4. **Layer rows** (§2.4): dot / outline / eye / lock columns, drag-down toggles, Alt-solo, inline rename, drag-reorder.
5. **Context menus** (§2.3, §2.5, §2.8): exact labels, order and Windows shortcuts, greyed-not-hidden items, light native look.
6. **Colour chips and the swatch popup** (§3.1) and the Properties "Color and Style" section (§3.4).
7. **Free Transform** (§1.5): box, 8 handles, pivot circle, hit zones, cursors, modifier keys.
8. **Edit in place** (§1.9): dimming, origin crosshair, edit-bar breadcrumb, every way out.
9. **Onion skin** (§1.10): blue past and green future tints, fading per frame, ruler markers, settings panel.
10. **Drawing tools while dragging** (§1.6, §1.8): Shift and Alt modifiers, arrow-key corner radius, live colour preview, snap ring and Snap Align lines.
11. **Tool tab per tool** (§3.5), then the Color and Swatches panels (§3.2, §3.3).
12. **Home screen and New Document dialog** (§4.1–§4.3), and what Untitled-1 looks like (§4.4).
13. **The rest:** Document Settings (§4.5), Layer Properties (§2.6), the Frame View menu (§2.7), the time-stretch flag (§2.2) and Span Based Selection (§2.1).

## How to read this document

- **Tags** read `[confidence · source]`.
  - **high**: an Adobe help page, a screenshot made or printed by Adobe, or several independent sources that agree.
  - **med**: one good source, or a clear screenshot of a 2019–2023 build that nothing contradicts for 2024.
  - **low**: recall or an estimate. Treat it as a placeholder until it is checked against an Animate 2024 screenshot.
  - A split tag such as `[behaviour: high · SEL; look: low · R]` grades the parts of a claim separately.
- **R** means recall: a researcher's memory, not verified. A few recall items carry [high · R] because the researcher judged the behaviour long-stable since Flash; they are still unverified.
- **P1** means the part-1 research behind `animate-ui.md`. It is cited where it covers the same ground, with its own evidence in brackets.
- **Resolved:** marks a place where sources disagreed. The higher confidence wins. On a tie, newer evidence wins (2024, then 2020–23, then older). Where this document overlaps part 1, it follows part 1 unless its own claim is stronger, so both documents give one answer. §5 lists every resolution.
- **Clean room.** Everything is described in our own words. Adobe text appears only as short UI labels: menu items, dialog titles, buttons, tooltips, preset names. The researchers measured screenshots printed in books and posted in forums but reproduced nothing from them. minnamate draws all its own glyphs and uses its own fonts (Source Sans 3 or Segoe UI instead of the proprietary Adobe Clean).
- **Research limits.**
  - helpx.adobe.com returns HTTP 403 to scripted fetches, so its pages were read as text from Wayback snapshots (Dec 2024 to Jan 2025) or from search-engine extracts.
  - The shared web-search budget ran out before the fact-check passes, so no new searches were possible.
  - Nothing was checked on a v24 install.
  - Screenshot colours are approximate: book figures are CMYK print (about ±10%), compressed forum screenshots about ±6 per channel.
- **Source keys** are listed at the start of each area, with URLs in §6. A few keys name the same page in different areas: LINES = H-draw, DPREF = H-pref, ONION = FBF, H-pi = H-panels, H-layers = LY, H-stage = STAGE, H-wn19 = WN19, H-wn20 = WN20 and H-24 = WN24; TL is the timeline page everywhere. CIB22, CIB23 and CIB24 are the 2022, 2023 and 2024 releases of *Adobe Animate Classroom in a Book*; a `-fig` suffix means a screenshot printed in the book (macOS).

---

## 1. Stage interaction

**Sources for this area.**
- helpx pages, read as text from Wayback snapshots dated Dec 2024 to Jan 2025:
  - **SEL** Selecting objects · **PREF** Set preferences (last updated May 2023) · **DPREF** Drawing preferences · **LINES** Draw lines and shapes · **MODES** Drawing modes · **ARR** Arranging objects · **RESHAPE** Reshape lines and shapes
  - **ONION** Frame-by-frame animation (onion skinning) · **TL** Timeline (modern timeline) · **TF** Transforming and combining graphic objects · **SYM** Symbols · **SNAP** Snapping art into position (May 2023) · **STAGE** Use the Stage and Tools panel · **PEN** Drawing with the Pen tool · **MOVE** Moving and copying objects
  - **WN23 / WN24**: feature summaries for 23.0 (Oct 2022) and 24.0 (Oct 2023).
- **CIB24**: CIB 2024, chapter 2, read in full (Peachpit excerpt, pages 3 to 6).
- **CS5 pN**: the "Using Flash Professional CS5/CS5.5" PDF, page N. It describes legacy behaviour; the visuals may have changed since.
- **FADE**, **BOX**: Adobe community threads, re-read for the fact-check.
- **PP-CS4**: a Peachpit Flash CS4 excerpt whose link now redirects to an article index. Claims that relied on it are re-tagged R.
- Behaviour is well documented; colours and pixel sizes are not. This area's researcher looked at no screenshots, so every [low] value is a placeholder until the checklist in §7.1 is done.

### 1.0 Constants

All overlays are drawn in screen space and never scale with stage zoom.

| Item | Value | Conf |
|---|---|---|
| Raw-shape selection pattern | White dots on a 4×4 px tile aligned to the screen: one #FFFFFF px at (0,0) and one #000000 px at (1,1). The dark pixel keeps the dots visible on white fills. | white dots [high · CIB24]; tile [low · R] |
| Object highlight box | 1 px solid line, no fill | [med · R] |
| Default highlight colour (all five types) | Light blue; placeholder #3FA9F5. theme.rs `SELECTION_BOX` is #1F8FFF; both are placeholders. | [low · R; BOX says "blue"] |
| Outline colour of the first layer | **Resolved:** #00FFFF (cyan). This area recalled #4F80FF from saved XFL [med · R]; P1 found "Layer_1" with color="#00FFFF" in several Animate-saved files and in 2020–24 screenshots [high · P1], and #4F80FF belongs to older files whose first layer is "Layer 1". Always draw the colour stored in the document. | [high · P1] |
| Free Transform box | 1 px #000000 | [med · R] |
| Free Transform handle | 6×6 px solid #000000 square. An optional 1 px #FFFFFF rim is a minnamate choice for visibility on the dark pasteboard, not Animate behaviour. | fill [med · R]; size [low]; rim [our choice] |
| Transformation point | Circle, 8 px diameter, #FFFFFF fill, 1 px #000000 ring | shape [high · CIB24, which calls it a circle icon]; fill [med · R]; size [low] |
| Registration point (instance) | "+" 9×9 px, 1 px #000000 | [low · R] |
| Symbol-edit origin | "+" about 16×16 px, 1 px #000000 | existence [high · SYM]; size [low] |
| Hit tolerance | Set by the Drawing preference "Click accuracy" (default Normal). Placeholders: handles 5 px radius; rotate band 4 to 16 px outside each corner; skew band within 3 px of the edge line. | preference [high · DPREF]; px [low] |
| Marquee | 1 px solid #000000, no fill. theme.rs currently uses `MARQUEE` #7A7A7A, because black vanishes on the dark pasteboard (P1 puts the pasteboard at about #1D1D1D–#252525). Decide after check 4 in §7.1. | [med · R] |
| Lasso path | 1 px #000000 | [med · R] |
| Snap ring | Idle: 7 px diameter, 1 px #000000. Snapped: 14 px diameter, 2 px. | behaviour [high · SNAP, CS5 p128]; sizes [low] |
| Snap Align line | 1 px **dotted** (1 on, 2 off). Colour unverified; use #000000 for now. | dotted [high · SNAP, CS5 p129]; colour [low] |
| Ruler guides | Green; placeholder #33CC66 | green default [med · STAGE, CS5 p17]; hex [low] |
| Edit-in-place dimming | Everything outside the edited symbol or group drawn at about 50% alpha over the stage colour | dims [high · SYM, ARR]; amount [low] |
| Onion tints | Past about #3366FF (blue), future about #33CC33 (green). theme.rs `ONION_BEFORE` #2F6FFF and `ONION_AFTER` #22B14C are close enough. | tints exist [high · ONION]; hues [med · R]; hex [low] |
| Onion opacity | **Resolved:** "Starting opacity" 70%, "Decrease by" 20% per further frame, read off the settings panel in a 2020 Windows screenshot [P1]; this area's 75% / 15% were guesses [low]. A floor of about 10% is our own choice. | controls [high · TL, ONION]; values [high for 2020, med for 2024 · P1]; floor [low] |
| Stage-rotation pivot | Crosshair, plus a red line showing the current view angle | [high · STAGE]; size [low] |

### 1.1 Selection appearance (Selection tool, V)

1. **Raw shapes (Merge Drawing mode).**
   - Selected fills and strokes get the dot overlay; CIB24 says a selected shape is shown with white dots [high · CIB24].
   - The overlay is clipped exactly to the selected area. A marquee or lasso that cuts through a shape selects only the enclosed part, and only that part shows dots [behaviour: high · SEL; look: R].
   - No bounding box and no handles [high · R].
   - The Highlight color preferences don't change it [med · R].
   - For very thin strokes, keep the dotted band at least 3 px wide so a selected hairline stays visible [low · R].
2. **Drawing objects (Object Drawing mode).**
   - A rectangular bounding box in the "Drawing objects" highlight colour [high · MODES, CS5 p93].
   - Also a thin outline along the shape's regions in the layer colour, with small circles at the anchor points. The circles are display-only and can't be dragged with the Selection tool [high · LINES].
   - LINES gives no version, and neither the 23.0 nor the 24.0 feature summary mentions the outline (WN23, WN24). Treat it as current behaviour from an unknown version.
   - No dots. Double-clicking enters the object; the breadcrumb item is "Drawing Object" (§1.9) [med · R].
3. **Primitives (Rectangle Primitive, Oval Primitive).**
   - A box in the "Drawing primitives" colour [med · R].
   - Small round control points you can drag with the Selection tool: corner radius on the rectangle; start angle, end angle and inner radius on the oval [med · R]. The same values are editable in Properties [high · LINES].
4. **Groups.** A box in the "Groups" colour, tight around the contents [med · R].
5. **Symbol instances.**
   - A box in the "Symbols" colour; one forum user describes them as large square blue boxes [med · BOX].
   - The box is the instance's local bounds transformed by its matrix, so a rotated or skewed instance gets a rotated parallelogram [med · R].
   - Inside the box: the transformation point (white circle, at the centre by default) and the registration point ("+" at the symbol origin). When the two coincide, the circle sits on top [med · R].
   - The Selection tool shows no handles; transforming needs the Free Transform tool (Q) [high · BOX, R].
6. **Classic text.**
   - A box in the "Other elements" colour [med · R], plus a small type handle (about 5 px) in one corner.
   - Hollow circle = the field extends as you type. Hollow square = fixed width or height.
   - Corner: static horizontal text = top-right; dynamic and input text = bottom-right; vertical text = bottom-left (right-to-left) or bottom-right (left-to-right).
   - Scrollable dynamic text gets a solid black handle [high · CS5 p272; size low].
7. **Bitmaps, video and components.** A box in the "Other elements" colour [med · R].
8. **Multiple selection.** Each item keeps its own highlight; the Selection tool draws no combined box. The Properties panel shows the combined W/H/X/Y [Properties: high · SEL; visuals: med · R].
9. **Highlight color preferences.**
   - Either "Use layer color", or one swatch each for "Drawing objects", "Drawing primitives", "Groups", "Symbols" and "Other elements", in that order [existence: high · PREF, SEL, CS5 p33, which offers a colour per object type; order: med · R].
   - With "Use layer color", boxes take the layer's outline colour [high · PREF].
   - Location: Edit > Preferences > General in the 2023 docs [PREF]. 24.0 reorganised the dialog; the theme now lives at Edit > Preferences > Edit Preferences > Interface > Color Theme [WN24]. Whether Highlight color moved too is unverified [low].
10. **View > Hide Edges** hides all highlights; the selection itself stays [behaviour: high · SEL]. **Resolved shortcut:** Ctrl+Shift+E [high · P1, Adobe's shortcut table]; this area's recalled Ctrl+H [med · R] loses.
11. **Subselection tool (A).**
    - Clicking a path shows its outline and anchor points in the layer's outline colour [high · PEN, CS5 p107].
    - Corner points are small squares and smooth points are small circles [high · PEN, CS5 p105].
    - "Show Solid Points" (Drawing preferences) swaps solid and hollow, but the current docs contradict each other just as CS5 does (DPREF vs PEN). Implement: preference off (assumed default) = unselected points hollow, selected points solid [med].
    - Tangent handles are thin lines ending in small dots [med · R; MODES describes direction lines ending in direction points].

### 1.2 Hover feedback

- Stage objects get no hover outline or highlight with the Selection or Subselection tools. The only feedback is the cursor badge (§1.7) [med · R; no current doc mentions a hover highlight].
- Documented exceptions:
  - **Width tool (U):** width points and handles appear on the stroke under the pointer, and the pointer shows "+" where a width point can be added [high · LINES, CIB24].
  - **Asset Warp tool:** a badge appears on the pointer over a shape or drawing object. Warp pins are small solid circles: white = open mode, black = fixed mode; Alt+click toggles the mode [high · TF; badge glyph low].
  - **Live Color Preview:** hovering a colour swatch temporarily recolours the selected shape on stage [high · LINES].
  - **Pen tool:** draws a preview segment from the last point to the pointer ("Show Pen Preview") [high · PREF, DPREF].
  - **Timeline:** hovering across the onion-skin span previews the frames as coloured outlines (§1.10) [med · ONION].

### 1.3 Marquee and Lasso

- **Marquee.**
  - Starts only on empty canvas. A drag that starts on an object moves the object; on an unselected stroke or fill edge it bends the edge; near an end or corner point it moves that point (§1.4) [high · R, RESHAPE].
  - Drawn as a 1 px solid black rectangle with no fill, updated live [med · R].
  - On release, raw shapes inside the rectangle are selected and cut at its edge. Groups, instances, text and drawing objects must be fully enclosed unless "Contact-sensitive Selection and Lasso tools" is on (off by default) [high · SEL, DPREF, CS5 p33].
  - Holding Shift adds to the selection [high · SEL].
- **Lasso group.** Since the 2020 toolbar, three separate tools share one slot: "Lasso Tool", "Polygon Tool" and "Magic Wand Tool" [high · SEL]. In CS5, Polygon and Magic Wand were Lasso modifiers.
  - **Resolved keys:** SEL's tool table prints L beside all three. P1 read the shortcut dialog itself as Lasso L, Polygon Shift+L, Magic Wand no key [high · P1]. Use P1's keys.
  - **Lasso:** a freehand 1 px black path follows the pointer. On release it closes with a straight segment and the enclosed area gets dots; double-clicking the start point also closes it [behaviour: high · SEL; look: med]. Alt+click inserts straight segments [high · SEL].
  - **Polygon:** each click adds a vertex, a rubber-band segment runs from the last vertex to the pointer, and a double-click closes the shape [clicks: high · SEL; rubber band: med · R].
  - **Magic Wand:** selects similar colours in a broken-apart bitmap; the result shows dots [behaviour: high · SEL; look: med]. Its Tool tab options are Threshold (1–200) and Smoothing [med · P1].
- **"Shift select" preference** (on by default). On: a click replaces the selection, Shift+click adds, and Shift+click on a selected item removes it. Off: plain clicks add [high · SEL, CS5 p33].

### 1.4 Selection tool behaviour

- **Click.** On a fill: selects that fill. On a stroke: selects that segment. On a group, instance, text block or drawing object: selects the whole object [high · SEL].
- **Double-click.** On a fill: selects the fill plus its strokes. On a stroke: selects all connected strokes [high · SEL, CIB24]. On a group: edits inside it [high · ARR]. On a drawing object: edits inside it [med · R]. On an instance: Edit in Place [high · SYM].
- **Moving.**
  - Drag moves [high · MOVE]. Alt-drag moves a copy [high · CS5 p129]. Shift-drag constrains to 45° [high · MOVE].
  - Arrow keys nudge 1 px and Shift+arrow 10 px; with Snap To Pixels on, the step is a document pixel [high · MOVE].
- **Reshaping.** Dragging an unselected edge bends the curve; dragging near an end or corner point moves that point [high · RESHAPE].
- **New corner point.** helpx says Ctrl-drag on Windows (Option on Mac) [RESHAPE]; CIB24 says Option/Alt-drag. Accept both on Windows [med; the sources conflict].
- **Shortcuts.**
  - Holding Ctrl in any tool gives a temporary Selection tool [high · SEL].
  - Ctrl+A selects all, skipping locked and hidden layers [high · SEL]. Ctrl+Shift+A deselects all [high · SEL].
  - A click on empty canvas deselects [high · R].
  - Clicking a frame in the Timeline selects everything on that layer between its keyframes [high · SEL].

### 1.5 Free Transform tool (Q)

- **Layout.**
  - One box around the whole selection, with its edges initially parallel to the Stage. 8 handles sit on the corners and edge midpoints [high · TF, CS5 p118]. The box is a 1 px black outline and the handles are small solid black squares [colours: med · R; size: low].
  - An instance (probably also a group or text block) that is already rotated or skewed shows its own transformed box [med · R; TF only says the box starts aligned].
  - The transformation point is a circle (CIB24), initially at the object's centre (TF). You can drag it anywhere, even outside the object (CIB24). Double-clicking it puts it back at the centre [high · TF, CIB24].
  - On instances the registration "+" stays visible [med · R].

| Zone | Cursor | Drag does | Modifiers |
|---|---|---|---|
| Inside the box, off the pivot | Arrow + 4-way badge | Move | Shift: 45° [med] |
| On the transformation point | Arrow + small hollow circle badge | Moves the pivot; double-click resets it | — |
| Corner handle | Diagonal double arrow (NW-SE or NE-SW) | Scales both axes; dragging purely horizontally or vertically scales one axis [high · TF] | Shift: keep proportions [high · TF, CIB24]. Alt: origin = transformation point [high · TF, CIB24] |
| Edge handle | Horizontal or vertical double arrow | Scales one axis | Alt: origin = transformation point [high · TF] |
| Just outside a corner | Circular arrow (about 3/4 of a ring with an arrowhead) | Rotates around the pivot | Shift: 45° steps. Alt: rotate around the opposite corner [high · TF] |
| On the edge line between handles | Two parallel half-arrows pointing opposite ways | Skew | Alt: origin = transformation point [high · CS5 p119, where Alt switches the origin for a scale or a skew] |
| Ctrl + a handle (shapes only) | Hollow arrowhead | Distort: a corner moves freely; Ctrl on an edge midpoint moves the whole edge | Shift+Ctrl on a corner: taper [high · TF, CIB24] |

- Cursor glyphs are from recall [med · R]; the zones are documented [high · TF].
- Clicking outside the selection ends the transform [high · TF].
- **Drag feedback (unresolved conflict).** TF and CS5 say the bounding box previews the transformation while you drag. Recall says modern Animate redraws the object live. Implement live redraw plus the box, and verify (§7.1) [doc: high; live: med · R].
- **Option modes.**
  - "Rotate and Skew": corners rotate, edges skew.
  - "Scale": corners keep proportions; Shift makes it non-uniform.
  - "Distort".
  - "Envelope": square anchor points plus round tangent handles.
  - All four are also on Modify > Transform [high · TF]. That submenu also holds Free Transform, Scale and Rotate... (Ctrl+Alt+S), Rotate 90° CW (Ctrl+Shift+9) and CCW (Ctrl+Shift+7), Flip Vertical, Flip Horizontal and Remove Transform (Ctrl+Shift+Z) [high · P1].
  - Distort and Envelope can't modify symbols, primitives, bitmaps, video, sound, gradients, object groups or text [high · TF].
  - Where the four mode buttons sit in 2024, the Tools-panel options area or Properties > Tool, is unverified [low]. P1 notes that the Tools-panel options area holds the modifiers of the current tool [high · P1] and that Free Transform probably has no Tool tab [low · P1], which leans towards the toolbar.
- **Default scale origin (to test).**
  - CS5 p119: shapes, groups and text scale from the point opposite the one you drag; instances scale from the transformation point. The current TF page garbles this sentence.
  - CIB24 (a shape) says Alt scales around the pivot, which implies the opposite corner by default.
  - Recall: instances also scale from the opposite handle.

### 1.6 Drawing tools while dragging

- **Live Color Preview.** Every drawing tool previews with the real stroke and fill while dragging [high · LINES]. Outline-only previews were pre-2014 Flash [R]. The preview draws on top and merges into its layer on release [low · R].
- **Rectangle, Oval and the primitives.**
  - Drag from corner to corner. Shift gives a square or circle [high · LINES]. Alt draws from the centre [high · LINES].
  - Alt+click without dragging opens a settings dialog: width, height, corner radius (rectangle only) and draw from centre [high · LINES]. LINES names it the "Oval and Rectangle Settings" dialog; recall gives per-tool window titles, "Rectangle Settings" and "Oval Settings" [med · R]. Use the per-tool titles until a screenshot settles it (§7.1).
  - During a rectangle drag, the Up and Down arrow keys change the corner radius live [high · LINES, P1].
- **Line.** Shift gives 45° steps [high · LINES]. Alt draws from the centre [low; LINES' Alt note is generic].
- **PolyStar.** The click point is the centre; the drag sets radius and rotation [med · R]. "Options" sets Style (Polygon/Star), Number Of Sides (3 to 32) and Star Point Size (0 to 1) [high · LINES]; since 2020 these sit inline in the Tool tab (§3.5).
- **Snapping while drawing.** With Snap to Objects on, the snap ring appears at the start and end points and grows within "Connect lines" distance of existing points [med · R; distance: high · DPREF].
- **After drawing.** New drawing objects are not left selected [high · LINES]. Merge shapes aren't either [med · R].
- **Object Drawing toggle.**
  - The J key, or the "Object Drawing" button at the bottom of the Tools panel or in the Properties panel [high · CIB24, MODES].
  - Works with Pencil, Line, Pen, Brush, Oval, Rectangle and PolyStar [high · MODES]. Rectangle Primitive and Oval Primitive have no toggle [high · P1].
  - **Resolved glyph:** a 13×13 light square outline holding a light-grey filled circle about 9 px across. Off (Merge Drawing) by default, and the state is remembered between sessions [high · P1]. This area had the glyph at [low].
- **Pencil.**
  - Modes: "Straighten" (also recognises lines and shapes, per the Drawing preferences), "Smooth" and "Ink".
  - Shift constrains to horizontal or vertical [modes and Shift: high · LINES, DPREF].
  - The raw stroke is drawn live and processed on mouse-up [med · R].
- **Brush (B).**
  - Brush Mode options: "Paint Normal", "Paint Fills Only", "Paint Behind", "Paint Selection", "Paint Inside" [high · LINES]. **Resolved label:** "Paint Fills Only", as in the current help and P1's panel notes. A 2020 toolbar capture and older builds read "Paint Fills" [high for 2020 · P1, H-draw]. Check on a 2024 screenshot.
  - Shift constrains to horizontal or vertical [high · LINES].
  - "Zoom size with stage" is **on by default**: LINES says Animate scales the brush with zoom unless you clear it [high · LINES].
  - Brush and Eraser modes sit in the Tools-panel options area and can be given shortcuts [high · STAGE].
- **Eraser (E).**
  - Modes: "Erase Normal", "Erase Fills", "Erase Lines", "Erase Selected Fills", "Erase Inside", plus the Faucet [high · RESHAPE].
  - Double-clicking the Eraser tool erases everything on the stage and pasteboard [high · RESHAPE].

### 1.7 Cursors

Canvas 32×32. The arrow body is about 11×18 px, black with a 1 px white outline. Badges are about 8×8 px, roughly +11,+11 from the hotspot [med · R].

| Tool / context | Glyph | Hotspot | Conf |
|---|---|---|---|
| Selection, idle | Black arrow | Tip | high · R |
| Selection over a movable fill or object | Arrow + 4-way cross with tiny arrowheads | Tip | med · R |
| Selection over an unselected stroke or fill edge | Arrow + small shallow arc | Tip | high · RESHAPE, CIB24 |
| Selection over an end or corner point | Arrow + small right angle (two 6 px lines) | Tip | high · RESHAPE |
| Selection with Alt over a selection | Arrow + small "+" | Tip | low |
| Subselection: idle / over a path / over an anchor | White arrow / + solid 4 px square / + hollow 4 px square | Tip | med · R |
| Free Transform, away from the selection | Arrow (badge unknown) | Tip | low |
| Lasso / Polygon / Magic Wand | Rope loop / polygon loop / wand | Rope end / wand tip | low |
| Pen (P) | Nib, point at top-left. Badges: "x" = initial anchor of a new path [med · R]; none = next point; "+" = add point; "-" = delete point; small circle = close path [high · PEN, CS5 p103]; "/" = continue path [med · R]; "^" = over a smooth point, to convert it or retract its handles [high · PEN]; a join-paths badge [glyph low] | Nib point | as tagged |
| Pen with "Show Precise Cursors" on, or Caps Lock toggled | Crosshair | Centre | high · PEN, CS5 p107 |
| Line, Rectangle, Oval, PolyStar, primitives | Thin "+" crosshair about 15 px, 1 px black | Centre | med · R |
| Text | Crosshair + small "T"; I-beam over existing text | Centre | low |
| Pencil | Pencil, tip at bottom-left | Tip | med · R |
| Brush (B) | Brush tip shape at actual size, filled with the fill colour; scales with zoom while "Zoom size with stage" is on | Centre | low/med · R |
| Paint Brush / Fluid Brush | Circle outline at brush size | Centre | low |
| Width (U) | Base glyph unknown (use an arrow); "+" badge where a width point can be added | Tip | badge: high · CIB24; base: low |
| Asset Warp | Badge over a shape or drawing object | Tip | existence: high · TF; glyph: low |
| Paint Bucket / Ink Bottle | Tilted bucket / tilted bottle, each with a drip | Drip tip | glyph: med; hotspot: low |
| Eyedropper | Dropper, tip at bottom-left. Pencil badge over a stroke, brush badge over a fill. | Tip | med · R |
| Eraser (E) | Outline of the eraser tip at its size; Faucet mode shows a faucet | Centre | med · R |
| Hand (H; or hold Space) | Open hand; closed hand while dragging | Centre | open: high · R; closed: low; Space: high · STAGE |
| Rotation (shares the Hand slot; or hold Shift+Space) | Rotate glyph [low]. On stage, a crosshair pivot (click to move it) with a red line showing the angle; the Center Stage button resets the view. | Pointer | high · STAGE |
| Zoom (Z) | Magnifier with "+", or "-" while Alt is held. Dragging zooms to a rectangle. | Lens centre | behaviour: high · STAGE; glyph: high · R |

### 1.8 Snapping feedback

- **Snap to Objects.**
  - **Resolved location:** not on the toolbar since 2020 [high · P1]. It is a magnet quick action in the Properties Doc tab (since 20.0), View > Snapping > Snap To Objects, and an item on the stage right-click menu [high · P1]. SNAP (2023) still mentions a Snap To Objects modifier for the Selection tool, which is the pre-2020 toolbar magnet.
  - While you drag, a small black ring sits under the pointer and becomes a larger ring once within snapping distance [high · SNAP, CS5 p128].
  - The ring is anchored where you grabbed the object, so grabbing a corner or the centre gives predictable snaps [high · SNAP].
  - Snapping distance = the Drawing preference "Connect lines" [high · SNAP, DPREF].
- **Snap Align.**
  - **Dotted** lines appear along an edge when it sits at the set spacing from another object or the stage border, or when centres line up [high · SNAP].
  - Settings: View > Snapping > "Edit Snapping..." > "Advanced": Movie Border; Horizontal and Vertical spacing; Horizontal and Vertical Center Alignment. A "Save Default" button keeps them [high · SNAP].
  - The doc's example uses 18 px as the default spacing [med · SNAP].
  - Line colour unverified [low].
  - Snap Align is also a Doc-tab quick action [P1].
- **Snap to Pixels.** Shows a pixel grid at 400% zoom and above. C toggles snapping temporarily; X hides the grid temporarily [high · SNAP].
- **Snap Bitmaps to Pixels** (View > Snapping) keeps bitmaps on integer positions. No overlay [high · SNAP].
- **Grid and guides.** Snap to Grid and Snap to Guides exist; guides take precedence where they fall between grid lines [high · STAGE]. Guides are green by default [med · STAGE, CS5 p17; hex low].
- **Smart guides.** Animate has no Illustrator-style smart guides; Snap Align is the closest equivalent [med · R].

### 1.9 Editing symbols and groups

- **Entering Edit in Place.** Double-click the instance, right-click > "Edit in Place", or Edit > "Edit In Place" [high · SYM].
- **What it looks like.**
  - The symbol's content stays full colour, in its on-stage position and transform. Everything else is dimmed and can't be edited [high · SYM, ARR]. Draw the dimmed content at about 50% [low].
  - The fade isn't drawn in View > Preview Mode > "Full"; switching to "Anti-Alias" restores it (the original poster confirmed this) [med · FADE]. The Preview Mode items are Outlines, Fast, Anti-Alias, Anti-Alias Text and Full [high · P1].
  - A crosshair marks the symbol origin [high · SYM].
  - The Timeline switches to the symbol's own timeline [high · R].
- **Edit bar** (the strip at the top of the Stage; part 1 has its full layout).
  - A Back button (left arrow) at the far left [high · SYM].
  - Then the current scene name, then the symbol name to the right of it, then any deeper levels [high · SYM].
  - Each item is a type glyph plus a name [low].
  - **Resolved separator:** follow part 1's ">" (6 px, secondary text colour) [low · P1] so the two documents agree. This area found no documented separator and suggested glyphs plus a 6 to 8 px gap [low]. Check on a screenshot.
  - Clicking an earlier item returns to that level [med · R].
- **Exiting.** Click the Back button; pick the current scene from the Edit bar's Scene menu or click the scene name; use Edit > "Edit Document" (Ctrl+E); or double-click outside the symbol content [high · SYM]. Ctrl+E is [high · P1]: the Edit menu item reads "Edit Symbols" in the document and "Edit Document" inside a symbol (this area had the key at [med · R]).
- **Symbol-editing (isolation) mode.**
  - Enter with Edit > "Edit Symbols", a double-click on the symbol's icon in the Library, or right-click > "Edit" (on the stage or in the Library) [high · SYM].
  - Only the symbol shows, with its registration crosshair; a new symbol's origin is at the centre of the window [high · SYM].
  - No stage rectangle [low · R].
  - Exit the same way as Edit in Place [high · SYM].
- **Edit in New Window** opens a separate window; close it with its Close box to return [high · SYM]. In the tabbed UI it probably appears as another document tab [low · R].
- **Groups** dim the same way. Enter with a double-click or Edit > "Edit Selected"; exit with Edit > "Edit All" or a double-click on a blank spot [high · ARR].
  - The breadcrumb item reads "Group"; drawing objects work the same way with a "Drawing Object" item [med · R].

### 1.10 Onion skin

Merged with P1, which read the timeline toolbar, its menus and the settings panel off screenshots.

- **Button.**
  - **Resolved position:** in the timeline toolbar's centred cluster, first in the group "Onion, Edit Multiple, Tween group, Center Frame". That group follows the "< Keyframe >" group and comes before "Loop, Step back, Play, Step forward" [high for 2020–23, med for 2024 · P1]. This area's "just left of the Loop button" [med · TL] loses.
  - A click toggles it. Press and hold opens a menu: "Selected range", "All frames", "Anchor markers", a separator, then "Advanced Settings" [med · TL, P1].
  - **Resolved label:** the ONION help page spells the last item "Advance Settings" [high that the page does]. A community thread and P1 give "Advanced Settings". Use "Advanced Settings" [med] and check.
  - Toggled on, the button gets a filled rounded chip about 30×30 (#494949) behind the unchanged white glyph; it does not turn blue [med · P1].
  - Toggle shortcut: Ctrl+Alt+O [med · P1, from a 2018 thread]. P1 also notes a conflicting 2018 claim (Alt+Shift+O), so make it rebindable.
- **Settings. Resolved form:** a floating "Onion skin settings" panel, not a modal dialog [high · P1, from a 2020 Windows screenshot; TL]. Contents:
  - a mode dropdown ("Range");
  - a Prev colour swatch with its frame count (2), and a Next colour swatch with its frame count (2);
  - a bracket with a lock that links Prev and Next: this is "Constraints", which keeps the past and future ranges equal [high · TL, ONION];
  - an outline / fill segmented toggle; outline mode is colour-coded too [high · ONION];
  - "Starting opacity" (opacity of the first onion frame on each side), 70%, and "Decrease by" (the drop per further frame), 20%;
  - a keyframes-only toggle (a square with a dot; "Show keyframes only" skips tweened in-between frames) and an "Anchor markers" toggle (an anchor);
  - a reset button.
  - The panel has no Present swatch [P1].
- **On stage.**
  - The current frame draws in full colour [high · ONION].
  - Past frames are tinted blue, future frames green [mechanism: high · ONION; hues: med · R, and a 2020 community post confirms blue and green as the defaults · P1; hex: low]. One 2020 install showed #99FFCD and #00FF00 [low · P1].
  - Each frame fades more the further it is from the playhead [high · ONION].
  - Onion frames draw beneath the current frame [med · R].
  - Whether onion skins show during playback is unverified [low · P1].
- **Colours.** Edit > Preferences > "Onion Skin Color" has Past, Present and Future swatches [high · ONION]. The default Present colour is unknown (possibly red) [low].
- **Editing.**
  - Only the current frame is editable unless "Edit Multiple Frames" is on [high · TL, ONION].
  - **Resolved menu labels:** pressing and holding Edit Multiple Frames shows "Selected range", "All frames" and "Anchor markers", with a check on the current mode [high · P1, from a 2022 screenshot]. The help's "edit selected range" and "edit all frames" are prose. Edit Multiple Frames uses the same marker range as onion skin [med · P1].
  - Locked layers aren't onion-skinned [high · ONION].
- **Timeline header markers.**
  - Start and end markers follow the playhead unless anchored [high · ONION].
  - Drag a marker to resize that side; Ctrl+drag resizes both sides; Shift+drag moves the whole range [high · ONION].
  - Right-click an onion frame in the header to exclude or include it [high · ONION, TL].
  - **Resolved look:** across the ruler's tick and number tiers, the span between the markers is lightened by about 10% white and bounded by two 2 px vertical bars, past colour on the left and future colour on the right [med · P1, from a 2020 screenshot]. This area's bracket handles with a tinted band were [low · R].
  - The playhead is blue [high · TL].
- **Range. Resolved default:** 2 frames before and 2 after [med · P1; the settings panel shows Prev 2 and Next 2]. This area had it at [low].
- **Legacy menu.** The CS5 "Modify Onion Markers" menu ("Always Show Markers", "Anchor Onion", "Onion 2", "Onion 5", "Onion All"; CS5 p235) belongs to the legacy timeline. The modern timeline replaced it (TL), so don't build it. P1 notes that an Adobe staffer mentioned a "Control > Timeline > Modify Markers" submenu in 20.0 [low · P1].

### 1.11 egui notes (egui 0.36.2, checked against the local crate source)

- Paint all overlays after the artwork, in screen space, on top of every layer [layer order: low · R]. Snap 1 px lines to pixel centres with `emath::GuiRounding::round_to_pixel_center(ppp)`.
- **Dot pattern.** A 4×4 texture with `TextureOptions::NEAREST_REPEAT`. In 0.36 the struct has four fields (magnification, minification, wrap_mode, mipmap_mode), so a two-field literal doesn't compile. Draw it on a Mesh using the selected geometry's own triangles, with uv = screen_px / 4.
- **Badged cursors.** Set `CursorIcon::None` over the stage and paint the glyph yourself. Built-in fallbacks: Default, Move, Crosshair, Text, Grab/Grabbing, ZoomIn/ZoomOut, ResizeNwSe/ResizeNeSw/ResizeHorizontal/ResizeVertical, Copy (for Alt-drag duplicate), NotAllowed.

---

## 2. Timeline interactions and menus

**Sources for this area.**
- helpx pages, read by the first draft from 2025 archive copies. The fact-check could not re-read them (helpx returned 403 and the archive was blocked), so their tags carry over:
  - **FK** frames-keyframes · **TL** timeline · **LY** timeline-layers · **SP** setting animation tween spans in the timeline · **CT** classic-tween-animation · **FBF** frame-by-frame-animation · **SND** using-sounds · **BAS** animation-basics
  - **WN19, WN20, WN21**: the What's New pages for 2019, 2020 and 2021.
- **JSFL**: Adobe's JSFL API reference on GitHub (AdobeDocs/developers-animatesdk-docs), Layer_object and Timeline_object pages.
- **CIB22**: CIB 2022, Lesson 3: its text and the screenshots in its sample PDF (Animate 22 on macOS), cited by book page.
- **CIB23**: CIB 2023, Lesson 1, pp. 21–27.
- **SS19**: two screenshots in a community thread from April 2019 (Animate 19, Windows, Dark theme): the layer context menu and the Layer Properties dialog.
- **SS20**: screenshots in two community threads from Nov 2019 and Jan 2020 (Animate 20, Windows, light theme): the Timeline panel menu and the layer header.
- **SS21**: a screenshot in a community thread from May 2021 (Animate 21, Windows), with colours sampled from it. Its researcher took the theme for Darkest, but its #323232 panel is the Dark-theme value (P1: Darkest panels are #1E1E1E), so read its colours as Dark.
- **PH20**: a phone photo in a Jan 2020 community thread showing the drag pointer.
- **COM-\***: text in community threads; URLs in §6.
- Nothing here was checked on a v24 install; the screenshots cover v19 to v22. Part 1 measured the 2024 timeline on Adobe's 2024 press screenshots, and this area follows it where they overlap.

### 2.1 Frame selection

**Model.** Keep a set of selected (layer, frame) cells plus an anchor cell. Keep the active layer (exactly one) separate from the selected layers (any number) [high · LY].

**Frame-based selection is the default.** Span Based Selection is off by default [high · CIB22 p.129, which names frame-based selection as the default; it is also unchecked in SS20].

| Gesture | Result | Tag |
|---|---|---|
| Click a cell | Selects that cell only. Its layer becomes selected and active, and the playhead jumps there. If the layer is unlocked and visible, the Stage selection becomes that layer's objects at that frame. The Properties panel switches to its Frame tab. | [high · FK, LY]; playhead and Stage [high · R]; Frame tab [med · P1] |
| Click a cell past the timeline's last frame | Selects it, but the playhead can't move past the last frame and stays put. With Auto Keyframe on, a blue dot in the ruler marks the selected frame. | [med · P1] |
| Drag starting on an unselected cell | Rectangle from the pressed cell to the pointer, across every layer crossed. Auto-scrolls at the edges. | [high · FK, CIB23]; auto-scroll [med · R] |
| Shift+click a cell outside any tween span | Selects the rectangle from the anchor to the clicked cell. Clicking the same frame on another layer selects that frame on every layer in between. | [high · FK, CIB23] |
| Shift+click a cell inside a tween span | Selects that whole span, even in frame-based mode (CIB22 gives Shift-click as the way to select an entire span). | [high · CIB22 p.129, P1]. How it combines with an existing range is [low]; suggestion: add the span to the selection. |
| Ctrl+click | Adds one cell (non-contiguous). Ctrl+click on a selected cell removes it. | add [high · FK]; remove [med · R] |
| Click a layer name | Selects the layer and all its frames; the whole frame row takes the selection tint. Also selects the layer's Stage objects at the current frame. | [high · LY, SS21]; Stage part [med · R] |
| Click a collapsed folder name | Selects all frames of the layers inside | [med · LY] |
| Double-click a frame | Selects its whole span, from its keyframe to the end of the span | [med · R] |
| Edit > Timeline > Select All Frames, Ctrl+Alt+A | Every frame on every layer | [high · FK]; shortcut [high · CIB22 p.137, where the Mac menu shows ⌥⌘A] |
| Mouse-down on an already-selected cell | Keeps the selection, so a drag moves it. Releasing without dragging does not collapse a whole-layer selection. | [med · COM-keeps-selecting] |
| Right-click | If the cell under the pointer is not selected, select it first, then open the menu | [high · R] |

**Span Based Selection.** A toggle in the Timeline panel menu (§2.7) [high · SS20, CIB22 p.129]; a forum post calls that menu the "Timeline hamburger menu" [COM-selecting]. Before 2020 it was a checkbox in Edit > Preferences > General [high · FK, SP].
- A click selects the span from its keyframe up to the frame before the next keyframe [high · FK].
- Shift+click adds more spans, including non-adjacent ones [high · FK, SP].
- Ctrl+click selects a single keyframe [high · CIB22 p.129].
- Ctrl+Alt+click and Ctrl+Alt+drag select single frames or ranges inside spans [low · SP, pre-2020 wording, unverified].
- A clicked span drags as one unit [high · CIB22].

**Selection look** (Dark theme):
- **Selected cells. Resolved:** blue about #5A8AD0, opaque on empty cells and blended at about 50% over spans. A #909090 static span becomes #7690B3, and a gold motion-tween span turns grey-green [med · P1, from 2023 and 2024 screenshots]. This area modelled it from SS21 as #2A94FF at about 35% (#909090 becomes #6E91B7) [med · SS21, CIB22 p.128]. Both reproduce the observed shift, but only P1's rule covers empty cells, so use it. Keyframe dots and property diamonds stay drawn on top, unchanged.
- **Selected frame block:** a thin (1–2 px) brighter blue outline, about #4A8AD0 [med · CIB22 p.128; colour low]. Its right edge is the stretch handle, which TL describes as the highlighted right edge; near it the pointer becomes ↔ [high · CIB22 p.131].
- **Selected layer's name cell. Resolved:** #40618E with a white name [med · P1, from Adobe's 2024 press screenshots; CIB23 prints #3B5C96]. SS21 (2021) shows #366192 with 1 px dark navy lines (about #182839) above and below the row [med · SS21]. Add the navy lines only if a 2024 screenshot shows them. (An early draft's #4A4A4A was wrong.)
- **Unselected parts** [med · SS21]: name cells #333333, panel #323232, row separators 1 px #252525.
- **Playhead. Resolved:** about #4A80CC [med · P1, from 2024, 2023 and 2020 screenshots]; SS21 sampled about #3479C2 with a darker edge [med · SS21]. CIB22 calls it the blue playhead [high]. Its head is a translucent one-cell tab with no frame number (part 1).

### 2.2 Moving, copying and resizing frames

- **Drag a selection or a keyframe:** the block moves in whole cells and can change layer [high · CIB22, SP].
  - Pointer: the normal arrow with a small dashed-outline square at its lower right, the "tiny box" of CIB22 [high · CIB22 pp.128–129, PH20].
  - A hollow outline of the block follows the pointer [med · R].
- **Drop onto occupied frames:** those frames are consumed (replaced) [high · SP].
- **A tween span dropped on an empty normal layer:** that layer becomes a tween layer [high · SP].
- **Alt+drag** copies instead of moving [high · FK, CT, SP]. A "+" by the pointer [low].
- **Static span length:** Ctrl+drag the span's first or last frame [high · FK].
- **Motion tween span length:**
  - Near either end the pointer becomes ↔; drag to resize [high · CIB22 p.127].
  - Property keyframes are redistributed proportionally, evenly across the new length [high · CIB22].
  - Dragging into another span replaces that span's frames [high · SP].
  - **Shift+drag an end** (the span must not be selected): changes the span's length without moving its property keyframes. The added frames are still tween frames and hold the last keyframe's values. In CIB22's example the last keyframe stays at frame 60 while frames are added through frame 191 [high · CIB22 pp.127–128].
  - Select a frame after the span and press F6: the span extends and gets a property keyframe [high · SP].
- **Adjacent tween spans:** drag the boundary between them to move it. Alt+drag the first frame of the second span to separate them [high · SP].
- **Classic tween length:** drag the start or end keyframe [high · CT].
- **Time-stretch.**
  - Select frames. CIB22 shows a span inside a tween; TL says frames on one or more layers.
  - Hover near the right edge of the selection until ↔ appears, then drag. The contents scale proportionally [high · CIB22 p.131, TL].
  - When the new length is a whole multiple of the original, a flag shows the factor. **Resolved list:** CIB22 lists "x0.5", "x2", "x4" and so on [high · CIB22 p.131]. P1 also writes "x3", which CIB22 doesn't show, so x3, x1.5, x0.25 and any other factor stay [low] until checked.
  - Flag look: a dark rectangle (about #1E1E1E) with white text of about 10 px and about 3 px padding, just above the dragged edge on that layer's row [med · CIB22 p.131 figure; P1 agrees].
- **Locked layers:** their frames can still be selected and moved, because the lock only blocks Stage edits [high · SP].
- **Insert feedback:** a newly inserted keyframe or blank keyframe glows briefly [existence: high · P1, from TL]. Suggested look: a blue halo fading over about 300 ms [low · P1].

### 2.3 Frame context menu (right-click a frame cell)

- **Shortcuts are printed**, right-aligned [high · CIB22 p.137]. The Mac menu in CIB22 shows F5, ⇧F5, ⌥⌘X, ⌥⌘C, ⌥⌘V, ⌥⌫ and ⌥⌘A. On Windows, write them as "Shift+F5", "Ctrl+Alt+X" and so on [med].
- **Disabled items stay in the menu, greyed out** [high · SS19, CIB22, P1].

**Normal frame (no tween).** **Resolved:** the order, labels and shortcut placement below come from a 2020 Windows screenshot read by P1 [high · P1]. This area's draft took its first block and its last three blocks from CS6 recall [med]; where the two differ, the screenshot wins. "---" marks a separator.

```
Create Classic Tween
Create Motion Tween
Create Shape Tween
Convert to Frame-by-Frame Animation   >
---
Insert Frame                          F5
Remove Frames                         Shift+F5
---
Insert Keyframe
Insert Blank Keyframe
Clear Keyframe                        Shift+F6
Convert to Keyframes                  F6
Convert to Blank Keyframes            F7
---
Cut Frames                            Ctrl+Alt+X
Copy Frames                           Ctrl+Alt+C
Paste Frames                          Ctrl+Alt+V
Paste and Overwrite Frames
Clear Frames                          Alt+Backspace
Select All Frames                     Ctrl+Alt+A
---
Copy Motion
Paste Motion
Paste Motion Special...
---
Reverse Frames
Synchronize Symbols
Split Audio
Show In Library
---
Actions                               F9
```

What the screenshot changed against this area's draft:
- The tween items run Classic, Motion, Shape; the draft had Motion first.
- F6 and F7 are printed on "Convert to Keyframes" and "Convert to Blank Keyframes", and Shift+F6 on "Clear Keyframe". "Insert Keyframe" and "Insert Blank Keyframe" print nothing, which matches the 2022 motion-tween menu, where Insert Blank Keyframe has no shortcut either [CIB22].
- "Copy Motion as ActionScript 3.0..." is not in this menu; it lives in Edit > Timeline [P1]. Whether AS3 documents add it here is [low].
- "Split Audio" sits in the Reverse Frames block, and it is enabled only on frames of a Stream-sync sound [high · SND, COM-trim-audio]. "Show In Library" was missing from the draft.
- The label is hyphenated: "Convert to Frame-by-Frame Animation" [high · P1].

**Presence of each item, from this area's sources:**
- Create Motion / Shape / Classic Tween: [high · CT, BAS, CIB22].
- Insert Frame, Remove Frames, Cut / Copy / Paste Frames, Paste and Overwrite Frames, Clear Frames, Select All Frames: [high · CIB22 pp.137 and 152 figures].
- Insert Keyframe, Insert Blank Keyframe, Clear Keyframe: [high · FK, CT, CIB22 p.137].
- Convert to Keyframes, Convert to Blank Keyframes: [med · R; JSFL confirms the commands exist; high · P1 screenshot].
- Copy Motion, Paste Motion: [high · CIB22 p.137]. Paste Motion Special...: [med · R; high · P1]. Both may be hidden or disabled in HTML5 Canvas documents [low].
- Reverse Frames: [high · CT]. Synchronize Symbols: [med · R; high · P1].
- Split Audio: [high · SND, COM-trim-audio]. Actions: [med · R; high · P1].

| Command | Windows shortcut | Tag |
|---|---|---|
| Insert Frame | F5 | [high · FK, CIB22] |
| Remove Frames | Shift+F5 | [high · FK, CIB22] |
| Insert Keyframe / Convert to Keyframes | F6 | [high · FK, CIB22 (Insert Keyframe, or F6), P1] |
| Insert Blank Keyframe / Convert to Blank Keyframes | F7 | [high · P1]; this area had [med · R] |
| Clear Keyframe | Shift+F6 | [high · P1]; this area had [med · R; the CIB22 index lists a shortcut for clearing keyframes] |
| Cut Frames | Ctrl+Alt+X | [high · CIB22 p.137] |
| Copy Frames | Ctrl+Alt+C | [high · CIB22, COM] |
| Paste Frames | Ctrl+Alt+V | [high · CIB22, COM] |
| Paste and Overwrite Frames | none | [high · CIB22] |
| Clear Frames | Alt+Backspace | [high · CIB22 p.137 (⌥⌫)] |
| Select All Frames | Ctrl+Alt+A | [high · CIB22 p.137] |
| Actions panel | F9 | [high · P1]; this area had [med · R] |

**What each command does:**
- **Insert Frame:** inserts N frames (N = the number of selected frames) at the first selected frame on each selected layer; later frames shift right. [high · JSFL] covers one layer; the multi-layer case is [med · R].
  - If the selected cell is a keyframe, the new frames go in after it, so the keyframe stays put [high · JSFL].
  - Used past a layer's end, it fills that layer up to the clicked frame [high · CIB23].
- **Remove Frames:** deletes the selected frames; later frames shift left [high · FK].
- **Insert Keyframe:**
  - On a regular frame, it turns that frame into a keyframe holding a copy of the content shown. The span length does not change [high · JSFL].
  - On a keyframe, the new keyframe goes in the next cell. If that cell is already a keyframe, nothing is inserted and the playhead moves there [high · JSFL].
  - Past the layer's end, it extends the layer first [high · R].
- **Insert Blank Keyframe:** the same placement rules, with empty content [high · JSFL].
- **Clear Keyframe:** turns the keyframe into a regular frame and deletes its content, so the span shows the previous keyframe's content [high · JSFL, CT].
- **Convert to (Blank) Keyframes:** every selected frame becomes a (blank) keyframe [high · JSFL].
- **Cut Frames:** copies, then empties the frames without shifting anything [low · R].
- **Paste Frames:** inserts the copied frames at the selection and shifts later frames right [med · R].
- **Paste and Overwrite Frames:** replaces exactly as many frames as were copied [high · LY].
- **Clear Frames:** removes the content and shifts nothing [med · R]. The range becomes a blank keyframe, and a keyframe is added after it so later frames keep their content. JSFL only says it deletes all the contents.
- **Reverse Frames:** needs a keyframe at both ends of the selection; otherwise it is disabled [high · CT].
- **Split Audio:** splits the stream sound at that frame [high · SND].
- **Actions:** opens the Actions panel on that frame [med · R].
- **Create Motion Tween on a shape, text or group** [high · CIB22 p.123]:
  - An alert explains that the item can't be tweened until it is a symbol, and asks whether to convert it and create the tween.
  - It has a "Don't show again." checkbox and the buttons Cancel and OK. OK is the default and is blue.
  - OK converts the item to a symbol (default name "Symbol 1") and makes the layer a tween layer.

**Variant inside a motion tween span.** Seen in Animate 2022 [CIB22 pp.137 and 152]. The top block is cut off in both figures.

```
Remove Motion Tween              (replaces Create Motion Tween)
...                              (rest of the top block)
---
Insert Frame                     F5
Remove Frames                    Shift+F5
---
Insert Keyframe                >
Insert Blank Keyframe
Clear Keyframe                 >
View Keyframes                 >
---
Cut Frames                       Ctrl+Alt+X
Copy Frames                      Ctrl+Alt+C
Paste Frames                     Ctrl+Alt+V
Paste and Overwrite Frames
Clear Frames                     Alt+Backspace
Select All Frames                Ctrl+Alt+A
---
Copy Motion
Paste Motion
---
Copy Properties
Paste Properties
Paste Properties Special...
---
Split Motion
Join Motions
...                              (rest cut off)
```

- The middle blocks, their order and their labels: [high · CIB22]. This variant has no Convert to Keyframes or Convert to Blank Keyframes [high].
- "Remove Motion Tween" exists [high · CIB22 p.124]; its position is [med].
- **View Keyframes submenu:** Position, Scale, Skew, Rotation, Color, Filter (each checked while shown), a separator, then All and None [high · CIB22 p.137]. Unchecking a type hides its diamonds in the span [high · P1]. (An early draft put All and None first, which was wrong.)
- **Insert Keyframe > and Clear Keyframe >:** the same property names [med · CIB22 text]. Whether "All" appears in them is [low].
- **Split Motion:** right-click a single frame [high · SP, CIB22 p.152].
- **Join Motions:** disabled unless two adjacent spans are selected [high · SP, CIB22 figure].
- **Expected below Join Motions but not visible:** Convert to Frame-by-Frame Animation > [high · SP, FBF]; Save as Motion Preset... [med]; Motion Path > and 3D Tween [low]; Synchronize Symbols and Actions [low].

**Inside classic and shape tweens:** "Remove Classic Tween" and "Remove Shape Tween" replace the matching Create item [low · R].

**Convert to Frame-by-Frame Animation submenu. Resolved:** "Keyframe every frame", "Keyframe every other frame", "Keyframe every third frame", "Keyframe every fourth frame", "Custom..." [high · P1, from a CIB22 figure]. This area had keyframe-interval presets on 1s, 2s and 3s plus a custom interval [med · TL; labels low]. The same submenu is also in the Modify menu [P1; placement low].

### 2.4 Layer interactions

**Header and rows, 2020 and later** [high · SS20, SS21, CIB22; identical in v20, v21 and v22]:
- **Header row above the layer list.**
  - Left, 2020–2023: New Layer (a square outline with a plus), New Folder (a folder) and Delete (a trash can). **Resolved for 2024:** these three moved into the toolbar row above the layer column, and Layer View moved to the left end of the header row [high · P1, from a CIB24 figure and Adobe's 2024 press screenshots].
  - Right, above the four state columns: a small dot, a hollow tall rectangle, an eye with a diagonal slash (tooltip "Show or Hide All Layers" [high · SS21]) and a padlock.
- **Each row, left to right:**
  1. the disclosure triangle (folders only);
  2. the type icon;
  3. the name;
  4. four right-aligned columns: **highlight (•), outline (▯), visibility (eye), lock**. P1 confirms this order from six 2020–24 screenshots. (An early draft's pencil, colour dot, eye, lock, outline order was wrong.)
- **Measurements** at 100% scale:
  - Row pitch is 19 px, including a 1 px separator [med · SS21]. SS21's row-height setting is unknown; v20 set to "Short" also measures 18–19 px. Medium and Tall are in §2.7.
  - **Resolved column pitch:** 25 px, with column centres 26 px (lock), 51 px (eye), 77 px (outline) and 99 px (dot) from the name column's right edge [med · P1, 2024]. SS21 measured about 24 px [med].
  - At top level, the type icon (about 12 px) starts about 28 px from the pane's left edge, and the name starts 24 px after the icon [med · SS21]. P1: the name is about 12 px, #D5D5D5 (white on the selected row), ellipsized before the dot column.
  - Folders: the ▼ (about 8×5 px) is centred about 16 px from the left edge. Children are indented about 16–18 px per level [med · CIB22 figures]; P1 read about 20 px from the same book. Use 16–20 and calibrate.
- **Outline cell:** a filled vertical bar of about 6×12 px in the layer's colour [high · SS21, CIB22, P1]. A hollow bar, outlined in the layer colour, means outline view [med · R; the Flash convention, and the header icon is the hollow form].
- **Visible, unlocked and unhighlighted cells are empty.** No dots are drawn [high · SS21, CIB22]. **Resolved hover state:** hovered or selected rows show a dim (about 35%) eye-with-slash and a dim padlock in those empty cells [med · P1]. CIB23's text talks about clicking dots, but no 2020+ screenshot shows any.
- **Locked:** a small light-grey padlock (about 10×12, about #C8C8C8) in the lock column [high · CIB22 figures, P1].
- **Hidden. Resolved:** the eye-with-slash at full brightness [med · P1; glyph low]. LY's "red X" is Flash-era wording; keep it only as a fallback.
- **No pencil.** No 2020–2024 screenshot draws a pencil on the active layer; the blue row highlight is the only mark of the active layer [high · P1; med · SS20, SS21, CIB22]. LY's pencil text predates the 2020 redesign. Whether a crossed pencil appears when the active layer is locked or hidden is [low].

**Selecting, renaming and reordering:**
- Click a name to select the layer and make it active; this also selects all its frames (§2.1). Shift+click adds a contiguous range; Ctrl+click adds non-contiguous layers [high · LY].
- Double-click a layer or folder name to rename it inline. Enter or a click outside commits [high · LY, CIB23]; Esc cancels [med · R].
- Double-click the type icon to open Layer Properties [high · LY, TL, CIB23].
- Drag layers up or down to reorder; a multi-selection moves together [high · LY, CIB23].
  - Dropping on a folder's name row moves the layers into that folder [high · LY].
  - The insertion marker is a 2 px line across the name pane at the gap. It is indented to the child level when the drop lands inside a folder or under a mask or guide [med · R]. Colour: the accent blue [low]; use the playhead blue (about #4A80CC) so the blues match.
  - Dragging a classic-tween layer under a guide turns the guide into a motion guide and indents the layer [high · CT].
- Dragging layers onto the Delete (trash) button deletes them [high · LY, CIB23, SS21]. In 2024 that button is in the toolbar row; in 2020–2023 it was at the left of the header row.

**Visibility, lock and outline:**
- Click an eye cell to hide or show the layer [high · LY].
- Shift+click an eye cell to make the layer semi-transparent while authoring. It has no effect on hidden layers [high · LY, CIB23]. The glyph for this state is [low].
- Shift+drag down the eye cells to make a range transparent; repeat to restore [med · COM-transparent].
- Click a lock cell to toggle the lock, or an outline cell to toggle outline view [high · LY].
- Dragging down any of these columns applies the toggle to every row crossed [high · LY].
- **Alt+click (solo)** [high · LY]:
  - in the eye column it hides all other layers, and Alt+click again shows everything;
  - in the lock column it locks the others;
  - in the outline column it shows the others as outlines.
- **Header icons:**
  - A click toggles that state for all layers and folders [high · LY].
  - Shift+click on the header eye makes all layers transparent; doing it again restores them [high · COM-transparent, the accepted answer from 2021].
  - The header eye is drawn crossed while every layer is visible [high · SS20, SS21]. Whether it changes when layers are hidden is [low].
- A toggle on a folder applies to its children [high · LY].

**Highlight column (•):**
- Empty at rest. On a hovered or selected row, a filled dot of about 6 px in the layer's colour appears [med · P1].
- A click turns layer highlighting on (P1 calls it pinning): the dot stays, the row is tinted with the layer colour at about 15% over #323232 (a cyan layer gives #3C5051) [med · P1], and an underline in the layer's colour runs across its frame row [high · TL, WN20].
- What clicking the header dot does is [low].

**Folders:**
- The triangle left of the folder icon shows ▼ when expanded and ▶ when collapsed; a click toggles it [high · LY, CIB22 figures].
- Folders nest [high · LY].
- In the frame grid a folder row has no frames and is drawn as a darker hatched strip [med · CIB22 figures]. P1's alternative, empty cells without the every-5th-column highlight, is [low].

**Keyframe navigation:**
- **Resolved position:** "<" and ">" sit either side of the Insert Keyframe group button in the toolbar's centred cluster, after the FPS and current-frame readouts [high for 2020–23 · P1]. This area had them at the right end of the toolbar [med · CIB22 figures pp.121–131, TL]. They act on the active layer.
- No such controls appear on layer rows in any 2020+ screenshot; they moved off the rows in 20.0 [P1].
- Keyboard: Alt+, and Alt+. jump to the previous and next keyframe [high · CIB22]. The Control menu items are "Step Backward To Previous Keyframe" and "Step Forward To Next Keyframe".
- , and . step one frame [high · COM-keeps-selecting]. Shift+, and Shift+. go to the first and last frame [high · P1].

**Creating, copying and deleting:**
- A new layer appears above the selected one, becomes current and is named "Layer_1", "Layer_2" and so on [high · LY, CIB23, JSFL, SS19–21].
- Paste Layers puts the layers above the selected layer, or into the folder when a folder is selected [high · LY, JSFL].
- Duplicate Layers appends "copy" to the name [high · LY]. The exact form ("Layer_1 copy" or "Layer_1_copy") is [low].
- Deleting a folder deletes everything in it [high · LY, JSFL].

**Masks and guides:**
- **Mask** (checkable): the layer becomes a mask, and the layer directly below becomes masked and is indented. Both lock so the effect shows on the Stage [high · R, LY].
- **More masked layers:** drag them into the indented slot under the mask [high · R], or set them to Masked in Layer Properties.
- **Show Masking** locks the mask and masked layers to preview the effect [high · JSFL timeline.showLayerMasking].
- **Guide** (checkable) makes a guide layer, which is not exported [high · LY].
- **Add Classic Motion Guide** inserts a motion-guide layer above the current layer and attaches the current layer to it. The current layer becomes type Guided and is indented. It works only on Normal layers [high · JSFL, CT].

**Type icons, as simple geometry:**
- Normal: a sheet of about 12×12 with one folded corner [high · SS20, SS21]; P1 puts the dog-ear at the lower left [high · P1].
- Motion tween layer. **Resolved:** a light diamond cut by a dark right-pointing arrow, about 16×11, the same as the Create motion tween button glyph [high · P1, from CIB22 and the 2024 screenshots]. This area read the same book as a horizontal double-headed arrow with a small diamond between the heads, about 14×10 [high · CIB22]; P1's reading has the 2024 screenshots behind it. CIB22 describes it as a special icon in front of the layer name.
- Folder: a closed-folder silhouette of about 13×10 [high · CIB22 figures]; P1 suggests it shows open while expanded [med · P1].
- These five are [low], as in the first draft: Mask = a rounded square with a light circular hole; Masked = a sheet with a down-left arrow; Guide = an L-shaped T-square; Motion guide = an arc with a dot; Camera = a body with a lens circle. P1's [low] suggestions differ slightly: masked = a normal sheet indented under its mask; camera = the camcorder glyph.

**XFL attribute names**, from an Animate 16.1 file and not re-checked [med]:
- `current`, `isSelected`, `locked`, `color`, `layerType="folder"`, `parentLayerIndex`, `autoNamed`. `heightLiteral` is [low].
- Outline colours seen there and on screen include #9933CC, #FF800A, #FF4FFF and #009999 [med]; SS21 also shows #00FFFF. The first layer of a new document is #00FFFF [high · P1].

### 2.5 Layer context menu (right-click a layer or folder name)

Exact labels and order as captured in Animate 19 on Windows, April 2019 [high · SS19]. That v24 is unchanged, apart from the 2021 addition, is [med]. **Resolved:** P1's version of this menu came from help prose ("Show Others as Transparent", "Delete Layer", "Delete Folder", "Expand Folder", "Collapse Folder") [med · P1]; the screenshot labels below win. "---" marks a separator.

```
Show All
Lock Others
Hide Others
Show Others Transparent
---
Insert Layer
Delete Layers
---
Cut Layers
Copy Layers
Paste Layers
Duplicate Layers
Merge Layers
Convert Layers to Symbol...      (added in 2021; position [low])
---
Guide
Add Classic Motion Guide
---
Mask
Show Masking
---
Insert Folder
Delete Folders
Expand Folders
Collapse Folders
---
Expand All Folders
Collapse All Folders
---
Properties...
```

- **Labels:** "Show Others Transparent" has no "as"; "Delete Layers", "Delete Folders", "Expand Folders" and "Collapse Folders" are plural [high · SS19]. TL's "Show Others as Transparent" is prose.
- **No shortcut text** appears in this menu [high · SS19].
- **Disabled in SS19**, with one normal layer and nothing copied: Show All, Lock Others, Hide Others, Show Others Transparent, Paste Layers, Merge Layers, Show Masking, Delete / Expand / Collapse Folders and Expand / Collapse All Folders [high · SS19].
- **Implied enable rules** [med]:
  - The "Others" items need another layer.
  - Show All needs a hidden layer.
  - Paste Layers needs copied layers.
  - Merge Layers needs two or more selected layers.
  - Show Masking needs a mask or masked layer.
  - Delete / Expand / Collapse Folders need a selected folder; Expand / Collapse All Folders need any folder.
- **Checkmarks:** Guide and Mask show one when the layer is that type [med · R].
- **Convert Layers to Symbol:** [high · WN21, CIB22 p.151, whose tip says to select the layers, right-click and choose it]. It asks for a symbol name and type [med]. The ellipsis is [low].
- **Behaviour:**
  - Lock Others and Hide Others act on every layer except the clicked one [med].
  - Show All makes every layer visible. In CS6 it also unlocked them [low].
  - Right-clicking an unselected layer selects it first [high · R].

### 2.6 Layer Properties dialog

Captured in Animate 19 on Windows, Dark theme [high · SS19]. That it is unchanged in v24 is [med].

**Opening it:** double-click the layer icon, choose Properties..., or use Modify > Timeline > Layer Properties [high · LY, TL]. With several layers selected, the edit applies to all of them [high · LY, JSFL setLayerProperty].

**Window:**
- About 442×479 px, including a 31 px native white title bar with "Layer Properties" and a close X.
- The body is #535353 in v19's Dark theme. Whether v24 restyled it, for example to the #323232 panel colour, is open (§7.2).
- Labels are right-aligned and end at x ≈ 80. Controls start at x ≈ 88.
- Each label has an underlined access key.

```
      Name: [Layer_1            ]                (    OK    )
            [ ] Lock                             (  Cancel  )
            [ ] Attach To Camera      (greyed when there is no camera)
Visibility: ( ) Visible
            (o) Transparent   50      (scrubbable number, dotted underline)
            ( ) Invisible
      Type: (o) Normal
            ( ) Mask
            ( ) Masked                (greyed unless a mask is directly above)
            ( ) Folder
            ( ) Guide
Outline color: [swatch]
            [ ] View layer as outlines
Layer height: [100%  v]
```

**Row centres**, as y from the window top [high · SS19]:

| Row | y |
|---|---|
| Name | 57 |
| Lock | 84 |
| Attach To Camera | ≈118 |
| Visible | 152 |
| Transparent | 181 |
| Invisible | 208 |
| Normal | 244 |
| Mask | 268 |
| Masked | 293 |
| Folder | 318 |
| Guide | 343 |
| Outline color | 393 |
| View layer as outlines | 419 |
| Layer height | 449 |

**Control sizes:**
- Name field about 128×20, fill #454545. Swatch about 20×16. Dropdown about 78×20.
- OK spans x 339–431 and y 46–71; Cancel spans y 82–107.
- Both buttons are pills with a light 1 px border. OK is the default and has a heavier border.

**Details to keep:**
- The Visibility order is Visible, Transparent, Invisible. The transparency value shows "50" with no % sign; 50 is the default [med · SS19].
- There is an "Attach To Camera" checkbox, and Lock sits directly under Name.
- "Guided" is not offered, although JSFL's layerType still includes "guided".
- Layer height choices are 100%, 200% and 300% [high · JSFL layer.height].
- (An early draft's 320×360 size was wrong.)

### 2.7 Frame View menu (the Timeline panel menu)

It opens from the ≡ button at the far right of the Timeline panel's tab row; CIB22 calls it the "Frame View menu" [high · CIB22 p.129, SS20, COM-frame-height].

Contents in v20 (Windows) and v22 (macOS) [high · SS20, CIB22 p.129]. "---" marks a separator.

```
Standard                     (radio, default)
Preview                      (radio)
Preview in Context           (radio)
---
Short                        (radio)
Medium                       (radio)
Tall                         (radio)
---
Timeline control - Bottom    (toggle; off = controls above the frame grid)
Match FPS                    (toggle)
Span Based Selection         (toggle, default off)
Customize Timeline Tools     (v21 and later, no ellipsis)
---
Lock
---
Help
---
Close
Close Group
```

- Note the lower-case "in" in "Preview in Context". The heights are Short / Medium / Tall (not Large), and "Timeline control - Bottom" is one toggle, not a Header Top / Header Bottom pair.
- **Resolved ending:** "Close" and "Close Group", not "Close Tab Group" [high · SS20]. P1 only guessed that Close items follow Help [low].
- Preview shows a thumbnail of each keyframe span's contents; Preview in Context scales the thumbnails to the whole stage [high · P1].
- While the toolbar is at the bottom, the item probably reads "Timeline control - Top" [med · P1].

**Frame width and zoom:**
- The Tiny / Small / Normal / Medium / Large frame widths and Tinted Frames are gone since v20 [high]; they are absent from both screenshots.
- Frame width is set with the Resize Timeline View slider at the top right of the timeline toolbar, with a "Reset Timeline Zoom To The Default Level" button to its left [high · CIB23, TL, CIB22 p.129 figure].
- **Resolved:** Alt+mouse wheel also zooms [med · COM-zoom, one user in 2022]; P1's Ctrl+wheel is [low].

**Defaults and behaviour:**
- "Timeline control - Bottom" is off [med]; it is unchecked in CIB22.
- **Match FPS** drops frames during playback to hold the document frame rate [high · P1, from a LinkedIn Learning transcript]. Default: on in SS20 and in that 2020 transcript, off in CIB22 and CIB23, so [low]. Use off.
- Row height: the default is not Short. A November 2019 poster called the default frames very tall, and both CIB books show Short checked only after telling readers to choose it. It is probably Medium [low].

**Row heights. Resolved:**
- Short about 18–19 px [med · SS20]; P1's estimate of about 16 is [low].
- Medium about 24–26 px [low]. P1 measured 25 on Adobe's 2024 press screenshots, which it assumes show Medium.
- Tall about 32–34 px [low].
- A layer set to 200% or 300% height in Layer Properties multiplies just that row [high · JSFL].

**Other items:**
- Lock probably pins the panel [low]; on the Properties panel's menu, Lock stops an accidental drag from moving the panel [high · P1].
- Customize Timeline Tools opens a popover titled "Customize timeline", with a reset button called "Reset Timeline Controls" [high · CIB23, P1].
- Edit > Preferences > General > UI Appearance: Compact also tightens the timeline toolbar slightly [med · COM-frame-height].

### 2.8 Menu look on Windows (context menus and panel menus)

These are native Windows popup menus, and they stay light even in the dark UI theme [high · SS19 (v19 context menu), SS20 (v20 panel menu); P1 agrees for menubar dropdowns, context menus and panel menus in 2019–2022]. That v24 still uses native menus is [med · R].

Measured on SS19 (Windows 10, 100% scale):
- **Background:** #F0F0F0 (#F2F2F2 in places, which is P1's reading), with a 1 px #CCCCCC border and the system drop shadow.
- **Text:** Segoe UI 9 pt (about 12 px), #000000. Disabled text is #6D6D6D.
- **Hover:** a #91C9F7 fill, inset about 3 px from the border; the text stays black.
- **Rows:** 22 px pitch.
- **Separators:** a 1 px #D7D7D7 line in a 7 px band, starting at the text column.
- **Labels and shortcuts:** labels start 35 px from the menu's left edge, after the check-mark gutter (about 28 px per P1). Shortcut text is right-aligned, with a gap of 24 px or more [P1]; the submenu arrow sits at the far right.
- **Checked item:** a check mark in the gutter; on Windows 10 it sits on a light-blue square [high · SS20].
- **Width:** fits the longest label plus its shortcut. The SS19 layer menu is about 227 px.
- **Windows 11:** native menus there get rounded corners and a grey hover [low · R]. Copy the Windows 10 look unless you are targeting Windows 11.
- **Font:** Segoe UI ships with Windows and can't be bundled; load it from the system at runtime and fall back to Source Sans 3 [P1].
- The menubar strip itself stays dark (#2F2F2F), with menu titles about #C0C0C0 [P1].
- (An early draft's dark #323232 menus with a #2680EB hover were wrong.)

### 2.9 Build order within this area

Build the frame-grid selection and the two context menus first. Then, in order:
1. the four layer columns and their header toggles;
2. rename and drag-reorder;
3. Alt-solo;
4. the time-stretch handle with its xN flag;
5. Span Based Selection;
6. the Frame View menu.

---

## 3. Colour controls and drawing-tool properties

**Sources for this area.**
- helpx pages, known only from search-engine extracts made in the first pass (direct fetches return 403):
  - **H-color** color.html and multimedia-and-video/color.html · **H-colors** colors.html · **H-sfg** strokes-fills-gradients · **H-panels** authoring-panels · **H-draw** draw-simple-lines-shapes (= LINES) · **H-pref** drawing-preferences (= DPREF) · **H-pb** working-with-paint-brush · **H-brush** custom-brushes · **H-fluid** fluid-brushes
- adobe.com/learn tutorials, fetched: **L-grad** (making gradients with the Color panel) · **L-color** (managing color in a document) · **L-draw** (drawing in Animate CC).
- Community threads, fetched: **C-drag** (23.0.1: dragging from the fill chip no longer samples a colour) · **C-spec** (24.0.3: the Color panel square shows wrong colours) · **C-stroke** ("Where is the stroke?", Oct 2024) · **C-oval** (resetting the Oval tool) · **C-odm-a / -b / -c** (Object Drawing threads: 2022; art brushes; the Pen in 21.0.1) · **C-swatch** (swatches panel mixed up, 2017).
- **O-f8**: *Flash 8 Cookbook*, chapter 1, section 26. **JSFL**: the AdobeDocs JSFL repository (Stroke, Fill, Oval and Rectangle objects; fetched).
- Theme constants named below are in `apps/editor/src/theme.rs`.

### 3.0 Shared visual vocabulary (100% UI scale)

- **Colour chip.**
  - **Resolved shapes:** in the Properties panel a chip is a rounded rectangle about 30×16 px with radius 3 [high · P1, measured]. This area's "square of about 20×20" [low · R] now applies only to the Color panel, where nothing was measured. Tools-panel chips follow part 1: a fill square with a 1 px black outer and a 1 px white inner border, and a stroke ring 6 px thick (1 px black, 1 px white, 2 px of the stroke colour, 1 px white, 1 px black) around a 10×10 hole of about #404040 [high · P1]. Elsewhere use a 1 px #1A1A1A border (`theme::BORDER`) [low · R].
  - Solid colours are drawn opaque, so alpha is not shown on the chip. Gradients are drawn as the gradient itself (linear runs left to right, radial is centred). A bitmap fill shows a tiny thumbnail. None shows the "no color" glyph [low · R].
  - A chip whose colour comes from a tagged swatch shows a small white triangle in its bottom-right corner, the same mark as in the Swatches panel [high · L-color].
  - Pre-2020 chips had a tiny black triangle in the lower-right corner, and the help still tells you to click the triangle next to the Stroke Color or Fill Color control. Whether 2024 chips still draw it is unknown [low · R].
- **"No color" glyph. Resolved:** a white square with a 2 px red diagonal from the bottom-left corner to the top-right corner, red about #E0302A [high · P1, measured]. This area had #FF0000 [med · R].
- **Fill glyph (paint bucket), 14 px:** a bucket tilted about 45° to the right, an arc handle over it, and a drop at its lower-right lip. **Stroke glyph (pencil), 14 px:** a pencil on a 45° diagonal with the tip at the lower left [med · R]. The Color panel uses them. The Tools panel has had no such glyphs beside its chips since 2020 [high · P1], and P1's Properties captures label the rows with the words "Fill" and "Stroke" (§3.4).
- **Black and white button:** two 8×8 squares overlapping on the diagonal, black at top-left and white at bottom-right, each with a 1 px outline. **Swap colors button:** a quarter-circle arc of radius about 6 px with an arrowhead at each end [med · R]. Keys: D = Black and White, X = Swap Colors [high · P1].
- **Color Picker button:** a circle about 14 px across, filled with a hue wheel (a conic gradient red → yellow → green → cyan → blue → magenta → red) [med · R].
- **Numeric fields** (alpha, sizes, angles). **Resolved look:** white value text over a thin 1 px #474747 underline, with no box; up to 2 decimals; units (%, °, pt, px) as a suffix inside the field [high for 2020–22 · P1]. Blue "hot text" (this area's #4A9FF5 [low]) is the pre-2020 look. Caveat: in 2024 the timeline's current-frame number turned light blue, about #8FB8E8 [med · P1], so 24.0 may have recoloured these fields too. Check a 2024 screenshot.
  - Drag horizontally to scrub at 1 unit per px [med · R]; the cursor becomes a pointing hand with a left/right arrow [med · P1]. Shift multiplies by 10 [low · R].
  - Click to type. Enter commits, Esc reverts, Tab moves to the next field [med · R].
- **Sliders. Resolved:** a round 14 px hollow knob on a 2 px track [med · P1, measured]; this area's 10 px #C8C8C8 thumb on a #5A5A5A track was [low · R]. The value sits to the right or above (§3.4).

### 3.1 Swatch popup (the stroke and fill colour popup)

**Opened from:**
- the Tools-panel Stroke or Fill chip;
- any Properties colour chip (Tool tab, Object tab, text colour, the Doc tab's Stage colour);
- the Color panel's Stroke or Fill chip;
- a double-click on a gradient pointer in the Color panel.

[high · H-sfg, H-color, C-drag, L-grad]

About 240×200 px. Background is the panel colour #323232 (`theme::PANEL_BG`), with a 1 px #1A1A1A border and a soft shadow. Put its top-left corner at the chip's bottom-left; flip it above or to the left of the chip when it would leave the window [low · R].

```
┌────────────────────────────────────────────────────┐
│[preview][#0066CC   ] Alpha: 100 %         [⊘] [◍] │  header ≈30 px
├────────────────────────────────────────────────────┤
│ (tagged swatches, if any)                          │
│ c0 │blk│ 18 web-safe columns × 12 rows              │  solids, 20 cols
│ ...                                                │
│ [g1][g2][g3][g4][g5][g6][g7]                       │  gradient row
└────────────────────────────────────────────────────┘
```

The hex value in the sketch is only an example; the first-run default fill is unconfirmed (§7.3).

**Header, left to right:**
1. **Preview swatch**, about 40×20. While the pointer hovers a swatch or any pixel, it shows that colour; otherwise it shows the current colour [med · R].
2. **Hex field**, about 62×20, showing `#RRGGBB` in upper case. It is editable [high · H-sfg, which tells you to type a hex value into the box]. It updates live while hovering, and Enter applies the value and closes the popup [med · R].
3. **"Alpha:"** label followed by a number field `100 %` (range 0–100) [med · R].
4. **No Color button**, 18×18, using the glyph from §3.0.
   - The help text, carried over from Flash CS3, says the button only appears while you create an oval or rectangle, and that No Color can't be applied to an existing object: select its stroke or fill and delete it instead [med · H-sfg].
   - That passage may predate Flash 8. Recall says the same passage also calls gradients fill-only, which JSFL `Stroke.shapeFill` contradicts. Recall also says that in CC and 2024, picking None on a selected shape's Properties stroke chip removes the stroke [low · R].
   - The button is absent for gradient pointers, text colour and Stage colour [med · R].
   - **Implement:** show the button when Rectangle, Oval, their primitives or PolyStar is the active tool, and on the Properties chips of a selected shape. Applied to a selection, None deletes that part, which is the end state the help text prescribes anyway.
5. **Color Picker button**, 18×18 (hue-wheel glyph). It opens the OS colour dialog (on Windows, the system "Color" dialog) and applies the result [high · H-sfg, which names a System Color Picker button in the popup]. Ponytail: until the native dialog is wired, use egui's built-in HSV picker in a small window.

**Grid.**
- Cells are about 10×10 px with 1 px #000000 lines between them (11 px pitch) and a 4 px inset. The popup always shows 20 columns [low · R].
- The default palette is the Web 216 colours plus a few premixed gradients [high · O-f8]. L-grad also calls the popup grid the 216 web-safe colours.
- The exact order, row-major across 20 columns, is [med · R]:

```
K    = [00,33,66,99,CC,FF]
COL0 = [000000,333333,666666,999999,CCCCCC,FFFFFF,FF0000,00FF00,0000FF,FFFF00,00FFFF,FF00FF]
for row in 0..12:
    push(COL0[row]); push(000000)            # grays+primaries column, then a black spacer column
    b = K[row % 6]; rset = row < 6 ? K[0..3] : K[3..6]
    for r in rset: for g in K: push(rgb(r,g,b))
```

- The spacer column is made of real black swatches. The result:
  - top-left is black, and the top half runs from greens toward blues and cyans;
  - the first row of the lower half runs from red through orange to yellow;
  - the last row runs from magenta through pink to white, so bottom-right is white.
- **Gradient row.** 7 gradients start on a new row after the solids; the count is [med · R]. Likely contents [low · R]: two greyscale gradients (a radial from a white centre to black, a linear from black to white), three radials (red, green and blue centres fading to black), one linear rainbow and one warm "gold" linear.
- **Gradients on strokes.** Strokes can carry gradient and bitmap fills [high · JSFL `Stroke.shapeFill`]. The stroke popup shows the gradient row [med · R].
- **Custom swatches.** New solids are appended after the last solid, before the gradients. New gradients go after the existing gradients [med · R].
- **Tagged swatches** (Animate 2019 and later) form their own group above the regular grid [low · R]. Each has a small white triangle, about 4 px, in its bottom-right corner [high · L-color].

**How the selected swatch is marked** [med · R]: the hovered cell gets a 1 px white square outline inside the black grid line, and the preview and hex follow the hover. No persistent current-colour marker is drawn.

**Behaviour:**
- **Opening.** Mouse-down on a chip opens the popup, and it stays open after release. While it is open, the cursor is an eyedropper [high · C-drag]. The eyedropper works over swatches, the Stage and other panels [med · R].
- **Press-drag-release from the chip.** On release it picks whatever lies under the cursor, Stage pixels included: you drag the eyedropper out of the colour well [high · C-drag].
  - On 23.0.1 for macOS this broke on the Tools chip, where the drag selects the object instead; it still worked from Properties chips [high · C-drag].
  - The 24.x status is unknown. Implement the working behaviour on every chip.
- **Gradient pointers** sample the same way: double-click the pointer, then click the Stage or drag outside the panel [high · C-drag, L-grad].
- **Picking.** A click on a swatch or a pixel sets the colour and closes the popup. The colour becomes the tool default and is applied to the matching part (fill or stroke) of the current selection [high · H-sfg].
- **What each chip shows.** Tools-panel chips show the tool defaults and do not follow the selection. The Properties Color and Style chips show the selection [med · C-drag, 23.x].
- **Alpha.** Each swatch carries its own alpha (100% by default), so picking a swatch sets both colour and alpha [low · R].
- **Esc** closes the popup without changing anything [med · R].
- **egui note.** egui can't read pixels outside its own window. Ponytail: sample only from our last rendered frame, and add OS screen capture if users ask for it.

### 3.2 Color panel (Window > Color, Ctrl+Shift+F9)

- **Shortcut:** Ctrl+Shift+F9 [high · P1, from Adobe's shortcut table]; Flash CS3 used Shift+F9. (This area had it at [med · R].)
- **Panel group.** "Color" and "Swatches" share one panel group as tabs [med · H-color, which mentions Colors and Swatches tabs; R].
  - Neither is shown in Essentials by default [low · R].
  - Floating size is about 270×330 px [low].
  - Users on 24.0.3 report that the panel became much larger with the new UI and was later scaled down [med · C-spec].

Rows, top to bottom (P = pencil glyph, B = bucket glyph):

```
[P][■stroke] [B][■fill]   [■□] [⊘] [⇄]
Color type: [Solid color        ▾]
(Flow: [ext][refl][rep]   ☐ Linear RGB)     ← gradients only
┌───────────────┐┌──┐  ○ H: 0 °
│ color square  ││sl│  ○ S: 0 %
│  ≈160×120     ││  │  ○ B: 0 %
│               ││  │  ○ R: 0
│               ││  │  ○ G: 0
└───────────────┘└──┘  ○ B: 0
                        A: 100 %
# [000000]  [ preview bar / gradient bar        ]
                              [Add to Swatches]
```

1. **Stroke and Fill controls**, plus Black and white, No color and Swap colors.
   - Clicking a glyph (pencil or bucket) makes that control *active*. The Color panel and the Swatches panel then edit the active control [high · L-color].
   - The active control shows a pressed background, #545454 (`theme::BUTTON_ACTIVE`) [low].
   - Clicking a chip opens the swatch popup [med · R].
   - Black and white sets the stroke to #000000 and the fill to #FFFFFF [high · H-sfg].
   - Layout: both controls on one row with the three buttons at the right [low · R]. A stroke-above-fill stacked layout is possible.
2. **Color type dropdown:** `None`, `Solid color`, `Linear gradient`, `Radial gradient`, `Bitmap fill`. The default is `Solid color`. Some docs call it "Type" [high · H-color, L-grad].
3. **Gradient-only controls.**
   - "Flow:" with three toggle buttons, about 22×18 each: *Extend color* (the default), *Reflect color* and *Repeat color*. Older docs call it "Overflow".
   - A `Linear RGB` checkbox.
   - [high · JSFL fill.overflow / linearRGB] for the values, [med · R] for the labels.
4. **Color square and vertical slider**, with radio buttons and fields for **H** (0–360 °), **S** and **B** (0–100 %), and **R**, **G**, **B** (0–255). Below them is **A** (0–100 %), which has no radio button.
   - The selected radio button sets which component the slider drives [high · L-grad, which switches the control to Brightness and then drags its slider].
   - The square shows the other two components, following the Photoshop convention [med · R]:
     - H: x = S, y = B
     - S: x = H, y = B
     - B: x = H, y = S
     - R: x = B, y = G
     - G: x = B, y = R
     - B: x = R, y = G
   - The default radio button is H [low]. L-grad switches to B, so B is not the default.
   - Edits apply live to the selection [med · R].
   - On 24.0.3 the square shows colours that don't match the value actually picked [high · C-spec]. Render the square correctly; don't copy the bug.
5. **Hex field** (a `#` label plus a 6-digit field) and a **preview bar** for the current colour. In gradient modes the preview bar becomes the gradient bar [med · R].
6. **Gradient bar.** Full width, about 18 px tall, with a checkerboard behind it to show alpha; the checkerboard is [low].
   - **Pointers** sit below the bar. Each is a "house" shape about 9×13 px: a square filled with the stop colour, plus a triangular roof touching the bar.
     - The selected pointer's roof is filled black [high · L-grad].
     - Unselected roofs are white with a dark outline [low · R].
   - A new gradient defaults to two stops, black on the left and white on the right [high · L-grad].
   - Click on the bar to add a pointer [high · L-grad]. The maximum is 15 [med · R].
   - Drag a pointer sideways to move it [med · R]. Drag it down and off the panel to delete it [high · L-grad].
   - Double-click a pointer to open the swatch popup with the 216-colour grid, which samples with an eyedropper [high · L-grad, C-drag].
   - The fields, square, hex and alpha all edit the selected pointer [high · H-color].
7. **Bitmap fill mode.** The square block is replaced by an `Import…` button and a grid of library bitmap thumbnails, about 48 px each. Choosing Bitmap fill when the library has no bitmaps opens the import dialog [high · H-color] for the dialog, [med · R] for the layout.
8. **`Add to Swatches` button.** It adds the current colour to the Swatches panel [high · L-color]. It sits at the bottom right [med · R].
9. **Panel menu (≡):** `Add Swatch` [high · H-color], `Help` [med · R], then **Resolved:** `Close` and `Close Group`, the ending seen on the Timeline and Properties panel menus [med · SS20, P1], instead of this area's recalled `Close Tab Group`. Other panel menus start with `Lock` [P1], so this one probably does too [low].

### 3.3 Swatches panel (Window > Swatches, Ctrl+F9)

- **Grid and reflow.** The same cell grid as the popup, but it reflows to the panel width: columns = floor(width / 11), filled in row-major order. The default palette only lines up at 20 columns [med · R].
  - C-swatch doesn't mention width. Its accepted fix for a mixed-up panel is the menu item **Load Default Colors**.
- **Groups.** Tagged swatches come first, then the document swatches [high · H-colors]. Swatches are saved in the document [med · L-color].
- **Click a swatch** to set the active stroke or fill and apply it to the selection [high · H-colors]. The clicked swatch keeps a white outline [med · R].
- **Empty area.** Hovering it shows a paint-bucket cursor; clicking adds the current colour as a swatch [med · R].
- **Ctrl-click a swatch** to delete it; the cursor is scissors [low · R].
- **Tagged swatches:**
  - **Create:** select a swatch, choose Convert to Tagged Swatch, type a name, then click OK [high · L-color].
  - **Mark:** a white triangle in the bottom-right corner. Properties chips show the same mark for elements that use the swatch [high · L-color].
  - **Edit:** double-click the tagged swatch (or select it and click Edit), change it, then click OK. Every element that uses it updates [high · L-color].
  - **Dialog:** "Tagged Color Definition", with a Name field, Solid / Linear gradient / Radial gradient, HSB or RGB values, and OK and Cancel [med · H-color].
  - **Unlink:** a Properties button that detaches an element from its tagged swatch [med · H-color].
- **Bottom bar**, about 26 px, holding icon buttons:
  - **Convert to Tagged Swatch** (tag glyph) [high · H-color];
  - **Edit** (pencil glyph) [med · H-color];
  - **Delete** (trash glyph) [low · R].
- **Panel menu:**
  - `Duplicate Swatch`, `Delete Swatch`
  - `Add Colors…`, `Replace Colors…`
  - `Load Default Colors`, `Save Colors…`, `Save as Default`, `Clear Colors`
  - `Web 216`, `Sort by Color`
  - — `Help` — `Close`, `Close Group`
  - [high · C-swatch] for Load Default Colors, [high · O-f8] for Save Colors, [med · R] for the rest. The ending uses `Close Group` for the same reason as in §3.2.

### 3.4 Properties panel, Tool tab: shared parts

- **Tabs:** `Tool | Object | Frame | Doc`. The Tool tab shows the active tool. For a tool with no properties, the Tool tab is disabled and the Doc tab is shown [high · H-panels].
  - Tools with no Tool tab: Selection, Subselection and Hand [med · P1]; probably also Free Transform, Zoom and Rotation [low · P1].
  - **Tab switching** [med · P1]: selecting a stage object shows Object; clicking a frame shows Frame; picking a drawing tool shows Tool; clicking empty stage or pasteboard shows Doc. The user can click any enabled tab.
- **Header row. Resolved:** the tool glyph plus the tool's name without the word "Tool", for example "Rectangle", "PolyStar" or "Classic Brush" [high · P1, measured], in a row about 48 px tall [med · P1]. This area recalled "Rectangle Tool" with a 16 px glyph in a row of about 34 px [med · R].
- **Object Drawing toggle (J).**
  - **Resolved placement:** a quick-action button in the row under the Tool tab's header [high · P1], and a toggle in the Tools-panel options area at the bottom of the toolbar [high · C-odm-a (2022), P1]. This area's guess, the right end of the first Color and Style row [low · R], is dropped.
  - Glyph: a 13×13 light square outline holding a light-grey filled circle about 9 px across [high · P1]. The on state shows a pressed background (#565656 for Properties quick actions [P1]) [med · R]. The shortcut is J [high · R, P1].
  - Supported tools: Pencil, Line, Pen, Brush, Oval, Rectangle and PolyStar [high · MODES]. Rectangle Primitive and Oval Primitive have no toggle [high · C-odm-a, P1]. Object Drawing with the Pen was broken in 21.0.1 [med · C-odm-c].
  - It is greyed out while an art-brush style other than the plain line is selected [high · C-odm-b]. The same for pattern brushes is [low · R].
  - Off (Merge Drawing) by default, and the setting persists across sessions [high · P1] (this area had persistence at [low · R]).
- **Section "Color and Style"** [high · C-drag (23.0.1), C-stroke (Oct 2024), P1].
  - It was called "Fill and Stroke" before 2020 [med · R]. `apps/editor/src/panels.rs` still says "Fill and Stroke" (the `theme::section(ui, "Fill and Stroke", …)` call); rename it.
  - Section headings are Title Case: "Color and Style", "Rectangle Options", "Oval Options" [med · C-drag, L-draw, C-oval].

| Row | Widgets, left to right | Default | Values and notes | Conf |
|---|---|---|---|---|
| Fill | swatch (§3.0), the word "Fill", then at the right a 12 px checkerboard alpha glyph and `100 %` | first-run fill unconfirmed (P1 reads #0066CC [low]) | Only the rows that fit the tool or selection show; stroke-only tools have no Fill row | [high · P1, H-panels] |
| Stroke | the same layout with the word "Stroke" | #000000 | Alpha sits up front, next to each chip (H-panels says it moved forward) | layout [high · P1]; default [med] |
| Stroke size | "Stroke" label at the left and the value at the right of one line, then a full-width slider | 1 (shown as 1.00) | 0.10–200 | layout [med · P1]; range [med · R] |
| Style | dropdown (line preview plus name), then a "⋯" button for rarer options | Solid | Hairline, Solid, Dashed, Dotted, Ragged, Stipple, Hatched, then brushes added from the Brush Library. In 2024 the item may read "Solid line". This area also recalled an *Edit stroke style* (pencil) button and a *Brush Library* button. | "⋯" [high · P1]; list [med · JSFL stroke.style, R]; extra buttons and label [low] |
| Width | width-profile dropdown (profile thumbnails), then "⋯" | Uniform | Uniform plus 6 built-in variable-width profiles. Recalled *Add to profile* (+) and *Delete profile* (trash) buttons. | "⋯" [high · P1]; list [med · R]; buttons [low] |
| Scale | `Scale` dropdown and a `Hinting` checkbox (one 2023 source calls it "Stroke Hinting") | Normal, unchecked | Normal / Horizontal / Vertical / None | values [high · JSFL]; layout [med · P1] |
| Cap | 3-button segmented control | Round | None / Round / Square | items [high · JSFL, P1]; default [med] |
| Join | 3-button segmented control, then the `Miter` field | Round; Miter 3 | Miter / Bevel / Round. Miter (1–60) is enabled only when Join = Miter. | values [high · JSFL]; order [med · P1]; defaults [med · R] |

- **Resolved layout.** P1's measured captures show one row per colour (swatch, word, alpha), segmented Cap and Join buttons, and a "⋯" after Style and after Width [high and med · P1]. This area's single row with bucket and pencil glyphs (bucket + Fill chip + alpha; pencil + Stroke chip + alpha) [med · H-panels] and its Cap/Join icon dropdowns [layout med] lose. Because Scale, Cap and Join stay visible in those captures, this area's suggestion to move them behind the "…" is dropped. helpx says the less-used stroke options moved behind "…", naming the style and width-profile options [low · H-panels].
- **Object tab (shape selected)** [med · C-stroke, accepted answer]:
  - The chips appear only for the parts that are selected; both fill and stroke must be selected for both chips to show.
  - Setting a stroke colour here does not add a stroke to a shape that has none. Use the Ink Bottle (S) for that.
- **Cap glyphs** [med · R]: a thick bar ending flush at a guide line (None); a bar with a semicircular end (Round); a bar running past the guide with a square end (Square).
- **Join glyphs** [med · R]: an L-shaped corner with a sharp outer point (Miter); a rounded outer corner (Round); a chamfered outer corner (Bevel).
- **Edit stroke style dialog ("Stroke Style")** [fields: high · JSFL; labels: med · R]:
  - `Type`: Solid / Dashed / Dotted / Ragged / Stipple / Hatched.
  - `Thickness`, `Sharp corners`, `Zoom 4x`, and a preview.
  - Per-type fields: Dashed = dash length and gap length; Dotted = dot spacing; Ragged = pattern, wave height and wave length; Stipple = dot size, variation and density; Hatched = hatch thickness, space, jiggle, rotate, curve and length.
- **Default colours:** stroke #000000 [med]; the first-run fill is unconfirmed. The Black and White button resets to a black stroke and a white fill [high · H-sfg, P1].

### 3.5 Tool tab per tool

Top to bottom. Every tool starts with the header row, then its quick actions (such as Object Drawing), then Color and Style, then "<Tool> Options" [pattern: med · P1].

- **Rectangle (R) and Rectangle Primitive (R).** A long press on the tool button opens the flyout that holds the primitive [high · L-draw].
  - Color and Style.
  - **Rectangle Options:**
    - Four corner-radius fields laid out 2×2 (TL TR / BL BR), each with a corner glyph [med · R, P1]. A single row of four fields is possible [low].
    - A **lock (chain) button**, on by default. While locked, only TL is editable and the other three mirror it [high · JSFL lockFlag]; the default is [med · R, P1].
    - A slider, default 0.00. Dragging left inverts the corners; the right end, 100.00, gives the roundest corners [high · L-draw]. That makes the range −100…100 [med · L-draw, R]. Negative (scooped) corners are documented for the primitive [high · P1].
      - Whether typed values can exceed ±100 is [low]. `panels.rs` currently accepts −100 to 999 (`DragValue … range(-100.0..=999.0)`); keep that for typing, but clamp the slider to ±100.
    - A `Reset` button [high · P1].
  - **Behaviour:**
    - Shift draws a square, and Alt draws from the centre [high · LINES]. Alt-click on the Stage opens the settings dialog: Width, Height, Corner radius, Draw from center (§1.6) [high · LINES].
    - Up and Down arrows while dragging change the radius [high · LINES, P1]. This area had [med] for the primitive and [low] for the plain rectangle.
    - A primitive stays editable in the Object tab until it is broken apart [med · R].
- **Oval (O) and Oval Primitive (O).**
  - Color and Style.
  - **Oval Options:** `Start angle` (0–360) and `End angle` (0–360), each a slider plus a number field; `Inner radius` (0–99 %, the share of the fill removed); a `Close path` checkbox, on by default; `Reset` [high · JSFL, C-oval, P1]. The order is [med · R].
  - Shift draws a circle. Alt-click opens the settings dialog (Width, Height, Draw from center) [med · R].
- **PolyStar** (no shortcut: as a JSFL "extensible" tool it can't take a direct shortcut [high · P1]).
  - Color and Style, then **Tool Options**, with labels above the fields [high · P1]: Style (polygon / star); Number of Sides (3–32, default 5); Star point size (0–1, default 0.50; it only affects stars).
  - These are inline since the 2020 redesign [med · H-panels, which lists PolyStar among the modernised tools; R].
- **Line (N).** Color and Style (stroke only) and Object Drawing. Shift snaps to 45° [high · R].
- **Pencil (Shift+Y).**
  - Color and Style (stroke only).
  - **Pencil Mode:** `Straighten` / `Smooth` / `Ink`. The glyphs are a zigzag polyline, an S-curve and a jittery freehand line.
  - **Smoothing** slider: 0–100, default 50, enabled only in Smooth mode.
  - [high · H-draw, H-pref] for the labels and smoothing. Placement [med]: an icon flyout in the Tools-panel options area, mirrored as a dropdown in the Tool tab.
- **Paint Brush (Y)** (vector strokes, art and pattern brushes) [high · H-pb]:
  - Color and Style. Its Style list includes brushes added from the Brush Library.
  - **Paint Brush Options:** a `Draw as Fill` checkbox; drawing mode Straighten / Smooth / Ink; Smoothing 0–100, default 50.
  - Pressure and tilt toggles for width are [low].
- **Classic Brush (B)** (fill-based).
  - Quick actions: Object Drawing; **Brush Mode**, a button with a tiny corner triangle that opens a popup [P1]; **Lock Fill**, a gradient square with a small padlock [med · P1; existence high · H-sfg].
  - Color and Style: fill chip and alpha only.
  - **Brush Mode:** `Paint Normal` (default), `Paint Fills Only`, `Paint Behind`, `Paint Selection`, `Paint Inside` [high · H-draw, LINES]. **Resolved label:** "Paint Fills Only" (current help and P1); a 2020 toolbar capture and older builds read "Paint Fills" [high for 2020 · P1, H-draw]. Check on a 2024 screenshot.
  - **Brush shape** dropdown: circle, flat ellipses, square, flat rectangles, and 45° and 135° slashes [med · R]. A `+` button opens the custom-shape dialog (shape oval / rectangle / polygon, angle, flatness %); a pencil edits a custom brush and a trash icon deletes it [P1]. **Resolved dialog title:** "Nib Options" [high · P1, read with the custom-brushes help page]. This area's fact-checker had marked that name as unsourced and suggested "Brush Options" [low].
  - **Size** slider, 1–200 px, value at the right [high · P1] (this area: [med · R]).
  - **Zoom size with stage**, checked by default [high · LINES, P1], and **Sync settings with Eraser**, unchecked [P1].
  - Smoothing 0–100 [med · H-brush]. Its default is unknown: one draft read 100, and the Pencil defaults to 50 [low · P1].
  - Pressure and Tilt toggles, shown only when a pressure tablet is connected [high · P1] (this area: [med · R]).
  - Object Drawing works only in Paint Normal mode [med · C-odm].
- **Fluid Brush (B, same flyout).**
  - Fill chip, then sliders for Size, Taper, Angle, Roundness, `Stabilizer`, `Curve Smoothing`, `Velocity` and `Pressure`.
  - A Min. size slider is possible [low].
  - [med · H-fluid] for which options exist, [low] for their order.
- **Paint Bucket (K).**
  - Fill chip and alpha.
  - **Gap Size** icon dropdown: `Don't Close Gaps`, `Close Small Gaps` (default), `Close Medium Gaps`, `Close Large Gaps`. The glyphs are a ring with no gap, then rings with progressively larger gaps [labels: high · H-sfg, R, P1; default: med].
  - `Lock Fill` toggle [high · H-sfg].
  - The Eyedropper (I) hands off to this tool: clicking a fill switches to Paint Bucket with Lock Fill on, and clicking a stroke switches to Ink Bottle (S) [high · H-sfg].
- **Eraser (E).**
  - **Eraser Mode:** `Erase Normal` (default), `Erase Fills`, `Erase Lines`, `Erase Selected Fills`, `Erase Inside` [high · RESHAPE, R, P1 (2020 capture)].
  - `Faucet` toggle: a click deletes a whole fill or stroke segment [high · RESHAPE, R].
  - Shape (round or square) and a Size slider [med · R]. A "size follows Stage zoom" checkbox [low].
  - Pressure and Tilt toggles when a pressure tablet is connected [high · P1].
  - Double-clicking the Eraser tool button erases everything on the Stage and pasteboard [high · RESHAPE].
- **Text (T).** [med · R, plus the showPIControl list in JSFL] unless tagged; P1 measured most of it [high · P1].
  - **Header row:** a text-type dropdown, `Static Text` (default) / `Dynamic Text` / `Input Text`; an orientation control (Horizontal, Vertical, Vertical Left to Right), for static text only [P1].
  - **Character:**
    - Family dropdown, plus a globe button for web fonts (HTML5 Canvas only, otherwise disabled) [med · P1]; Style dropdown; a full-width `Embed…` button that opens Font Embedding [P1].
    - Size (in points, whatever the ruler units [high · P1]) and Letter spacing, plus `Auto kern`.
    - Colour chip with alpha; its popup has no No Color button.
    - Anti-alias (P1 labels the row "Render"): `Use device fonts`, `Bitmap text [no anti-alias]`, `Anti-alias for animation`, `Anti-alias for readability`, `Custom anti-alias…`.
    - Toggle buttons: Selectable, Render as HTML, Show border, Superscript, Subscript.
  - **Paragraph:**
    - Format: align left / center / right / justify.
    - Spacing: indent and line spacing.
    - Margins: left and right.
    - Behavior: `Single line` / `Multiline` / `Multiline no wrap` / `Password`. Password is for Input text, and the Behavior row is shown disabled for static text [med · P1].
  - **Options:** Link, Target.
  - **Defaults:** P1's capture shows 12 pt, letter spacing 0, Anti-alias for readability, indent 0 px, line spacing 2 pt and margins 0 px. The default font is [low].

### 3.6 Shortcuts

- **Swatches panel:** Ctrl+F9 [high · R, P1].
- **Color panel:** Ctrl+Shift+F9 [high · P1]; Flash CS3 used Shift+F9.
- **Object Drawing:** J [high · R, P1]. **Black and White:** D. **Swap Colors:** X [high · P1].
- **Tools** [high · R]: R (Rectangle and Rectangle Primitive), O (Oval and Oval Primitive), N (Line), B (Classic Brush and Fluid Brush), K (Paint Bucket), S (Ink Bottle), I (Eyedropper), E (Eraser), T (Text).
  - Y (Paint Brush) and Shift+Y (Pencil) are [med · R, P1]; the Pencil moved off Y when the Paint Brush arrived in 2018.
  - Pressing a shared key again cycles through its group [med · P1].
- **Gradient Transform:** F [high · L-grad].

---

## 4. Home screen, New Document and Document Settings

**Sources for this area.**
- helpx pages: **H-doc** documents · **H-stage** using the Stage and Tools panel (= STAGE) · **H-layers** timeline layers (= LY) · **H-tmpl** templates · **H-pi** authoring panels (= H-panels) · **H-canvas** creating and publishing an HTML5 Canvas document · **H-vr** virtual reality · **H-as** ActionScript · **H-time** time.
- What's New pages: **H-24** 2024 (= WN24) · **H-wn19** v19.0, October 2018 (= WN19) · **H-wn20** v20.0, November 2019 (= WN20) · **H-wn20.5** v20.5, June 2020. The last three were read from archived copies.
- **CIB22, CIB23, CIB24**: the books' sample PDFs (text). **CIB22-fig, CIB23-fig, CIB24-fig**: the screenshots printed in them, macOS shots of v22, v23 and v24, which were measured.
- LinkedIn Learning: **LIL-ET** Essential Training ("Choosing a document type", "Creating a document for animation", "Modifying document settings") · **LIL-NF** "Modern interface and start screen" (v19) · **LIL-UI** "User interface improvements" (v19.x) · **LIL-MOD** "Application modernization" (v20) · **LIL-SPEC** "Modernized Adobe Spectrum interface" (v24).
- adobe.com/learn tutorials **AL-doc**, **AL-ws**, **AL-soc**; community threads **CF-new**, **CF-start**, **CF-preset**, **CF-fps**, **CF-24**, **CF-res**.
- **Evidence note.** Values marked "measured" come from the CIB figures. Their text fields measure 31 px against Spectrum's standard 32 px field, so 1 figure px is roughly 1 UI px at 100%. The figures are printed in CMYK; colours were sampled and neutralised to greys and are accurate to about 10%. The screenshots were only measured. Nothing from them is reproduced, and the temporary crops were deleted.

### 4.0 Version history (what "the Animate 2024 Home screen" is)

- **v19.0, October 2018.** A start screen held the intent tabs, the presets, a Details pane and a Create button. Recent files were in the left pane, "Learn" opened tutorials, and sample files sat at the bottom [high · H-wn19, H-doc].
- **v19.x, 2019.** A Frame Rate field was added to Details [high · LIL-UI].
- **v20.0, November 2019.** A **Home screen** replaced the start screen [high · H-wn20, CIB23]:
  - it has tutorials, popular presets, recent documents, a **Create New** button and an **Open** button;
  - its content is tailored to how familiar the user is with Animate and to their plan;
  - the intent tabs moved into the New Document dialog.
- **v20.5, June 2020.** New users get an onboarding panel of hands-on tutorials [high · H-wn20.5].
- **v24.0, October 2023.** The whole UI was restyled on Adobe's Spectrum design system ("version 7", per LIL-SPEC). The theme setting is at Edit > Preferences > Edit Preferences > Interface > Color Theme [high · H-24, LIL-SPEC]. Home still has More Presets and Create New [high · CIB24], and the New Document layout is the same as in v23 [high · CIB24-fig].

**Two UI technologies.**
- **Home and New Document are web pages hosted inside Animate.** A v19 user described the start and new-file screen as a bundled web page [med · CF-start]. They use Spectrum styling: pill-shaped buttons, flat tiles, a blue selection border and small uppercase section headings [high · CIB23-fig, CIB24-fig]. The page can stop responding; a v24 user reports that Create New did nothing [med · CF-new].
- **Document Settings is a native dialog** [med · R].
- So build two style variants.

### 4.1 Launch sequence

1. **Splash window.** A splash window appears first [med · R]. minnamate: show its own logo, or skip it.
2. **First-launch question.** Animate asks whether you are new to it or experienced [high · CIB23, LIL-MOD].
   - New users get the **Basic** workspace and beginner preferences; everyone else gets **Essentials** and expert preferences.
   - From v20.5, new users also see the tutorial onboarding panel [high · H-wn20.5].
   - minnamate: skip both and always use Essentials.
3. **Home at launch.**
   - Home is the default workspace at launch. It fills the window under the application bar, with no panels and no timeline [high · CIB23]. It shows whenever no document is open [med · P1].
   - It cannot be switched off [med · CF-start, a 2020 user reply].
   - Launching by double-clicking a .fla or .xfl opens that file [med · CIB23 tip].
4. **Getting back to Home.**
   - Home reappears when all documents are closed; a CF-new poster closes all files and then uses the "Start a new file fast" section [high · CF-new].
   - The **Home button** sits at the top left [high · CIB23].
   - **Home button glyph:** a solid (filled) house about 16×14 px, mid-grey (about #B0B0B0), with a pitched roof over a square body and a small door notch at the bottom centre [med · CIB23-fig].
   - On macOS it comes right after the window buttons, followed by the app name. On Windows it is at the far left of the top bar [low]; P1 suggests right after the app icon, before File [low · P1].

### 4.2 Home screen (v20 to v24)

**Contents**
- **Create New** opens the New Document dialog [high · H-wn20, CIB23, CIB24]. Make it a blue primary pill [low · R, the Creative Cloud Home pattern].
- **Open** opens the system file dialog, the same as File > Open (Ctrl+O) [high · CIB23, H-wn20].
- **"Start a new file fast"** is a strip of quick tiles [high · CIB23-fig, CF-new, LIL-ET].
  - **v23 tiles, left to right:** Full HD, Android 16:9, Banner, iPad 3-4 Gen, Canvas, More Presets [high for v23 · CIB23-fig; med for v24, because H-wn20 says the content is tailored].
  - **One click on a tile creates the document** [high · CF-new, where a click on Full HD does so].
  - **More Presets** opens the New Document dialog [high · CIB23, CIB24].
  - LIL-ET's presenter finds the HD preset by clicking a small arrow, so some arrow reveals more presets. Where it sits is unknown [low].
- **Recent documents** are listed [high · H-wn20, CIB23]. The columns are unknown [low].
- **Tutorials** appear on Home [high for v20 · H-wn20; low for v24]. minnamate: skip them.

**Quick-tile strip, measured** [med · CIB23-fig]
- **Background:** about #232323.
- **Heading:** "Start a new file fast", bold, about 16 px, white, centred. Its top is about 69 px above the top of the glyphs, and the tile group is centred under it.
- **Tiles have no box or border at rest.** Each tile is a line glyph with a name under it.
  - Tile centres are about 98 px apart.
  - Glyphs are about 62–75 × 37–50 px, drawn with strokes about 1.5 px wide, colour about #DEDEDE.
  - The name is bold, about 12 px, white, about 29 px below the glyph.
  - **There is no size line** under the name.
- **Glyphs, in our own words:**
  - **Full HD:** a wide rectangle with a play triangle inside.
  - **Android 16:9:** a rounded gamepad, with a plus-shaped d-pad on the left and four small circles on the right.
  - **Banner:** a browser window, with a thin line under its top edge and a tall narrow rectangle on the right.
  - **iPad 3-4 Gen:** a rectangle with a mortarboard inside.
  - **Canvas:** a page with its top-right corner folded.
  - **More Presets:** a circle with three dots in it.
- **Hover and pressed states:** unknown [low]. Suggestion: a #2E2E2E rounded rectangle behind the tile on hover.

**Rest of the layout** [low · R, the Creative Cloud Home pattern; not seen in any source]
- A left rail about 216 px wide, with Create New on top and Open under it.
- The main area shows the quick strip, then Recent.
- **Recent rows:** a page glyph and the file name, with the full path as a tooltip. Fill the list from the same 10 entries as File > Open Recent.
- Drop the Home/Learn navigation rows; they belong to v19.

**Font:** Adobe Clean is proprietary. Substitute Source Sans 3 (SIL Open Font License) or Segoe UI.

**Behaviour**
- Clicking a tile creates "Untitled-N" with that preset and switches to the workspace [high].
- Clicking a recent row opens that file [high · R]. If the file is missing, show an error alert [low].
- Dropping a .fla or .xfl onto the window opens it [med · R].
- With documents open, the Home button shows Home, and clicking a document tab returns to that document [low].

### 4.3 New Document dialog

**Opening it and its size**
- **Entry points:** File > New (**Ctrl+N**) [high · H-wn19], Home **Create New** and Home **More Presets** [high · CIB23, CIB24].
- **Title:** "New Document" [high · H-doc, figures].
- **Size:** about **775 × 575 px** of client area, plus the system title bar. The v23 and v24 figures agree within 2% [med · measured]. Probably a fixed size [low].

**Tab strip** (top 109 px)
- **Background:** about #1C1C1C. There is no divider line; the strip simply meets the lighter body below it.
- **Seven tabs, left to right:** Character Animation, Social, Game, Education, Ads, Web, Advanced; Advanced is at the far right [high · H-doc, CIB23, figures].
- **Each tab is a line icon centred above a text label**, not text only [sizes: med · measured]:
  - the icon box is about 30 × 28 px, starting about 36 px below the top of the strip;
  - the label is about 13 px, with its baseline about 90 px below the top of the strip;
  - the first label starts 25 px from the left edge, and labels are about 44 px apart.
- **Selected tab:** white label, with a 2 px white underline exactly as wide as the label, at the bottom of the strip [high · figures].
- **Other tabs:** label about #9A9A9A, icon about #808080. Hover colour unknown [low].
- **Icons, in our own words:**
  - **Character Animation:** two overlapping outline circles, a small one at upper left and a larger one at lower right.
  - **Social:** a paper plane.
  - **Game:** a gamepad.
  - **Education:** a mortarboard.
  - **Ads:** a megaphone.
  - **Web:** a globe.
  - **Advanced:** a page with its top-right corner folded.

**Body, left pane** (x 0 to 502, background about #232323)
- **Section heading** at x 26, about 33 px below the strip.
  - On the intent tabs it reads "PRESETS (n)", where n is the number of presets.
  - On Advanced there are two headings: "PLATFORMS (5)" and "BETA PLATFORMS (4)".
  - Style: about 11 px, uppercase, semibold, about #9C9C9C.
- **Tile grid:** starts about 23 px below the heading text. Three columns, with 18 px gaps across and 14 px gaps down.
- **Preset tiles:** 136 × 136 px, fill about #2E2E2E, corner radius about 4.
  - **Glyph:** about 72 × 50 px, 28 px from the top of the tile. It looks like a video player: a rounded rectangle with a centred play triangle and a scrub line near the bottom carrying a small circle.
  - **Name:** about 12 px, bold, near-white, baseline about 101 px from the top of the tile.
  - **Size:** about 12 px, about #959595, written with thousands separators, for example "1,280 x 720".
- **Platform tiles** (Advanced tab): about 135 × 115 px, glyph and name only.
- **Selected tile:** a 2 px blue border. The printed sample is about #4D78C5; on screen it is a Spectrum blue [high · AL-ws, which describes the selection as blue; figures].
- **Scrolling:** the pane scrolls with a thin scrollbar, about 5 px wide and about #4E4E4E [high on the Advanced tab · CIB24-fig].

**Body, Details panel** (x 502 to 760, about 258 px wide)
- **Panel:** runs the full height of the body. A 15 px margin on the right and a 9 px margin at the bottom show the #1C1C1C background.
- **Style:** background about #2E2E2E, inner padding about 23 px. Its heading, "DETAILS", has the same style as "PRESETS".
- **Fields:** a label above each one (about 12 px, about #999999).
  - Inputs are **31–32 px tall**, fill about #232323, with a 1 px border about #363636 and a small corner radius.
  - Values are about 13 px, about #D0D0D0.
  - Each label-plus-field block takes about 70 px of height.
- **Fields, top to bottom:**
  1. **Width** and **Height** side by side, each 93 px wide, 25 px apart. The values have **no "px" suffix** [high].
  2. **Units:** a full-width dropdown (about 212 px) with a small V-shaped chevron at the right. On the intent tabs it shows "Pixels" and is **greyed out and disabled** [high · CIB23-fig]; on the Advanced tab it is enabled [high · CIB24-fig].
  3. **Frame Rate:** 93 px wide, shown with two decimals, for example "30.00" [high].
  4. On the intent tabs, a **Platform Type** dropdown at full width, for example "ActionScript 3.0" [high · CIB23-fig]. On the Advanced tab this is **replaced** by the platform name in grey and a one-line description of the platform; for HTML5 Canvas it talks about canvas-based assets, interactivity and publishing web content [high · CIB24-fig]. minnamate writes its own descriptions.

**Buttons** (inside the Details panel, bottom right, about 23 px from its right and bottom edges)
- **Create:** a blue pill about 72 × 31 px with bold white text. The printed sample is about #4673BF; the screen colour is in the Spectrum accent-blue family (about #378EF0).
- **Make Default:** on the Advanced tab only. An outlined pill about 122 × 31 px with a light 1.5–2 px border and white text, about 12 px to the left of Create [high · CIB24-fig].
- **There is no Cancel button and no footer** [high · figures]. You close the dialog with the title-bar X. Esc probably works too [low].

**Behaviour**
- **Clicking a tile** loads its values into Details, where you can adjust them [high · CIB23].
- **Clicking a tab** shows that tab's tiles and selects the first one [med · CIB23-fig].
- **Factory first open** shows Character Animation with **HD** selected [med · CIB23-fig]. Whether the dialog reopens on the last-used tab is unknown [low].
- **Create** opens "Untitled-N" with the Details values [high].
- **Shortcuts:** double-clicking a tile, or pressing Enter, creates the document [low].
- **Make Default** stores the Details values as the defaults for new documents [existence: high · CIB24-fig; behaviour: med · LIL-NF, CF-preset].
- **Value ranges:** width and height 1–8192 [low–med · R; the help page's 2880 limit dates from CS5.5]; frame rate 0.01–120 [med · R].
- **Units options. Resolved order:** Inches, Inches (decimal), Points, Centimeters, Millimeters, Pixels [med · P1]. This area listed Pixels first [low–med · R].

**Presets** (store these as a data table)

| Tab | Platform | FPS | Presets |
|---|---|---|---|
| Character Animation | ActionScript 3.0 [high · CIB23, CIB23-fig, LIL-ET] | 30 [high · CIB23-fig, LIL-ET] | "PRESETS (4)", in this order: **HD 1,280 x 720 · Full HD 1,920 x 1,080 · 4K 3,840 x 2,160 · Standard 640 x 480** [high · CIB23-fig] |
| Social | unknown [low] | unknown [low] | Presets for Facebook, Twitter and YouTube, including Twitter "In-stream Photo" [med · AL-soc]. Sizes not verified [low] |
| Game | unknown | unknown | Phone and device sizes [med · CF-start]. "Android 16:9" is probably here, since its Home glyph is a gamepad [low–med] |
| Education | unknown | unknown | "iPad 3-4 Gen" is probably here, since its Home glyph is a mortarboard [low–med] |
| Ads | HTML5 Canvas [high · CIB23] | unknown | **Square 250 x 250** [high · CIB23]. "Banner" may be here [low]. Standard IAB ad sizes as placeholders [low] |
| Web | unknown | unknown | Includes 640 x 480 [med · AL-ws] |

- Frame rates differ from preset to preset [high · CF-fps].
- The Home "Canvas" tile probably means Advanced > HTML5 Canvas with the default values [low].

**Advanced tab** [high · CIB24-fig, unless noted]
- **PLATFORMS (5):** HTML5 Canvas, ActionScript 3.0 and AIR for Desktop in the first row; AIR for iOS and AIR for Android in the second. This order replaces one taken from AL-doc's narration.
- **BETA PLATFORMS (4):** VR Panorama, VR 360, then the WebGL-glTF tiles.
  - Each tile has a small "BETA" tag at the top right of its glyph.
  - The fourth tile is scrolled out of view; it is probably WebGL-glTF Extended [med · H-wn19].
- **Script files:** a third section offering ActionScript and Animate JavaScript files may sit below [low · H-as].
- **Glyphs, in our own words:**
  - **HTML5 Canvas:** a monitor with a shield emblem.
  - **ActionScript 3.0:** a page with a folded corner, marked "</>" and "AS".
  - **AIR platforms:** a page or phone outline with a stylised mark.
  - **VR Panorama:** a curved panorama frame with mountains.
  - **VR 360:** a wireframe sphere marked "360°".
  - **WebGL:** a rounded badge.
  - minnamate must draw its own generic glyphs and not copy the HTML5 or AIR marks.
- **Factory Details values:** frame rate **24.00** [high · CIB23, CIB24-fig]; size **550 × 400 px**, units **Pixels** [med · H-doc, H-stage].
- **Default selection:** HTML5 Canvas is selected when the tab opens [low–med].
- **minnamate:** enable only ActionScript 3.0 and show the other tiles disabled.

### 4.4 The default new document and "Untitled-1"

**Defaults**
- **Create right away** on a factory first open gives **Character Animation / HD: 1280 × 720, 30.00 fps, ActionScript 3.0, white stage** [med · CIB23-fig]. Full HD is the default **Home tile**, not the dialog's default.
- **Advanced / Make Default baseline:** 550 × 400, 24 fps, white, Pixels [med].

**On first open**
- **Document tab:** "Untitled-1". The number counts up within the session [med · R]. After the first edit, an asterisk (*) appears in the tab and in the title bar [high · H-doc].
- **Edit bar:** at the left a clapperboard icon, "Scene 1" and a chevron; at the right the zoom field showing "100%", with a stepper and a dropdown [high · CIB23-fig]. Part 1 has the full edit-bar layout.
- **Zoom:** **100%**, with the stage centred. The view is **not** fitted to the window [med; P1 agrees]. In LIL-ET a 1280 × 720 document opens at 100%, and the presenter then chooses Fit in Window by hand. Zoom range is 8% to 2000% [high · H-stage].
- **Timeline:**
  - One layer, **Layer_1**, selected (highlighted), with its colour chip; the first layer's colour is cyan #00FFFF [high · P1].
  - Frame 1 holds a **blank keyframe**, drawn as a hollow circle, and the playhead is on frame 1.
  - The header shows "30.00 FPS" (or "24.00 FPS") and "1 F".
  - [high · CIB23, CIB23-fig, H-time]
- **Stage:** white, on the dark-grey pasteboard, with the pasteboard visible [high · CIB23].
- **Properties panel:** tabs Tool, Object, Frame, Doc, with **Doc** active [high · CIB23-fig, H-pi]. With the Selection tool and nothing selected, only Doc is enabled [high · P1].
- **Other state:** the Library is empty, the document is not dirty, and the undo history is empty [high · R].

### 4.5 Document Settings dialog

**Opening it**
- **Modify > Document…** (**Ctrl+J**) [menu path: high · H-doc; shortcut: med · R, P1, plus a weak third-party shortcut list].
- **Properties > Doc tab > Document Settings > "More settings"** (sentence case) [high · CIB23-fig]. In v19 this button was labelled "Advanced Settings" [high · LIL-NF, H-canvas].
- **Title:** "Document Settings" [high].

**Rows, top to bottom** (row order [med · R]; each row's existence is tagged separately)
1. **Units:** a dropdown, default Pixels [med · R], with the options in §4.3. It also sets the units for the rulers and the Info panel [high · H-doc, H-stage, where it is called "Ruler Units"].
2. **Stage size:** a Width field, a link toggle, a Height field, and a **"Match contents"** button.
   - Link and Match contents exist [high · H-stage, H-pi, H-vr].
   - The button's casing here is copied from the Properties panel [high · CIB22-fig, CIB23-fig]. P1 writes "Match Contents" for the dialog; the dialog's casing is [low].
3. **Scale:** [ ] **"Scale Content"** [high · H-stage].
   - An indented sub-checkbox for locked and hidden layers may follow [low]. In CS5.5 the "Include Locked and Hidden Layers" option lived in Preferences instead.
4. **Anchor:** a 3 × 3 grid of cells [high · H-stage]. **Resolved default:** the centre cell [P1, read with H-stage]; this area guessed top-left [low].
5. **Stage color:** a swatch, white by default [high], with [ ] **"Apply to pasteboard"** next to it. The label is confirmed in the Properties panel [high · CIB22-fig, CIB23-fig]; its presence in this dialog is [med].
6. **Frame rate:** a field showing "24.00" [high].
   - [ ] **"Scale Spans"** is confirmed only in the Properties panel, unchecked by default [high · CIB22-fig, CIB23-fig]. Help text calls it "Scale Frame Spans" [P1].
   - Whether it also appears in this dialog is [low].
7. [x] **"Use Advanced Layers"**, on by default [high · H-layers, P1; LIL-NF for v19]. Still verify it on v24.
8. **Buttons:** **Make Default** on the left [high · H-doc, H-layers, CF-preset]; **Cancel** and **OK** on the right, OK in blue [existence: high; order and colour: low]. P1 lists them as Make Default, Cancel, OK.

**Behaviour**
- **On open:** the fields show the active document's current values [high].
- **Link toggle:** off by default. When on, Width and Height keep their proportions [high · H-stage].
- **Scale Content on:** Width and Height are forced to linked and the link toggle is disabled [high · H-stage]. Content on every frame is scaled [med · CS5.5 help].
- **Scale Content off:** the anchor sets the direction of growth [high · H-stage]. Example: 550 × 400 → 750 × 600 with the bottom-right anchor makes the stage grow to the left and upward.
- **Match contents** [low]: one of two behaviours, to verify. Either the stage becomes the content's bounding box and the content moves to (0,0); or, as in the classic Flash wording recalled here, the stage is resized so the space around the content is equal on all sides and the content does not move.
- **Stage color swatch:** opens the shared swatch popup (§3.1). HTML5 Canvas documents also offer **No Color** and **Alpha %** [high · H-canvas, H-stage].
- **Scale Spans with a frame-rate change:** every span is multiplied by new fps ÷ old fps, so durations in seconds stay the same [med · CIB22].
- **Units change:** the displayed values convert at 72 pixels per inch [med · R].
- **Make Default:** saves the current values as the defaults for new documents. They then appear in New Document > Advanced [high · CF-preset]. The dialog stays open [low].
- **OK, Cancel and keys:** OK applies everything as one undo step [low]. Cancel or Esc discards; Enter means OK [med · R].
- **Validation:** Width and Height 1–8192, frame rate 0.01–120. Bad input shows an alert and reverts [low].

**Properties-panel copy of these settings** (Doc tab > "Document Settings"; reuse these widgets) [high · CIB22-fig, CIB23-fig]
- A section header with a disclosure chevron.
- **W** and **H** fields stacked vertically, joined on their right by a thin bracket with a small padlock-style link icon at its midpoint. Adobe calls the toggle "Link"; it is unlinked by default [high · P1].
- A "Match contents" button.
- [ ] Scale Content.
- A **Stage** swatch beside [ ] Apply to pasteboard.
- An **FPS** field beside [ ] Scale Spans.
- A full-width "More settings" button.
- Above this section there is a "Publish Settings" section with a Profile dropdown and its own "More settings" button.
- P1 lays the section out in two columns (left: W/H, Stage, FPS; right: Match contents and the three checkboxes), shows units only as a "px" suffix, and gives the defaults 550 × 400 px, 24 fps and a white stage.

**Dialog geometry and style** [low · R; no figure of this dialog was available]
- **Overall:** dialog about 500 × 380. Rows 28 px tall. Number fields 72 × 24. Swatch 22 × 22. Buttons 28–32 px tall.
- **Anchor grid:** cells 9 × 9 with 3 px gaps. The selected cell is filled #E3E3E3; the others have a 1 px #909090 outline. When disabled, the whole grid drops to 40% opacity.
- **Checkbox:** 14 × 14. When checked, a blue fill with a white tick, as in the Properties-panel figure [med]. P1's Properties captures show a different look: a 14 px box with a 1.5 px #9A9A9A border that, when checked, fills #C8C8C8 with a #262626 tick [med · P1]. Unresolved; see §7.3.
- **Colours:** dialog about #323232; fields about #232323 with a #363636 border, taken from the New Document measurements; labels about #C8C8C8.

### 4.6 Related items, recommended to skip

- **File > New from Template…** [high · H-tmpl, H-doc]:
  - a Category list: Advertising, Animation, Banners, Media Playback, Presentations, Sample Files;
  - a Category Items list, a preview, a description, and OK / Cancel.
- **Also skip:** the v20.5 onboarding panel and the first-run question; the Home tutorials; the VR, WebGL and AIR platforms.
- **Theme. Resolved:** v24 offers Light, Dark and Darkest at Edit > Preferences > Edit Preferences > Interface > Color Theme, and Dark is the default [high · P1, from the full LIL-SPEC transcript and WN24]. This area only had the truncated narration listing light, dark and a third [med · LIL-SPEC]. v19 to v23 had Darkest, Dark, Light and Lightest [high · H-wn19, LIL-NF]. The default is dark grey [high · CIB23].

### 4.7 Build order within this area

1. **Home:** Create New, Open, and the "Start a new file fast" strip with the six v23 tiles. Full HD and More Presets work; the other tiles open New Document on the matching tab. A recent-files list.
2. **New Document:** seven tabs with icons, and presets from a data table. Details: W, H, Units (disabled), FPS, and Platform Type locked to ActionScript 3.0. Create inside the Details panel; Make Default on the Advanced tab; no Cancel button.
3. **Untitled-N:** Layer_1 with a blank keyframe on frame 1, 100% zoom, and the Properties panel on the Doc tab.
4. **Document Settings:** first stage size with the link toggle, Match contents, stage colour, frame rate, Make Default, and OK / Cancel; later Scale Content, Anchor and Scale Spans.

---

## 5. Resolved contradictions (summary)

Each row is explained where it appears. "P1" is the part-1 research.

| Topic | Use | Instead of | Why |
|---|---|---|---|
| First layer outline colour (§1.0) | #00FFFF | #4F80FF | [high · P1] saved files and screenshots beat [med · R] |
| Hide Edges key (§1.1) | Ctrl+Shift+E | Ctrl+H | [high · P1] beats [med · R] |
| Lasso group keys (§1.3) | Lasso L, Polygon Shift+L, Magic Wand none | L for all three | P1 read the shortcut dialog; SEL prints the slot key |
| Object Drawing glyph and state (§1.6, §3.4) | 13×13 square with a 9 px grey disc; off by default; remembered; quick action in the Tool tab | circle in a square; persistence and placement guessed | [high · P1] beats [low] |
| Classic Brush mode label (§1.6, §3.5) | "Paint Fills Only" | "Paint Fills" | current help and P1; a 2020 capture disagrees, so check |
| Snap to Objects location (§1.8) | Doc-tab magnet, View > Snapping, stage right-click | Selection-tool modifier | [high · P1] |
| Edit Document key (§1.9) | Ctrl+E at [high] | Ctrl+E at [med] | P1 |
| Breadcrumb separator (§1.9) | ">" as in part 1 | glyphs plus a gap | tie at [low]; follow part 1 |
| Onion button position (§1.10) | first in "Onion, Edit Multiple, Tween group, Center Frame" | just left of Loop | [high · P1] beats [med · TL] |
| Onion menu label (§1.10) | "Advanced Settings" | "Advance Settings" | UI evidence (community, P1) over the help page's spelling; check |
| Onion settings (§1.10) | floating panel; 70% / 20%; 2 + 2 frames | modal dialog; 75% / 15%; range unknown | P1 screenshot beats [low] |
| Onion ruler markers (§1.10) | lightened band between 2 px past and future bars | bracket handles | [med · P1] beats [low · R] |
| Edit Multiple Frames menu (§1.10) | "Selected range / All frames / Anchor markers" | "Edit selected range / Edit all frames" | screenshot [high · P1] beats help prose |
| Frame selection colour (§2.1) | #5A8AD0, opaque on empty cells, about 50% over spans | #2A94FF at about 35% | tie at [med]; P1 is newer and covers empty cells |
| Selected layer row (§2.1) | #40618E | #366192 | tie at [med]; P1 is from 2024 |
| Playhead (§2.1) | #4A80CC | #3479C2 | tie at [med]; P1 is from 2024 |
| Time-stretch factors (§2.2) | x0.5, x2, x4 attested | x3 attested | CIB22 [high] shows no x3 |
| Normal frame menu (§2.3) | 2020 Windows order: Classic first, F6/F7 on the Convert items, Show In Library, no Copy Motion as ActionScript 3.0 | CS6-recall order | [high · P1] beats [med] |
| Convert to Frame-by-Frame submenu (§2.3) | Keyframe every frame / other / third / fourth frame / Custom... | presets on 1s, 2s, 3s plus custom | [high · P1] beats [med] |
| 2024 layer header (§2.4) | New Layer, New Folder, Delete in the toolbar row; Layer View in the header | all in the header row | [high · P1] 2024 figures |
| Toggle column pitch (§2.4) | 25 px | 24 px | tie at [med]; P1 is from 2024 |
| Hover and hidden cells (§2.4) | dim eye-slash and padlock on hover; bright eye-slash when hidden | dots on hover | [med · P1] beats [low] |
| Motion-tween layer icon (§2.4) | diamond cut by a right arrow, 16×11 | double-headed arrow with a diamond | same book; P1 adds 2024 screenshots |
| Keyframe arrows (§2.4) | in the centred toolbar cluster | at the right end of the toolbar | [high · P1] beats [med] |
| Layer menu labels (§2.5) | SS19 labels (Show Others Transparent; plural Delete / Expand / Collapse Folders) | P1's prose labels | [high · SS19] beats [med] |
| Frame View menu ending (§2.7) | Close, Close Group | not listed | [high · SS20] |
| Timeline zoom by wheel (§2.7) | Alt+wheel | Ctrl+wheel | [med] beats [low] |
| Short row height (§2.7) | 18–19 px | about 16 px | [med · SS20] beats [low] |
| Properties chip shape (§3.0) | 30×16 rounded rectangle | 20×20 square | [high · P1] beats [low] |
| No Color red (§3.0) | #E0302A | #FF0000 | [high · P1] beats [med] |
| Numeric fields (§3.0) | white text on a grey underline | light-blue hot text | [high · P1, 2020–22] beats [low]; 2024 to check |
| Sliders (§3.0) | 14 px hollow knob on a 2 px track | 10 px #C8C8C8 thumb | [med · P1] beats [low] |
| Panel menu ending (§3.2, §3.3) | Close, Close Group | Close Tab Group | SS20 and P1 menus beat recall |
| Color panel key (§3.2) | Ctrl+Shift+F9 at [high] | Ctrl+Shift+F9 at [med] | P1 |
| Tool tab header (§3.4) | "Rectangle", row about 48 px | "Rectangle Tool", row about 34 px | P1 measured |
| Color and Style rows (§3.4) | one row per colour; segmented Cap and Join; "⋯" after Style and Width; Join order Miter, Bevel, Round | one row with glyphs; icon dropdowns; rows behind "…" | [high / med · P1] |
| Up/Down radius keys (§1.6, §3.5) | [high] for both rectangle tools | [med] / [low] | LINES, P1 |
| Classic Brush shape dialog (§3.5) | "Nib Options" | "Brush Options" | [high · P1] beats [low] |
| Classic Brush size and tablet toggles (§3.5) | 1–200 at [high]; tablet only at [high] | [med] | P1 |
| Units order (§4.3) | Pixels last | Pixels first | [med · P1] beats [low–med] |
| Anchor default (§4.5) | centre | top-left | P1 beats [low] |
| Use Advanced Layers default (§4.5) | on, at [high] | on, at [med] | H-layers, P1 |
| v24 themes (§4.6) | Light, Dark, Darkest; default Dark | light, dark and a third | [high · P1] |

**Left open** (the sources tie or are all weak): Free Transform drag feedback (build live redraw plus the box), the default scale origin for instances, the new-corner-point modifier (accept both Ctrl and Alt), the Alt-click dialog titles (per-tool titles for now), the marquee colour on the pasteboard (theme.rs keeps #7A7A7A for now) and the checkbox look. **Provisional**, chosen above but still waiting for a 2024 screenshot: the colour of numeric fields, the words-not-glyphs Properties colour rows, the "Paint Fills Only" label and the "Advanced Settings" label. All of these are in §7.

---

## 6. Sources

P1 refers to the part-1 research; its sources are listed in `animate-ui.md`.

**Stage interaction (§1)**
- SEL https://helpx.adobe.com/animate/using/selecting-objects.html
- PREF https://helpx.adobe.com/animate/using/set-preferences.html
- DPREF (= H-pref) https://helpx.adobe.com/animate/using/drawing-preferences.html
- LINES (= H-draw) https://helpx.adobe.com/animate/using/draw-simple-lines-shapes.html
- MODES https://helpx.adobe.com/animate/using/drawing.html
- ARR https://helpx.adobe.com/animate/using/arranging-objects.html
- RESHAPE https://helpx.adobe.com/animate/using/reshape-lines-shapes.html
- ONION (= FBF) https://helpx.adobe.com/animate/using/frame-by-frame-animation.html
- TL https://helpx.adobe.com/animate/using/timeline.html
- TF https://helpx.adobe.com/animate/using/transforming-combining-graphic-objects.html
- SYM https://helpx.adobe.com/animate/using/symbols.html
- SNAP https://helpx.adobe.com/animate/using/snapping-art-position.html
- STAGE https://helpx.adobe.com/animate/using/using-stage-tools-panel.html
- PEN https://helpx.adobe.com/animate/using/drawing-pen-tool.html
- MOVE https://helpx.adobe.com/animate/using/moving-copying-objects.html
- WN23 https://helpx.adobe.com/animate/using/whats-new/2023.html
- WN24 https://helpx.adobe.com/animate/using/whats-new/2024.html
- CS5 https://help.adobe.com/archive/en_US/flash/cs5/flash_cs5_help.pdf
- CIB24 (chapter 2) https://www.peachpit.com/articles/article.aspx?p=3192442 (seqNum=3 to 6)
- FADE https://community.adobe.com/t5/animate-discussions/what-happened-to-the-faded-bg-when-entering-a-symbol/m-p/10849234
- BOX https://community.adobe.com/t5/animate-discussions/how-can-i-change-bounding-box-on-my-symbols/td-p/13459776
- R: recall.

**Timeline (§2)**
- helpx pages, read from 2025 archive snapshots by the first draft and not re-read:
  - FK https://helpx.adobe.com/animate/using/frames-keyframes.html
  - TL https://helpx.adobe.com/animate/using/timeline.html
  - LY https://helpx.adobe.com/animate/using/timeline-layers.html
  - SP https://helpx.adobe.com/animate/using/setting-animation-tween-spans-in-the-timeline.html
  - CT https://helpx.adobe.com/animate/using/classic-tween-animation.html
  - FBF https://helpx.adobe.com/animate/using/frame-by-frame-animation.html
  - SND https://helpx.adobe.com/animate/using/using-sounds.html
  - BAS https://helpx.adobe.com/animate/using/animation-basics.html
  - WN19, WN20, WN21 https://helpx.adobe.com/animate/using/whats-new/2019.html, …/2020.html, …/2021.html
- JSFL https://github.com/AdobeDocs/developers-animatesdk-docs
  - Layer_object: layer1.md (color), layer4.md (height), layer5.md (layerType).
  - Timeline_object: timeline.md (addMotionGuide), timelin2–5 (clear and convert), timeli16 (deleteLayer), timeli27 (insertBlankKeyframe), timeli28 (insertFrames), timeli29 (insertKeyframe), timeli35 (pasteLayers), timeli45 (setLayerProperty), timeli48 (showLayerMasking), timeli51 (mergeLayers).
- CIB22 https://www.peachpit.com/articles/article.aspx?p=3131585 (seqNum 4, 5, 7, 14) and the sample PDF https://www.peachpit.com/content/images/9780137623587/samplepages/9780137623587_Sample.pdf (screenshots on book pp. 121–131, 137 and 151–152).
- CIB23 (Lesson 1, pp. 21–27) and the CIB 2024 sample index, as cited by the first draft.
- Community threads (text and attached screenshots):
  - SS19 https://community.adobe.com/t5/animate-discussions/layer-transparency/m-p/10396605
  - SS20 and COM-frame-height https://community.adobe.com/t5/animate-discussions/how-can-i-adjust-frame-height-in-the-timeline-in-animate-2020/m-p/10742431 (panel menu, Preferences)
  - SS20, PH20 and COM-selecting https://community.adobe.com/t5/animate-discussions/selecting-frames-on-timeline/m-p/10842447 (header and menu)
  - SS21 and COM-transparent https://community.adobe.com/questions-540/layers-show-others-transparent-137272 (Shift+click on the header eye)
  - COM-keeps-selecting https://community.adobe.com/t5/animate-discussions/animate-keeps-selecting-everything/m-p/14018589
  - COM-zoom https://community.adobe.com/t5/animate-discussions/timeline-zoom-resize-hotkey-shortcut-mouse-control/m-p/11071973
  - https://community.adobe.com/t5/animate-discussions/shortcut-to-extend-the-duration-of-multiple-frames-at-once/m-p/10010738
  - COM-trim-audio https://community.adobe.com/t5/animate-discussions/trim-audio/m-p/13267658
- R: recall of Animate 2020–2024 and Flash CS6 behaviour.

**Colour controls and tool properties (§3)**
- H-color https://helpx.adobe.com/animate/using/color.html (403 on direct fetch; search-engine extracts) and https://helpx.adobe.com/animate/desktop/multimedia-and-video/color.html
- H-colors https://helpx.adobe.com/animate/using/colors.html
- H-sfg https://helpx.adobe.com/animate/using/strokes-fills-gradients.html (403)
- H-panels (= H-pi) https://helpx.adobe.com/animate/desktop/workspace-and-workflow/authoring-panels.html (403)
- H-draw https://helpx.adobe.com/animate/using/draw-simple-lines-shapes.html
- H-pref https://helpx.adobe.com/animate/using/drawing-preferences.html
- H-pb https://helpx.adobe.com/animate/using/working-with-paint-brush.html
- H-brush https://helpx.adobe.com/animate/using/custom-brushes.html
- H-fluid https://helpx.adobe.com/animate/using/fluid-brushes.html
- L-grad https://www.adobe.com/learn/animate/web/make-gradients-color-panel
- L-color https://www.adobe.com/learn/animate/web/managing-color-in-a-document
- L-draw https://www.adobe.com/learn/animate/web/drawing-in-animate-cc
- C-drag https://community.adobe.com/questions-540/in-animate-23-0-1-i-can-no-longer-drag-from-the-fill-color-swatch-to-sample-a-single-flat-color-from-a-gradient-it-selects-the-object-instead-of-sampling-color-1550053
- C-spec https://community.adobe.com/questions-540/color-swatch-colors-not-accurate-picking-wrong-colors-143110
- C-stroke https://community.adobe.com/t5/animate-discussions/where-is-the-stroke/m-p/14899514/highlight/true
- C-oval https://community.adobe.com/questions-540/how-to-reset-oval-tool-106916
- C-odm-a https://community.adobe.com/t5/animate-discussions/i-cannot-find-quot-object-drawing-mode-quot-but-every-tutorial-has-it/td-p/12838167
- C-odm-b https://community.adobe.com/t5/animate-discussions/can-t-turn-object-drawing-mode-off/m-p/13226194
- C-odm-c https://community.adobe.com/t5/animate-discussions/object-drawing-mode-doesn-t-work-with-the-pen-tool-version-21-0-1/m-p/11720420/highlight/true
- C-swatch https://community.adobe.com/t5/animate-discussions/my-swatches-panel-has-become-all-mixed-up-how-do-i-get-it-back-to-normal/m-p/9004168
- O-f8 https://www.oreilly.com/library/view/flash-8-cookbook/0596102402/ch01s26.html
- JSFL https://github.com/AdobeDocs/developers-animatesdk-docs (Stroke_object/stroke_summary.md and stroke1, 11, 13, 16, 20, 21.md; Fill_object/fill5, 7, 9.md; Oval_object/OvalObjec2, 3.md; Rectangle_object/RectangleObject_summary.md and RectangleObjec2, 3.md)
- R: recall of the Flash CS3–CS6 and Animate CC 2015–2024 UI.

**Home, New Document and Document Settings (§4)**
- H-doc https://helpx.adobe.com/animate/desktop/introduction-to-animate/documents.html
- H-stage https://helpx.adobe.com/animate/desktop/workspace-and-workflow/using-stage-tools-panel.html
- H-layers https://helpx.adobe.com/animate/desktop/workspace-and-workflow/timeline-layers.html
- H-tmpl https://helpx.adobe.com/animate/desktop/workspace-and-workflow/templates.html
- H-pi https://helpx.adobe.com/animate/desktop/workspace-and-workflow/authoring-panels.html
- H-canvas https://helpx.adobe.com/animate/desktop/platforms/creating-publishing-html5-canvas-document.html
- H-vr https://helpx.adobe.com/animate/desktop/interactivity/virtual-reality.html
- H-as https://helpx.adobe.com/animate/desktop/platforms/actionscript.html
- H-time https://helpx.adobe.com/animate/using/time.html
- H-24 https://helpx.adobe.com/animate/desktop/using/whats-new/2024.html
- H-wn19, H-wn20, H-wn20.5 (archived copies) https://helpx.adobe.com/animate/using/whats-new/2019.html, …/2020.html, …/2020-2.html
- CIB23: 2023 release, ISBN 9780137982424, Lesson 1 sample, text and figures on pp. 10–15 and 18–21.
- CIB24: https://ptgmedia.pearsoncmg.com/images/9780138317713/samplepages/9780138317713_Sample.pdf (p. 54 text, p. 55 figure).
- CIB22: https://ptgmedia.pearsoncmg.com/images/9780137623587/samplepages/9780137623587_Sample.pdf (p. 131 text and Properties-panel figure).
- LinkedIn Learning:
  - LIL-ET https://www.linkedin.com/learning/adobe-animate-essential-training/choosing-a-document-type, …/creating-a-document-for-animation, …/modifying-document-settings
  - LIL-NF https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/modern-interface-and-start-screen
  - LIL-UI https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/user-interface-improvements
  - LIL-MOD https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/application-modernization
  - LIL-SPEC https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/modernized-adobe-spectrum-interface
- adobe.com/learn:
  - AL-doc https://www.adobe.com/learn/animate/web/document-type-and-usage
  - AL-ws https://www.adobe.com/learn/animate/web/manage-workspace
  - AL-soc https://www.adobe.com/learn/animate/web/design-content-for-social-media
- Community threads:
  - CF-new https://community.adobe.com/t5/animate-discussions/file-gt-new-is-broken/m-p/15492067
  - CF-start https://community.adobe.com/t5/animate-discussions/how-to-disable-the-new-startscreen-in-animate-cc-2019/m-p/11160089
  - CF-preset https://community.adobe.com/t5/animate-discussions/how-do-i-save-a-new-document-preset-in-adobe-animate/m-p/11026695
  - CF-fps https://community.adobe.com/t5/animate-discussions/is-it-possible-to-change-the-fps-in-adobe-animate-2020/m-p/14655811
  - CF-24 https://community.adobe.com/t5/animate-discussions/change-to-24-fps/m-p/11463126/highlight/true
  - CF-res https://community.adobe.com/t5/animate/my-adobe-animate-resolution-is-too-large/m-p/10690814
- Other: the Flash Professional CS5/CS5.5 user guide (only for the old "Scale content" and "Match Contents" wording); http://tapanvfx.blogspot.com (a weak source: a shortcut list showing "Document… Ctrl+J"); R (recall) for the items tagged [low] or R.

---

## 7. Open questions

The fact-checkers' open questions and screenshot checklists, merged and de-duplicated. Where part 1 already suggests an answer, it is noted, but the question stays until a 2024 screenshot confirms it. One general limit applies to every area: the shared web-search budget was used up, helpx blocks scripted fetches and the archive was partly blocked, so a follow-up session with a fresh search budget, or user-supplied Animate 2024 screenshots or saved .fla files, is what closes the [low] items.

### 7.1 Stage interaction

Check these first against a real Animate 2024 (screenshot diff), in this order:
1. The selection dot tile: geometry, spacing and the dark companion pixel, including at 200% UI scale.
2. The default highlight hex values, whether "Use layer color" starts on, and whether Highlight color moved from General to the new Interface category in 24.x.
3. Free Transform handle size and transformation-point circle size, and whether move and Free Transform drags redraw the object live or only preview the bounding box (as TF and CS5 say).
4. Marquee and lasso colour on the white stage versus the dark pasteboard. Black (spec, recall) would vanish on the pasteboard; `apps/editor/src/theme.rs` currently uses `MARQUEE` #7A7A7A and `SELECTION_BOX` #1F8FFF. Pick one after the check.
5. Snap ring sizes, the Snap Align line colour and the guide green hex.
6. How much Edit in Place dims the rest of the stage, and the default View > Preview Mode (the fade disappears in "Full", per FADE).
7. Onion skin: past, present and future hex values; whether 70% / 20% and the 2 + 2 range still hold in 24.x; how the ruler markers are drawn; the exact long-press labels ("Advanced Settings" or "Advance Settings"); whether onion skins show during playback.
8. The Edit-bar breadcrumb separator: none, ">" or glyphs only.
9. The default scale origin for symbol instances: the opposite handle (recall; CIB24 by implication) or the transformation point (CS5 p119).
10. Whether the Brush cursor is filled with the fill colour, and whether the Hand cursor switches to a closed hand while dragging.
11. Where the Free Transform mode buttons live in 2024: the Tools-panel options area or Properties > Tool. (Snap To Objects is settled by P1.)
12. The new-corner-point modifier on Windows: Ctrl (helpx RESHAPE), Alt (CIB24), or both.

Further questions:
- Which version introduced the drawing-object outline with anchor circles? It is not in the 23.0 or 24.0 summaries, and point releases were not checked.
- The Alt-click shape dialog: per-tool titles ("Rectangle Settings", "Oval Settings") or one "Oval and Rectangle Settings" dialog?
- The Brush mode label in 24.x: "Paint Fills Only" or "Paint Fills"?
- The Snap To Objects shortcut: P1's own sections disagree (Ctrl+Shift+/ in its menu listing, Ctrl+Shift+U from the shortcut dialog).

### 7.2 Timeline

- Re-verify every helpx-based [high] tag (FK, TL, LY, SP, CT, WN20) against live pages or a v24 install. In the fact-check helpx returned 403, web.archive.org was blocked and the search budget was used up, so those tags carry over from the first draft without re-reading.
- The top block of the v24 frame menu for classic, shape and motion tween spans: are the labels "Remove Classic Tween" and "Remove Shape Tween"? Where does "Convert to Frame-by-Frame Animation >" sit, and is it greyed on non-tween frames? (The 2020 Windows menu settles the normal-frame order.)
- Are F6, F7 and Shift+F6 still printed only on the Convert items and Clear Keyframe in v24, as in 2020?
- What follows Join Motions in the motion-tween menu, and in what order (Save as Motion Preset..., Motion Path >, 3D Tween, Convert to Frame-by-Frame Animation >, Synchronize Symbols, Actions)?
- Does "Copy Motion as ActionScript 3.0..." appear in the frame menu of AS3 documents?
- Glyphs in 2020 and later: the hidden-layer mark (crossed eye or red X); the transparent-layer mark; whether hover shows dim glyphs (P1 says so) rather than dots; what the highlight cell shows when it is on; the mask, masked, guide, motion-guide and camera layer icons.
- Does any pencil or crossed pencil still appear in v24, for example on a locked or hidden active layer?
- Frame View defaults and sizes: the default row height and the pixel heights of Medium and Tall; the Match FPS default; what Lock does on the Timeline panel.
- Does v24 on Windows 11 still use native Win32 popup menus, and with the Windows 11 rounded corners and grey hover? This decides the colours in §2.8.
- The layer context menu in v24: the position and ellipsis of "Convert Layers to Symbol"; any other 2020–2024 additions; whether "Show Others Transparent" was renamed "Show Others as Transparent", the wording TL used in 2023.
- Duplicate Layers naming: "Layer_1 copy" or "Layer_1_copy"? Is the default folder name "Folder_1"?
- Did the Layer Properties dialog change after v19: a Spectrum restyle, a different body colour, different button placement?
- The exact selection tint and alpha in 2024; whether whole-layer selections also get the bright outline; whether the navy lines around the selected layer row are still drawn. (The v24 default theme is settled: Dark, with #323232 panels [P1].)
- Which time-stretch factors produce a flag (x3? x1.5? x0.25?), and are there modifier keys during the stretch?
- The exact results of Clear Frames (a blank keyframe plus a trailing keyframe?) and Cut Frames (a clear or a removal?).
- In Span Based Selection mode, what do Ctrl+Alt+click and Ctrl+Alt+drag do? How does Shift+click on a tween combine with an existing range in frame-based mode?
- Folder rows in the frame grid: a darker hatched strip (CIB22) or plain empty cells (P1's guess)?

### 7.3 Colour controls and tool properties

1. Does picking No Color on a selected shape's stroke or fill chip in the 24.x Properties panel remove that part, or is the button hidden or blocked as the legacy help text says?
2. Which rarer options sit behind the "⋯" buttons in 24.x? Is there a Brush Library button, and does the style read "Solid" or "Solid line"?
3. The Object Drawing toggle in the 24.x Tool tab: P1 shows it as a quick action (the 2022 Tools-panel location is confirmed); confirm its exact position.
4. Swatch popup in 24.x: does the tagged-swatch group sit above the grid? What are the 7 default gradients and their order? Is the current colour marked? Do chips still draw the small corner triangle?
5. Color panel in 24.x: are the stroke and fill controls side by side or stacked? Which side are the H/S/B/R/G/B fields on? Which radio is selected by default? What are the pixel sizes after the C-spec resize, and how big are its chips?
6. The Classic Brush custom-shape dialog title ("Nib Options" per P1, or something else), and the exact option order for Classic Brush, Fluid Brush (is there a Min. size?) and Eraser in the 24.x Tool tab.
7. The first-run default fill colour in Animate 2024. #0066CC is only P1's [low] reading; the Black and White defaults (white fill, black stroke) are confirmed.
8. Can typed rectangle corner radii exceed ±100? `panels.rs` currently accepts up to 999.
9. Numeric fields in 24.x: white text on a grey underline (2020–22, P1) or light blue like the 2024 timeline readout?
10. Do the Properties colour rows carry bucket and pencil glyphs (H-panels) or only the words "Fill" and "Stroke" (P1's captures)?
11. Checkbox look: a grey fill with a dark tick (P1's Properties captures) or a blue fill with a white tick (the CIB figure behind §4.5)?
12. The Color panel shortcut (Ctrl+Shift+F9) is settled by P1's shortcut table, but this area's researcher couldn't reconfirm it; glance at it on 24.x.

### 7.4 Home, New Document and Document Settings

- Home layout outside the quick strip in v24: is there a left rail with Create New and Open, where is the recent list, does it have columns, and are tutorials still shown?
- What is the small arrow in the "Start a new file fast" area, through which LIL-ET's presenter finds the HD preset: a per-tile menu, a carousel arrow or an expand chevron?
- Is the v24 Home tile set still Full HD, Android 16:9, Banner, iPad 3-4 Gen, Canvas, More Presets, or does it change with beginner/expert mode and the user's plan? Which size, fps and platform does each tile create, and which New Document tab does each map to?
- The full preset lists, with sizes, fps and platform, for the Social, Game, Education, Ads and Web tabs. Which tab holds "Banner", "Android 16:9" and "iPad 3-4 Gen"?
- Does the New Document dialog reopen on the last-used tab and preset? Do Esc and Enter work? Does double-clicking a tile create the document? How does its title bar look on Windows?
- On the Advanced tab, is the hidden fourth BETA tile WebGL-glTF Extended, and is there a script-file section further down (ActionScript file, Animate JavaScript file)?
- Which options does the Platform Type dropdown list on the intent tabs?
- Which Units options does Animate 2024 offer, and in what order (P1's order is used for now)? Are the real Width/Height limits 1–8192 and the frame-rate limits 0.01–120?
- The Document Settings dialog in v24: the exact row order and labels; whether there is a locked/hidden-layers sub-option; whether "Scale Spans" is in the dialog; the dialog's casing of "Match contents"; which anchor cell is the default (centre per P1).
- Is "Use Advanced Layers" still checked by default for new v24 documents? The help says yes, but this is unverified after v19.
- What does Match contents actually do: shrink to the content's bounding box and move the content to (0,0), or keep the content in place with equal margins (the classic Flash behaviour)?
- In what order and colours do the Document Settings buttons appear on Windows (Make Default left; Cancel/OK or OK/Cancel; is OK blue)? Does Make Default close the dialog?
- The exact screen colours of Spectrum blue (Create, selected tile) and the v24 Dark-theme greys on these pages. The measurements come from CMYK print, so they are only accurate to about 10%.
- Where the Home button sits on Windows: at the far left of the top bar, or right after the app icon before File (P1's suggestion)?
