# Adobe Animate UI reference for minnamate

Target: Adobe Animate 2024 (24.0.x, the final release line) on Windows, in the Essentials workspace, with the default Dark theme, at 100% UI scale. minnamate's editor (`apps/editor`, egui 0.36) should match it at first glance and in behaviour.

This file merges four researched and fact-checked notes: workspace and theme (§4), timeline (§5), tools (§6), and Properties, Library, menus and shortcuts (§7). §1–§3 give the build order and the values to build with, after resolving where the notes disagreed. §8 lists what is still unknown. Written 13 Oct 2026.

**Tags and sources**
- **[high]**: Adobe help text, an Adobe screenshot, a dated 2024 screenshot, or two independent sources agree.
- **[med]**: one good source, a 2020–23 screenshot that nothing contradicts for 2024, or long-stable behaviour.
- **[low]**: recall or an estimate. Measure it on a real install before relying on it.
- Each area part (§4–§7) keeps its own source list at its end, so "S4" inside §5 means the timeline list's S4. In §1–§3 and §8, keys carry an area prefix: **WS** = §4 workspace, **TL** = §5 timeline, **TO** = §6 tools, **PM** = §7 panels and menus. **P** is a pixel reading from Adobe help-page screenshots of 2021–22 builds (PM). **R** / **recall** is the researcher's own knowledge.
- Sizes are logical px at 100% UI scale. Multiply by the OS scale (egui `pixels_per_point`).
- Most 2024 measurements come from two macOS 2× screenshots of Animate 2024 in TechRadar's 2024 review (TO S2a/S2b; the same images are TL S31), halved to 1×. Using them for Windows at 100% is an inference [med].
- Colours sampled from JPEG screenshots are good to about ±3 levels. Thin lines, small dots and small text sample dimmer than they really are.

**Clean-room rules** (they apply to everything below)
- Everything here describes Animate in our own words. Only short UI labels are reproduced. Don't paste Adobe help text into code or docs.
- Glyph descriptions are geometry for drawing original art in `icons.rs`. Never copy, trace or embed Adobe images, icons or fonts, and don't commit screenshots.
- No Adobe fonts, even open-licensed ones: no Adobe Clean, no Source Sans, no Source Han.
- Nothing is branded "Flash" or "Animate": no "An" lettering. Write "minnamate Help" and "About minnamate", and leave out Adobe-service items.

---

## 1. Build order

Ranked by how much each item changes the first impression of a screenshot. Finish a group before starting the next one.

**A. Layout and theme**
- [ ] **A1. Palette (§2).** The most visible fixes are: a pasteboard darker than the panels (#252525); #252525 for gaps and hairlines; #2F2F2F for the app bar and all tab strips; text #EBEBEB with secondary text #A5A5A5; and a single #545454 fill for every pressed, active or toggled button.
- [ ] **A2. Essentials frame (§4.1).**
  - Tools column, 48 px, at the far left and full height.
  - Document column: document tabs (26), edit bar (30), stage, and the Timeline (~280) under the stage only.
  - At the right, full height: the collapsed icon strip (36), then Properties / Library / Assets (~300).
  - 3 px #252525 splitters between all of them.
- [ ] **A3. Stage view (§4.5).** White stage with a 1 px black outline, centred at 100% on the dark pasteboard, with 7 px pill scrollbars.
- [ ] **A4. App bar (§4.2).** A 30 px #2F2F2F row holding the logo, Home, the 11 menu titles (#C0C0C0), Test Movie, Share, the workspace switcher and 46×30 window controls.
- [ ] **A5. Edit bar (§4.4).**
  - Left: Back, Edit Scene, Edit Symbols, breadcrumb.
  - Right: Rotation, Center Stage, Clip, zoom arrows, zoom field.
- [ ] **A6. Panel chrome (§4.6, §3.2).**
  - 11 px dock title strips with « / ».
  - 22 px tab strips: the active tab is #323232 with a #F3F3F3 semibold label; inactive labels are #818181; tabs have no close boxes.
  - A four-line panel menu at the right end of each strip.
  - Segoe UI at 12 px.

**B. Timeline**
- [ ] **B1. Geometry (§5.1).** Tab strip 22, toolbar row 34, two-tier ruler 33, rows 24, layer column ~280, cells ~10.
- [ ] **B2. Frame grid (§5.6).**
  - Cells: #2F2F2F with #252525 lines, and #3A3A3A for every 5th column.
  - Spans: #909090 static spans with a row of dots along the top; dark blank-keyframe spans.
  - Keyframe dot at 65% of the row height; a 2×8 end-of-span pill.
  - Muted tween fills, with the arrow at dot height.
  - #5A8AD0 for selected frames.
- [ ] **B3. Ruler and playhead (§5.5, §5.7).**
  - Playhead: a translucent #4A80CC tab one cell wide, with a point at the bottom and no number.
  - Frame numbers every 5 frames, left-aligned, with frame 1 unlabelled.
  - Italic seconds labels in the top tier.
- [ ] **B4. Layer column (§5.2, §5.4).**
  - 2024 toolbar row: New Layer, New Folder, Delete; then Camera, Parenting, Layer Depth.
  - Header row: Layer View; then the dot, outline, eye and lock columns.
  - Rows: 6×12 colour swatch, empty eye and lock cells, #40618E for the selected row, cyan for the first layer.
- [ ] **B5. Toolbar (§5.2).**
  - Readouts: "24.00 FPS" in grey, "12 F" in blue.
  - The centred cluster in the 2024 default order.
  - Zoom controls at the right.
  - #545454 chip for toggles that are on.
- [ ] **B6. Menus (§5.3, §5.8).** Panel menu, frame context menu, layer context menu.

**C. Tools**
- [ ] **C1. Column (§6.1–§6.3).**
  - 30 px header.
  - 16 buttons of 32×26 on a 30 px pitch, in four sections with 2 px separators.
  - #545454 active highlight with radius 4; #EBEBEB icons; flyout triangles.
- [ ] **C2. Bottom block (§6.8).** The "…" button, Black and White plus Swap, overlapping 22×22 fill and stroke swatches with the red no-colour slash, and option slots from T+594.
- [ ] **C3. Icons (§6.7).** Redraw the 16 toolbar glyphs in the 2024 style: about half are solid shapes, the rest 1 px outlines.
- [ ] **C4. Flyouts (§6.5).** Dark popup, 24 px rows, a blue check on the current member; the group button shows the member used last.
- [ ] **C5. Edit Toolbar drawer (§6.9).** Add, remove and Reset only.

**D. Panels**
- [ ] **D1. Properties chrome (§7.2.1).** Tool / Object / Frame / Doc row with the #4A4A4A pill, header row, quick-action row, collapsible sections with 2 px dividers.
- [ ] **D2. Controls (§7.1).** Scrubby numbers (white text over a #474747 underline), inputs, dropdowns, buttons, checkboxes, swatches.
- [ ] **D3. Doc tab (§7.2.2).** This is what Properties shows on launch.
- [ ] **D4. Object, Frame and Tool tabs (§7.2.3–§7.2.5).**
- [ ] **D5. Library (§7.3).**

**E. Menus**
- [ ] **E1. Menu look (§7.1).** Light, native-style dropdowns: #F2F2F2 background, #91C9F7 hover row, black text, 22 px rows.
- [ ] **E2. Menu contents and Windows shortcuts (§7.4, §7.5).**
- [ ] **E3. Panel and context menus (§5.3, §5.8, §7.3).**

**F. Calibrate**
- [ ] Compare against one real Animate 2024 screenshot taken on Windows at 100%, using the checklist in §8.9. Measure there; don't commit the screenshot.

---

## 2. Palette

Default Dark theme, 2024. Each token has one value. Where the notes disagreed, the order of preference was: a value measured on 2024 screenshots, then a reading from 2021–22 Adobe screenshots, then a value derived from Spectrum. §2.6 lists what lost. The "theme.rs now" column compares with `apps/editor/src/theme.rs` as of this writing.

**Main finding.** The 2024 screenshots use the *classic* Spectrum values for backgrounds (#252525 / #2F2F2F / #323232) and the *newer* Spectrum values for foregrounds (#EBEBEB icons, #545454 active fill, #D1D1D1 scrollbar thumbs, #3F3F3F slider tracks). The palette follows that mix. This partly answers the workspace note's question about which Spectrum gray ramp 24.x uses.

### 2.1 Chrome and text

| Token | Hex | Used for | Conf · source | theme.rs now |
|---|---|---|---|---|
| app_bg | #252525 | Window background seen through the 3 px splitters; 1 px outer window-edge lines; floating-panel title strips | [high · TO S2a; med · TL S31] | `APP_BG` #1D1D1D → change |
| border | #252525 | Structural lines: panel edges, the line under tab strips, timeline hairlines and grid dividers, row separators, Properties section dividers (2 px), Tools section separators (2 px) | [high · TO S2a (#272727), PM P (#242424); med · TL S31] | `BORDER` (same); `DIVIDER` #3F3F3F → #252525 |
| bar_bg | #2F2F2F | App (menu) bar, document tab row, panel tab strips, the Tools panel's tab row | [high · TO S2a; med · TL S31, PM P] | `PANEL_HEADER` (same); `BAR` #262626 → #2F2F2F |
| panel_bg | #323232 | Panel bodies, active tabs, edit bar, dock title strips, timeline toolbar, header, ruler and layer rows | [high · WS S9, TO S2a, TL S31, PM P] | `PANEL_BG` (same) |
| panel_alt | #2F2F2F | Striped rows and subtle wells (egui `faint_bg_color`) | [low · WS] | faint_bg #2D2D2D ≈ |
| panel_inset_bg | #262626 | Recessed areas inside panels: Output text, code editor | [low · WS] | `INSET` (same) |
| tab_active | #323232 | Fill of the active panel or document tab; it merges into the body below | [high · TO, TL, PM] | uses `PANEL_BG` (same) |
| tab_inactive | #2F2F2F | Inactive tabs have no fill of their own, so they show bar_bg | [med · TL S31, PM P] | (same) |
| tab_text_active | #F3F3F3 | Active tab label, semibold | [med · TL S31; PM P #F4F4F4] | literal in `tab_strip` (same) |
| tab_text_inactive | #818181 | Inactive tab label | [med · TL S31; PM P #737373] | literal in `tab_strip` (same) |
| text | #EBEBEB | Body text, field values, icons and glyphs at rest | [med · TO S2a icons (high), PM P values #E6E6E6] | `TEXT` (same) |
| text_strong | #FFFFFF | Active-tool icon, selected layer name, hovered text, active document tab | [high · TO S2a, TL S31] | white (same) |
| text_dim | #A5A5A5 | Labels, ruler numbers and seconds, FPS readout, idle panel-menu glyph, secondary info | [med · TL S31 (#A1A1A1–#A5A5A5), TO S2a (#A8A8A8)] | `TEXT_DIM` #B9B9B9 → change; `RULER_TEXT` (same) |
| text_disabled | #707070 | Disabled text and glyphs, disabled tabs | [med · PM P (#6D6D6D–#707070), WS; TL #6E6E6E] | `TEXT_DISABLED` (same) |
| icon_disabled | #555555 | Solid areas of a dimmed (unsupported) tool icon; dimmed Swap Colors | [med · TO S2a, S5a] | — |
| section_title | #C8C8C8 | Properties section titles, semibold | [med · PM P] | — |
| menubar_text | #C0C0C0 | Menu titles on the app bar | [med · PM P] | — |
| layer_name | #D5D5D5 | Timeline layer names (white on the selected row) | [med · TL S31] | `LAYER_NAME` (same) |

### 2.2 Controls

| Token | Hex | Used for | Conf · source | theme.rs now |
|---|---|---|---|---|
| input_bg | #262626 | Text inputs, dropdowns, zoom field | [med · PM P] | `INSET` (same) |
| input_border | #4F4F4F | 1 px outline of inputs, dropdowns, push buttons and Properties icon buttons | [med · PM P (#4A4A4A–#555555)] | `OUTLINE` #545454 ≈ |
| field_underline | #474747 | 1 px line under a scrubby numeric value; no box | [high · PM P] | — |
| button_bg | #2E2E2E | Push buttons and Properties icon buttons at rest. Toolbar, app-bar, edit-bar and timeline icon buttons are flat (no fill) | [high · PM P] | `BUTTON` #3C3C3C → change (inactive widgets are drawn on `PANEL_BG` now) |
| button_hover | #3F3F3F | Hover fill of flat icon buttons, menu-bar titles and list rows | [med · WS; low · TO] | `BUTTON_HOVER` (same) |
| button_active | #545454 | Pressed buttons, the active tool, and every toggled-on button: tool options, timeline toggles, Properties quick actions, the "…" pill | [high · TO S2a] | `BUTTON_ACTIVE` (same); `TOGGLED` #494949 → #545454 |
| category_pill | #4A4A4A | Selected Tool / Object / Frame / Doc tab in Properties | [high for 2021–22 · PM P; 2024 unverified] | — |
| group_divider | #545454 | 1 px vertical lines between button groups (timeline toolbar; Properties quick actions, read #4A4A4A in 2021–22) | [med · TL S31] | — |
| accent | #2680EB | 2 px keyboard-focus ring, docking drop zones, links, selected looping-mode outline, drawer hover border | [med · WS, PM P] | `ACCENT` (same) |
| accent_hover | #378EF0 | Accent hover; the check mark in tool flyouts | [med · WS, TO S1] | — |
| cta | #1473E6 | Primary dialog button, white text | [med · WS S41] | — |
| cta_hover | #0D66D0 | Primary button, hover | [med · WS S41] | — |
| cta_down | #095ABA | Primary button, pressed | [med · WS S41] | — |
| selection | #40618E | Selected list rows (Library) and text selection | [med · TL S31 (layer rows); low elsewhere] | `SELECTION` #23528C → change |
| checkbox_border | #9A9A9A | 14 px checkbox, 1.5 px border, radius 2 | [med · PM P] | — |
| checkbox_on | #C8C8C8 | Checked fill, with a #262626 check mark | [med · PM P] | — |
| scrollbar_thumb | #D1D1D1 | 7 px pill thumb with no visible track | [med · TL S31] | — |
| slider_track | #3F3F3F | 2 px slider tracks | [med · TL S31] | — |
| slider_knob | #E0E0E0 | Ring knob: 12 px on the timeline zoom slider, 14 px hollow in Properties | [med · TL S31, PM P] | — |
| warning_badge | #F5C400 | Warning badge (a yellow triangle with a dark "!") on unsupported tools | [med · TO S1, S5e] | — |
| no_color_slash | #E0302A | 2 px red diagonal on "no colour" swatches | [high · TO S2a] | — |

### 2.3 Menus, popups and window

| Token | Hex | Used for | Conf · source | theme.rs now |
|---|---|---|---|---|
| menu_bg | #F2F2F2 | Menu-bar dropdowns, context menus and panel menus: light and native-style | [high for 2019–22 · PM P, TL S32; med for 2024] | — |
| menu_hover | #91C9F7 | Hovered menu row | [same] | — |
| menu_text | #000000 | Menu labels and shortcuts | [same] | — |
| menu_text_disabled | #6D6D6D | Disabled menu items | [same] | — |
| menu_separator | #D7D7D7 | 1 px separators, starting after the check gutter | [same] | — |
| popup_bg | #1E1E1E | Custom dark popups: tool flyouts, app-bar popovers | [med · TO S1] | — |
| popup_border | #5A5A5A | 1 px popup outline | [med · TO S1 (#5A5A5A–#6E6E6E)] | `OUTLINE` used for `window_stroke` |
| popup_hover | #2C2C2C | Hovered flyout row | [low · TO] | — |
| shadow | #000000 at 50% | Menus, popovers and floating panels: offset y 1, blur 4 | [med · WS S41] | (same) |
| tooltip_bg | #FFFFFF | Tooltips: near-black text, 1 px grey border (pre-2024 captures) | [low · PM P] | — |
| window_close_hover | #C42B1C | Close-button hover, with a white glyph | [med · WS, Windows 11 convention] | — |

### 2.4 Stage

| Token | Hex | Used for | Conf · source | theme.rs now |
|---|---|---|---|---|
| stage_default | #FFFFFF | Default stage colour | [high · WS S4, S38] | — |
| stage_outline | #000000 | 1 px stage-bounds outline, no shadow | [med · WS S40, S1] | `STAGE_OUTLINE` (same) |
| pasteboard | #252525 | The area around the stage | [med · WS S10, S22; 2024 inferred from the TO and TL background readings] | `PASTEBOARD` #1D1D1D → change |

The default guide colour is green [high · WS S1]; no hex has been measured.

### 2.5 Timeline

| Token | Hex | Used for | Conf · source | theme.rs now |
|---|---|---|---|---|
| frame_cell | #2F2F2F | Empty frame cells | [med · TL S31] | `FRAME_CELL` (same) |
| frame_cell_5th | #3A3A3A | Every 5th empty column, in empty areas only and not on folder rows | [med · TL S31] | `FRAME_CELL_5TH` (same) |
| frame_line | #252525 | Grid dividers, ruler ticks, 5-frame lines (same as border) | [med · TL S31] | `FRAME_LINE` (same) |
| frame_span | #909090 | Static span with content | [med · TL S31] | `FRAME_SPAN` (same) |
| frame_span_blank | #4A4A4A | Span after a blank keyframe | [low · TL S29, S12] | `FRAME_SPAN_BLANK` (same) |
| keyframe | #1E1E1E | Keyframe dot, tween arrow, end-of-span pill, frame-script "a", the 1 px line at a keyframe's left edge | [med · TL S31] | `KEYFRAME` (same); `FRAME_SPAN_EDGE` #252525 ≈ |
| classic_tween | #78609E | Classic tween span | [med · TL S31] | `CLASSIC_TWEEN` (same) |
| shape_tween | #AE6F42 | Shape tween span | [low · TL S15] | `SHAPE_TWEEN` (same) |
| motion_tween | #9C8B43 | Motion tween span | [med · TL S31] | `MOTION_TWEEN` (same) |
| ik_pose | #6E9550 | IK pose layer span | [low · TL S4] | — |
| frame_selected | #5A8AD0 | Selected frames: opaque on empty cells, about 50% over spans (a static span becomes #7690B3) | [med · TL S29, S31] | `FRAME_SELECTED` (same) |
| playhead | #4A80CC | Playhead tab (about 90% opaque) and 1 px line; the auto-keyframe dot | [med · TL S31] | `PLAYHEAD` (same) |
| layer_highlight | #40618E | Selected (active) layer row, with a white name. Several selected rows share it | [med · TL S31; the CIB 2023 print reads #3B5C96] | `LAYER_SELECTED` (same) |
| layer_hover | #393939 | Hovered layer row | [low · TL] | `LAYER_HOVER` (same) |
| hot_text | #8FB8E8 | The timeline's current-frame readout only. Properties numbers are not blue | [med · TL S30, S31] | `CURRENT_FRAME_TEXT` (same); `HOT_TEXT` #4B9CF5 → drop |
| loop_range | #2A5A90 | Loop bar in the top tier of the ruler | [low · TL S12] | — |
| loop_handle | #4C89CE | Handles at both ends of the loop bar | [low · TL S12] | — |
| onion_past | #2F6FFF | Onion skins before the playhead (a placeholder blue) | [low · TL S16] | `ONION_BEFORE` (same) |
| onion_future | #00FF00 | Onion skins after the playhead | [low · TL S16, S32] | `ONION_AFTER` #22B14C → change |
| layer_color_first | #00FFFF | Outline colour of the first layer in a new document | [high · TL S23, S29–S32] | `LAYER_COLORS[0]` (same) |
| label_name | #D62D2D | Frame-label flag (red) | [low hex; high colour family · TL S4] | `LABEL_NAME` (same) |
| label_comment | #2E9B3E | Comment "//" (green) | [low hex; high family · TL S4] | `LABEL_COMMENT` (same) |
| label_anchor | #D8A21C | Anchor glyph (gold) | [low hex; high family · TL S4] | `LABEL_ANCHOR` (same) |

- **Pinned layer row:** the layer's outline colour at about 15% over panel_bg; cyan gives #3C5051 [med · TL S31]. It is a formula, not a token.
- **Layer colour sequence for new layers:** #00FFFF, then #9933CC, then #FF800A [med · TL S29, S23]. Other values seen in Animate-saved files: #FF4FFF, #4FFF4F, #808080, #009999, #FF0000, #669900, #0000FF and #4F80FF (older "Layer 1" files). Always draw the colour stored in the document. `LAYER_COLORS` already follows this.

### 2.6 Colour decisions (resolved contradictions)
1. **Background ramp.** WS proposed values from the newer Spectrum ramp for 2024 [low]: frame #1D1D1D, bar #262626, pasteboard #1D1D1D. The 2024 screenshots measure #252525 lines and gaps and #2F2F2F strips [high · TO S2a; med · TL S31], so those classic values win. The newer ramp shows up only in foregrounds.
2. **Dock gaps.** PM read #141414–#1A1A1A on 2021–22 screenshots [med, P]. 2024 measures #252525 [high], which wins.
3. **Dividers inside panels.** WS proposed light 1 px lines (#3E3E3E / #3F3F3F) [med]. Every measured divider is dark: TO #272727 [high], PM #242424 [high, P], TL #252525 [med]. So they use `border`. Light lines (#545454) appear only as vertical separators between button groups (`group_divider`).
4. **Toggled-on fill.** Candidates were WS #707070 [low], TL #494949 (2020–22 captures) [med], PM #565656 (2021–22) [high, P] and TO #545454 (2024) [high]. #545454 is used everywhere; the 2024 course also says active states became easier to see (TL S36).
5. **Text.** WS gave #EBEBEB (2024 estimate) or #E3E3E3 (classic), PM read #E6E6E6, and TO measured icons at #EBEBEB [high]. #EBEBEB wins. TL's #D5D5D5 is kept for layer names only.
6. **Secondary text.** WS recommended at least #B9B9B9, or #D1D1D1 if the newer ramp is confirmed [low/med, derived from Spectrum]. 2024 measurements are #A1A1A1–#A8A8A8 (TL, TO), so #A5A5A5 wins. PM's 2021–22 label reading (#8E8E8E) and inactive tab reading (#737373) are taken to be the older, lower-contrast look.
7. **Inactive tab label.** #818181 (TL, 2024) wins over #737373 (PM, 2021–22) and WS's text_secondary.
8. **Icon glyphs.** PM read Properties and Library glyphs at #B8B8B8–#B9B9B9 (2021–22). TO shows the toolbar at #BABABA in 2020–23 and #EBEBEB in 2024. In 2024 all glyphs use `text` [med, inferred].
9. **Hot text.** WS drew scrubbable Properties numbers in blue (#4B9CF5) [low]. PM says that since 2020 they are white over a grey underline [high, P]. They are not blue. The only blue number is the timeline's current-frame readout (#8FB8E8, TL [med]).
10. **Selection rows.** WS's #23528C [low] loses to the measured #40618E (TL). The Library selection colour [low] follows it.
11. **Menus.** WS says dropdowns follow the dark theme [med, recall]. Windows screenshots from 2019–22 builds (PM [high, P]) and a 2020 Windows context menu (TL S32 [high]) show light native menus, so menus are light. This is the top open question for 24.x (§8.1). WS's dark menu spec is kept as a fallback in §4.2.
12. **Popups.** Tool flyouts are custom dark popups (#1E1E1E, TO [med]). WS's dark menu_bg #262626 [low] now applies only to the dark-menu fallback.
13. **Pasteboard.** WS's 2020–23 value #252525 [med] wins over its 2024 estimate #1D1D1D [low], because it matches the 2024 background readings.
14. **Panel-menu glyph colour.** TL read #EBEBEB and TO read #A8A8A8, both on 2024 images. Draw it in text_dim at rest and text on hover (WS's rule), which spans both readings.
15. **Push buttons.** WS proposed Spectrum pills [low]. PM shows radius-4 rectangles [high, P], which win.
16. **Scrollbars.** WS proposed 10 px with a pasteboard-coloured track and a #545454 thumb [low]. TL measured 7 px pills in #D1D1D1 with no track [med]. TL's style is used everywhere.
17. **Onion colours.** The future colour #00FF00 is supported by both S16 ("green") and S32. The past colour stays a placeholder blue, because S32's #99FFCD contradicts S16's "blue".

---

## 3. Metrics

### 3.1 Sizes

| Item | Value | Conf · source | theme.rs now |
|---|---|---|---|
| App bar (menus and window controls) | height 30 | [low · WS] | `APP_BAR_H` (same) |
| Menu title | 12 px text, ~8 px side padding, ~24 px hit height, hover radius 4 | [low · WS] | |
| Window control cell | 46×30 | [med · WS] | |
| App-bar right-cluster button | 28×24 with a 16 px glyph, 4 px apart | [low · WS] | |
| Native menu item | height 22; 28 px check gutter; ≥24 px before the shortcut | [med · PM P] | |
| Document tab row | height 26; tabs 80–220 wide | [low · WS] | `DOC_TABS_H` (same) |
| Edit bar | height 30; buttons 24×24; zoom field 64×22 | [low · WS] | `ICON_BUTTON` 22 |
| Dock title strip (collapse chevrons) | 11, between 1 px lines | [high on macOS, med on Windows · TO S2a] (WS ~8 [low]) | `dock_strip` 8 → 11 |
| Panel tab strip | height 22 | [low · TL S31] (PM 24 on 2021–22, an upper bound; WS 24 [low]) | `TAB_STRIP_H` 24 → 22 |
| Tools panel's own tab row | 16 | [high on macOS · TO S2a] | |
| Panel-menu hit box | 24×24 | [low · WS] | |
| Splitter / dock gap | 3 | [high · TO S2a] (WS 2 [low]) | `GAP` 2 → 3 |
| Tools column | inner width 48, plus a 1 px outer edge line | [high · TO S2a] (WS ~44 [low]) | `TOOLS_W` 44 → 48 |
| Tools header | 30 | [high on macOS · TO S2a] | |
| Tool button | 32×26 on a 30 px pitch; 8 px side margins; 16×16 icon | [high · TO S2a] | `TOOL_BUTTON` 32 → 32×26 |
| Tool section separator | 2 px, with a 4 px gap above and below | [high · TO S2a] | |
| Tool flyout | rows 24; width ~222–227; 14 px icons; 4 px padding at top and bottom | [med · TO S1] | |
| Edit Toolbar drawer | 240×625; 40×40 cells with 16 px gutters | [med · TO S1] | |
| Collapsed icon strip | 36 wide; 32×32 buttons with 18 px glyphs | [low · WS] | |
| Properties / Library dock | ~300 wide (minimum ~250) | [low · WS, PM] | |
| Properties rows | category tabs 40; header 48 (Doc 56); quick actions 44; section header 32; content rows 28; side padding 16; label column 24 (W/H/X/Y) or 64 (word labels); icon buttons 28×28 | [med · PM P; upper bounds for 2024] | |
| Library rows | document row 28; preview 140; info row 26 (search 140×20); column header 20; rows 20; bottom toolbar 24 | [high layout · PM P] | |
| Timeline panel | height ~280 including the tab strip | [low · TL S31] (WS ~290 [low]) | |
| Timeline toolbar row | 34 (2020–23 ≈ 40) | [med · TL S31] | |
| Timeline toolbar buttons | hit box 28×28, 30 pitch, 12–14 px glyph; toggled chip ~30×30, radius 4 | [med · TL S31] | |
| Timeline header row = ruler | 33 (top tier 14 + 1 px line + lower tier 18) | [med · TL S31] | |
| Layer row (Medium, assumed default) | 24 (Short ~16, Tall ~32) | [low · TL] | |
| Layer name column | ~280 (S31: 300; S32: ~258) | [low · TL] | |
| Layer toggle columns | 25 pitch | [med · TL S31] | |
| Frame cell | ~10 wide at default zoom; slider from ~4 to at least 15 | [low · TL] | |
| Keyframe dot | Ø ~5.5, centre at ~65% of the row height | [med · TL] | |
| End-of-span pill | 2×8 | [high · TL] | |
| Scrollbars | 7, pill-shaped thumb | [med · TL S31] (WS stage ~10 [low]) | |
| Stage rulers | 18 | [low · WS] | |
| Corner radius | 4 for buttons, fields, menus and popups; 2 for buttons under 24 px | [high · TO S2a (active tool); low otherwise] | (same) |

### 3.2 Typography

| Text | Size (px) | Weight | Conf · source |
|---|---|---|---|
| UI family | Segoe UI, loaded from Windows at run time and not bundled; Inter (SIL OFL), bundled, on other platforms | — | clean-room decision (WS §8; overrules PM's Source Sans 3) |
| Menu titles and items, document tabs, breadcrumb, zoom field | 12 | regular | [low · WS; PM P: menus use Segoe UI 9 pt = 12 px] |
| Panel tab labels | 12 | semibold when active | [low size · WS; med weight · TL, PM] |
| Properties labels and values | 12 | regular | [med · PM P] (WS 11 [low]) |
| Properties section titles | 12–13 | semibold | [med · PM P] |
| Properties category tabs | 13 | regular | [med · PM P] |
| Layer names | ~12 | regular | [med · TL S31] |
| Timeline frame numbers | ~11 | regular | [med · TL S31] (WS 10 [low]) |
| Timeline seconds labels | ~10 | italic | [med · TL S31] |
| FPS and current-frame readouts | ~13, with an ~8 px superscript "FPS" or "F" | regular | [med · TL] |
| Tool flyout names | ~12 | regular | [med · TO S1] |
| Edit Toolbar drawer title | ~13 | regular | [med · TO S1] |
| Tooltips | 11 | regular | [low · WS] |
| Stage ruler numbers | 9–10 | regular | [low · WS] |

- Users of 24.0.13 describe the fonts as unusually small, so don't go larger [med · WS S32].
- Everything is regular weight except section titles and active tab labels (semibold).

### 3.3 Other resolved contradictions
1. **Tools column** is 48 px (TO [high]), not 44 (WS [low]). Buttons are 32×26 on a 30 px pitch (TO [high]).
2. **Splitters** are 3 px (TO [high], measured next to Tools), not 2 (WS [low]).
3. **Dock title strip** is 11 px in panel_bg (TO [high on macOS]), not ~8 px in the frame colour (WS [low]).
4. **Tab strips** are 22 (TL, 2024). The 24 values come from a WS estimate [low] and PM's 2021–22 reading, which is an upper bound for 24.0.
5. **Timeline height** is ~280, from TL's measured ratio [low]; WS estimated ~290 [low].
6. **Scrollbars** are 7 px pills (TL [med]), not 10 px (WS [low]).
7. **Font sizes.** Properties text is 12 (PM [med]) rather than 11 (WS [low]). Frame numbers are 11 (TL [med]) rather than 10 (WS [low]). Active tabs are semibold (TL, PM [med]); WS said "not bolder" [low].
8. **Fallback font.** Inter (WS fact-check, clean-room rule) replaces Source Sans 3 (PM), which is an Adobe font.
9. **Panel-menu glyph.** In 2024 it is four 10×1 lines on a 2 px pitch (TL, TO). Three lines came from a WS estimate [low] and PM's 2021–22 screenshots.
10. **Menus are light** (PM [high for 2019–22], TL [high for 2020]) rather than dark (WS [med, recall]). Still open for 24.x.
11. **Push buttons** are radius-4 rectangles (PM [high, P]), not Spectrum pills (WS [low]).
12. **Tool shortcuts.** Every tool has its own key and no key cycles through a group (TO [high], from an April 2024 shortcut export). PM's "a shared key cycles the group" [med] and its shared L/R/O group keys are dropped. Rectangle Primitive is Shift+R, Oval Primitive Shift+O, Polygon Shift+L, and Magic Wand has no key.
13. **Rotation tool key** is Shift+H (TO [high], two sources) rather than H (WS [high], from one helpx line that PM notes prints "(H)"). Listed in §8.1.
14. **Snap to Objects key** is Ctrl+Shift+U (TO [high]) rather than Ctrl+Shift+/ (PM [med]). Listed in §8.1.
15. **Onion-skin toggle** is Ctrl+Alt+O (PM [med]) rather than "no confirmed default" (TL [low]). Make it rebindable. Listed in §8.1.
16. **Test** is Ctrl+Enter [high · WS S4, PM S4]; TL had it at [med, R].
17. **Window > Workspaces** includes Basic and has no New Workspace item (WS [high]/[med], S4, S13), rather than PM's list [med].
18. **Panel menus** end with Lock, Help, Close, Close Group (PM P; TL's screenshots show Lock and Help). WS's "Close Tab Group" is kept as an open wording question.
19. **Brush mode label** is "Paint Fills Only" (PM [high], 2021+), not "Paint Fills" (TO, from a 2020 capture).
20. **Rotation tool** exists both in the edit bar (WS [high], since 2020) and in the toolbar's Hand group (TO [high]). Build both.
21. **Smooth / Straighten** appear both in the toolbar options area for the Selection tool (TO [high], 2024) and in Properties > Object > Shape Options (PM [high, P]). Build both.
22. **Theme list and default** are Light, Dark, Darkest, with Dark as the default (WS [high], S36). This answers the PM and TL open questions about it.
23. **Assets tab.** Essentials groups Properties / Library / Assets (WS [high]), which answers PM's question.
24. **"Apply to pasteboard"** is a checkbox in Properties > Doc > Document Settings (PM [med, P]), which answers part of WS's question.
25. **Output tab.** The Timeline group has one (TL [high existence]), which answers part of WS's question.
26. **Tools column chevron.** The left column shows «, and » when docked right (TO [high]). WS's sketch had » on it.

---

## 4. Workspace layout, window chrome and theme

Windows, Essentials, default theme, Animate 2024 (v24.x). Source keys in this part refer to the list at its end. All glyphs are original geometric suggestions.

### 4.0 Version context (2024 compared with earlier versions)
- **2019 (19.0, Oct 2018):** flat "modern" look. Preferences offered four UI themes: Darkest, Dark, Light, Lightest [high, S8].
- **2020 (20.0, Nov 2019), redesigned UI:**
  - In Essentials the toolbar moved to the **left** as a single column; dragging its edge wider reflows it into more columns. The Properties/Library group moved to the **right** [high, S5, S6].
  - A dedicated **Test Movie button** joined the application bar, right next to the workspace switcher [high, S5].
  - Small arrows that step the zoom, the zoom dropdown and the stage **Rotation tool** now sit above the stage [high, S5].
  - New **Basic** workspace. The first launch asks whether you're new or experienced: new users get Basic, others get Essentials [high, S4, S5].
  - Window > Workspaces no longer has New Workspace; saving moved into the switcher popover [med, S13].
  - Large paddings and fonts drew complaints [high, S23, S28].
- **20.5 (June 2020) and later:** a **Share** button ("Quick Share and Publish") at the upper right [high, S14, S35]. The **Assets** panel is tabbed with Properties [high, S16].
- **2023 (23.x):** the theme (a different level of gray) is still chosen in Preferences > **General** [high, S4].
- **2024 (24.0, Oct 2023), "Sleek user interface":**
  - Rebuilt on Adobe Spectrum version 7: cleaner, slightly more compact panels that leave more room for the stage, with raised contrast and readability [high, S3, S7, S21, S36].
  - Toggle buttons: an active toggle has a **lighter** background and an inactive one a **darker** background [med, S7/S36, same instructor].
  - The theme moved to Edit > Preferences > Edit Preferences… (Ctrl+U) > **Interface** > Color Theme, with three choices: Light, Dark, Darkest. **Default: Dark** [high, S3, S36, S11, S25].
  - TechRadar found the change very subtle [high, S21], so the 2020–23 layout still applies [med, inference].
  - Users still describe the tools as big and chunky (July 2024) and the fonts as unusually small (24.0.13) [med, S24, S32].
  - Animate is now in maintenance mode: 24.0 was the last feature release, and fixes run to 24.0.15. The 24.x UI is the last one to match [high, S39, S26].

### 4.1 Whole window, Essentials workspace

```
 x=0    48 51                                                        1578 1581  1617 1620               1920
 +-------------------------------------------------------------------------------------------------------+
 |[M][H] File Edit View Insert Modify Text Commands Control Debug Window Help    [>] [^] [W]    _  [] X  | app bar h~30
 +------+--+-----------------------------------------------------------+--+------+--+--------------------+ 1 px #252525 line
 |  <<  |  | Untitled-1* x | scene.fla x                               |  |  <<  |  |                 >> | doc tabs h~26 / dock title strips h~11
 | T  = |  +-----------------------------------------------------------+  | [i]  |  | Properties Library Assets = | tab strips h~22
 | [T]  |  | <- [S]v [C]v  Scene 1 > Symbol 1   [R] [+] [#]  < > [100% v]|  | [i]  |  | Tool Object Frame Doc | edit bar h~30
 | [T]  |  +-----------------------------------------------------------+  | [i]  |  |                    |
 | ---- |  |                                                         ^ |  | [i]  |  |                    |
 | [T]  |  |      pasteboard       +-----------------+               | |  | [i]  |  |  Properties body   |
 | [T]  |  |                       |  STAGE (white)  |               | |  |      |  |                    |
 | ---- |  |                       +-----------------+               v |  |      |  |                    |
 | [..] |  |  <================== horizontal scrollbar ==========>     |  |      |  |                    |
 | fill |  +-----------------------------------------------------------+  |      |  |                    | 3 px splitter
 | opts |  | Timeline | Output                                       = |  |      |  |                    | tab strip h~22
 |      |  | toolbar row: layer buttons | FPS, frame, cluster, zoom    |  |      |  |                    | h~34
 |      |  | header row: Layer View, toggles | ruler                   |  |      |  |                    | h~33
 |      |  | layer list            | frame grid                        |  |      |  |                    | group h~280
 +------+--+-----------------------------------------------------------+--+------+--+--------------------+
   48    3               ~1527 document column                        3   36    3        ~300
```

Legend:
- [M] is the app logo (use minnamate's own logo). [H] is the Home button; its Windows position is unverified. [>] is Test Movie, [^] is Share, [W] is the workspace switcher.
- [T] is a tool button, [i] is a collapsed-panel icon, `=` is the panel menu (hamburger), and `<<` / `>>` are the dock collapse and expand buttons. The Tools column's header holds `<<` and, below it, the clipped "Tools" tab stub and its panel menu (§6.1).
- [S]v is the Edit Scene menu and [C]v is the Edit Symbols menu.
- [R] is the Rotation tool, [+] is Center Stage, [#] is "Clip content outside the stage", `< >` are the zoom-step arrows, and [100% v] is the zoom field with its dropdown.

| Region | Dock / span | Default size at 1920×1080 | Conf |
|---|---|---|---|
| Application bar | top, full window width | h ≈ 30 | [med] position, [low] px |
| Tools panel | left edge, full height below the app bar; one column by default; dragging its edge wider reflows it into more columns | w = 48 plus a 1 px outer line | [high] left and single column (S5, S6); [high] px on macOS (§6.1); this note first estimated 44 [low] |
| Document column | centre, between the left and right docks | w ≈ 1527 | derived |
| ↳ Document tab row | top of the document column only | h ≈ 26 | [med] position, [low] px |
| ↳ Edit bar | below the tabs, document column only; part of the document window; can't be hidden or torn off (2020+) | h ≈ 30 | [high] position (S4, S18), [med] no-hide (S27), [low] px |
| ↳ Stage viewport | fills the rest of the document column | ≈ 1527 × 710 | derived |
| Timeline group (tabs "Timeline", "Output") | bottom of the document column only, not under the docks | h ≈ 280 including the tab strip | [high] below the stage (S4, S17); [high] tabs (§5.1); [med] span; [low] px (§5 measured ~273 of a ~900 px window; this note first estimated ~290) |
| Collapsed-icon strip (the extra panels in Essentials) | right side, just left of Properties, full height | w ≈ 36 | [med] exists (S5; S30 for pre-2020), [low] side, contents, px |
| Properties / Library / Assets tab group, in that order | right edge, full height | w ≈ 300 | [high] grouping and order (S4, S16, S38), [low] px |
| Splitters (gaps) | between every docked group and column | 3, in app_bg | [high] measured next to Tools (§6.1); [low] elsewhere; this note first estimated 2 |

**Layout rules**
- On a window resize, docked panels keep their size and the stage viewport takes the difference [med, recall]. Dragging the bar between the stage and the Timeline resizes the Timeline [high, S17].
- Maximized on Windows 11, the client area is about 1920×1032 because of the 48 px taskbar. Keep the docks the same size and shrink the viewport [high, Windows convention].
- With no document open, Animate shows the Home screen (presets, recent files) instead of the workspace [med, S4].
- New and opened documents show the stage centred at 100% [med, S38, S4].
- The icon strip's contents in Essentials are unverified. Likely Color, Swatches, Align, Transform/Info, CC Libraries and Motion Presets [low].
- Workspaces in the switcher list: Animator, Basic, Classic, Debug, Designer, Developer, Essentials, Small Screen, plus user-created ones [med, S29, S4, S5, S34]. Alphabetical order [low].
- Window > Workspaces lists the workspaces and `Reset "<current>"`, which restores the layout after a Yes/No confirmation [high, S4]. It has no New Workspace item in 2020+ [med, S13]. Manage Workspaces... is there too (§7.4).
- F4 hides or shows all panels [high, S2, S4]. View > Screen Mode > Full Screen Mode switches to full screen, and Esc returns [high, S4].

### 4.2 Application bar (Windows)
One custom-drawn row replaces the OS title bar: the logo, menus, right-hand buttons and window controls all share it [med, recall]. It is about 30 px tall [low], with a bar_bg (#2F2F2F) background [med, PM P]. No document title is shown [low].

1. **App icon** [low]: 16×16 at x = 8, vertically centred. Clicking it opens the standard Windows system menu (Restore / Move / Size / Minimize / Maximize / Close). Use minnamate's own logo.
2. **Home button.** Glyph: a house, a 12 px square under a 45° roof with a door gap at the bottom centre. It exists, sits at the top left of the interface, and returns to the Home screen [high, S4]. On macOS it is in the app bar next to the window buttons [med, S13]. On Windows, put it right after the app icon, before `File` [low].
3. **Menus, in order** [high, recall; PM P]: `File  Edit  View  Insert  Modify  Text  Commands  Control  Debug  Window  Help`.
   - Text is 12 px in menubar_text (#C0C0C0) [med, PM P], with about 8 px horizontal padding and about 24 px hit height [low].
   - Hover: a rounded button_hover fill, radius 4. While open: button_active [low].
   - A click opens the dropdown. While one menu is open, hovering another title switches menus [high, Windows convention]. Alt shows the accelerators [low].
   - **Dropdowns are light and native-style** (menu tokens in §2.3; details in §7.1). This note first assumed menus follow the dark UI theme [med, recall], but Windows screenshots of 2019–22 builds show light menus [high, PM P; TL S32]. Item 7 keeps the dark variant as a fallback in case 24.x turns out to theme its menus.
4. **Empty area**: drag to move the window; double-click to maximize or restore [high, Windows convention].
5. **Right cluster.** The items are [high]; their left-to-right order (Test, Share, Workspace) is [low]. Test Movie and the switcher sit next to each other [high, S5, S13].
   - Each button is about 28×24 with a 16 px glyph, 4 px apart. Hover button_hover, pressed button_active, radius 4 [low].
   - **Test Movie**: a click runs Control > Test (Ctrl+Enter) [high, S4, S5]. Glyph: a right-pointing triangle outline, 10×12, with a 1.5 px stroke [med, recall]. The tooltip text, e.g. "Test Movie (Ctrl+Enter)", is [low].
   - **Share** ("Quick Share and Publish"). Glyph: an open-topped tray, 12×8, with an arrow rising from its centre [low]. A click opens a popover (popup_bg) anchored under the button with two choices [high, S14, S35]:
     - `Social Share`: Twitter or YouTube. Animate encodes a video through Media Encoder and uploads it.
     - `Publish`: Video (MP4), Animated GIF, or HTML5 Canvas (only for Canvas documents), then a Publish button.
     - Draw each choice as a row with a ">" chevron [low].
   - **Workspace switcher**: an icon-only button [med, S37, recall]. Glyph: a 16×12 rectangle split by a vertical line near its right third, plus a 6 px "v" chevron [low]. The popover [high unless noted, S4, S37]:
     - one row per workspace, with a check on the current one [med];
     - a **reset** button (a circular arrow) beside each workspace name;
     - at the bottom, a `New Workspace` label, a name field, and a **Save Workspace** icon next to the field;
     - deleting a custom workspace goes through Manage Workspaces [low; S2 is a generic Creative Suite page].
   - No search field and no help icon in 2020+ [low, recall].
6. **Window controls** [med, Windows 11 convention]:
   - Three 46×30 cells: minimize (a 10 px horizontal line), maximize (a 10×10 square outline; restore is two offset squares), close (a 10 px X). Glyphs are 1 px, in `text`.
   - Hover: white at 8% over the bar. Close hover: #C42B1C with a white glyph.
7. **Dark-menu fallback** (only if 24.x turns out to theme its menus) [low]:
   - Background #262626 with a 1 px #545454 border, radius 4, and the `shadow` token.
   - Items are 24 px tall: a 20 px check column, the label, then the shortcut right-aligned in text_dim, with ">" for submenus.
   - Separator: 1 px #3F3F3F with 4 px vertical margin. Disabled items use text_disabled.
   - Hovered row: button_hover, with the text turning text_strong.

### 4.3 Document tab row [low unless noted]
- **Size and background:** about 26 high, bar_bg. Tabs are left-aligned from the document column's left edge, with a 1 px app_bg separator between tabs.
- **Tab contents:** 10 px left padding, a 12 px label, an 8 px gap, a 16×16 close box (an 8×8 "x" with a 1.5 px stroke), then 6 px right padding. Tab width follows the text, from about 80 to 220 px, with an ellipsis when it is too long.
- **Labels** [med, recall]: new documents are `Untitled-1`, `Untitled-2`…; saved files show their file name, e.g. `scene.fla`. An unsaved document gets `*` right after the name: `Untitled-1*`.
- **Active tab:** fill panel_bg so it merges with the edit bar below; label text_strong; the close box is always visible.
- **Inactive tab:** no fill (bar_bg shows), label tab_text_inactive, as on panel tabs. On hover the label turns `text` and the close box appears. This note first gave a hover fill of about #2D2D2D, halfway between its estimated bar (#262626) and the panel colour. With bar_bg at #2F2F2F there is no room for a visible step, so hover changes only the label and close box.
- **Close box hover:** a 16×16 button_active square with radius 2.
- **Interactions:** a click activates the tab. Dragging reorders tabs; dragging a tab out of the row floats the document [med, S2]. When tabs overflow, a ">>" button at the right end lists the hidden documents.

### 4.4 Edit bar (above the stage)
- **Container:** about 30 high [low], fill panel_bg [low], 1 px bottom line in `border` [low]. It is part of the document window and has no hide or tear-off option in 2020+ [med, S27].
- **Left group**, starting at x + 8, with 4 px gaps.
  - Evidence: Back sits at the left end of the edit bar [high, S18]. Symbol names appear to the right of the scene name [high, S18]. In 23.0.2 the Edit Symbols ("club") button sits at the upper left of the stage panel [med, S12]. Help text from 2019 and earlier put Edit Scene and Edit Symbols at the upper right [high for old versions, S19]. The order below is [low].
  1. **Back** (24×24). Glyph "<-": a 12 px shaft with a chevron head. A click goes up one level [high, S18]. It only does something inside a symbol; at scene level draw it dimmed to 35% (or hidden, unverified) [low].
  2. **Edit Scene ▾** (24×24 plus a 6 px chevron). Glyph: a clapperboard, a 14×9 rectangle under a hinged bar tilted about 15°, with 3 diagonal stripes. The menu lists all scenes; picking one switches to it [high, S19].
  3. **Edit Symbols ▾** (tooltip "Edit Symbols"). Glyph: a "club", three 6 px circles in a triangle above a short stem [med, S12]. The menu is a hierarchical list of the library (folders become submenus); picking a symbol opens it for editing in isolation [high, S12]. Versions 21–23 broke this for files from older versions; don't copy the bug.
  4. **Breadcrumb**, 12 px text: the scene glyph and `Scene 1`, then for each nested symbol a symbol-type glyph and the symbol name, e.g. `Scene 1 > Symbol 1` [high content, S18]. The separator is a 6 px ">" in text_dim [low]. The last crumb is text_strong and the others `text` [low].
     - Clicking the scene name returns to the main timeline [high, S18]. Clicking an intermediate symbol crumb jumps to that level [med].
     - Other exits: Edit > Edit Document, or double-clicking outside the symbol's content [high, S18, S27]. Ctrl+E toggles between symbol and document editing [high, S26].
- **Right group**, right-aligned with 4 px gaps, in the order `[R] [+] [#]  < >  [zoom field]` [low order]:
  1. **Rotation tool** (a 24×24 toggle). Glyph: a small rectangle tilted 15° inside a 270° circular arrow. A click selects the stage Rotation tool, which moved here from the toolbar in 2020 [high, S5]; it is also a member of the toolbar's Hand group (§6.3). The tool [high, S1]:
     - shortcut: S1 prints H (shared with Hand), but the April 2024 shortcut table gives **Shift+H**, which this reference uses (§3.3, §8.1);
     - Shift+Space gives a temporary rotation from any tool;
     - click to set the pivot crosshair, then drag to rotate. A red line on the crosshair shows the angle.
  2. **Center Stage**. Glyph: a 12 px circle with a crosshair whose lines extend 3 px beyond it. S1 says the button resets the stage to its default view: it re-centres the stage and clears the view rotation [high, S1]. Zoom is left unchanged [low].
  3. **Clip content outside the stage** (a toggle, **off by default**) [high, S4, S40]. Glyph: a 10×10 square with four L-shaped crop marks just outside its corners. When on, artwork beyond the stage edge is hidden, as in the published result [high, S1, S40]; the pasteboard still shows [med]. View > Magnification > Clip to Stage is the menu equivalent [med, S4].
  4. **Zoom-step arrows**: S5's small arrows raise and lower the zoom [high existence, S5]. Draw two 16×24 buttons with small left/right chevrons (not −/+) that step to the previous or next preset [low glyph, placement and step].
  5. **Zoom field** (about 64×22, input_bg fill, radius 4, plus a 12 px "v") [low size]. It shows the current zoom, e.g. `100%`; you can type a value and press Enter [med]. The valid range is **8%–2000%** [high, S1]. Dropdown items:
     - `Fit in Window` [high, S4, S38], `Show Frame`, `Show All` [high, S1]
     - separator
     - `25%`, `50%`, `100%`, `200%`, `400%`, `800%` [med, S33, recall]
- **Zoom shortcuts** [high, S26]: Ctrl+= zoom in, Ctrl+- zoom out, Ctrl+1 100%, Ctrl+2 Show Frame, Ctrl+3 Show All. Ctrl+4 gives 400% and Ctrl+8 800% [med, §7.5].

### 4.5 Stage, pasteboard and viewport
- **Stage:** the default colour is white #FFFFFF [high, S4, S38].
  - A thin outline marks the stage bounds. It was added in Animate CC 2017, and S1 describes it as a black outline shown by default. Draw a 1 px #000000 rectangle on the stage edge [med, S40, S1; low exact colour].
  - No drop shadow [low].
- **"Pasteboard = stage colour"** has been an option since January 2017 [high, S1]. With it on, only the outline shows the stage bounds. It appears as the "Apply to pasteboard" checkbox in Properties > Doc > Document Settings [med, §7.2.2]. Where it lives in the 24.x Preferences is [low].
- **Pasteboard colour:** it follows the UI theme and is darker than the panels. A CC 2019 user reported it as practically black in Darkest and a mid grey in Lightest [med, S10]. Spectrum-classic sets the pasteboard background alias to gray-50 [high, S22].
  - Dark: **#252525** (the palette value). That is [med] for 2020–23. A newer-ramp **#1D1D1D** was considered for 2024 [low], but the 2024 backgrounds measured in §5 and §6 follow the classic ramp, so #252525 stays.
  - S1's sentence describing a light gray pasteboard is pre-2019 text; ignore it.
- **Centring:** new and opened documents show the stage centred at 100% [med, S38]. Hand tool: Space+drag pans [high, S1, recall]. Double-clicking Hand gives a fit, and double-clicking Zoom gives 100% [low, recall].
- **Scroll range:** the stage plus at least one viewport size of margin on every side, extended to include off-stage objects [low].
- **Scrollbars:** on the right and bottom edges of the viewport. Use the timeline's style: a 7 px pill thumb in scrollbar_thumb, with no visible track [med, §5.8]. This note first proposed about 10 px, with a pasteboard-coloured track and a #545454 thumb of radius 5, lighter on hover [low].
- **Rulers:** off by default. Toggle with View > Rulers (Ctrl+Shift+Alt+R) [high, S1, S4, S26].
  - When shown, they are 18 px strips at the top and left of the viewport, filled with panel_bg. Ticks are text_dim and numbers are 9–10 px [low].
  - The default unit is pixels [high, S1]; the origin is the stage's top-left corner [med].
  - Dragging from a ruler creates a guide [high, S1]; the default guide colour is green [high, S1].
- **Grid:** off by default [med]. View > Grid > Show Grid, Ctrl+' [high, S1].

### 4.6 Panel chrome
- **Title strip at the top of each dock column or floating group**: S2 describes a plain empty bar above the tabs [high, S2].
  - Measured on the Tools column in 2024, it is **11 px tall in panel_bg**, between 1 px `border` lines [high on macOS, §6.1]. This note first estimated about 8 px in the frame colour [low].
  - It carries a **double-chevron button**: two 4 px chevrons, measured at #C8C8C8 in 2024 (text_dim at rest, text_strong on hover) [high, S2; colour from §6.1].
  - The left (Tools) column shows « at x ≈ 5–12. An expanded right-hand column shows » (collapse); the collapsed icon strip shows « (expand) [med, recall; high for the Tools column, §6.1].
  - Dragging the strip moves the whole group [high, S2].
- **Tab strip** (about **22** high; this note first estimated 24, and PM reads 24 on 2021–22 screenshots) [low px]:
  - Background bar_bg, with a 1 px `border` line under it (§5.1).
  - Each tab is a 12 px label with 10–12 px side padding. Active tab: fill tab_active, label tab_text_active, semibold. Inactive tab: no fill, label tab_text_inactive; on hover the label turns `text` [low].
  - Tabs have **no close box** [med; §5.1].
  - The **panel menu (hamburger)** sits at the right end of the strip, the panel's upper-right corner [high, S2, S17, S18].
    - Hit box: 24×24.
    - Glyph: four 10×1 px lines on a 2 px pitch, 7 px tall [med geometry; §5.1, §6.1]. This note first drew three lines spaced 3 px, and 2021–22 Properties screenshots show three 12 px lines.
    - Colour: text_dim at rest and `text` on hover. The 2024 readings range from #A8A8A8 to #EBEBEB.
    - Menu contents: panel-specific items, a separator, `Lock` (2020+; it stops the panel from moving when dragged by accident) [high, S15], `Help`, then `Close` and `Close Group` [med; the wording is open, §8.1].
- **Tab and strip interactions** [high, S2]:
  - Right-clicking a tab gives Close (and Close Group [med]).
  - Double-clicking a tab or the empty strip minimizes the group to its strip, or restores it.
  - Dragging a tab moves that panel; dragging the title strip moves the whole group.
- **Docking highlights while dragging** [high, S2, S17, S30], in `accent`:
  - A **narrow blue bar** (3–4 px) between groups, or along a dock's or the window's edge, means "insert as a new group or column".
  - A **blue outline** (2 px) around a group, with its tab strip tinted, means "add as a tab" [med look].
  - Releasing anywhere else floats the panel. Holding Ctrl while dragging prevents docking. Esc cancels the drag.
- **Splitters:** the 3 px app_bg gaps. On hover the cursor becomes a resize cursor, and dragging resizes the neighbours [high behaviour, S17; px from §6.1]. Properties needs a width of at least about 250 px [low].
- **Collapsed icon strip** [high behaviour, S2; low px]:
  - Each panel is a 32×32 button with an 18 px glyph in text_dim. Widening the strip reveals text labels; narrowing it hides them.
  - Clicking an icon opens that panel as a flyout to its left. Clicking its tab, its icon or » collapses it again.
  - With "Auto-Collapse Icon Panels" on, clicking elsewhere closes the flyout [high existence, S20; where the option lives in 24.x is low].
- **Floating panel:** a small window with a 20 px title strip in app_bg carrying » and "x" [med].

### 4.7 Themes and colours
**Options and default**
- **2019–2023:** Darkest, Dark, Light, Lightest [high, S8, S9], set in Preferences > General [high, S4].
- **2024:** Edit > Preferences > Edit Preferences… (Ctrl+U) > Interface > **Color Theme** offers **Light, Dark, Darkest**. Lightest is gone [high, S36, S11, S3]. On macOS the path starts at Animate > Settings [high, PM S17].
- **The default is Dark** [high, S36, S4, S25].
- **Panel background by theme, measured on 20.0:** Darkest **#1E1E1E**, Dark **#323232**, Light **#F5F5F5**, Lightest **#FFFFFF** [high for 2020, S9]. These equal Spectrum-classic gray-100 in each theme [high, S22].
- **Which gray ramp 24.x uses is not documented.** 24.0 is built on Spectrum version 7, with raised contrast [high, S36].
  - Spectrum's newer ramp keeps Dark gray-100 at #323232, so the panels look unchanged. That matches the reviewer's "very subtle" (S21).
  - The newer ramp also darkens the base and brightens the text, which matches "higher contrast".
  - **Evidence from the 2024 screenshots (§5, §6):** backgrounds match the classic ramp (#252525, #2F2F2F, #323232) and foregrounds match the newer one (#EBEBEB, #545454, #D1D1D1, #3F3F3F). The palette (§2) uses that mix.

Roles follow Spectrum: base = gray-50, layer-1 = gray-75, layer-2 = gray-100, content = gray-800, subdued content = gray-700, disabled = gray-400 [high as Spectrum facts, S41; low as Animate values].

This note's original candidate tokens are kept below for reference. The last column shows what §2 adopted.

| Token (this note) | 2024 estimate (newer ramp) | 2020–23 (classic) | Use | Conf | Adopted in §2 |
|---|---|---|---|---|---|
| `frame` | #1D1D1D | #252525 | splitter gaps, dock title strips, outer border | [low] | app_bg / border #252525 |
| `bar` | #262626 | #2F2F2F | app bar, document tab row, panel tab strips | [low] | bar_bg #2F2F2F |
| `panel` | #323232 | #323232 | panel bodies, active tabs, edit bar, timeline body | [high] 2020 (S9), [med] 2024 | panel_bg #323232 |
| `panel_alt` | #2D2D2D | #2F2F2F | striped rows, subtle wells | [low] | panel_alt #2F2F2F |
| `inset` | #262626 | #252525 | text, number and dropdown fields; Output and code areas; idle toggles in 2024 | [low] | input_bg / panel_inset_bg #262626 |
| `pasteboard` | #1D1D1D | #252525 | area around the stage | [low] 2024, [med] 2020–23 | pasteboard #252525 |
| `menu_bg` | #262626 | #252525 | menus, popovers | [low] | dark-menu fallback only (§4.2) |
| `hover_fill` | #3F3F3F | #3E3E3E | hovered buttons, rows, menu titles (≈ white at 7% over `panel`, S22) | [med] | button_hover #3F3F3F |
| `pressed_fill` | #545454 | #4A4A4A | pressed state | [low] | button_active #545454 |
| `toggled_fill` | #707070 | #5A5A5A | toggled-on tool and quick-action buttons (lighter than idle, S7) | [low] | button_active #545454 (measured) |
| `divider` | #3F3F3F | #3E3E3E | 1 px lines inside panels | [med] | border #252525 (measured dark) |
| `border` | #545454 (hover #707070) | #4A4A4A (hover #5A5A5A) | field and button outlines | [low] | input_border #4F4F4F |
| `text` | #EBEBEB | #E3E3E3 | body text, values | [low] / [med] | text #EBEBEB |
| `text_strong` | #FFFFFF | #FFFFFF | active tab, headings, hovered text | [med] | text_strong #FFFFFF |
| `text_secondary` | #D1D1D1 | #B9B9B9 | labels, inactive tabs, shortcuts, idle icons | [low] / [med] | text_dim #A5A5A5 (measured) |
| `text_disabled` | #707070 | #6E6E6E | disabled text and icons | [low] / [med] | text_disabled #707070 |
| `accent` | #2680EB (hover #378EF0) | same | focus ring, drop zones, links | [med] | accent / accent_hover |
| `cta` | #1473E6 / hover #0D66D0 / down #095ABA | same | primary dialog button, white text | [med] | cta / cta_hover / cta_down |
| `selection_row` | #23528C | same | selected list rows. Spectrum lists use a 10–15% blue tint (≈ #33404F), but Animate's selections look stronger | [low] | selection #40618E (measured) |
| `hot_text` | #4B9CF5 | same | scrubbable numbers in Properties | [low] | dropped: Properties numbers are not blue (§7.1) |
| `shadow` | black 50%, blur 4, offset y 1 | same | menus, popovers, floating panels (Spectrum's dark drop shadow is black at 50%, S41) | [med] | shadow |
| `stage` | #FFFFFF | same | default stage colour | [high] | stage_default |

Spectrum ramps, verified [high, S22]. Gray 50 / 75 / 100 / 200 / 300 / 400 / 500 / 600 / 700 / 800 / 900:
- Classic dark: #252525 / #2F2F2F / #323232 / #3E3E3E / #4A4A4A / #5A5A5A / #6E6E6E / #909090 / #B9B9B9 / #E3E3E3 / #FFFFFF.
- Newer dark: #1D1D1D / #262626 / #323232 / #3F3F3F / #545454 / #707070 / #909090 / #B2B2B2 / #D1D1D1 / #EBEBEB / #FFFFFF.

**Other themes, key values** (2020 panel colours from S9; the rest derived from Spectrum)

| Theme | panel | frame / inset | pasteboard | text / secondary | Conf |
|---|---|---|---|---|---|
| Darkest, 2024 estimate | #1D1D1D | #000000 / #0E0E0E | practically black, ≈ #0E0E0E | #EBEBEB / #D0D0D0 | [low] |
| Darkest, 2020–23 | #1E1E1E | #080808 / #1A1A1A | practically black, ≈ #080808 (S10) | #C8C8C8 / #A2A2A2 | [high] panel, [low] rest |
| Light, 2024 estimate | #F8F8F8 | #E6E6E6 / #FFFFFF | mid grey, ≈ #B1B1B1 | #222222 / #464646 | [low] |
| Light, 2020–23 | #F5F5F5 | #E1E1E1 / #FFFFFF | ≈ #B3B3B3 | #4B4B4B / #6E6E6E | [high] panel, [low] rest |
| Lightest (2023 and earlier only) | #FFFFFF | #EAEAEA / #FFFFFF | mid grey, ≈ #BCBCBC (S10) | #505050 / #747474 | [high] panel, [low] rest |

The accent is about #2680EB in every theme [low]. The Darkest toolbar measured on a 2022 capture is in §6.10.

**States**
- Toolbar, app-bar and edit-bar icon buttons: flat when idle, button_hover on hover, button_active while pressed, and button_active with a white glyph when on [med; hex high from §6.6]. Radius 4, or 2 for buttons under 24 px [low; radius 4 measured on tool buttons].
- 2024 Properties quick-action toggles are different: off, they have a darker-than-panel fill (button_bg #2E2E2E); on, a lighter one (button_active) [med behaviour, S7, S36; hex PM P]. This note first proposed `inset` and #707070 [low].
- Fields: input_bg with a 1 px input_border, radius 4 [med, PM P].
- Push buttons: radius-4 rectangles with button_bg and a 1 px input_border [high, PM P]. This note's Spectrum-pill guess [low] is dropped. A dialog's OK button uses `cta`.
- The focus ring is 2 px `accent`, 1 px outside the control, and only for keyboard focus [med, S22].
- Disabled controls are drawn at about 40% opacity [low].

### 4.8 Typography
- **Family:** Spectrum's typeface is Adobe Clean, and Animate 2020+ probably uses it [low, recall]. It is proprietary: don't ship or copy it.
  - On Windows, load **Segoe UI** from the system at run time without bundling it, as `theme.rs` already does. It runs about 6–8% wider [low].
  - For other platforms, bundle **Inter** (SIL OFL, no Adobe origin) as the fallback.
  - Don't use Source Sans or Source Han. They are Adobe-made, and the clean-room rule excludes Adobe fonts even under an open licence. PM proposed Source Sans 3; that is overruled.
- **Sizes:** see §3.2. This note's own estimates were [low]: 12 for menus, document tabs, panel tabs, breadcrumb and zoom field; 11 for Properties labels, values and tooltips; 12 bold for Properties section headers; 10 for timeline frame numbers. The measured PM and TL values replace the Properties and frame-number sizes.
  - Users of 24.0.13 report unusually small fonts, so don't go larger [med, S32].
- **Weight and colour:**
  - Regular weight everywhere except section headers and active tabs, which are semibold [med, §5.1, §7.2.1]. This note first said the active tab is brighter but not bolder [low].
  - Values use `text`, labels text_dim, disabled items text_disabled.

### 4.9 egui 0.36 mapping, and changes to `apps/editor/src/theme.rs`
- **Visuals fields:**
  - `panel_fill = window_fill = panel_bg`.
  - `extreme_bg_color = input_bg`, `text_edit_bg_color = Some(input_bg)`.
  - `faint_bg_color = panel_alt`.
  - `selection.bg_fill = selection`, `hyperlink_color = accent`.
  - `weak_text_color = Some(text_dim)`.
  - `widgets.noninteractive.bg_stroke = (1, border)`; `window_stroke = (1, popup_border)`.
  - `widgets.inactive.weak_bg_fill = button_bg` for push buttons (flat icon buttons paint nothing at rest), `bg_stroke = (1, input_border)`.
  - `widgets.hovered.weak_bg_fill = button_hover`, `widgets.active.weak_bg_fill = button_active`.
  - `window_corner_radius = menu_corner_radius = 4`, widget radius 4 [low]. Already done.
  - `popup_shadow`: offset [0, 1], blur 4, black at 50%. Already done.
  - `disabled_alpha ≈ 0.4`.
- **What still differs from theme.rs** (full list in the "theme.rs now" columns of §2 and §3):
  - `APP_BG` #1D1D1D → #252525; `BAR` #262626 → #2F2F2F (same as `PANEL_HEADER`); `PASTEBOARD` #1D1D1D → #252525. These three come from this note's newer-ramp estimates, which the 2024 measurements contradict.
  - `TEXT_DIM` #B9B9B9 → #A5A5A5. It also colours idle icons and some secondary text, so check those after the change.
  - `DIVIDER` #3F3F3F → #252525 for structural lines; add `group_divider` #545454 for the vertical separators between button groups.
  - `TOGGLED` #494949 → #545454 (same as `BUTTON_ACTIVE`).
  - `SELECTION` #23528C → #40618E; drop `HOT_TEXT` #4B9CF5 (use `CURRENT_FRAME_TEXT` for the one blue number).
  - `OUTLINE` #545454 → input_border #4F4F4F for fields and buttons, and popup_border #5A5A5A for `window_stroke`. Optional, since the differences are small.
  - `BUTTON` #3C3C3C → button_bg #2E2E2E for push buttons and Properties icon buttons.
  - Metrics: `TOOLS_W` 44 → 48; `TOOL_BUTTON` 32 → 32×26; `TAB_STRIP_H` 24 → 22; `GAP` 2 → 3; `dock_strip` 8 px in `APP_BG` → 11 px in panel_bg, with the chevron at the left for a left-docked column. `APP_BAR_H` 30 and `DOC_TABS_H` 26 stay.
  - The `tab_strip` panel-menu glyph is drawn in `TEXT`; draw it in text_dim at rest (§2.6 item 14).
  - Already matching: `PANEL_BG`, `PANEL_HEADER`, `INSET`, `BORDER`, `TEXT`, `TEXT_DISABLED`, `BUTTON_HOVER`, `BUTTON_ACTIVE`, `ACCENT`, `STAGE_OUTLINE`, all the timeline colours, `LAYER_COLORS`, the tab label colours.

### Sources (workspace part)
- S1 Stage and Tools panel: https://helpx.adobe.com/animate/desktop/workspace-and-workflow/using-stage-tools-panel.html
- S2 Workflow and workspace (generic CS5-era text): https://helpx.adobe.com/animate/desktop/workspace-and-workflow/workflow-workspace.html
- S3 24.0 feature summary: https://helpx.adobe.com/animate/using/whats-new/2024.html (also https://helpx.adobe.com/animate/desktop/introduction-to-animate/whats-new.html)
- S4 Classroom in a Book 2023 sample: https://pearson.com/en-au/media/fm3nuwv2/9780137982424.pdf
- S5 LinkedIn "Application modernization": https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/application-modernization
- S6 LinkedIn "Custom toolbars": https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/custom-toolbars
- S7 LinkedIn "Modernized Adobe Spectrum interface": https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/modernized-adobe-spectrum-interface
- S8 LinkedIn "Modern interface and start screen": https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/modern-interface-and-start-screen
- S9 CEP appSkinInfo thread: https://community.adobe.com/questions-540/panel-background-color-for-cep-panels-hostenvironment-appskininfo-gives-inaccurate-results-106342
- S10 "Pasteboard custom colour" thread: https://community.adobe.com/t5/animate-discussions/pasteboard-custom-colour/m-p/10200667
- S11 "New UI in 24.0" thread: https://community.adobe.com/questions-540/the-new-user-interface-in-animate-version-24-0-142377
- S12 "Club Edit Symbols" thread: https://community.adobe.com/questions-540/is-the-club-edit-symbols-menu-supposed-to-actually-do-something-141787
- S13 "Save custom workspace in 2020" thread: https://community.adobe.com/questions-540/how-can-i-save-my-custom-workspace-in-animate-2020-105752
- S14 Quick share help: https://helpx.adobe.com/animate/desktop/exporting-and-publishing/social-share-publish-animations.html
- S15 Authoring panels help: https://helpx.adobe.com/animate/desktop/workspace-and-workflow/authoring-panels.html
- S16 Assets panel help: https://helpx.adobe.com/animate/desktop/workspace-and-workflow/work-with-assets-panel-animate.html
- S17 Timeline help: https://helpx.adobe.com/animate/desktop/workspace-and-workflow/timeline.html
- S18 Symbols help: https://helpx.adobe.com/animate/desktop/multimedia-and-video/symbols.html
- S19 Scenes help (pre-2020 wording): https://helpx.adobe.com/animate/desktop/animation/scenes.html
- S20 Set preferences help (CC 2015-era text): https://helpx.adobe.com/animate/desktop/workspace-and-workflow/set-preferences.html
- S21 TechRadar 2024 review: https://www.techradar.com/pro/software-services/adobe-animate-2024-review
- S22 Spectrum CSS vars (Apache-2.0), classic and newer: https://cdn.jsdelivr.net/npm/@spectrum-css/vars@2.3.0/dist/ and https://cdn.jsdelivr.net/npm/@spectrum-css/vars@9.0.8/dist/
- S23 "UI in 2020 so big" thread: https://community.adobe.com/questions-540/why-is-the-ui-in-animate-cc-2020-so-big-105665
- S24 July 2024 workspace thread: https://community.adobe.com/questions-540/workspace-143332
- S25 Dark-mode toggle thread: https://community.adobe.com/questions-540/does-adobe-animate-have-a-dark-mode-light-mode-toggle-i-only-manage-to-find-results-concerning-the-142299
- S26 Keyboard shortcuts help: https://helpx.adobe.com/animate/desktop/introduction-to-animate/animate-keyboard-shortcuts.html
- S27 Breadcrumb thread: https://community.adobe.com/questions-540/adobe-animate-missing-breadcrumb-bar-138174
- S28 "November 2019 update is live" thread: https://community.adobe.com/t5/Animate/The-November-2019-update-to-Animate-is-now-live/td-p/10712756
- S29 Workspaces glossary: https://helpx.adobe.com/animate/desktop/using/workspaces.html
- S30 Webucator, working with panels (pre-2020 layout): https://www.webucator.com/article/how-to-work-with-panels-in-adobe-animate/
- S31 Adobe Learn, managing the workspace (client-rendered, not re-checked): https://www.adobe.com/learn/animate/web/manage-workspace
- S32 24.0.13 small-font thread (Japanese): https://community.adobe.com/質問-137/adobe-animateのuiとワークスペースを改善して欲しい-1628951
- S33 Flash CS5 Missing Manual, zoom presets (blocked, not re-checked): https://www.oreilly.com/library/view/flash-cs5-the/9781449380267/apbs03.html
- S34 Small Screen workspace mention: https://community.adobe.com/questions-540/attempting-animate-cc-137255
- S35 LinkedIn "Quick share improvements": https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/quick-share-improvements
- S36 LinkedIn Adobe Animate Essential Training, "Updated interface with Adobe Spectrum v7" (24.0): https://www.linkedin.com/learning/adobe-animate-essential-training/updated-interface-with-adobe-spectrum-v7
- S37 Same course, "Managing Animate preferences": https://www.linkedin.com/learning/adobe-animate-essential-training/managing-animate-preferences
- S38 Same course, "Modifying document settings" and "Working with assets": https://www.linkedin.com/learning/adobe-animate-essential-training/modifying-document-settings and https://www.linkedin.com/learning/adobe-animate-essential-training/working-with-assets
- S39 Fixed issues in Animate (24.0–24.0.15, maintenance-mode note): https://helpx.adobe.com/animate/desktop/troubleshooting/fixed-issues.html
- S40 LinkedIn "Stage clipping and outlines" (Animate CC 2017): https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/stage-clipping-and-outlines
- S41 Adobe spectrum-tokens 12.26.0 (Apache-2.0), color-aliases and color-component: https://cdn.jsdelivr.net/npm/@adobe/spectrum-tokens@12.26.0/src/
- recall: the researcher's own knowledge.

**What this part's fact-checker changed** (kept for traceability)
- The 2024 theme list and default became [high]: the 24.0 LinkedIn course (S36) lists Light, Dark and Darkest with Dark as the default, and S11 independently lists the same three. The draft had [med] and took the default from a 2023 book (S4).
- Named the design-system change as Spectrum version 7 (S7, S36). Added that until 2023 the theme was picked in Preferences > General (S4), and that 2024 moved it to Interface (S3, S36).
- Toggle states: S7/S36 say an inactive toggle has a darker background and an active one a lighter background. The draft drew all icon buttons flat when idle.
- Rebuilt the colour table, which had mixed the two ramps, into a 2024 column and a 2020–23 column, mapped to Spectrum roles verified in spectrum-tokens 12.26.0 (S41) and spectrum-css vars 2.3.0 / 9.0.8 (S22).
- Pasteboard: Spectrum-classic does define its pasteboard alias as gray-50 (#252525 in dark), so the draft's #252525 stands for 2020–23; added the 2024 estimate #1D1D1D [low]; flagged S1's light-gray wording as pre-2019.
- Stage border: the draft said "no border"; S40 and S1 describe an outline, so it became a 1 px black outline with no shadow [med].
- Workspace reset: one reset button per row (S37, S4), not one in front of the current workspace only.
- Removed an unsourced "21.0.4 menu-jumping bug" claim (not in S39) and unsourced 2020–23 pixel figures (Tools about 53, edit bar about 34, tab strip about 28, menu items about 28).
- Zoom-step buttons: left/right chevrons instead of minus/plus, because S5 calls them arrows [low].
- Symbol-editing exits: double-clicking outside the symbol content upgraded to [high, S18]; added Edit > Edit Document and S18's placements (Back at the left of the edit bar, symbol name right of the scene name).
- Center Stage: "zoom stays the same" isn't in S1, so it was downgraded to [low]. Clip content: off by default cites S4 and S40; its menu twin downgraded to [med].
- Share popover: content [high] (S14, S35); the ">" chevrons [low]; HTML5 Canvas only for Canvas documents; Publish has a Publish button.
- Workspace switcher shown as an icon: re-sourced to S37 plus recall. Test Movie action cites S4. Home button: existence and top-left placement [high] (S4); macOS placement added (S13); Windows placement [low].
- Panel menu: Lock upgraded to [high] (S15). Fixed the dock-arrow direction (expanded right column shows », collapsed strip shows «).
- Fonts: removed the Source Sans 3 recommendation (Adobe-made). Kept runtime Segoe UI and bundled Inter. "Animate uses Adobe Clean" downgraded to [low].
- Added maintenance mode (S39, S26); no New Workspace in Window > Workspaces in 2020+ (S13); the Home screen when no document is open (S4); new documents centred at 100% (S38); the tab order Properties / Library / Assets (S4, S38); dragging the Timeline splitter resizes the Timeline (S17).
- "Layout same as 2020–23" is now an inference [med]. Grid and guide defaults downgraded to [med]; added Ctrl+' and the green guide colour (S1).
- Source fixes: a typo for S27; S19 and S20 marked pre-2020; S31 and S33 marked not re-checkable. Noted that the Edit Symbols menu broke in 21–23 for older files (S12).

---

## 5. Timeline panel

Animate 2024 (v24.x), Windows, Essentials workspace, default Dark theme, 100% UI scale. Source keys S1…S38 in this part refer to the list at its end; **R** means recall.

**How to read this part**
- Pixel sizes are logical px at 100%. Unless noted, they were measured on the 2024 screenshots (S31: macOS, 2×, values halved). Windows at 100% should give the same logical sizes [low].
- Colours are hex values sampled from JPEG screenshots of the default Dark theme, accurate to about ±3 levels. Thin features (1 px lines, small dots) sample paler and less saturated than they really are.
- Glyphs are described as simple geometry, to be drawn as original art.

**Evidence base.** Visual facts were checked pixel by pixel on four screenshot sets:
- **S31**: the 2024 screenshots published with TechRadar's review. This part calls them Adobe press images; §6 calls them the reviewer's own captures (§8.1).
- **S30**: Classroom in a Book (CIB) 2024 figures.
- **S29**: CIB 2023 figures (Animate 23.0), including its Customize Timeline panel and panel menu.
- **S32**: a Windows beginner's guide made with Animate 2020.

The web-search budget ran out during this check. Everything else rests on local copies of the cited pages, plus recall where tagged.

**Calibration warning.** The weakest values are the default row height, the default frame-cell width, the layer-column width, the panel height, the shape-tween and IK hex values, and the onion-skin colours and marker look. Keep them in one constants table so a single comparison with a real 100% Windows screenshot can fix them all.

**Biggest changes from the earlier draft**
- In 2024, **New Layer / New Folder / Delete sit in the toolbar row, and Layer View moved down to the header row**. 2020–2023 had them the other way round.
- The default toolbar order is `< Keyframe >` | `Onion, Edit Multiple, Tween, Center Frame` | `Loop, Step back, Play, Step forward`.
- The layer toggle columns are **dot, outline, eye, lock**, with lock at the far right.
- There is no pencil icon, and visible or unlocked layers show empty eye and lock cells.
- The playhead tab carries no number, and frame 1 is not numbered on the ruler.
- Keyframe dots sit below the row's middle.
- Tween fills are muted: purple #78609E, gold #9C8B43.
- Static spans are #909090, and blank-keyframe spans are dark gray.
- The end-of-span marker is a small dark bar, and the loop range is a blue bar.

### 5.1 Placement, chrome and metrics

**Position**
- Docked at the bottom, under the stage, between the Tools bar (left, full height) and the right panel column (Properties/Library, full height). The right column runs the full height (§4.1).
- [high; S29 says that in Essentials the Timeline sits below the Stage; S31]

**Default height:** about 280 px at 1920×1080 [low; in S31 it is 273 px of a window about 900 px tall; R].

**Resizing and docking** [high; S1]
- Drag the bar between the stage and the timeline to resize.
- Drag the tab to float or re-dock the panel; holding Ctrl while dragging prevents docking.
- Drag the bar between the layer names and the frames to change the name-column width.

**Tab strip**
- About 22 px tall, background bar_bg #2F2F2F, with a 1 px #252525 line under it.
- Tabs: "Timeline" (active: near-white #F3F3F3 semibold text on #323232) and "Output" (inactive, #818181) [high existence; med colours; S29, S30, S31].
- No close boxes on the tabs [med; S31].
- The **panel menu** glyph sits at the right end of this strip, not in the toolbar row. It is four 10×1 px lines with 1 px gaps (7 px tall) [high position; med geometry; S31, S32]. It was read as #EBEBEB here. Draw it text_dim at rest and `text` on hover, to cover the #A8A8A8 reading in the Tools header (§2.6).

**Show/hide:** Window > Timeline, or Ctrl+Alt+T [high; S25].

**No bottom status bar.** Since v19 the controls are at the top of the panel [high; S8, whose 19.0 notes say some controls moved from the bottom of the timeline to the top].

**Toolbar position**
- The panel-menu item **"Timeline control - Bottom"** moves the toolbar row under the frame grid [high; S29 and S12 menu screenshots; S9 lists top and bottom options].
- The label probably reads "Timeline control - Top" while the toolbar is at the bottom [med].
- The help page's "Header Top / Header Bottom" wording is out of date [S1].

**Metrics (2024, logical px)**

| Element | Value | Confidence / source |
|---|---|---|
| Tab strip height | 22 | [low; S31] |
| Toolbar row height | 34 (2020–23 ≈ 40) | [med; S31 / low; S32] |
| Layer header row = ruler height | 33 (top tier 14 + 1 px line + tick/number tier 18) | [med; S31] |
| Layer row, Medium (assumed default) | 24 (S31 shows 25; S32 shows ≈ 22) | [low] |
| Layer row, Short / Tall | ≈ 16 / ≈ 32 | [low; ratios from S29/S12, which use Short] |
| Layer (name) column width | ≈ 280 (S31: 300; S32: ≈ 258) | [low] |
| Toggle-column pitch | 25 | [med; S31] |
| Frame cell width at default zoom | ≈ 10 (S32, S29); S31 shows 15 at an unknown zoom; slider minimum ≈ 4 | [low] |
| Scrollbar thickness | 7 (pill-shaped thumb, no visible track) | [med; S31] |
| Hairlines (borders, separators, grid dividers) | 1 px #252525 | [med; S31] |

**Layout sketch (2024)**
```
+-[Timeline][Output]--------------------------------------------------------------------------------[≡]-+ tab strip ~22
| [+][Fo][Tr]               [Cam][Par][Dep] | 24.00 FPS  12 F   [<][K][>] | [On][EM][Tw][CF] | [Lp][◀|][▶][|▶]   [↶] o------ [^^]  | toolbar ~34
| [LV]                    [•]  [▯]  [Ø]  [L] | |    1s  |    |   (top tier: long line every 5 frames, "1s","2s")             | header = ruler ~33
|                                            | ||||||||||||||| (ticks)  5    10   15  (numbers, no "1")                    |
| [pg] Layer_1                  ▮            | ●· · · · ▮ (cells; every 5th empty column lighter)                              | rows ~24
|                                            | (=== horizontal scrollbar thumb ===)                                             |▐ vertical bar
+------------------ layer column ~280 -------+----------------------------------- frame grid ---------------------------------+
```

### 5.2 Toolbar row (top of panel)

**General layout**
- Background #323232, with 1 px #252525 lines above and below.
- The left block sits above the layer column; the frame block starts at the grid's left edge [high; S29–S32].
- In the frame block [med; S31]:
  - The two readouts are left-aligned, starting about 35 px right of the column divider.
  - The customizable button cluster is **horizontally centred in the frame block**. In both S31 shots the cluster's centre matches the block's centre within 1 px.
  - The zoom controls are right-aligned with about 12 px of margin.

**Button style**
- Hit box about 28×28, 30 px pitch, glyph about 12–14 px, colour #EBEBEB. No background at rest [med; S31].
- Groups are separated by a 1×25 px divider (#545454, group_divider) with about 18 px of space on each side [med; S31].
- Hover: a faint light chip behind the glyph (button_hover). Pressed: a slightly darker chip [low; R].
- **Toggled on** (Onion Skin, Loop, Edit Multiple Frames, Camera, Parenting): a filled rounded-square chip about 30×30 (radius about 4) behind the unchanged white glyph. It is **not** a blue glyph [med; S32 shows onion on; S12 shows loop on].
  - The 2020–22 screenshots read the chip as #494949. The 2024 course says it is now easier to see which options are active [S36], and the 2024 toolbar measures #545454 for the active tool (§6.6). Use button_active **#545454** [med].
- **Disabled**: glyph about #6E6E6E (text_disabled). Examples: Create Tween with nothing tweenable; Parenting when it doesn't apply [med; S29, S30 show dimmed icons].
- **Grouped buttons** have a 3 px filled triangle in the lower-right corner of the glyph box [high; S1, S29, S12].
  - Press and hold (about 0.5 s) opens the group's menu.
  - A plain click runs the item that carries the check mark (the last one used).

#### Left block, above the layer column (2024 order)
1. **New Layer.**
   - Glyph: a 13×13 square outline (1.5 px) with a centred "+" whose arms are 7 px long.
   - Tooltip: "New Layer" [med; R].
   - Click: inserts a layer above the selected one; the new layer becomes active [high; S3].
   - It sits in this row in 2024 [high; S30, S31].
2. **New Folder.**
   - Glyph: a filled folder, 14×11 (a 14×8 body plus a 6×3 tab at the top left).
   - Tooltip: "New Folder" [med; R].
3. **Delete.**
   - Glyph: a trash can, 12×13 (a lid bar with a small handle; a tapered bin outline with two inner vertical lines).
   - Tooltip: "Delete" [low].
   - Layers can also be dragged onto it to delete them [high; S3].
   - *(flexible gap)*
4. **Add/Remove Camera** (first of the right-aligned group).
   - Glyph: a filled movie camera, 12×8: a rounded 8×8 body plus a filled triangle on its right side pointing left (a camcorder shape, not a still camera).
   - It is a toggle; turning it on adds a "Camera" layer at the top of the list.
   - [high existence; S1. high glyph; S31, S32. med tooltip; R]
5. **Show Parenting View.**
   - Glyph: an org chart about 14×14: a 7×4 top box, a stem, a 10 px connector, and **two** 7×5 boxes below. It also has the corner triangle.
   - A click toggles the parenting view.
   - Press and hold opens the parenting popup with Propagate Scale/Skew/Flip toggles (22.0 and later).
   - [high; S3 describes the long press; S31, S32]
6. **Layer Depth.**
   - Glyph: 12×12. An L-shaped axis (a vertical line on the left, a baseline at the bottom) with a diagonal arrow rising from the corner to the upper right and ending in a small filled head.
   - A click opens the Layer Depth panel (also Window > Layer Depth).
   - [high; S1, S31, S32]

**Positions**
- Items 1–3 are centred at about 17, 50 and 82 px from the panel's left edge [med; S31].
- Items 4–6 are centred about 92, 59 and 28 px from the column's right edge [med; S31].
- The order of 4–6 is **Camera, Parenting, Layer Depth** [high; S29–S32].

**2020–2023 difference:** this row held **Layer View** at the far left plus items 4–6, and items 1–3 were in the header row instead [high; S29, S32].

#### Frame block, left to right
7. **Frame-rate readout "24.00 FPS".**
   - The number is about 13 px in #A1A1A1 (text_dim), followed by a superscript "FPS" (about 8 px, same gray, top-aligned). Not blue.
   - It shows the document frame rate; during playback it shows the rate actually achieved [high; S1 note, S29–S32].
   - Whether it can be edited here (drag or click, like hot text) is unverified [low; R].
8. **Current-frame readout "12 F".**
   - The integer is about 13 px, followed by a superscript "F".
   - In 2024 the number is light blue, about #8FB8E8 (hot_text). In 2020–2023 it was white [med; S30, S31 vs S29, S32].
   - It follows the playhead [high; S29 says the selected frame's number shows above the timeline's left end].
   - Scrubbing it or typing into it to jump is unverified [low].

**Centred cluster, default set** [high for 2020–2023, from S29's Customize panel defaults, S32 and S12; med that 2024 is unchanged]

9. **Previous keyframe.**
   - Glyph: an open chevron "<", 5×9, 1.5 px stroke (a chevron, not a triangle).
   - Jumps to the previous keyframe on the active layer (Alt+,).
   - Tooltip unverified [low].
   - These arrows replaced the per-layer arrows of older versions [high; S35].
10. **Insert Keyframe group.**
    - Glyph: a light **filled** 12×12 rounded square with a dark mark in the middle showing the current command: a dot (d ≈ 4) = Keyframe; a ring (d ≈ 5) = Blank Keyframe; a short vertical bar (2×6) = Frame.
    - When Auto Keyframe is on, a small "A" sits at the square's top right [high; S29 says the keyframe icon shows a letter A].
    - Long-press menu, with an icon in front of each item and a right-aligned check mark on the current command: "Keyframe", "Blank Keyframe", "Frame", a separator, then "Auto KeyFrame" (a toggle) [high; S29 screenshot, S1].
    - A click applies the current command to the selected frame(s) [high; S1].
    - Inserting a keyframe or blank keyframe makes the cell glow briefly [high existence; S1].
    - Auto Keyframe is **on by default** [med; S29 tells readers to turn Auto Keyframe off, and its first screenshots show the "A"; S13].
11. **Next keyframe.**
    - Glyph: a ">" chevron. Shortcut: Alt+.
    - *A divider follows.*
12. **Onion Skin.**
    - Glyph: two overlapping discs, d ≈ 10. The left one is mid-gray (#9A9A9A); the right one is white, drawn on top and offset about 5 px. Corner triangle.
    - It is a toggle.
    - Long-press menu: "Selected range", "All frames", "Anchor markers", a separator, then "Advanced Settings" [med; S1 lists these four, S28].
    - "Advanced Settings" opens a **floating "Onion skin settings" panel** [high; S32 screenshot; S1]:
      - a mode dropdown ("Range");
      - a Prev colour swatch with its frame count (2);
      - a Next colour swatch with its frame count (2);
      - a bracket with a lock that links Prev and Next (Constraints);
      - an outline/fill segmented toggle;
      - a "Starting opacity" slider (70%);
      - a "Decrease by" slider (20%);
      - a keyframes-only toggle (a square with a dot) and an anchor-markers toggle (an anchor);
      - a reset button.
13. **Edit Multiple Frames.**
    - Glyph: three tall light bars (3×12, 1 px gaps), each with a small dark dot near its bottom. Corner triangle.
    - It is a toggle.
    - Long-press menu: "Selected range", "All frames", "Anchor markers", with a check mark on the current mode [high; S12 screenshot].
    - Hidden by default when the Basic workspace or beginner mode is active [low; S19].
14. **Create Tween group.**
    - The glyph shows the last-used tween type. The classic glyph (the default) is a filled light 9×9 square at the lower left overlapping a second square at the upper right, with a dark right-pointing arrow knocked out of the second square.
    - A click creates the current tween type [high; S1].
    - Long-press menu: "Create classic tween", "Create motion tween", "Create shape tween". They are in sentence case, carry a check mark on the current type, and items that don't apply are dimmed [high; S12 screenshot].
    - The default type is Classic [med; S29's Customize panel shows the classic glyph on the group].
15. **Center Frame.**
    - Glyph: a solid 16×11 rounded rectangle with a dark "T" knocked out of its lower middle: a 10×3 bar at mid-height and a 3 px stem running through the bottom edge. It reads as a playhead inside a viewport.
    - A click scrolls the grid so the playhead is centred.
    - Tooltip: "Center Frame" [med; S1].
    - [glyph and position high; S29, S32, S12]
    - *A divider follows.*
16. **Loop.**
    - Glyph: a 16×12 "tray" with a 2 px left wall, a 2 px floor, and a right wall on the lower half only. A 2 px shaft runs along the top, from the top-left corner to a filled 6×6 arrowhead pointing right at the top right.
    - It is a toggle; when on, the loop-range bar appears in the ruler (§5.5).
    - [high; S12, S31, S32]
17. **Step back one frame.**
    - Glyph: a filled left-pointing 6×10 triangle with a 2 px vertical bar to its **right** (◀|).
    - Click: back one frame. Press and hold: first frame.
    - [high; S1, S18, S32]
18. **Play.**
    - Glyph: a filled right-pointing 9×11 triangle.
    - Toggles playback, same as Enter.
    - The glyph shown while playing is unverified [low].
19. **Step forward one frame.**
    - Glyph: a 2 px bar, then a filled right-pointing triangle (|▶).
    - Click: forward one frame. Press and hold: last frame.
    - [high]

**Optional buttons** (added through Customize Timeline Tools) [high; S29, S31]
- Individual "Keyframe", "Blank Keyframe" and "Frame" buttons (the three square glyphs above).
- "Auto Keyframe" (a dark "A" in a light square).
- "Remove Frames" (a light square with a dark-rimmed disc holding a white minus at its lower right).
- The three individual tween buttons:
  - motion = a light 16×11 diamond cut by a dark right arrow (a horizontal shaft plus a small triangular notch pointing right);
  - shape = a light disc overlapping a light square that has the knocked-out arrow;
  - classic = the group glyph above.
- The 2024 TechRadar screenshots use a customized toolbar: the five frame buttons, Onion, the three tween buttons, Loop and Play [S31]. Don't mistake that for the default.

**Right-aligned zoom controls**
20. **Reset zoom.**
    - Glyph: 14×10, a curved arrow (a 3/4 arc with its head on the left, pointing left) above a 12 px base line.
    - Tooltip / callout: "Reset Timeline Zoom To The Default Level" [high existence; S29. med glyph; S31].
21. **Zoom slider.**
    - A 2 px track (#3F3F3F, slider_track), about 80 px long. The knob is a 12 px ring (2 px stroke, about #E0E0E0, hollow centre).
    - Left end = narrowest cells (about 4 px); right end = widest. It is continuous and changes only the frame-cell width.
    - Tooltip / name: "Resize Timeline View" [med; S29, S1, S31].
22. **Large-zoom glyph.**
    - Two filled triangles ("mountains"), small on the left and large on the right, 16×10.
    - Whether it can be clicked is unknown [low; S29, S31].

**Customize Timeline Tools** (panel menu > "Customize Timeline Tools")
- It opens a popover titled "Customize timeline", with a reset icon (↶) at its top right. S1 calls the reset "Reset Timeline Controls" [high; S1, S29].
- Tiles are about 30×30 with rounded corners, in three rows:
  - Row 1: `<` · Keyframe · Blank Keyframe · Frame · Auto Keyframe · **Frame Buttons Group** · Remove Frames · `>`
  - Row 2: Onion Skin · Edit Multiple Frames · Classic · Motion · Shape · **Tween Buttons Group** · Center Frame
  - Row 3: Loop · Step back · Play · Step forward
- Items currently shown in the toolbar are lighter filled tiles (about #5A5A5A). Hidden items are dark tiles with a 1 px outline [high; S29].
- The defaults (lighter tiles) are: <, Frame Buttons Group, >, Onion, Edit Multiple, Tween Buttons Group, Center Frame, Loop, Step back, Play, Step forward [high; S29].
- Clicking a tile toggles it, and the change applies live. Clicking outside closes the popover [high; S1].
- Choosing a group deselects its individual buttons, and choosing an individual button deselects its group [high; S1].
- `<` and `>` are added and removed as a pair, and so are the two step buttons [high; S1].
- A Remove Frames button exists since 21.0 [high; S13, what's new 21.0].

### 5.3 Panel menu (≡ at the right end of the tab strip)
Exact order from the 2022 and 2023 screenshots [high; S12, S29]:
1. "Standard" (radio, checked by default)
2. "Preview" (radio): each keyframe span shows a thumbnail of its contents.
3. "Preview in Context" (radio): thumbnails scaled to the whole stage [high; S1, S29].
4. separator
5. "Short" (radio)
6. "Medium" (radio)
7. "Tall" (radio)
8. separator
9. "Timeline control - Bottom"
10. "Match FPS" (check). It drops frames during playback to hold the document frame rate [high; S35].
11. "Span Based Selection" (check, off by default) [high; S2, S12]
12. "Customize Timeline Tools"
13. separator
14. "Lock"
15. separator
16. "Help"

Notes:
- "Close" / "Close Group" may follow "Help"; they aren't visible in the screenshots [low]. Use the same ending as other panel menus (§4.6).
- **Default row height:** both books show "Short" checked, but S29's text tells readers to *choose* Short to see more layers. So the default is assumed to be **Medium** [low].
- **Match FPS default:** S35 (2020) says it is on by default, but the 2022 and 2023 menus show it unchecked [low; use off and calibrate].
- There are no frame-size items (the zoom slider replaced them) and no "Tinted Frames" [high; S29, S26].

### 5.4 Layer area

**Column metrics**
- The name column is about 280 px wide (S31: 300), resized with the splitter between layers and frames [low width; high behaviour; S1].
- The divider is a 1 px #252525 line [med; S31].
- Four toggle columns sit at the right of the name column, on a 25 px pitch. Left to right: **Highlight (dot), Outline, Visibility (eye), Lock**; Lock is rightmost [high; S29–S32, six screenshots].
- Column centres, measured from the column's right edge: lock 26, eye 51, outline 77, dot 99 px [med; S31].
- Row heights are in §5.1; rows are separated by 1 px #252525 [med].

**Header row** (same height as the ruler, background #323232)
- **Left, 2024: Layer View.**
  - Glyph: a stack of sheets, 13×14: a filled top rhombus (about 13×7) with two chevron-shaped lower edges beneath it, about 3 px apart.
  - A click switches between showing all layers and only the active layer [high; S1; S29 describes a view-only-the-active-layer option above the timeline].
  - S1 adds a press-and-hold "Active Layer Only" mode, but no corner triangle is visible in the 2020–2024 screenshots [low].
  - Tooltip unverified [low].
  - It is in the header row in 2024 [high; S30, S31], and was in the toolbar row in 2020–2023.
- **Left, 2020–2023:** New Layer, New Folder, Delete [high; S29, S32].
- **Right, centred over the toggle columns:**
  1. **Highlight header**: a filled dot, d ≈ 5. What clicking it does is unknown [high glyph; low behaviour].
  2. **Outline**: a tall rectangle outline, 8×12, with a 2 px stroke. A click turns outlines on or off for all layers [high; S3].
  3. **Visibility**: an eye outline with a pupil, crossed by a diagonal slash from top left to bottom right, 14×14. A click hides or shows all layers [high; S3, S31].
  4. **Lock**: a 10×12 padlock (filled body with a keyhole, arched shackle). A click locks or unlocks all layers [high; S3].
  - Tooltips are probably still "Show All Layers as Outlines", "Show or Hide All Layers" and "Lock or Unlock All Layers" [low; R].
- All header glyphs are #EBEBEB [med; S31].

**Layer row anatomy, left to right** (2024)
1. **Folder disclosure triangle**: 7 px, ▼ expanded / ▸ collapsed, at the far left. Children are indented about 20 px per level [med; S12].
2. **Type icon**: about 12–16 px, light gray, its left edge about 28–30 px from the panel edge. Double-click it to open Layer Properties [high; S1, S3].
   - **Normal**: a 12×12 square sheet (light outline, mid-gray fill) with its lower-left corner dog-eared [high; S29, S31].
   - **Folder**: a folder glyph, open when expanded [med; S12].
   - **Motion-tween layer**: the light diamond cut by a dark right arrow (the Create motion tween glyph), 16×11 [high; S12, S31].
   - **Camera**: the camcorder glyph [low; R].
   - **Mask, masked, guide, classic motion guide, armature**: not verified for 2020+. Suggested drawings [low; R]: mask = a sheet with a circular hole; masked = a normal sheet indented under its mask; guide = a T-square or ruler; motion guide = a curved path with end dots; armature = two discs joined by a bar.
3. **Name**: about 12 px, #D5D5D5 (layer_name), white on the selected row, starting about 50 px from the left. Left-aligned, and ellipsized before the dot column [med; S31].
   - Double-click to rename inline [high; S3, S29].
   - Default names: "Layer_1", "Layer_2", … with an underscore [high; S29, S31].
4. **No active-layer pencil.** The help text that mentions a pencil and a red X dates from the CS era. In 2020–2024 the active layer is simply the highlighted row [high; S29–S32].
5. **Highlight (pin) dot**, in the dot column:
   - Empty at rest.
   - On a hovered or selected row, a filled dot of about 6 px in the **layer's colour** appears.
   - Clicking it pins the colour: the dot stays, and the row is tinted with the layer colour at about 15% over #323232 (a cyan layer gives #3C5051) [med; S31].
   - Pinning also draws an underline in the layer colour across the layer [high existence; S1].
6. **Outline swatch**: a filled **6×12** rectangle in the layer colour, centred in the outline column. It is not a 10×10 square [high; S29–S32].
   - In outline mode it becomes hollow, outlined in the layer colour [med; R].
7. **Visibility cell**:
   - Empty when the layer is visible.
   - A dim (about 35%) eye-with-slash on hovered or selected rows.
   - When hidden, the eye-with-slash is shown at full brightness [med; S29, S31; hidden glyph low].
   - Shift+click sets the layer to "transparent" [high; S1, S3]. The glyph for that state is unknown [low].
8. **Lock cell**:
   - Empty when unlocked; a dim padlock on hovered or selected rows.
   - When locked, a padlock of about 10×12 in about #C8C8C8 [high; S29].

**States**
- Normal row: #323232.
- Selected / active row: **#40618E** (layer_highlight) with a white name [med; S31]. S29 prints it as #3B5C96.
- Several selected rows share that colour [med; R].
- Hover row: #393939 [low].
- Pinned row: the layer colour at 15% (above).
- Dragging layers to reorder shows a 2 px insertion line in the accent blue [low].

**Parenting view** [high; S3]
- A column right of the names shows each layer's "colour head", a small rounded rectangle in the layer colour.
- Drag a head onto another layer to make it that layer's child; a dotted line follows the pointer while dragging.
- The parent is the leftmost head, and children are listed to its right.
- You can also click the strip next to a name to pick a parent from a list, or choose "Remove parent".

**Outline colours**
- The first layer of a new document is **#00FFFF (cyan)** [high; S23 shows Animate-saved "Layer_1" with color="#00FFFF" in several files; S29–S32 screenshots].
- The next layers in the CIB lesson were purple **#9933CC**, then orange **#FF800A** [med; S29, S23].
- Other values seen in Animate-saved files: #FF4FFF, #4FFF4F, #808080, #009999, #FF0000, #669900, #0000FF, and #4F80FF (older "Layer 1" files) [med; S23].
- Some newer files contain arbitrary colours. **Always draw the colour stored in the document** (XFL `DOMLayer color`) [high].

**Layer interactions** [high; S3 unless noted]
- Clicking a name selects the layer, makes it active, and **selects all its frames**.
- Shift+click selects a contiguous range of layers; Ctrl+click toggles individual layers.
- Drag names to reorder; drop onto a folder to nest; drop onto Delete to delete.
- New layers are inserted above the selected one and become active.
- Alt+click in the Eye, Lock or Outline column applies that state to all *other* layers; Alt+click again restores them.
- Dragging down a column toggles every row the pointer passes.
- Click the triangle to expand or collapse a folder. "Expand All Folders" and "Collapse All Folders" are in the context menu.

**Layer context menu**: labels from S3 and recall; the order is approximate [med labels; low order]. "/" marks a separator.
- Show All, Lock Others, Hide Others, Show Others as Transparent / Insert Layer, Delete Layer, Cut Layers, Copy Layers, Paste Layers, Duplicate Layers, Merge Layers / Convert Layers to Symbol (21.0+) / Guide, Add Classic Motion Guide / Mask, Show Masking / Insert Folder, Delete Folder, Expand Folder, Collapse Folder, Expand All Folders, Collapse All Folders / Properties…

### 5.5 Ruler (frame header) and markers

**Overall:** about 33 high, background #323232 (the same as the panel), with 1 px #252525 lines at the top, between the two tiers, and at the bottom [med; S31].

**Top tier (about 14 px)**
- 1 px **dark** (#252525) vertical lines span the tier at the left edge of every 5th frame (5, 10, 15…).
- Seconds labels "1s", "2s"… are italic, about 10 px, #A5A5A5 (the same gray as the frame numbers).
- Each seconds label is left-aligned at frame N × fps: frames 24 and 48 at 24 fps; frame 30 at 30 fps.
- [high existence; S1. med placement; S31, S29]

**Lower tier (about 18 px)**
- A dark tick (#252525, about 9 px) hangs down from the tier line at every frame boundary.
- Frame numbers appear every 5th frame (5, 10, 15…), about 11 px, #A5A5A5. Each is **left-aligned about 2 px inside its frame's left edge**, below the ticks.
- **Frame 1 is not labelled** [high; S29–S32].
- When cells get narrow, labels change to every 10th frame (10, 20, 30…) [high; S31].

**Clicking or dragging anywhere in the ruler** moves the playhead, snapped to whole frames, and the stage updates live [high; S1].

**Loop range** (Loop on)
- A **solid blue bar** fills the top tier from the start frame to the end frame, with small lighter-blue triangular handles at both ends pointing outward [high look; S12 screenshot. low hex: bar ≈ #2A5A90, handles ≈ #4C89CE].
- Drag a handle to set the start or end.
- Drag across the loop strip to create a new range [high; S1].
- Shift+drag moves the whole range [high; S6].
- Playback loops inside the range, and Export Video pre-fills its frame range from it [high; S1, S9].

**Onion-skin range** (Onion on)
- Across the tick and number tiers, the span from the start marker to the end marker is lightened (about +10% white).
- It is bounded by two 2 px vertical bars in the past (left) and future (right) colours [med; S32].
- Default range: 2 frames before and 2 after the playhead [med; S32's settings panel shows Prev 2 / Next 2].
- The markers follow the playhead unless "Anchor markers" is on [high; S1].
- Ctrl+drag a marker to resize both sides equally; Shift+drag to shift the whole range [high; S25 shortcut table].
- Right-click a frame inside the range to exclude or include it [high; S1].

**Edit Multiple Frames range**
- It uses the same marker range as onion skin; an S28 reply says the markers serve both features [med; S28].
- "All frames" puts the brackets at the timeline's first and last frames [high existence; S12. low look].

**Auto-keyframe marker**
- With Auto Keyframe on, selecting a frame beyond the timeline's last frame draws a blue dot (about 6 px, playhead blue) in the tick tier above that frame. The playhead can't move there.
- [high existence; S34, whose 20.5 notes describe a blue dot outside the existing frame range marking the auto-keyframe frame. med position; S29 figure]

### 5.6 Frame grid

**Empty cells**
- Fill #2F2F2F with 1 px #252525 dividers [med; S31, S29, S32].
- **Every 5th column** (frames 5, 10, 15…) is lighter: **#3A3A3A**. It runs through all rows, but only in empty areas: never inside content spans, and not on folder rows [med; S31, S12].
- Default width about 10 px. The slider goes from about 4 px to at least 15 px [low].
- When zoomed out, the cells read as narrow rounded strips [low; S31].
- There are no per-frame dividers inside spans.

**Static spans** (a keyframe followed by plain frames)
- **With content**: fill **#909090** [med; S31; Spectrum gray-600].
- **After a blank keyframe** (no content): a **dark gray** fill, about #4A4A4A. It is *not* transparent [med; S29, S12].
- Both kinds show a row of tiny dark dots along the top edge, one per frame boundary [med; S29, S31, S32].
- There is a 1 px dark line at each keyframe's left edge, where two spans meet [med; S32].
- Span ends have slightly rounded corners [low; S31].

**Keyframe**
- A filled near-black circle (about #1E1E1E; the JPEG reads #262626), **d ≈ 5.5 px**.
- Centred horizontally in its cell. Its centre is at **about 65% of the row height**, below the middle; it is not vertically centred.
- The dot keeps its size when cells get narrow.
- [med; S31, S29, S32]

**Blank keyframe:** a hollow circle of the same size, with a dark stroke of about 1.5 px, on the dark-gray cell [high; S29].

**End of a span:** a small **dark vertical pill**, about 2×8 px, at the right end of the last frame of a static span of 2 or more frames, spanning about 55–80% of the row height. It is not a white box [high; S29, S31, S32].

**Tweens**
- **Classic tween**:
  - Fill **#78609E**, a muted purple [high family; S14. med hex; S31].
  - A 1 px near-black line runs **at the keyframe-dot height**, from just right of the start dot to just before the next keyframe, and ends in an **open** arrowhead (two strokes about 4 px long) [high; S31].
  - The closing keyframe is its own one-frame gray cell with a dot [high; S31].
  - Keyframe boundaries inside a tween row are thin lines in a lighter purple [med; S31].
- **Shape tween**:
  - The fill is a brownish orange, about **#AE6F42** [med family; S15. low hex].
  - Same arrow as the classic tween [high; S4].
- **Broken or incomplete classic or shape tween**: a dashed line replaces the arrow [high; S4]. The dash pattern is unknown [low].
- **Motion tween**:
  - Fill **#9C8B43**, a muted, gold-tinted colour [high family; S12. med hex; S31].
  - A black dot in the first frame means a target object is assigned; a hollow dot means the target was removed [high; S4].
  - **Property keyframes** are small solid black diamonds, about 5 px, drawn at dot height. They appear only where a property is keyed, so the last frame gets a diamond only if something is keyed there [high; S12, S31].
  - Right-click > View Keyframes > (type) filters which diamonds are shown [high; S12].
  - The span's end has a thin light edge [low; S31].
- **IK pose layer**: a green fill, about #6E9550, with a black diamond for each pose [high glyph; S4. low hex].

**Annotations in keyframe cells** [high meaning; S4. low look for 2020+]
- **Frame script**: a small "a" (about 8 px, near-black) above the keyframe dot.
- **Label**: a red flag followed by the label text, clipped to the span.
- **Comment**: a green "//" followed by text. Labels that start with "//" become comments [high; S26].
- **Anchor**: a gold anchor followed by text.
- **Sound**: a waveform drawn across the span [med; R].

**Selection and feedback**
- **Selected frames** use blue ≈ #5A8AD0. It is opaque on empty cells. Over spans it is blended at about 50%: a #909090 static span becomes #7690B3, and a gold span becomes a gray-green. Glyphs stay visible on top [med; S29, S31, S12].
- **Selection edge**: hovering the right edge of a selection shows a ↔ cursor; dragging it stretches or compresses the selected frames. When the new length is a multiple of the original, a small dark tag with white text ("x0.5", "x2", "x3"…) appears at the top of the row by the edge [high; S1, S12].
- **Cursors**: ↔ near the ends of tween spans. An arrow with a tiny box appears when you start dragging a keyframe [high; S12].
- **Insert glow**: a short glow on newly inserted keyframes and blank keyframes [high existence; S1]. Suggested look: a blue halo fading over about 300 ms [low].
- **Folder rows**: no keyframes drawn; the frame area looks like empty cells without the 5th-column highlight [low; S12].
- **Preview / Preview in Context**: thumbnails in each keyframe span [high; S1, S2, S29].

### 5.7 Playhead

**Colour:** about **#4A80CC** [med; S31 head samples #4479C5–#4879BB; S29, S32].
- It has been blue since v19; it was red before [high; S1 and S29 both describe it as blue].

**Head** [high; S29, S30, S31, S32, S12]
- A **translucent blue tab exactly one frame cell wide**, spanning the full height of the ruler.
- Its bottom edge comes to a small downward point, about 4 px.
- Ruler labels (frame numbers, "1s") show through it.
- **It carries no frame number**; the current frame appears in the toolbar readout instead.

**Line:** 1 px, from the head's point down through all rows, at the centre of the cell, drawn above the frames [high].

**Dragging**
- Drag the head, or anywhere on the ruler, to scrub; it snaps to frames [high; S1].
- It stops at the timeline's last frame. Selecting a frame past the end selects it, but the playhead stays put (see the blue dot in §5.5) [med; S29, S32 figures, S34].
- Auto-scroll when dragging past the grid edges is unverified [low].

### 5.8 Behaviours

**Click and drag in the grid**
- **Click a cell**: selects that frame, makes its layer active, moves the playhead there (when the frame is inside the timeline's range), and switches the Properties panel to its Frame tab [high selection; S3. med playhead and Frame tab; S12, S29].
- **Drag across cells**: selects a rectangular range of frames and layers [high; S2].
- **Shift+click** extends the selection; **Ctrl+click** adds or removes single frames [high; S2].
- **Double-click a frame**: selects its whole keyframe span [low; R].
- **Span Based Selection** (panel menu, off by default) [high; S2, S12]:
  - One click selects a whole span; Ctrl+click then selects a single keyframe.
  - Shift+click selects a whole motion tween even when this option is off.
- **Drag** a selected keyframe or span to move it; **Alt+drag** to copy it [high; S2].
- **Ctrl+drag** the first or last frame of a static span to change its length [high; S2].
- **Tween span ends** [high; S12]:
  - A plain drag of the end changes the length and spaces the keyframes evenly across it.
  - **Shift+drag** adds hold frames without moving the last keyframe.

**Keyboard** (Windows default set; the timeline needs focus where relevant). The S32 frame menu shows the real Windows bindings. S22 (hotkeyref) is unreliable: it lists Shift+F6 as "Convert to blank keyframe" and Ctrl+2 as "Zoom 100%".

| Key | Action | Confidence |
|---|---|---|
| F5 | Insert Frame | [high; S2, S29, S32] |
| Shift+F5 | Remove Frames | [high; S12, S32] |
| F6 | Insert Keyframe. The menu shows it on "Convert to Keyframes", so a multi-frame selection becomes all keyframes | [high; S2, S32] |
| Shift+F6 | Clear Keyframe | [high; S32] |
| F7 | Insert Blank Keyframe ("Convert to Blank Keyframes") | [high; shape-tween help, S32] |
| Enter | Play / stop (Control > Play) | [high; S12] |
| , / . | Step back / forward one frame | [high; S37] |
| Shift+, / Shift+. | First frame / last frame | [high; S18, Adobe staff] |
| Alt+, / Alt+. | Previous / next keyframe on the active layer | [high; S1] |
| Alt+Shift+, / Alt+Shift+. | Page back / forward | [med; S1] |
| Ctrl+Alt+X / C / V | Cut / Copy / Paste Frames | [high; S32] |
| Alt+Backspace | Clear Frames | [high; S32] |
| Ctrl+Alt+A | Select All Frames | [high; S32] |
| F9 | Actions panel | [high; S32] |
| Ctrl+Alt+T | Show/Hide Timeline | [high; S25] |
| Space+T (hold) | Temporary Time Scrub tool: drag on the stage to scrub | [high; S1, S12] |
| Ctrl+Enter | Test | [high; WS S4, PM S4] (this part had [med; R]) |
| Ctrl+drag onion marker | Resize both sides equally | [high; S25] |
| Shift+drag onion marker | Shift the whole range | [high; S25] |
| Onion Skin toggle | Ctrl+Alt+O, rebindable. 2018 claims conflict (Ctrl+Alt+O vs Alt+Shift+O); PM cites a 2018 thread for Ctrl+Alt+O [med] | [low here; med · PM C8] |
| Left / Right arrows | Move the playhead (timeline focused) | [low] |

**Playback**
- The playhead advances at the document frame rate.
- "Match FPS" drops frames to keep time [high; S35].
- With Loop off, playback stops at the last frame; with Loop on, it repeats inside the loop range [high; S12].
- The FPS readout shows the live rate while playing [high; S1].
- When the playhead leaves the visible area, the view pages forward [low].
- Whether onion skins show during playback is unverified [low].

**Scrolling and zoom**
- A vertical scrollbar sits right of the rows and a horizontal one under the frame grid [high; S1].
- Both are 7 px pill-shaped thumbs (#D1D1D1) with no visible track [med; S31].
- The mouse wheel scrolls layers [med; R]. Shift+wheel scrolls frames and Ctrl+wheel zooms [low].
- The Hand tool can pan the timeline horizontally [high; S1, S9].

**Frame context menu**: exact labels, order and Windows shortcuts from S32 (2020) [high]. "/" marks a separator.
- Create Classic Tween, Create Motion Tween, Create Shape Tween, Convert to Frame-by-Frame Animation ▸ /
- Insert Frame `F5`, Remove Frames `Shift+F5` /
- Insert Keyframe, Insert Blank Keyframe, Clear Keyframe `Shift+F6`, Convert to Keyframes `F6`, Convert to Blank Keyframes `F7` /
- Cut Frames `Ctrl+Alt+X`, Copy Frames `Ctrl+Alt+C`, Paste Frames `Ctrl+Alt+V`, Paste and Overwrite Frames, Clear Frames `Alt+Backspace`, Select All Frames `Ctrl+Alt+A` /
- Copy Motion, Paste Motion, Paste Motion Special… /
- Reverse Frames, Synchronize Symbols, Split Audio, Show In Library /
- Actions `F9`

Notes:
- Items that don't apply are grayed, not hidden.
- Inside a motion tween, the menu also offers Insert Keyframe ▸ (property types) and View Keyframes ▸ [high; S12].
- On Windows in 2020 this was a **light native menu**: light background, black text, gray disabled items, right-aligned shortcuts [high for 2020; S32. low for 2024]. See the menu tokens in §2.3.

### 5.9 Version notes (the 2024 state is what's described above)
- **19.0 (Oct 2018)** [S8, S14, S15]:
  - Spectrum restyle.
  - Controls moved from a bottom bar to the top.
  - Tween colours changed: classic purple, shape brownish orange, motion gold.
  - Four UI themes: Darkest, Dark, Light, Lightest.
  - Layer parenting added.
  - Blue playhead [med].
- **20.0 (Nov 2019), "Modern Timeline"** [S9, S35]:
  - Onion skin Advanced Settings.
  - Layer highlight colour.
  - Single (active) layer view.
  - The Hand tool pans the timeline; Time Scrub tool added.
  - Grouped long-press buttons (keyframe, edit multiple, tween).
  - A long press on the step buttons goes to the first or last frame.
  - Previous/next-keyframe arrows moved from each layer row to the toolbar.
  - The panel menu gained Match FPS and "Timeline control" top/bottom.
  - Insert glow.
  - Larger "touch-friendly" rows and icons.
  - Beginner/Expert preference sets; the Basic workspace is used for beginners.
- **20.5 (Jun 2020)** [S34]: Auto Keyframe, with the blue dot outside the frame range.
- **21.0 (Oct 2020)** [S13, what's new 21.0]: Customize Timeline Tools (with Remove Frames); Convert Layers to Symbol.
- **22.0 (Oct 2021)** [S3]: parenting propagates Scale/Skew/Flip, controlled from the popup opened by a long press on Show Parenting View.
- **23.0 (Oct 2022)** [S29]: no timeline layout change found. S29's figures are the reference for button order, menus and defaults.
- **24.0 (Oct 2023)**:
  - "Sleek user interface" (described in the LinkedIn course as Spectrum version 7) [S11, S36].
  - The theme is set at Preferences > Interface > Color Theme [S11].
  - Timeline changes:
    - **New Layer / New Folder / Delete moved into the toolbar row, and Layer View moved to the header row** [high; S30, S31].
    - The current-frame number is drawn in hot-text blue [med; S31].
    - The toolbar row is slightly shorter: about 34 px versus about 40 px before [low].
  - The Dark theme panel colour is #323232 [med; S31]. This part left the 2024 theme list unverified [low], but §4.7 confirms Light, Dark, Darkest with Dark as the default [high, WS S36].

### 5.10 Colour readings, mapped to the palette

| Reading | Hex | Confidence | Palette token (§2) |
|---|---|---|---|
| panel_bg (toolbar, header, layer rows, ruler) | #323232 | [high; S31, S32] | panel_bg |
| tab_strip_bg | #2F2F2F | [med; S31] | bar_bg |
| tab_text active / inactive | #F3F3F3 / #818181 | [med; S31] | tab_text_active / tab_text_inactive |
| hairline (borders, row separators, column divider, grid dividers, ruler ticks and 5-frame lines) | #252525 | [med; S31] | border / frame_line |
| icon | #EBEBEB | [med; S31] | text |
| icon_disabled | #6E6E6E | [low] | text_disabled (#707070) |
| toolbar_divider | #545454 | [med; S31] | group_divider |
| toggle_on_chip | #494949 | [med; S32, S12] | button_active #545454 (2024) |
| fps_text | #A1A1A1 | [med; S31] | text_dim (#A5A5A5) |
| current_frame_text (2024) | ≈ #8FB8E8 | [med; S31] | hot_text |
| ruler_text (numbers and seconds) | #A5A5A5 | [med; S31] | text_dim |
| layer_name / selected name | #D5D5D5 / #FFFFFF | [med; S31] | layer_name / text_strong |
| layer_row_selected | #40618E | [med; S31] | layer_highlight |
| layer_row_hover | #393939 | [low] | layer_hover |
| layer_row_pinned | layer colour at 15% over panel_bg | [med; S31] | formula |
| grid_empty | #2F2F2F | [med; S31] | frame_cell |
| grid_5th_column | #3A3A3A | [med; S31] | frame_cell_5th |
| span_static (content) | #909090 | [med; S31] | frame_span |
| span_blank (no content) | ≈ #4A4A4A | [low; S29, S12] | frame_span_blank |
| keyframe / arrow / end-marker ink | ≈ #1E1E1E (near-black) | [med; S31] | keyframe |
| classic_tween | #78609E | [med; S31] | classic_tween |
| motion_tween | #9C8B43 | [med; S31] | motion_tween |
| shape_tween | ≈ #AE6F42 (brownish orange) | [low; S15 family] | shape_tween |
| ik_pose | ≈ #6E9550 (green) | [low; S4 family] | ik_pose |
| selection | ≈ #5A8AD0 (opaque on empty cells, about 50% over spans → #7690B3 on static) | [med; S29, S31] | frame_selected |
| playhead | ≈ #4A80CC (tab about 90% opaque) | [med; S31, S29, S32] | playhead |
| loop_range bar / handles | ≈ #2A5A90 / ≈ #4C89CE | [low; S12] | loop_range / loop_handle |
| onion past / future | blue and green by default (S16); one 2020 install shows #99FFCD / #00FF00 | [low; S16, S32] | onion_past / onion_future |
| scrollbar_thumb | #D1D1D1 | [med; S31] | scrollbar_thumb |
| slider track / knob ring | #3F3F3F / ≈ #E0E0E0 | [med; S31] | slider_track / slider_knob |
| label_flag / comment / anchor | red / green / gold | [high family; S4. low hex] | label_name / label_comment / label_anchor |
| first layer outline colour | #00FFFF | [high; S23, S29–S32] | layer_color_first |

### 5.11 Differences from the minnamate editor (noted at research time; recheck `timeline.rs`)
`theme.rs` already carries the timeline colours above; the layout items below were noted against `apps/editor/src/timeline.rs` during research.

**Colours (theme.rs), now applied:** `MOTION_TWEEN` #9C8B43, `CLASSIC_TWEEN` #78609E, `SHAPE_TWEEN` ≈ #AE6F42, `FRAME_SPAN` #909090, `FRAME_CELL` / `FRAME_CELL_5TH` #2F2F2F / #3A3A3A, `PLAYHEAD` ≈ #4A80CC, `LAYER_SELECTED` #40618E. Still open: `TOGGLED` #494949 → #545454 (§5.2).

**Toolbar layout**
- **Keep** New Layer / New Folder / Delete at the left of the toolbar row; that is correct for 2024.
- Add Camera, Parenting and Layer Depth, right-aligned over the layer column, in that order.
- Readouts: the FPS readout comes **before** the current-frame readout (the code had them reversed). Render them as "24.00" plus a superscript "FPS" (gray) and "1" plus a superscript "F" (blue).
- Remove "Go to first frame" and "Go to last frame" buttons; press-and-hold on the step buttons and Shift+, / Shift+. do this.
- Remove "Onion Skin Outlines"; it is a toggle inside the Onion skin settings panel.
- Rebuild the cluster as `< [Keyframe group] >` | `Onion, Edit Multiple, Tween group, Center Frame` | `Loop, Step back (◀|), Play, Step forward (|▶)`, horizontally centred in the frame block.
- Add the reset-zoom button, the zoom slider and the mountain glyph at the right.
- The panel menu (≡) belongs at the right end of the tab strip.

**Header row (left of the ruler)**
- Add Layer View at the left.
- The right-side order must be **dot, outline, eye, lock**; the code had eye, lock, outline.

**Layer rows**
- Drop the pencil.
- Visibility and lock cells are **empty** when visible / unlocked; show a dim eye-slash and a dim lock only on hovered or selected rows.
- Use a filled **6×12** outline swatch, not a hollow 10×10 square.
- Add the highlight-dot column.
- The default layer colour is cyan #00FFFF for the first layer; use the stored colour.

**Ruler**
- Two tiers, about 33 px tall.
- Top tier: italic seconds labels and dark lines every 5 frames.
- Lower tier: dark per-frame ticks and left-aligned numbers every 5 frames; **no "1"** (the code labelled frame 1 and centred its numbers).

**Playhead:** replace the numbered tab with a translucent blue tab one cell wide, spanning the whole ruler, with a pointed bottom.

**Spans**
- Draw the tween arrow at the keyframe-dot height, not at the span's centre.
- Replace the end marker (a white box with a black stroke) with a dark 2×8 pill.
- Fill blank-keyframe spans dark gray instead of leaving them unfilled.
- Add the per-frame dots along the top of static spans.
- Selection: opaque blue on empty cells, about 50% blue over spans.

**Metrics** (to calibrate): row height 22 → about 24; cell width 10 can stay (about 10 at default zoom); layer column 250 → about 280; toolbar 32 → about 34; ruler 24 → about 33.

**Frame context menu:** replace the earlier 9 items with the full S32 list and its Windows shortcuts. F6 and F7 are shown on the "Convert to…" items.

### Sources (timeline part)
**S1–S28 (from the draft, re-checked)**
- S1: helpx "How to use the timeline in Animate" (archived 2024-08-28; current copy at helpx.adobe.com/animate/desktop/workspace-and-workflow/timeline.html, Jun 2026, same text). Some of its sentences are CS-era leftovers: the Header Top/Bottom wording, a Center Frame button at the bottom, and Large/Medium/Short sizes.
- S2: helpx frames-keyframes.
- S3: helpx timeline-layers. It contains stale text: a pencil icon, a red X, and a new-layer button at the bottom.
- S4: helpx animation-basics. CS-era colours: blue motion and classic tweens, light green shape tween.
- S5: helpx time.html.
- S6: helpx frame-by-frame-animation (onion skinning).
- S7: helpx classic-tween-animation.
- S8: what's new 19.0.
- S9: what's new 20.0 (Nov 2019).
- S10: what's new 20.5.
- S11: what's new 24.0 (Oct 10, 2023): "Sleek user interface", Preferences > Interface > Color Theme.
- S12: Classroom in a Book (2022 release): Peachpit motion-tween chapter and the Pearson sample PDF (ISBN 9780137623587), figures on pp. 120–142.
- S13: LinkedIn Learning "Adobe Animate New Features".
- S14: Community, v19 timeline preferences (purple classic tween; colours follow Spectrum).
- S15: Community, shape tween described as brownish orange (Apr 2019).
- S16: Community, onion skin colours blue and green by default (Mar 2020).
- S17: Community, "no rewind button".
- S18: Community, first/last frame in 2020. An Adobe employee confirms long-press and Shift+, / Shift+.
- S19: Community, Edit Multiple Frames hidden (2021; a 2024 reply says to switch to the Essentials workspace).
- S20: Community, CEP panel background colours.
- S21: UCSC library guide.
- S22: hotkeyref. **Unreliable; not used for any claim.**
- S23: Animate-saved XFL files on GitHub (chrismrau Lessons; shad0wshayd3-FO4/BakaInterface-Scaleform). "Layer_1" color="#00FFFF".
- S24: Pearson CIB 2024 sample.
- S25: helpx Animate keyboard shortcuts (Ctrl+Alt+T; onion marker Ctrl/Shift+drag).
- S26: Community, timeline notations.
- S27: Community, onion skin button shortcuts (2018).
- S28: Community, retracting the onion and edit-multiple bars (2021): "Advanced Settings" in the long-press menu.

**S29–S38 (added in the check)**
- S29: *Adobe Animate Classroom in a Book (2023 release)*, Pearson sample PDF, ISBN 9780137982424, Lesson 1 pp. 14–27. Text plus figures: timeline header, panel menu, Customize timeline panel, Insert Keyframe menu, Auto Keyframe "A".
- S30: *Adobe Animate Classroom in a Book 2024 Release*, sample PDF ISBN 9780138317713, Lesson 2 figures (p. 56): the 2024 timeline header layout.
- S31: Screenshots of Animate 2024 (macOS, 2×) published in TechRadar's "Adobe Animate (2024) review", 27 Mar 2024, credited to Adobe there: https://www.techradar.com/pro/software-services/adobe-animate-2024-review
- S32: "Adobe Animate: Beginner's Guide" (a school PDF exported from Google Docs in 2020; Animate 20.x on Windows). Figures: full timeline, frame context menu with shortcuts, onion skin on, "Onion skin settings" panel.
- S33: helpx's current mirror of the help centre (Jun 2026): frames-keyframes, shape tweening (Insert > Timeline > Blank Keyframe or F7), the parenting section of timeline-layers.
- S34: helpx what's new 20.5 (June 2020): Auto keyframe and its blue dot outside the existing frame range.
- S35: LinkedIn Learning "Timeline workflow enhancements" transcript (Match FPS drops frames and is on by default there; keyframe arrows moved from per-layer to the toolbar).
- S36: LinkedIn Learning "Modernized Adobe Spectrum interface" transcript (Animate 2024, Spectrum v7; active options easier to see).
- S37: Community "Select Timeline Hotkey?" (Apr 2018; Colin Holgate notes that , and . step to the previous and next frames).
- S38: helpx what's new 21.0 and 22.0 (Customize Timeline Tools, Convert Layers to Symbol; Enhanced Parenting).

**What this part's fact-checker changed** (kept for traceability)
- Panel menu: it sits at the right end of the tab strip, its glyph is four 10×1 lines (S31, S32); the real item is "Timeline control - Bottom" (S29/S12, S9), not "Header Top/Bottom".
- 2024 layout: New Layer/New Folder/Delete are in the toolbar row and Layer View in the header row (S30, S31); the draft described the 2020–23 arrangement as 2024. Camera/Parenting/Layer Depth order fixed (four screenshots).
- Glyphs redrawn from screenshots: Layer View, Camera, Parenting, Layer Depth, New Layer; step buttons un-mirrored; keyframe arrows are open chevrons; the keyframe group is a filled square with a dot, ring or bar plus an "A" for Auto Keyframe; Create Tween and Edit Multiple Frames glyphs and menus corrected (S12, S29).
- Default toolbar order corrected from S29's Customize panel defaults plus S32/S12.
- Onion skin Advanced Settings is a floating panel with the listed controls (S32); no "Present" swatch.
- Zoom controls corrected: reset button, Resize Timeline View slider with a ring knob, mountain glyph (S29, S31).
- FPS readout is gray; the current-frame readout is "12 F" with a superscript F, blue only in 2024. Toggled-on buttons use a chip behind a white glyph, not a blue glyph.
- Panel menu rebuilt from screenshots; Match FPS default downgraded to [low].
- Toggle column order fixed to dot, outline, eye, lock (six 2020–24 screenshots); pencil and red-X cell glyphs removed (stale help text); outline swatch is a filled 6×12 rectangle; highlight dot in its own column, with a ~15% tint and an underline when pinned.
- Selected-layer colour #40618E (measured); first-layer colour #00FFFF (Animate-saved XFL plus screenshots), with #4F80FF belonging to old "Layer 1" files.
- Ruler corrected: ~33 px, two tiers, frame 1 not labelled, numbers every 5 (every 10 when zoomed out), left-aligned, #A5A5A5, italic seconds at frame N×fps, dark ticks. Loop range is a solid blue bar with handles. Playhead is a translucent cell-wide tab with no number, ~#4A80CC, and can't pass the last frame. The auto-keyframe dot is in the ruler (S34, S29).
- Grid colours corrected (#2F2F2F, #252525 dividers, lighter #3A3A3A 5th column in empty areas). Static span #909090 with dots; blank spans dark gray; keyframe dot at ~65% height, d ~5.5, fixed size; end marker a dark 2×8 pill; tween fills muted (#78609E, #9C8B43), shape and IK downgraded to [low]; classic arrow at dot height with an open head; motion-tween diamonds only on keyed frames.
- Selection colour ~#5A8AD0, opaque on empty cells and ~50% over spans.
- Keyboard table re-sourced from the S32 Windows context menu; S22 flagged unreliable; onion toggle left unconfirmed. Frame context menu replaced with the exact S32 list.
- Metrics recalibrated from the 2024 2× screenshots. Version notes: 24.0 added the layout swap and the blue frame number; 20.0 moved the keyframe arrows to the toolbar (S35); 23.0 is the reference state.

---

## 6. Tools panel

Animate 2024 (v24), Windows, Essentials workspace, default Dark theme. Every item carries a confidence tag and a source key; the keys are listed at the end of this part. All sizes are px at 100% UI scale.

**Evidence**
- Most measurements come from S2: two macOS Retina (2×) screenshots of Animate 2024 published with TechRadar's review. They are 3270×1808 at 144 dpi, captured in March 2024 and finished in Pixelmator Pro, so this part treats them as the reviewer's own captures, not Adobe press images (§5 calls the same images Adobe press screenshots; §8.1). Values here are halved to 1× [high].
  - S2a: toolbar docked left, Selection active, a timeline frame selected.
  - S2b: toolbar docked right, a Rectangle Primitive selected.
- The S2 geometry agrees with the 2024 *Classroom in a Book* screenshots (S1), and a 2022 2× capture (S5c) has the same 48 px column. No 2024 Windows capture at 100% was found, so applying these numbers to Windows is an inference [med].
- **Button space** means a 32×26 box with its origin at the top left, x to the right and y down.
- **Clean-room:** coordinates are placement guides for drawing original icons in the same style. Do not trace screenshots. The screenshot copies made for measurement were deleted after use.

### 6.1 Placement and panel chrome
- **Position.** The panel docks at the far left edge of the window in its own full-height dock column. It runs from just under the application bar to the window bottom [high · S2a, S1 p.68, S4].
  - Flyouts and the Edit Toolbar drawer open to its right [high · S1 pp.56, 65, 68].
  - Document tabs, Stage and Timeline sit to its right. The Timeline does not run underneath it [high · S2a, S4].
  - Animate 2020 moved the toolbar to the left in Essentials [high · S4 first-launch capture, S6 captures].
  - In 2019 and earlier the default layout docked it at the far right [med · S12].
- **Columns.** Single column by default [high · S1, S2].
  - Dragging the outer edge wider reflows the tools into more columns [med · S9].
  - Adobe's shared workspace help says the double-arrow one-or-two-column toggle from Photoshop and Illustrator isn't available in Animate. So the « strip must not do that [high · S13].
  - Docked horizontally (for example in the Animator workspace) it becomes a single row, with vertical separators between sections, the colour block at the end, and a visible "Tools" tab label [high for 2020 · S4; med for 2024].
- **Width and background.** Inner width **48 px**, background **#323232** [high · S2a].
  - A 1 px #252525 line on the outer (window) edge [high · S2a].
  - A ~3 px #252525 splitter between the column and the document area [high · S2a].
- **Header (top 30 px)**, from the top down [high on macOS · S2a; med on Windows]:
  - a 1 px #252525 line;
  - an 11 px strip in #323232 with a small « double chevron (#C8C8C8, x ≈ 5–12). This is the dock column's collapse control. When the column is docked right it shows » at the right edge [high · S2b];
  - a 1 px #252525 line;
  - a 16 px tab row with a **#2F2F2F** background, holding:
    - at the left, the "Tools" tab: about 10 px wide, #323232 (the panel body colour), with a 1 px #252525 right edge. It is too narrow for its label, so only a clipped sliver of the "T" shows;
    - at the right (x ≈ 34–44), the panel-menu button: 4 horizontal 1 px lines, 10 px wide, 2 px pitch, **#A8A8A8**;
  - a 1 px #262626 line.
- **Name and menu.** The panel is called "Tools" and opens with Window > Tools (**Ctrl+F2**) [high · S3]. A customized toolbar is saved as part of the workspace [high · S8].

### 6.2 Button metrics (2024)

| Item | Value | Tag |
|---|---|---|
| Tool button (hit area and highlight) | 32×26 at x = 8…40 (8 px side margins) | [high · S2a] |
| Pitch inside a section | 30 (4 px gap) | [high · S2a] |
| Section separator | 2 px line #272727, x ≈ 8.5…39.5, 4 px gap above and below (adds 6 px to the pitch) | [high · S2a] |
| Icon box | 16×16, centred: x 8…24, y 5…21 in button space. Free Transform overhangs to about 19 px wide | [high · S2a] |
| Flyout indicator | a solid right triangle with its right angle at the bottom right: (26.5,24)–(30,24)–(30,20.5) in button space. Legs 3.5 px, 2 px in from the right and bottom edges, #EBEBEB | [high · S2a] |
| Icon colours | #EBEBEB; the active tool's icon is #FFFFFF; mid-grey #A2A2A2 for shape fills inside icons | [high · S2a] |
| Drawing at 1× on Windows | put 1 px strokes on pixel centres. Half-pixel values come from the 2× source and may round by 0.5 px | [med · inference] |

**Vertical layout** (y measured from the top of the column, T) [high · S2a; option slots S2b]

| Element | y |
|---|---|
| Header | T+0…30 |
| Selection | 35 |
| Free Transform | 65 |
| Lasso | 95 |
| *separator* | 125–127 |
| Fluid Brush | 131 |
| Classic Brush | 161 |
| Eraser | 191 |
| Rectangle | 221 |
| Line | 251 |
| *separator* | 281–283 |
| Pen | 287 |
| Text | 317 |
| Paint Bucket | 347 |
| Eyedropper | 377 |
| Asset Warp | 407 |
| *separator* | 437–439 |
| Hand | 443 |
| Zoom | 473 |
| "…" slot (no separator before it) | 503 |
| Black and White / Swap icons | 537–549 |
| Fill swatch | 555–577 |
| Stroke swatch | 566–588 |
| Tool-option slots | 594, 624, … (26 tall, 30 pitch) |

Below that, the column is empty #323232 down to the window bottom.

### 6.3 Default tools, top to bottom
The order and grouping are confirmed by S1 p.68, S2a and the S6 2020 capture; they have not changed since 2020 [high].

A group button shows its first member until another member is used, either from the flyout or by that member's shortcut. It then shows that member [high · S2b; shortcut case med · recall].

The ▸ column marks buttons that show the flyout triangle.

| # | Sec. | Button | Flyout members, in menu order (Windows shortcut) | ▸ |
|---|---|---|---|---|
| 1 | A | Selection Tool | Selection Tool (V) · Subselection Tool (A) | yes |
| 2 | A | Free Transform Tool | Free Transform Tool (Q) · Gradient Transform Tool (F) | yes |
| 3 | A | Lasso Tool | Lasso Tool (L) · Polygon Tool (Shift+L) · Magic Wand Tool (no key) | yes |
| — | | *separator* | | |
| 4 | B | Fluid Brush Tool (Shift+B) | none | no |
| 5 | B | Classic Brush Tool (B) | none | no |
| 6 | B | Eraser Tool (E) | none | no |
| 7 | B | Rectangle Tool | Rectangle Tool (R) · Rectangle Primitive Tool (Shift+R) · Oval Tool (O) · Oval Primitive Tool (Shift+O) · PolyStar Tool (no key) | yes |
| 8 | B | Line Tool (N) | none | no |
| — | | *separator* | | |
| 9 | C | Pen Tool | Pen Tool (P) · Add Anchor Point Tool (=) · Delete Anchor Point Tool (-) · Convert Anchor Point Tool (Shift+C) | yes |
| 10 | C | Text Tool (T) | none | no |
| 11 | C | Paint Bucket Tool | Paint Bucket Tool (K) · Ink Bottle Tool (S) | yes |
| 12 | C | Eyedropper Tool (I) | none | no |
| 13 | C | Asset Warp Tool (W) | none | no |
| — | | *separator* | | |
| 14 | D | Hand Tool | Hand Tool (H) · Rotation Tool (Shift+H) · Time Scrub Tool (Alt+Shift+H) | yes |
| 15 | D | Zoom Tool (Z) | none | no |
| 16 | | "…" (Edit Toolbar) | no key | no |

- **Shortcuts.** All of them match an export of Animate's own keyboard-shortcut table (S3, April 2024) [high].
  - The keys and order of the Rectangle and Paint Bucket groups are also visible in Animate's own flyouts [high · S1 pp.56, 65].
  - Rotation Shift+H and Time Scrub Shift+Alt+H also appear in S4, and Width U in S7.
- **Names.** Flyout entries are named "… Tool" [high · S1 pp.56, 65].
  - "Polygon Tool" and "Magic Wand Tool" are Adobe's own names [high · S14 selecting-objects].
  - So are Add, Delete and Convert Anchor Point Tool [high · S4].
  - The shortcut dialog uses other command names ("Lasso Polygon", "Pen Convert Anchor", "Sub Selection"). Don't use those as labels or tooltips [high · S3].
- **Tools without a key.** PolyStar and Magic Wand have no default key. PolyStar is a JSFL "extensible" tool and can't take a direct shortcut [high · S3, S5h].
- **No cycling.** In the 2024 defaults every tool has its own key, so no key cycles through a group [high · S3]. This overrides §7.5's first draft.

### 6.4 Hidden by default (only in the Edit Toolbar drawer)
These are the bright (addable) tiles in the 2024 drawer [high · S1 p.68, S6 2020 capture].

| Drawer group (forms one flyout when dragged in) | Tools (shortcut) |
|---|---|
| 3D | 3D Rotation Tool (Shift+W) · 3D Translation Tool (G). Both show a warning badge in HTML5 Canvas documents, where 3D is unsupported [high · S1 p.68, S10] |
| Paint Brush | Paint Brush Tool (Y) · Pencil Tool (Shift+Y) |
| Camera | Camera Tool (C) |
| Bone | Bone Tool (M) · Bind Tool (Shift+M) |
| Width | Width Tool (U) |

The drawer also holds the **Spacer**. All shortcuts in this table are [high · S3].

**Other keyboard behaviour**
- **Held keys.** These quick-access keys are hard-coded and can't be edited in Keyboard Shortcuts [med · S5g 2019].
  - Space: temporary Hand Tool [high · S6, S5g].
  - Shift+Space: temporary Rotation Tool [high · S6, S5g].
  - Space+T: temporary Time Scrub Tool [high · S7 timeline].
  - Ctrl+Space: temporary zoom-in [low · recall; S5g only says a quick zoom exists].
  - Ctrl while a drawing tool is active: temporary Selection Tool [low · recall, Flash-era behaviour].
- **Size keys.** "[" or Shift+[ decreases the stroke or brush size; "]" or Shift+] increases it [high · S3].
- **Colour and option keys.** D = Black and White, X = Swap Colors, J = Object Drawing, Shift+D = Global Transform [high · S3].

### 6.5 Flyout menu (2024)
- **Opening.** Press and hold a ▸ button, or right-click it [high · S1 p.56 (holding the Rectangle tool), S6 (pressing the visible tool's icon), S7 3D (click and hold), S4 (right-click)]. The hold time is about 0.4 s [low · guess].
  - The menu opens beside the button: to the right when docked left, and to the left when docked right [med · inference].
  - Its top lines up with the button's top, and its left edge touches the button's right edge [high · S1].
  - The button shows the pressed highlight while the menu is open [med · S1 pp.56, 65].
- **Shape and colours** [med · S1, colours read from print]:
  - Width about 222–227, sized to the content.
  - Background about **#1E1E1E** (popup_bg), darker than the panel.
  - A 1 px light-grey border (about #5A5A5A–#6E6E6E).
  - Corner radius about 4–5, with a soft drop shadow.
- **Rows** [med-high · S1, scaled against the 30 px toolbar pitch]:
  - Each row is **24 px** tall, with about 4 px of padding at the top and bottom of the menu.
  - A 14 px icon sits at x ≈ 8. Icons use the same glyphs; shape tools use mid-grey filled shapes with light outlines.
  - The name (#EBEBEB, about 12 px) starts at x ≈ 29.
  - The shortcut is right-aligned about 8 px from the right edge, written "Shift+R" on Windows.
  - The current member has a small blue check mark (Spectrum blue, about #378EF0, about 10×9) directly left of its shortcut, touching it. The 2024 book explains that this check marks the member the toolbar currently shows [high · S1 p.65; hex med].
  - The hovered row gets a slightly lighter background [med · S1 p.56]. Its colour is about #2C2C2C [low].
- **Choosing a member.** It becomes the active tool, and **the group button shows that member's icon** until another member is chosen [high · S2b, where the Rectangle slot shows Oval Primitive].
- **2020–23 look** [high · S6 2020 captures, S5a]:
  - The current item was marked by a small light-grey square bullet at the far left instead of a check.
  - Dark theme: background #3F3F3F with a 1 px #444444 edge.
  - Darkest theme: background about #1A1A1A with a #3F3F3F border.
  - Rows were taller, about 32 px [med · S5a, not re-checked].

### 6.6 Button states and interaction

| State | What to draw | Tag |
|---|---|---|
| Normal | no background; icon #EBEBEB | [high · S2a] |
| Hover | a rounded 32×26 background of about #3F3F3F–#454545 (button_hover). Tooltip after about 0.5 s, for example "Selection Tool (V)" | [low · recall] |
| Pressed / active tool | a rounded rect 32×26, **radius 4, #545454**; the icon becomes #FFFFFF. Only one tool is active at a time; Selection is active at startup | [high · S2a; startup med] |
| Disabled / unsupported | icon dimmed (solid areas about #555555). In the drawer, a yellow warning badge (about 11×10, about #F5C400 with a dark "!") sits at the icon's lower left. Examples: Fluid Brush without a DirectX 12 (feature level 12_0) GPU on Windows or Metal on macOS; 3D tools in HTML5 Canvas documents | [high · S5a, S5e, S1 p.68; badge on toolbar buttons med] |

**Interaction**
- A click selects the tool on mouse-down. On a group button, a plain click selects the member currently shown [med · recall].
- Picking a tool brings up the **Tool** tab of the Properties panel (tabs: Tool, Object, Frame, Doc) [med · S8 defines the tabs, S4].
  - The tab still follows later selections: S2a shows the Frame tab while Selection is the active tool [high · S2a].
- **Double-click behaviours**
  - Eraser: erases everything on the Stage and the pasteboard [high · S14 reshape].
  - Hand: fit the Stage in the window [low · recall, Flash-era].
  - Zoom: 100% [low · recall, Flash-era].

### 6.7 Icons
**Shared style** [high · S2a, S1]
- Monochrome glyphs in the 16×16 box: #EBEBEB on #323232, and #FFFFFF on the active highlight.
- About half are **solid silhouettes**: brushes, eraser body, pen nib, bucket, pin, hand, T, Subselection, camera, bone.
- The rest are **1–1.25 px outlines**: Selection arrow, lasso, zoom ring, eyedropper tube, transform box, line, polygon, convert-anchor.
- Diagonal tools lie at 45°, with the working end (tip or nib) at the lower left and the handle at the upper right.
- Shape tools use a mid-grey #A2A2A2 fill with a 1 px light border.
- The only colours are the gradient square, the yellow warning badge, the swatches and the red no-colour slash.
- **2020–23 differences** [high · S6 2020 capture]:
  - Glyphs were about #BABABA (Dark theme).
  - Shape icons had a dark fill.
  - The Selection arrow had a dark fill inside its light outline; the 2024 arrow is hollow.

**Toolbar glyphs** (button-space coordinates, approximate; draw original art)
1. **Selection:** a hollow pointer arrowhead with no tail, 1 px outline, no fill.
   - Tip at ≈ (11,5); a vertical left edge down to ≈ (11,21.5).
   - Inner corner at ≈ (15.5,16.5); right corner at ≈ (22.5,16.5).
2. **Free Transform:**
   - A small solid arrowhead (≈ (7,4.5)–(7,18)–(16.5,13.5)) over a 1 px square (≈ x 12.5–25.5, y 6–19.5).
   - The square is drawn as a broken outline, with eight solid 2×2 handles at the corners and edge midpoints.
3. **Lasso:**
   - A 1 px ellipse (≈ x 8–24, y 6–16.5).
   - A small 1 px loop (Ø ≈ 3.5) at its lower left, where the rope crosses.
   - A 1 px tail from the loop down to y ≈ 21, ending in a tiny leftward hook.
4. **Fluid Brush:**
   - Handle: a round-ended 3 px bar from ≈ (24,5.5) down-left to ≈ (16.5,13).
   - A 1 px gap.
   - Bristle tip: a solid teardrop about 4.5 px wide at the ferrule, tapering to a point at ≈ (8.5,20.5).
   - A "paint drop" disc centred at ≈ (21,17), r ≈ 3.5, fill #C9C9C9 with a 1 px #EBEBEB rim.
5. **Classic Brush:** the same brush without the drop.
6. **Eraser:**
   - A 9×13 block rotated 45°, with corners at about top (17.5,4.5), right (24.5,11.5), bottom (13.5,21) and left (8,15.5).
   - The upper-right two-thirds are solid; the lower-left third is outline only, with a 1 px seam between them.
7. **Rectangle:** a rect at x 8–24, y 6–20, fill #A2A2A2, 1 px #EBEBEB border.
8. **Line:** a 1.5 px diagonal with round caps, from ≈ (9.5,19.5) to ≈ (22.5,6.5).
9. **Pen:**
   - A solid fountain-pen nib pointing to the lower left (tip ≈ (8.5,20)).
   - A thin slit in the background colour runs from the tip to a small round hole near the nib's centre.
   - A 1 px gap, then a short solid collar at the upper-right end.
10. **Text:** a slab-serif capital T.
    - A 2 px top bar (≈ x 9.5–23) with small downward serifs at both ends.
    - A 2 px stem down to y ≈ 18.5.
    - A short foot bar about 6 px wide.
11. **Paint Bucket:**
    - A solid can (≈ 8×10) tilted about 45°, with its mouth facing the upper right.
    - A 1.25 px handle arc at its upper left.
    - A 1.5 px drip from the mouth's right corner down to y ≈ 18, ending in a Ø ≈ 2.5 drop.
12. **Eyedropper:**
    - A solid rounded bulb at the upper right (≈ (22,7)).
    - A short solid collar across the axis.
    - A ≈ 3 px-wide tube drawn as a 1 px outline, running down-left to a rounded closed tip at ≈ (9,20.5).
13. **Asset Warp:** a pushpin on the diagonal.
    - Solid head at the upper right: cap, short neck, and a ≈ 7 px base plate across the axis.
    - A 1.5 px needle from the plate down-left to ≈ (7.5,19).
14. **Hand:** a solid open hand.
    - Palm ≈ x 12–22, y 12–21, rounded at the bottom.
    - Four fingers about 2.3 px wide with round tips, fanning slightly, the middle finger tallest, with 1 px gaps between them.
    - The thumb sticks out to the left, to x ≈ 8.
15. **Zoom:** a 1.25 px ring centred at ≈ (13.7,11), r ≈ 5.5, and a 2 px handle to ≈ (23.5,20.5) with a round cap.
16. **"…":** three Ø4 dots centred at (10,14), (16,14) and (22,14).

**Flyout and hidden glyphs** (same style) [med · S1 p.68 drawer and flyouts at print resolution, S6 2020 palette capture]
- **Subselection:** the Selection arrowhead, solid filled.
- **Gradient Transform:** a ≈ 12×12 square with a 1 px light border, filled with a horizontal gradient from light to dark.
- **Polygon:** an irregular closed 1 px polygon (roughly a pentagon), with the lasso loop and tail at its lower left.
- **Magic Wand:** a 1.5 px diagonal stick with three small 4-point sparkles near its upper end.
- **Rectangle Primitive:** the Rectangle glyph plus a 2×2 handle on each corner.
- **Oval:** a ≈ 14×10 grey ellipse with a light border.
- **Oval Primitive:** the Oval plus a 1 px radius line from the centre to the right edge, a centre dot, and a 2×2 handle at the right end.
- **PolyStar:** a grey hexagon with a flat top and bottom (points left and right) and a light border.
- **Add / Delete Anchor Point:** the pen nib at about 80% size, shifted lower right, with a 5×5 "+" or a 5 px "−" at the upper left.
- **Convert Anchor Point:** a 1.25 px open polyline forming a sharp peak.
- **Ink Bottle:** a small solid tipped pot at the upper left, pouring towards a 1.25 px right-angle bracket at the lower right.
- **Rotation:** the hand at about 75% size, with a tilted square outline behind it and a curved arrow over its upper left.
- **Time Scrub:** the hand at about 70% size, placed low, under a 1 px double-headed horizontal arrow about 14 px long.
- **3D Rotation:** a wireframe globe: a circle with r ≈ 6, a tilted equator and one meridian.
- **3D Translation:** three 1.25 px arrows from one origin, pointing up, right and down-left.
- **Paint Brush:** the brush at about 75% size, with a 1.25 px S-shaped swash curling left of the tip.
- **Pencil:** a solid pencil on the diagonal, with a band near its top end and a cone tip at the lower left.
- **Camera:** a solid rounded body (≈ 10×8) with a trapezoid lens flaring out to the right.
- **Bone:** a solid bone on the diagonal: a 3 px shaft with double knobs at each end.
- **Bind:** a smaller bone with a 1 px dashed circle (Ø ≈ 8) around its upper-left knob.
- **Width:** an S-wave stroke of varying thickness, crossed by a short handle made of two small hollow circles joined by a line.
- **Spacer** (drawer only): a 16×2 light bar.

### 6.8 Bottom section
**Edit Toolbar "…"** [high · S2a, S1 p.68, S6]
- It occupies the slot after Zoom, with no separator before it.
- No background at rest. While the drawer is open (or on hover) it shows a pill of about 32×17 in #545454 [med · S1 p.68]. In 2020 the pill was about 32×16 in #4B4B4B [med · S6 captures].
- Clicking it toggles the drawer (§6.9).
- Adobe's help calls it the "More Options" icon in the toolbar, which opens the "Tools Palette". The 2024 book tells readers to click the three dots at the bottom of the Tools panel to edit the toolbar [high · S6, S1 p.68].
- Its tooltip is probably "Edit Toolbar…" [low · recall].

**Black and White and Swap Colors** [high · S2a, S3, S14]
- They share a 12 px row: Black and White at x 8–20, Swap at x ≈ 27–39.
- **Black and White (D):** sets the stroke to black and the fill to white [high · S14 color].
  - Glyph: two 9×9 squares offset by (3,3).
  - The back square (lower right) is black with a 1 px #B8B8B8 outline.
  - The front square (upper left) has a 1 px #B8B8B8 outline, then a 1 px black ring, then a white fill.
- **Swap Colors (X):** exchanges the stroke and fill colours.
  - Glyph: an ≈ 11×11 arc bulging up-right, with an arrowhead at each end: one pointing left at the top-left end, one pointing down at the bottom-right end.
  - It is drawn dimmed (about #555555) whenever the stroke or fill is "no colour". This was seen in S2a, S2b and S5c (2022); treat it as disabled in that state [med].

**Fill and stroke swatches** [high · S2a, S2b, S1 p.68, S6]
- **Layout:** two overlapping 22×22 squares. Adobe says the block was redone to resemble the Photoshop and Illustrator Tools panels [high · S6].
  - Fill: top left, at x 8–30.
  - Stroke: offset by (+11,+11), at x 19–41.
- **Fill:** a solid colour square with a 1 px black outer border and a 1 px white inner border [high · S2a].
- **Stroke:** a hollow ring 6 px thick around a 10×10 hole [high · S2a, S1 p.68, S6 2020 capture].
  - From the outside in: 1 px black, 1 px white, **2 px in the stroke colour**, 1 px white, 1 px black.
  - The hole is about #404040.
  - Evidence: S2a, with no stroke colour, reads 1 black + 4 white + 1 black; S1 p.68, with a black stroke, shows white rims either side of the black band; 2020 used the same layering with a 3 px band.
- **Overlap:** the fill is drawn in front in S2a, S2b, S5c and S6. In S1 p.68 the stroke is in front while stroke segments are selected. The colour targeted last probably comes to the front [med-low].
- **No pencil or bucket glyphs** next to the swatches since 2020. They were there in 2019 and earlier, and the Color panel still has them [high · S2, S4, S5c].
- **Clicking a swatch** opens the colour-swatches pop-up for that colour. It has a hex field, an alpha field, a No Color button, a colour-picker button and the swatch grid [med · recall].
- **"No colour"** is a white swatch or ring with a **2 px red (about #E0302A) diagonal from the bottom left to the top right**, clipped to the visible part [high · S2a].
- **There is no separate No Color button** in the 2020+ toolbar. Older help text still mentions one [high · S2, S4, S6 captures].

**Tool options**
- Adobe describes the options area as holding modifiers for the selected tool [high · S6].
- Slots are 32×26 on the 30 px rhythm with centred 16×16 glyphs, starting at T+594 [high · S2b].
- Toggles that are on use the #545454 highlight [med].
- Options that don't apply are drawn very faint (about #404040–#4A4A4A on #323232) [med · S2b].
- They mirror the Properties panel's Tool tab [med · S4].

| Tool | Options shown in the toolbar |
|---|---|
| Selection | **Smooth** (glyph: a 1 px S-curve, about 8×15) and **Straighten** (glyph: a 1 px step going down, right, then down; about 7×14). Each click applies Smooth or Straighten once to the selected raw shape. Enabled for raw shapes [high · S1 p.68, S14]. Dimmed when a primitive or symbol is selected [high · S2b]. Not shown when nothing on the Stage is selected [med · S2a]. The same commands also sit in Properties > Object > Shape Options (§7.2.4) |
| Line, Pen, Pencil, Classic/Fluid/Paint Brush, Rectangle, Oval, PolyStar | **Object Drawing** toggle (J). Glyph: a 13×13 light square outline holding a light-grey filled circle of about Ø9. Off (Merge Drawing) by default, and the state is remembered between sessions. Rectangle Primitive and Oval Primitive have no toggle [high · S1 pp.56, 59, S5c, S5i, S3] |
| Classic Brush | Object Drawing, then a **Brush Mode** pop-up: a button showing the current mode's glyph plus ▸. Modes: Paint Normal · Paint Fills Only · Paint Behind · Paint Selection · Paint Inside ("Paint Fills" in the 2020 capture; "Paint Fills Only" since 2021, §7.2.3) [high for 2020 · S6 capture; 2024 med] |
| Eraser | **Eraser Mode** pop-up: Erase Normal · Erase Fills · Erase Lines · Erase Selected Fills · Erase Inside [high for 2020 · S6 capture; 2024 med] |
| Pencil | Pencil Mode: Straighten · Smooth · Ink [med · S7 draw page, S3] |
| Paint Bucket | **Gap Size** menu: Don't Close Gaps · Close Small Gaps · Close Medium Gaps · Close Large Gaps [high · S1 p.65, S14, S3]. Lock Fill [med · S14] |
| Brushes, Eraser | Pressure and Tilt toggles, shown only when a Wacom pressure tablet is connected [high · S6, S14] |
| 3D Rotation, 3D Translation | **Global Transform** toggle (Shift+D), on (global) by default [high · S10, S7 3D, S3] |
| Zoom | Enlarge / Reduce modifiers (pre-2020 help text) [low · S6] |

**Snap to Objects is not on the toolbar** in 2020 and later [high · S2a, S1 p.68]. It lives in three places:
- the Properties panel's Doc tab, as a magnet icon above Publish Settings [high · S5d];
- View > Snapping > Snap To Objects (**Ctrl+Shift+U**) [high · S3] (§7.4 lists Ctrl+Shift+/ [med]; see §8.1);
- the Stage right-click menu [high · S5d].

In 2019 and earlier it was a magnet button in this options area.

### 6.9 Edit Toolbar drawer ("Drag and drop tools")
**Position and frame** [med · S1 p.68 proportions]
- A floating panel that abuts the toolbar's right edge. Its top is a few px above the first button.
- About **240** wide and 625 tall.
- Background about #333333, with a 1 px border of about #3E3E3E and a radius of about 4.
- Esc or a click outside closes it [high · S1].

**Header**
- About 48 px tall.
- Title "**Drag and drop tools**": sentence case, about 13 px, about #E0E0E0, at x ≈ 16 [high · S1 p.68].
  - Adobe's help calls the panel the "Tools Palette"; the 2024 book calls it the "Drag And Drop Tools panel".
  - The 2020 title was "All Tools" [high · S6 captures].
- A ≡ button sits at the right. Its menu has **Reset**, which returns the Tools panel to its default state [high · S6, S5b].
  - The 2020 menu was: Reset · (separator) · Comfortable (checked) · Compact · (separator) · Close [high · S6].
  - Whether the 2024 menu still offers Comfortable and Compact is unconfirmed [low].
- A 1 px divider of about #4C4C4C below the header.

**Grid** [med · S1 p.68]
- 16 px padding.
- 4 columns × 10 rows of **40×40 cells with 16 px gutters** (16 + 4·40 + 3·16 + 16 = 240).
- A tool group is a single rounded tile spanning its rows: n×40 + (n−1)×16 tall.
- Cell shading:
  - about #3D3D3D with an icon of about #6A6A6A if the tool is already on the toolbar;
  - about #4B4B4B with an #EBEBEB icon if it can be added.
- Hovering an addable cell shows a 2 px blue border (about #2680EB) [low · recall].
- Unsupported tools show the warning badge [high · S1, S5e].

**Fixed layout** (column: row → tile) [high · S1 p.68; identical in the S6 2020 capture]
- **Col 1:** 1–2 Selection/Subselection · 3 Classic Brush · 4 Eraser · 5–9 Rectangle/Rect Primitive/Oval/Oval Primitive/PolyStar.
- **Col 2:** 1–2 Free Transform/Gradient Transform · 3 Fluid Brush · 4 Line · 5–6 Paint Bucket/Ink Bottle · 7 Camera · 8–9 Bone/Bind.
- **Col 3:** 1–2 3D Rotation/3D Translation · 3–4 Paint Brush/Pencil · 5 Text · 6 Eyedropper · 7–9 Hand/Rotation/Time Scrub.
- **Col 4:** 1–3 Lasso/Polygon/Magic Wand · 4–7 Pen/Add/Delete/Convert Anchor · 8 Width · 9 Zoom · 10 Asset Warp.
- **Row 10, cols 1–3:** the Spacer tile (152×40, with a short horizontal bar glyph).

**Behaviour** [high · S6, S8; visuals med]
- Drag a tile onto the toolbar between buttons to insert it; an insertion line shows where.
- Drop it onto a tool or group to merge it into that group.
- Drag a tool from the toolbar into the drawer to remove it.
- Drag the Spacer into the toolbar to add a separator.
- Dragging a separator out of the panel tears that section off as a floating mini-toolbar; dragging it back merges it. Clicking the highlighted bottom section on hover flips it between horizontal and vertical. Skip this for v1.

### 6.10 Changes from 2020–2023
- **Density.** 2020–23 had two toolbar densities, chosen from the drawer's ≡ menu [high · S6 capture; the 20.0 blog mentions compact and comfortable · S8].
  - **Comfortable** (checked in the 20.0 help captures): an inner column of about 50 px, about 42 px pitch, and a highlight of about 34×34 with a 1 px lighter rim [med · S6 capture].
  - A 2022 capture shows 32×32 buttons at 34 pitch in a 48 px column, likely **Compact** [high · S5c].
  - 2024 uses 32×26 at 30 pitch. Adobe describes the 24.0 UI as sleeker, with more working space [high · S15]. A community expert says the toolbar was revised and made smaller in 24.0 [med · S16].
- **Icons:** about #BABABA (2024: #EBEBEB).
- **Active tool background:** #4B4B4B in Dark (2024: #545454), #383838 in Darkest [high · S6, S5c].
- **Darkest theme** (2022 capture): background #1E1E1E, frame #080808, active #383838 [high · S5c].
- **Flyouts:** taller rows with a square bullet; 2024 has 24 px rows with a blue check.
- **Fluid Brush:** the 2020 help capture shows no flyout triangle on it. One August 2020 user mentions a menu of its variations [low · S5a].
- **Colour "card" (2020):** a slightly lighter rounded card (about #3E3E3E, with a #3B–#3D edge) sat behind the colour block and options. 2022 (S5c) and 2024 are flat [high · S6, S4, S5c].
- **2024 colour theme:** set in Edit > Preferences > Interface > Color Theme [high · S15]. The default appears to be Dark (#323232), since both S1 and S2 use it [med]; §4.7 confirms Dark [high].

### 6.11 Suggested build order
1. Build the static column:
   - header, sections, separators and buttons with active and hover states;
   - flyouts, including the icon swap;
   - the swatches with Black and White, Swap and no-colour;
   - the options area with Object Drawing and Smooth/Straighten.
2. Add the drawer as add/remove/Reset only.

Leave tear-off, multi-column reflow, density switching and the per-tool mode pop-ups until they are needed.

### Sources (tools part)
- **S1:** *Adobe Animate Classroom in a Book 2024 Release* (Russell Chun), sample pages 52–69. Text on pp.56, 57, 59, 65 and 68; screenshots on pp.56, 65 and 68 (macOS).
  - https://ptgmedia.pearsoncmg.com/images/9780138317713/samplepages/9780138317713_Sample.pdf
  - Same pages: https://pearson.com/en-au/media/gaakc4nw/9780138317584.pdf
- **S2:** TechRadar, "Adobe Animate (2024) review": https://www.techradar.com/pro/software-services/adobe-animate-2024-review. Two macOS 2× screenshots on that page: S2a (toolbar docked left) and S2b (toolbar docked right). Direct image links are omitted under the clean-room rule.
- **S3:** an export of Animate's keyboard-shortcut table (April 2024): http://tapanvfx.blogspot.com/2024/04/adobe-animate-cc.html
- **S4:** a 2020 Windows beginner's guide: https://studentcabletelevision.com/wp-content/uploads/2020/05/Adobe-Animate-Website-Guide-Updated.pdf
- **S5:** Adobe Community threads.
  - a (Aug 2020): https://community.adobe.com/t5/animate/where-is-fluid-brush-animate-cc/td-p/11348164
  - b (Feb 2021): https://community.adobe.com/t5/animate-discussions/edit-gt-toolbar-setting-missing/m-p/11840638
  - c (Mar 2022, includes a 2× Darkest capture): https://community.adobe.com/questions-540/i-cannot-find-object-drawing-mode-but-every-tutorial-has-it-139378
  - d (Aug 2021): https://community.adobe.com/t5/animate-discussions/assist-me-quot-snap-to-objects-quot-button-is-missing-on-adobe-animate/m-p/12354312
  - e: https://community.adobe.com/t5/animate-discussions/fluid-brush-tool-is-grayed-out/td-p/10942377
  - g: https://community.adobe.com/questions-540/hand-tool-shortcut-only-on-click-139586 and https://community.adobe.com/questions-540/how-do-i-disable-the-keyboard-shortcut-for-the-rotation-tool-in-animate-105232
  - h (Mar 2023, PolyStar is a JSFL tool): https://community.adobe.com/t5/animate-discussions/how-to-add-shortcut-keys-to-tool-extensions/td-p/13649237
  - i (Apr 2024): https://community.adobe.com/t5/animate-discussions/object-mode-preview/m-p/14530693
- **S6:** helpx "Use the Stage and Tools panel" (read directly, page updated June 9, 2026), including its embedded 2020 captures: the toolbar, the "All Tools" palette and its ≡ menu, the Eraser and Brush mode pop-ups, the fill/stroke block. https://helpx.adobe.com/animate/using/using-stage-tools-panel.html
- **S7:** further helpx pages.
  - 3D graphics: https://helpx.adobe.com/animate/desktop/multimedia-and-video/3d-graphics.html
  - Draw lines and shapes (Width U, Pencil modes): https://helpx.adobe.com/animate/desktop/multimedia-and-video/draw-simple-lines-shapes.html
  - Timeline (Space+T): https://helpx.adobe.com/animate/using/timeline.html
- **S8:** Adobe blog, Animate 20.0: https://blog.adobe.com/en/publish/2019/11/04/adobe-animate-20-0-is-live-now
- **S9:** LinkedIn Learning, "Custom toolbars" (not re-checked): https://www.lynda.com/Adobe-Animate-tutorials/Custom-toolbars/577560/2260689-4.html
- **S10:** *Classroom in a Book 2022* sample (Lesson 3, Global Transform): https://ptgmedia.pearsoncmg.com/images/9780137623587/samplepages/9780137623587_Sample.pdf
- **S12:** Webucator: https://www.webucator.com/article/how-to-work-with-panels-in-adobe-animate/
- **S13:** helpx "Animate workflow and workspace": https://helpx.adobe.com/animate/desktop/workspace-and-workflow/workflow-workspace.html
- **S14:** helpx pages on selection, reshaping, strokes and colour.
  - Selecting objects: https://helpx.adobe.com/animate/desktop/multimedia-and-video/selecting-objects.html
  - Reshape lines and shapes: https://helpx.adobe.com/animate/desktop/multimedia-and-video/reshape-lines-shapes.html
  - Strokes, fills and gradients: https://helpx.adobe.com/animate/desktop/multimedia-and-video/strokes-fills-gradients.html
  - Color: https://helpx.adobe.com/animate/desktop/multimedia-and-video/color.html
- **S15:** helpx What's new, October 2023 (24.0): https://helpx.adobe.com/animate/using/whats-new/2024.html
- **S16:** Adobe Community, January 2025: https://community.adobe.com/questions-540/is-it-possible-to-resize-animate-s-ui-to-be-smaller-143885
- **recall:** the researcher's own knowledge of the product.

**What this part's fact-checker changed** (kept for traceability)
- S2 provenance: the TechRadar images are the reviewer's own macOS 2× captures, not Adobe press screenshots; S5c is a 2022 2× capture of unknown OS. The match to Windows at 100% is [med] (inference).
- Stroke swatch layering corrected to 1 black, 1 white, 2 colour, 1 white, 1 black (not a 4 px colour band); the hole is about #404040.
- Header: the tab stub shows a clipped "T"; added the measured colours (tab row #2F2F2F, menu lines #A8A8A8, « chevrons #C8C8C8 at x ≈ 5–12); the separator x-range refined to 8.5–39.5.
- Removed "pressing a shared key cycles tools" (every 2024 default key is unique, S3). S5h is cited for PolyStar being a JSFL tool.
- Shortcuts all re-verified against S3 (and several in S1, S4 and helpx) and upgraded to [high]; noted that the shortcut dialog's command names aren't the tool names. Polygon / Magic Wand / Anchor Point tool names upgraded to [high].
- Held keys: Space and Shift+Space confirmed [high]; Space+T confirmed; Ctrl+Space and Ctrl-for-Selection downgraded to [low]; the quick-access keys are hard-coded (S5g).
- "Selecting a tool switches Properties to the Tool tab" downgraded to [med]. Eraser double-click confirmed [high]; Hand and Zoom double-clicks downgraded to [low].
- Flyout: press-and-hold and right-click confirmed; hold time [low]; check-mark hex [med]; pressed highlight while open; 2020 colours and square bullet added; 2020–23 row height [med].
- Edit Toolbar naming ("More Options" icon, "Tools Palette", 2020 title "All Tools", 2020 ≡ menu) and Reset confirmed.
- Density: 2020–23 offered Comfortable and Compact; removal in 2024 unconfirmed. 2020 details corrected (colour card ≈ #3E3E3E, icons ≈ #BABABA, filled 2020 arrow, dark shape fills).
- Fluid Brush flyout downgraded to [low]. Tool options refined (Smooth/Straighten states, faint colour, Brush/Eraser/Pencil modes, Gap Size names, Lock Fill, Global Transform default on, Zoom Enlarge/Reduce [low], no "Faucet", Pressure/Tilt only with a tablet). Swap Colors dimmed when a colour is none raised to [med].
- Columns: the 1/2-column toggle isn't available in Animate; the horizontal layout is [high for 2020, med for 2024]. The warning badge is confirmed in the drawer; on toolbar buttons [med]. DX12 / Metal requirement added. Theme setting location confirmed; Dark default inferred [med].
- The S2 pixel measurements were re-verified and kept. The screenshot copies used for measurement were deleted.

---

## 7. Properties panel, Library panel, menus and shortcuts

Adobe Animate 2024 (v24.0.x), Windows, Essentials workspace, default Dark theme. Source keys refer to the list at the end of this part.

### 7.0 How to read this part
- **Tags:** [high] = stated in Adobe help text or shown in an Adobe screenshot; [med] = partly confirmed, or long-stable Flash/Animate behaviour; [low] = best recollection or inference; check it on a real install.
- **Source keys:** S1–S54 are Adobe help pages, C1–C10 Adobe Community threads, and L1 LinkedIn Learning. **P** is a pixel or colour reading the first author took from Adobe's own help-page screenshots (Windows and macOS captures, 2021–2022). **R** is recall.
- **The fact-check pass (Oct 2026):**
  - Every S and C page was re-read as text.
  - Web search ran out partway through, so the Window and Help menu order and a few shortcuts still rest on recall, and are tagged that way.
  - No Adobe image was downloaded, so P readings were not re-measured. They line up with Adobe's public Spectrum dark palette: #252525, #2F2F2F, #323232, #3E3E3E, #4A4A4A, #5A5A5A, #6E6E6E, #909090, #B9B9B9, #E3E3E3, accent #2680EB. That is a useful cross-check [med, R].
- **What changed in 2024:**
  - v24.0 (Oct 2023) advertised a "sleek user interface" with more working space [high, S17].
  - It also added a Color Theme setting [high, S17]. Windows: Edit > Preferences > Edit Preferences > Interface > Color Theme. macOS: Animate > Settings > Edit Preferences > Interface > Color Theme.
  - One 24.0 user (Mac) reports the theme list is still Darkest / Dark / Light [med, C3]. The thread says nothing else about what changed.
  - Adobe's 2021–2022 doc screenshots show panels of about #323232, which is the Dark theme. Treat Dark as the default [med, P]; §4.7 confirms it [high].
  - Because 24.0 claims more working space, heights measured on 2021–2022 screenshots are upper bounds. Expect 24.0 to be a few px tighter [low].
- **24.0 is the last feature release.** Adobe's 2026 help pages say Animate is in maintenance mode (bug and security fixes, no new features). The newest build listed is 24.0.15 [high, S1, S38].
- **Clean-room rule:** every icon below is plain geometry to redraw from scratch. Functional labels may be copied; product names may not. Write "minnamate Help" and "About minnamate", and drop Adobe-service items. Never draw the "An" lettering.

### 7.1 Shared visual tokens for these panels (Dark theme)
These are the 2021–22 P readings. The last column gives the palette token (§2) that applies in 2024.

| Reading | Approx. value | Use | Tag | 2024 palette (§2) |
|---|---|---|---|---|
| panel.bg | #323232 (#333333 in Windows captures, #303030 in Mac captures) | Background of the Properties and Library panels | high, P | panel_bg #323232 |
| panel.tabstrip | #2F2F2F | Row holding the panel tabs; also the application menubar strip. The draft also gave the menubar #323232; use #2F2F2F for both | med, P | bar_bg #2F2F2F |
| dock.gap | #141414–#1A1A1A | Gaps between docked panels | med, P | app_bg #252525 (measured in 2024) |
| section.divider | 2 px line, #242424 | Full-width line between Properties sections | high, P | border #252525, 2 px |
| pill.active | #4A4A4A, radius about 5 | Selected Tool/Object/Frame/Doc tab | high, P | category_pill |
| text.value | #E6E6E6 | Field values, active tab text, instance name | med, P | text #EBEBEB |
| text.section | #C8C8C8, semibold | Section titles | med, P | section_title |
| text.label | #8E8E8E | Field labels (W, H, X, Y, Profile, "Instance of:") | med, P | text_dim #A5A5A5 |
| text.dim | #707070 | Disabled text, the value after "Instance of:", disabled tabs (#6D6D6D) | med, P | text_disabled |
| field.underline | 1 px #474747 under the value, no box | Numeric fields | high, P | field_underline |
| input.box | fill #262626, 1 px border about #4A4A4A, radius 4 | Text inputs (instance name, frame label) | med, P | input_bg / input_border |
| dropdown.enabled | fill #262626–#2E2E2E, 1 px border #555555, radius 4, light "v" chevron at the right | Dropdowns | med, P | input_bg / input_border |
| dropdown.disabled | fill #3D3D3D, no border, text #6F6F6F | Disabled dropdowns | high, P | keep as read |
| button | fill #2E2E2E, 1 px border #4F4F4F, radius 4, centred text #E0E0E0 | "More settings", "Match contents", "Remove Tween" | high, P | button_bg / input_border / text |
| iconbutton | 28×28, fill #2E2E2E, 1 px border #4F4F4F, radius 4, glyph #B9B9B9 | Quick-action buttons | high, P | glyph in `text` (#EBEBEB) for 2024 |
| iconbutton.on | fill #565656 | Toggled-on quick action | high, P | button_active #545454 |
| focus / selected | 2 px outline about #2680EB | Selected looping mode, focused icon button | med, P | accent |
| checkbox | 14 px box, 1.5 px border #9A9A9A, radius 2. Checked: fill #C8C8C8 with a #262626 check mark | Checkboxes | med, P | checkbox_border / checkbox_on |
| tooltip | white fill, near-black text, 1 px grey border | Tooltips (pre-2024 captures; 24.0 not re-checked) | low, P | tooltip_bg |
| swatch | rounded rectangle about 30×16, radius 3, filled with the colour | Colour swatches | high, P | — |
| alpha glyph | 12 px checkerboard square, followed by "100 %" | Alpha next to fill/stroke | high, P | — |

- **Font:** the captures look like Adobe Clean, which is proprietary [med, P]. Draw with Segoe UI (system) or the bundled Inter (§4.8). The first draft proposed Source Sans 3; it is an Adobe font and is ruled out. Sizes [med, P]:
  - 12 px for labels and values;
  - 12–13 px semibold for section titles;
  - 13 px for the Tool/Object/Frame/Doc tabs.
- **Numeric fields ("scrubby numbers"):**
  - From 2020 on, the value is white text over a thin grey underline. It is **not** blue [high, P].
  - Help pages written before 2020 (S7, S48) still describe blue hot text, for example next to Ease. That is the old look; draw it only for a pre-2020 skin [high].
  - Drag horizontally on the number to scrub it; the cursor becomes a pointing hand with a left/right arrow. Click to type, Enter commits, Tab moves to the next field [med, R].
  - Values show up to 2 decimals (for example 212.95). Units (%, °, pt, px) appear as a suffix inside the field [high, P].
- **Draggable sections:**
  - Hovering a Properties section shows a "gripper", and dragging it reorders the sections [high, S2].
  - The gripper is a short horizontal bar, about 30×3 px, #5A5A5A, centred on the section's top edge [med, P].
- **Windows menus are light, not dark.** This covers menubar dropdowns, context menus and panel menus [high for 2019–2022 builds, P on S2/S12/S15; med for 24.0]:
  - Background #F2F2F2, hover row #91C9F7, text #000000, disabled text #6D6D6D. Separators are 1 px, about #D7D7D7, and start after the left gutter.
  - About 28 px left gutter for check marks. Shortcut text is right-aligned with a gap of 24 px or more. A small "›" at the far right marks a submenu.
  - Item height about 22 px at 100%. Font: Segoe UI 9 pt (12 px), which ships with Windows and can't be bundled, so load it from the system at run time; Inter is the fallback.
  - The menubar strip itself is dark (#2F2F2F), with menu titles about #C0C0C0.

### 7.2 Properties panel

#### 7.2.1 Panel chrome
- **Placement:**
  - Essentials puts a "Properties | Library" tab group in the right dock, with Properties active [high, P S2].
  - Adobe's Assets help places the Assets panel in the upper-right tab group together with Properties in most workspaces, so the group is **Properties | Library | Assets** [med, S24; high per §4.1].
  - Default width is roughly 300 px [low].
- **Tab strip** [med, P]: about 24 px on 2021–22 screenshots (22 in 2024, §3.1), background #2F2F2F.
  - Active panel tab: bold, #F4F4F4, on panel.bg (2024: tab_text_active, semibold). Inactive tab: #737373 (2024: #818181).
  - Hamburger at the far right: 3 horizontal lines, 12 px wide (2024: four 10 px lines, §4.6).
- **Panel menu:** Lock │ Help │ Close, Close Group [Lock high, S2; the rest med, P]. Lock stops an accidental drag from moving the panel [high, S2].
- **Category tab row (about 40 px):**
  - **Tool | Object | Frame | Doc** in 4 equal columns with centred labels [high, S2].
  - The selected tab gets the #4A4A4A pill: column width minus 8 px, about 28 px tall [med, P].
  - Enabled tab text #E8E8E8; disabled #6D6D6D. A 2 px divider sits below the row [med, P].
- **When each tab is enabled** [high, S2]:
  - **Tool:** only when the current tool has properties. Otherwise it is greyed and Doc is shown.
  - **Object:** only when something is selected on the stage.
  - **Frame:** only when a frame or span is selected in the timeline.
  - **Doc:** always.
  - Tools with no Tool tab: Selection, Subselection and Hand [med, R]; probably also Free Transform, Zoom and Rotation [low]. Their old toolbox options moved: Smooth/Straighten to Object > Shape Options (they also stay in the toolbar's options area, §6.8), Snap to Objects to Doc.
  - Auto-switching [med, R]: selecting a stage object shows Object; clicking a frame shows Frame; picking a drawing tool shows Tool; clicking empty stage or pasteboard shows Doc. The user can click any enabled tab.
  - New file, Selection tool, nothing selected: only Doc is enabled [high, P S2].
  - When objects of different types are selected, the panel shows how many objects are selected [high, S2; exact wording unverified].
- **Body, top to bottom** [med, P]:
  - Header row: 48 px, or 56 px for the two-line Doc header.
  - Quick-action row: 44 px. Buttons sit in left-aligned groups split by a 1×24 px #4A4A4A line (group_divider).
  - Collapsible sections.
  - The remaining space is empty panel.bg. The panel scrolls vertically when needed.
- **Section header (about 32 px)** [med, P]:
  - Chevron at x ≈ 16: an 8 px "v" when open, ">" when closed. Title at x ≈ 36. No header fill.
  - Optional icons at the right (Filters: "+" and gear). Clicking the header toggles the section.
  - Content is padded 16 px left and right, with rows about 28 px tall.
  - Short label columns (W/H/X/Y) are about 24 px wide; word labels (Profile, Ease, Name, Type, Sync) are about 64 px.

#### 7.2.2 Doc tab (in order) [section order med, P]
1. **Header** [high, P]
   - Icon, 20 px. Animate draws an outlined square with "An" lettering. **minnamate must draw its own document glyph** instead, for example a page with a folded corner [clean-room].
   - Line 1: file name (for example "Untitled-1"), about #9A9A9A.
   - Line 2: document type, about #8A8A8A, for example "HTML5 Canvas Document". The 20.0 build just said "Document" in italics.
2. **Quick actions: 4 toggle buttons.** Buttons 1 and 2 are on in the captures [med, P].
   1. **Snap to Objects:** a magnet, drawn as an upside-down U whose two leg tips are lighter. It has lived in the Doc tab since 20.0 [high, C6].
   2. **Snap Align:** a vertical line, a short arrow pointing at it from the right, and a hook curve below [glyph med, P; tooltip low].
   3. **Rulers:** an L-shaped ruler with tick dots along both arms [glyph med, P; tooltip low].
   4. **Lock Guides:** a padlock, drawn as a rectangle body with an arc shackle [glyph med, P; tooltip low].
3. **Publish Settings**, expanded [high, P]
   - Profile [Default ▾].
   - ActionScript 3.0 documents add a **Class** text field plus a pencil button. It holds the document class name, without ".as" [high, S36].
   - A Target row (Flash Player or AIR version) [low for 2024].
   - A full-width **More settings** button, which opens the File > Publish Settings dialog [high, P].
   - 2017–2019 builds showed a "Publish Settings..." button plus Target, Script and Class rows [med, R].
4. **Document Settings**, expanded, in two columns [high, P S2, S11]
   - **Left column:**
     - **W** [550] above **H** [400], joined by a thin bracket with a link toggle in the middle. Adobe calls it "Link", and it is unlinked by default [high, S11]. The draft drew it as a padlock [P].
     - **Stage** [white swatch].
     - **FPS** [24].
   - **Right column:**
     - **Match contents** button [high, S2].
     - Checkboxes: **Scale Content** [high, S11], **Apply to pasteboard** [med, P], **Scale Spans** [med, S12]. Scale Spans keeps timing when FPS changes; S12 calls it "Scale Frame Spans".
   - A full-width **More settings** below, which opens Modify > Document (Ctrl+J). Older builds labelled it "Advanced Settings" [med, S39].
   - Units appear only as a "px" suffix; they are changed in the dialog [med, R].
   - Checking Scale Content links W/H and disables the link toggle [high, S11].
   - For HTML5 Canvas, the stage colour accepts alpha or "No Color" (a transparent canvas) [high, S11, S39].
   - Defaults: 550×400 px, the "Match Default" size [high, S14]; 24 fps [high, S37]; stage #FFFFFF [med].
5. **SWF History**, collapsed by default: recent published SWF sizes, with **Log** and **Clear** buttons [med, R]. Probably hidden for HTML5 Canvas [low].
6. **Accessibility**, collapsed, with an on/off pill toggle at the right of the header [low, P]. The pill is about 28×14 with a white knob, on by default. Contents [low]: Make movie accessible, Make child objects accessible, Auto label, Name, Description.

**More settings → Document Settings dialog (Modify > Document, Ctrl+J)** [med, S11, S13, S14]:
- Units [Pixels ▾]: Inches, Inches (decimal), Points, Centimeters, Millimeters, Pixels [med].
- Stage size W × H with a Link button, plus **Match Contents** [high, S11, S14].
- Scale: [ ] Scale Content, with a 3×3 **Anchor** grid (centre selected). With Scale Content off, the stage grows away from the chosen anchor [high, S11].
- Stage Color swatch and Frame Rate [high].
- [x] Use Advanced Layers, on by default [high, S13].
- Buttons: **Make Default**, Cancel, OK [high, S13].

#### 7.2.3 Tool tab
- **Header:** the tool icon plus the tool name, for example "Classic Brush", "PolyStar" or "Rectangle" [high, P].
- **Color and Style** (renamed from "Fill and Stroke") [high, S2]:
  - **Fill** row: swatch, "Fill", then at the right the checkerboard alpha glyph and [100 %]. **Stroke** row: same layout. Alpha sits up front [high, S2].
  - Only the Fill and/or Stroke rows that fit the tool or selection are shown [high, S2].
  - **Stroke size:** label at the left, value at the right of the same line, then a full-width slider below. Default 1. The slider is a round 14 px hollow knob on a 2 px track [med, P].
  - **Style:** a dropdown showing a line preview, plus "⋯" for rarer options. **Width:** a width-profile dropdown, plus "⋯" [high, S2].
  - **Scale** [Normal ▾] (Normal / Horizontal / Vertical / None) and [ ] **Hinting**. S29 calls the checkbox "Stroke Hinting" [med, P, S29].
  - **Cap:** a 3-button segmented control: None, Round, Square. Round is selected [items high, S29; default med].
  - **Join:** Miter, Bevel, Round. Round is selected [items high, S29; order and default med, P].
  - **Miter:** [3] [item high, S29; default med].
  - Default colours: stroke #000000, fill #0066CC [low, P Color panel]. The toolbar's Black-and-White button resets to a black stroke and a white fill [high, S29].
- **Rectangle / Rectangle Primitive** [med, P, S26]:
  - Quick action **Object Drawing**: a square outline with a filled circle inside [high, P].
  - Color and Style, as above.
  - **Rectangle Options:**
    - Four corner-radius fields in a 2×2 grid, each with its corner glyph (⌜ ⌝ ⌞ ⌟).
    - A lock/constrain toggle, on by default. While locked, all four corners share one value.
    - A radius slider and a **Reset** button [high, S26].
    - Default radius 0. Negative values give inverse (scooped) corners; this is documented for the Primitive [high, S26].
    - Up/Down arrow keys while dragging change the radius [high, S26].
- **Oval / Oval Primitive:** **Oval Options** has Start angle, End angle, Inner radius (0–99, the percentage of fill removed), [x] Close path (on by default) and **Reset** [high, S26].
- **PolyStar** [high, P S2, S26]:
  - Quick action: Object Drawing.
  - Color and Style.
  - **Tool Options**, with labels above the fields: "Style:" [polygon ▾] (polygon / star); "Number of Sides:" [5], range 3–32; "Star point size:" [0.50], range 0–1, which only affects stars.
- **Classic Brush (B)** [high, S25 unless tagged]:
  - Quick actions [P]:
    1. Object Drawing.
    2. **Brush Mode:** a swirl/arrows glyph with a tiny corner triangle, meaning it opens a popup. Modes: Paint Normal, Paint Fills Only (older builds: "Paint Fills"), Paint Behind, Paint Selection, Paint Inside [high, S26, S38].
    3. **Lock Fill:** a gradient square with a small padlock [med, P].
  - Color and Style: Fill only.
  - **Classic Brush Options:**
    - Brush-shape dropdown: a filled circle with a corner triangle.
    - At its right: "+" creates a custom brush through the Nib Options dialog (shape, angle, flatness); a pencil edits it and a trash icon deletes it. Both work only on custom brushes.
    - **Size** slider, 1–200 px, with the value at the right.
    - **Smoothing** slider, 0–100 [default low: the draft read 100, and the Pencil defaults to 50 per S29].
    - [x] Zoom size with stage, [ ] Sync settings with Eraser.
  - The draft's "Save as preset / Remove preset" buttons appear nowhere in Adobe's brush docs, so they are left out [low].
- **Text tool** [high, P S2, S27]:
  - **Header:** a "T" glyph; a type dropdown [Static Text ▾ / Dynamic Text / Input Text]; an orientation button, for static text only: horizontal, vertical, vertical left-to-right.
  - **Character:**
    - Font-family dropdown, plus a globe button for web fonts (HTML5 Canvas only, otherwise disabled) [med, P].
    - Style dropdown [Regular ▾]. Faux Bold and Faux Italic live under Text > Style.
    - A full-width **Embed** button, which opens Font Embedding.
    - Size [12 pt]. Size is in points whatever the ruler units [high, S27].
    - Letter spacing [0], shown with a "VA" over a double arrow, and [x] Auto kern.
    - Fill swatch with [100 %].
    - Render [Anti-alias for readability ▾]. Options: Use device fonts, Bitmap text [no anti-alias], Anti-alias for animation, Anti-alias for readability, Custom anti-alias [med, R].
    - Icon buttons: Toggle Subscript and Toggle Superscript; then, at the right, Selectable (a T with an I-beam), Render text as HTML ("<>") and Show border around text (a lined box) [high, S27].
  - **Paragraph:**
    - A 4-button align control: Left, Center, Right, Justify.
    - Indent [0 px] and Line spacing [2 pt] on one row; Left margin [0 px] and Right margin [0 px] on the next.
    - Behavior: Single line / Multiline / Multiline no wrap, plus Password for Input text. New Dynamic and Input fields default to Single line [med, R]. It is shown disabled for static text.
- **Camera** [high, S2, S28]:
  - **Camera Settings** (title from P): X [0] and Y [0], each with a reset glyph (a curved back-arrow over a short bar); Zoom [100 %] with reset; Rotate [0 °] with reset. Labels sit above the values.
  - Then **Color Effects** and **Filters** sections. The camera gained these in the 2020–2022 updates; earlier builds had only Tint and Adjust Color checkboxes [high, S2, S28].
- **Other tools** follow the same pattern: header, quick-action mode toggles, Color and Style, then "<Tool> Options" [med, R]. This covers Pencil, Pen, Line, Paint Bucket ("Gap size"), Eraser, Width, Bone, Asset Warp, and Lasso / Polygon / Magic Wand. Magic Wand's options are Threshold (1–200) and Smoothing [med, S33].

#### 7.2.4 Object tab
- **Shape / Drawing Object / Group / Bitmap header** [high, P]:
  - Icon (a circle overlapping a square for shapes) plus a type name such as "Shape" or "Drawing Object".
  - Quick actions for shapes and drawing objects [glyphs med, P; tooltips low]:
    1. a vertical bar, an arrow, then a thick bar;
    2. a square containing 3 small squares (Group);
    3. two separate small boxed squares (Break Apart; disabled for raw shapes);
    4. after a separator: an arrow into a club-like glyph made of three circles and a stem (Convert to Symbol);
    5. brushes in a cup with a "+" badge (create brush).
  - Sections: Position and Size, then Color and Style, then **Shape Options** with two buttons, **Straighten** (an angled-line glyph) and **Smoothen** (an S-curve glyph) [high, P].
- **Symbol instance header** [high, P S2]:
  - Left: the type icon, drawn as our own art [glyphs low]: Movie clip = a filled square with a jagged, bitten top-right corner; Graphic = a triangle, circle and square cluster; Button = a rounded rectangle with a pointer.
  - Right, row 1: **behavior dropdown** [Movie Clip ▾] with Movie Clip / Button / Graphic [items high, S4; order med].
  - Right, row 2: **Instance name** box, placeholder "Instance Name" [high, S4]. Hidden for Graphic instances, which can't be named [med].
  - Next row: "Instance of:", the symbol name in dim text, then icon buttons:
    - a sliders glyph, likely symbol properties [low];
    - **Swap** (two opposing horizontal arrows), which opens the Swap Symbol dialog and also works on several selected instances [button high, S4, S31; glyph low];
    - for components only, **Show parameters** (3 stacked boxes), which opens the Component Parameters panel [high, S2].
  - Quick actions: Convert to Symbol (arrow into club glyph) [med, P].
  - Button instances also get a tracking option: Track as Button / Track as Menu Item [med, S4 + R].
- **Position and Size** [high, P]:
  - Left column: W above H, joined by a bracket with a constrain toggle (unlocked by default).
  - Right column: X above Y.
  - X/Y are the registration-point or transformation-point coordinates [high, S4].
  - Pre-2020 builds put X/Y on the first row and W/H on the second.
- **3D Position and View** (ActionScript 3.0 movie clips) [high, S30]: X, Y, Z; W and H, read-only, showing the apparent size; Perspective angle [55], range 1–180°; Vanishing point X/Y, defaulting to the stage centre; **Reset**.
- **Color Effect** [high, P, S4]:
  - A full-width style dropdown: **None, Brightness, Tint, Advanced, Alpha**. In the open list the current item has a blue check. The Frame and Camera tabs title this section "Color Effects".
  - Brightness: a slider, then [0] % (−100…100).
  - Tint: a swatch to the right of the dropdown; a Tint slider with [%] (0…100); Red, Green and Blue sliders with value fields.
  - Alpha: a slider, then [100] % (0…100).
  - Advanced: 4 rows (Alpha, Red, Green, Blue), each laid out as `[100] % × A + [0]` (×R, ×G, ×B for the colour rows). Result = value × % + offset, clamped to 0–255 for RGB and 0–100 for alpha.
- **Display**, for Movie Clip and Button only [high, S4, S10]:
  - [x] Visible. Works on movie clips, buttons and components; needs Flash Player 10.2+ [high, S4].
  - Blending [Normal ▾]. The list, with separators [high, P; the order agrees with S10's example list]: **Normal │ Layer │ Darken, Multiply │ Lighten, Screen │ Overlay, Hard Light │ Add, Subtract, Difference │ Invert, Alpha, Erase**. Alpha and Erase need Layer on the parent [high, S10].
  - Render [Original (No Change) ▾ / Cache as Bitmap / Export as Bitmap]. When Render isn't Original, a second dropdown [Transparent ▾ / Opaque] appears with a swatch [med, R].
  - Graphic instances get no Display section and no Filters section [high, S9, S10].
- **Looping** (Graphic instances) [high, P S5]:
  - Five 28 px icon buttons joined as a segmented row. Mode names and order are from S5:
    1. **Loop:** a box with a curved return arrow.
    2. **Play Once:** a play triangle with a "1" badge.
    3. **Single Frame:** a small frame between two bars.
    4. **Reverse Play Once:** a left-pointing triangle with a "1" badge.
    5. **Reverse Loop:** a mirrored loop arrow.
  - The selected mode gets the blue outline.
  - Below: a **Use Frame Picker** button, which opens the Frame Picker panel [med, S4]. The draft read the label as "Frame picker".
  - Right-aligned "First [1]" and "Last [ ]". Last is unset by default, meaning the symbol's final frame [high, S5].
  - A full-width **Lip Syncing** button [high, S4].
- **Filters** [high, P S2, S9]:
  - Header with "+" (add) and a gear (options) at the right.
  - "+" menu: Drop Shadow, Blur, Glow, Bevel, Gradient Glow, Gradient Bevel, Adjust Color │ Remove All, Enable All, Disable All [high, P, S9].
  - Each filter is a card: a rounded rectangle with a 1 px #474747 border and a slightly darker fill.
    - Card header: chevron, bold filter name, then an **eye** (enable) and a **trash** icon at the right.
    - Drag cards to reorder [high, S2].
    - Alt-click an eye: that filter on, all others off. Ctrl-click an eye: toggle all [high, S9].
  - Gear menu: Copy Selected Filter, Copy All Filters, Paste Filters, Save As Preset..., Edit Presets..., then the preset list. Applying a preset replaces the current filters [high, S9].
  - Filters apply to text, buttons, movie clips, components and compiled clips [high, S9].
  - Card fields [fields high, S9; defaults med]:
    - Drop Shadow: Blur X [4] and Blur Y [4] joined by a lock bracket; Strength [100 %]; Angle [45 °]; Distance [4]; Shadow swatch (#000000); [ ] Knockout; [ ] Inner shadow; [ ] Hide object; Quality [Low ▾] (Low/Medium/High).
    - Glow: Blur X/Y [4]; Strength [100 %]; Color (#FF0000); [ ] Inner glow; [ ] Knockout; Quality [Low].
    - Blur: Blur X/Y [4]; Quality [Low].
    - Bevel, Gradient Glow and Gradient Bevel add Type, Angle, Distance, and colours or a gradient bar with up to 15 stops.
    - Adjust Color: Contrast, Brightness, Saturation, Hue, and **Reset Filter**.
    - The 4 px blur and distance defaults are the draft's screenshot reading; Flash 8–CS6 seeded 5 px. Check on an install.
- **Bitmap:** header "Bitmap", "Instance of: <name>", Swap and Edit buttons, and Position and Size [med, S33].
- **Text field** (selected with the Selection tool): text-type header, instance name (Dynamic/Input only), Position and Size, Character, Paragraph, Options (link URL, target), Filters [med, R].
- **Warp Options** (warped asset): a **Reset Warped Asset** button, new in 24.0 [high, S17]. Other fields (Show mesh, handle mode) [low].
- **Accessibility** (movie clips and buttons in ActionScript 3.0) [low].

#### 7.2.5 Frame tab (in order)
1. **Header** [high, P]
   - Icon: a frame glyph (a rectangle with two vertical edge bars and 2 dots at the bottom), followed by "Frame".
   - At the right, a circled up-right-arrow button, which probably opens the Actions panel [low].
2. **Quick actions** [glyphs high, P; mapping and order med]:
   - two squares with an arrow (Create Classic Tween);
   - two diamonds with an arrow (Create Motion Tween);
   - a circle turning into a square (Create Shape Tween).
3. **Label** [high, P, S6]:
   - Name [input, italic placeholder "Type here"].
   - Type [Name ▾ / Comment / Anchor], disabled until a name is entered.
   - Labels go on keyframes; Enter commits.
4. **Tweening**, shown only when a tween span is selected [high, P S2, S7]
   - **Classic tween:**
     - Ease [All properties together ▾ / Each property separately], with a warning button [P].
     - A **Classic Ease** button opens the preset popup in three columns [med, P]: No Ease, Classic Ease, Ease In, Ease Out, Ease In Out, Custom; then Quad, Cubic, Quart, Quint, Sine, Back, Circ, Bounce, Elastic; then a graph preview. Double-click a preset to apply it [high, S7].
     - Next to it: a pencil (custom ease) and an intensity field [0], range −100…100. Negative values ease in [high, S7].
     - "Each property separately" adds rows for Position, Rotation, Scale, Color and Filters [high, S7].
     - Rotate [Auto ▾] (None / Auto / CW / CCW) with a times field [med]. S7 calls None the default, but that is legacy text. JSFL documents frame.motionTweenRotate as defaulting to "auto" (R), and minnamate's XFL reader already treats a missing attribute as Auto.
     - Checkboxes in 2 columns: [x] Snap, [ ] Orient to path, [ ] Color along path, [ ] Scale along path, [ ] Sync, [x] Scale [items high, S7, S41; defaults med].
     - A full-width **Remove Tween** button [high, S2].
   - **Shape tween:** Ease (with presets) and Blend [Distributive ▾ / Angular] [med, S44 + R].
   - **Motion tween:** Ease, Rotate (count + degrees), Orient to path, Sync graphic symbols [low].
5. **Sound** [high, P S8]:
   - Name [None ▾ / library sounds].
   - Effect [None ▾ / Left channel / Right channel / Fade left to right / Fade right to left / Fade in / Fade out / Custom], plus an edit-envelope button (a speaker with a pencil). The draft's "Fade to right / Fade to left" were wrong [high, S8].
   - Effects are not exported for HTML5 Canvas or WebGL [high, S8].
   - Sync [Event ▾ / Start / Stop / Stream] [high, S8].
   - [Repeat ▾ / Loop] with "x [1]" [high, S8].
   - Info line: "No sound selected", or the sample rate, channels, bits, duration and size [med, P].
6. **Color Effects**, 7. **Display**, 8. **Filters**: frame-level "layer effects" [high, S13].
   - S13 names the blend section "Display"; the draft read its title as "Blend" and also saw a [ ] Hide object checkbox and a render-with-background dropdown with a swatch.
   - These sections need Advanced Layers turned on.
   - They are supported in ActionScript 3.0 documents only.
   - They don't work on motion-tween or camera layers.
   - They can be animated with classic, shape and IK tweens.

### 7.3 Library panel
- **Placement:** second tab of the Properties group in Essentials. Window > Library, Ctrl+L [high, S1, S2].
- **Layout, top to bottom** (heights scaled from 1× screenshots) [layout high, P S2 S23; colours re-themed med]:
  1. **Document row (about 28 px)** [high, P, S3]:
     - A dropdown filling the width, listing the open documents and any opened external libraries.
     - **Pin current library:** a horizontal pushpin glyph.
     - **New library panel:** two overlapping squares.
  2. **Preview pane (about 140 px):** the selected item, centred and fitted, on a white (stage) background [high, S3].
     - Sounds show a two-channel waveform.
     - Multi-frame symbols and sounds get tiny **Stop ■** and **Play ▶** buttons at the top right.
  3. **Info row (about 26 px)** [high, S2, S3]:
     - "N items" at the left ("1 item" when singular), about #A0A0A0 (text_dim).
     - A search box at the right, about 140×20, with a magnifier inside at the left. It filters live by symbol name or linkage name.
  4. **Column header (about 20 px):** **Name, Linkage, Use Count, Date Modified, Type** [columns high, S3; order med, P].
     - Click a header to sort. A small ▲/▼ triangle button to the right of the headers reverses the order [S3].
     - Drag header borders to resize columns. Sorting applies within folders [high, S3].
  5. **List:**
     - Rows about 20 px. Build 20.0.1 fixed missing "compact view" for Library items [med, S38].
     - A 16 px type icon plus the name. The selected row is highlighted [colour low: grey #4A4A4A or a muted blue; the palette uses `selection` #40618E].
     - Folders show a disclosure triangle; double-click one to open or close it [high, S3].
     - Imported animated GIFs land in their own folder, with the bitmaps numbered in sequence [high, S2, S3].
  6. **Bottom toolbar (about 24 px), left to right** [high, P, S3, S31, S33]:
     - **New Symbol:** a page with a folded corner.
     - **New Folder:** a folder.
     - **Properties:** an "i" in a circle.
     - **Delete:** a trash can.
     - The pre-2020 horizontal scrollbar and the Flash-era Wide/Narrow State buttons (still described in S3) are gone [med].
- **Type icons** (16 px, monochrome, original art) [med, R]. Read at #B8B8B8 in 2021–22; in 2024 draw them in `text` (§2.6 item 8).
  - Movie Clip: a square with a jagged, bitten top-right corner.
  - Graphic: an overlapping triangle, circle and square.
  - Button: a rounded rectangle with a small pointing finger.
  - Bitmap: a frame with a mountain triangle and a sun dot.
  - Sound: a speaker with 2 arcs.
  - Video: a film strip (a rectangle with perforation dots top and bottom).
  - Font: a capital "A".
  - Folder: a closed folder, or an open variant when expanded.
  - Component: 3 stacked boxes (2 below, 1 on top).
  - Compiled clip: a box with a gear.
- **Interactions** [high, S3, S4, S31]:
  - Drag an item to the stage to create an instance on the current layer, in the current keyframe or the nearest keyframe to its left.
  - Double-click the icon to edit the symbol; double-click the name to rename it inline.
  - The trash deletes the item and every instance of it.
  - Drag items between folders; a name collision prompts to replace.
  - Shift/Ctrl multi-select. Drag items into another document's library.
  - Resolve Library Conflicts dialog: "Don't Replace Existing Items" / "Replace Existing Items". An item never replaces one of a different type; the new item gets "Copy" appended instead.
  - Dragging stage objects onto the Library to make a symbol is deprecated [med, S3; S31 still lists it].
- **Panel (hamburger) menu** [order med; tagged items as marked]:
  - New Symbol..., New Folder, New Font..., New Video...
  - │ Rename, Delete │ Duplicate..., Move to...
  - │ Edit, Edit with..., Edit Class, Play [Edit and Edit with high, S3]
  - │ Update..., Properties..., Component Definition..., Runtime Shared Library URL... [Update high, S3]
  - │ Select Unused Items [high, S3]
  - │ Expand Folder, Collapse Folder, Expand All Folders, Collapse All Folders [high, S3]
  - │ Lock, Help, Close, Close Group
  - The draft's Flash-8-era items (Shared Library Properties..., Keep Use Counts Updated, Update Use Counts Now) are probably gone [low].
- **Item context menu** adds:
  - **Generate Sprite Sheet...** and **Generate Texture Atlas...** [high, S32];
  - **Export PNG Sequence...** [high, S16];
  - **Export Asset...** and **Save As Asset...**, which send the item to the Assets panel (2020+) [high, S24].

### 7.4 Menus
Menubar order on Windows: File, Edit, View, Insert, Modify, Text, Commands, Control, Debug, Window, Help [high, P]. Drawn as light native-style menus (§7.1).

Notation: "│" is a separator and "▸" a submenu; "·" separates items written on one line. Keys are Windows keys. Items that don't apply to the document type are greyed out rather than removed [low].

- **File** [order med, P S15/S16]:
  - New... Ctrl+N. It opens the start-screen dialog with Character Animation, Social, Game, Education, Ads, Web and Advanced tabs [high, S14].
  - Open... Ctrl+O · Open Recent ▸ · Close Ctrl+W · Close All Ctrl+Alt+W [med]
  - │ Save Ctrl+S · Save As... Ctrl+Shift+S [high, S1] · Save as Template... [med, S14] · Save All [med] · Revert [high, S14]
  - │ Import ▸ [high, S1, S3, S8, S23]: Import to Stage... Ctrl+R · Import to Library... · Open External Library... Ctrl+Shift+O · Import Video...
  - Export ▸: Export Image... [high, S16] · Export Image (Legacy)... [med] · Export Movie... Ctrl+Shift+Alt+S [high, S1] · Export Video/Media... [high, C9, S38] (named "Export Video..." in older builds, S16) · Export Animated GIF... [high, S16]
  - │ Convert To ▸ [menu high, S15; items med]: ActionScript 3.0, HTML5 Canvas, AIR for Desktop, AIR for Android, AIR for iOS, plus any installed custom-platform types. WebGL glTF and VR types existed only in the 2019–2021 builds [low].
  - │ Publish Settings... Ctrl+Shift+F12 · Publish Shift+Alt+F12 [med]
  - │ AIR Settings... (AIR targets) · ActionScript Settings... [med]
  - File Info... (XMP metadata) [low; S35 mentions it]
  - │ Exit Ctrl+Q [med]
  - **Not in 2024; don't add** [med]:
    - Browse in Bridge: Flash CS-era. Its Ctrl+Alt+O was already the onion-skin toggle by 2018 (C8).
    - Sync Settings: gone after 18.0 (S25).
    - Page Setup / Print: removed with Flash Professional CC (S14).
    - Publish Preview: S35 still describes it, but Control > Test Movie > In Browser replaced it [low].
- **Edit** [items high, S1, S6, S7, S13, S20, S21, S31; order med]:
  - Undo Ctrl+Z · Redo Ctrl+Y. Redo reads "Repeat" when nothing can be redone (S20).
  - │ Cut Ctrl+X · Copy Ctrl+C · Paste in Center Ctrl+V · Paste in Place Ctrl+Shift+V · Paste Special... · Clear Backspace
  - │ Duplicate Ctrl+D · Select All Ctrl+A · Deselect All Ctrl+Shift+A · Invert Selection [med]
  - │ Find and Replace Ctrl+F [item high, S54; key med] · Find Next F3 [med]
  - │ Timeline ▸ [keys med]:
    - Remove Frames Shift+F5 · Cut Frames Ctrl+Alt+X · Copy Frames Ctrl+Alt+C · Paste Frames Ctrl+Alt+V · Paste and Overwrite Frames · Clear Frames Alt+Backspace · Select All Frames Ctrl+Alt+A
    - │ Cut Layers · Copy Layers · Paste Layers · Duplicate Layers
    - │ Copy Motion · Copy Motion as ActionScript 3.0... · Paste Motion · Paste Motion Special...
  - │ Edit Symbols Ctrl+E (reads "Edit Document" inside a symbol) · Edit Selected · Edit in Place · Edit All
  - │ Preferences ▸: Edit Preferences... Ctrl+U │ Export Preferences... · Import Preferences... [high, S17, C4; key med]. S16 also mentions switching between expert and novice preferences, so a novice/expert pair may live here too [low; item names unverified].
  - Font Mapping... [med] · Keyboard Shortcuts... [high, S42]
  - Script documents add Comment Selection Ctrl+M and Uncomment Selection Ctrl+Shift+M [high, S1, S49].
- **View** [items high, S1, S11; keys med unless tagged]:
  - Go To ▸: First Home · Previous Page Up · Next Page Down · Last End │ scene names [item high, S40]
  - │ Zoom In Ctrl+= · Zoom Out Ctrl+- [high, S1]. The zoom range is 8%–2000% [high, S11].
  - Magnification ▸:
    - Fit in Window [high, S11]
    - Clip to Stage [med, C1b; key low]. Ctrl+Shift+W belonged to the removed Pasteboard command, and C1a users had to reassign it in 18.0.
    - │ 25%, 50%, 100% Ctrl+1, 200%, 400% Ctrl+4, 800% Ctrl+8 [Ctrl+1 high, S1]
    - │ Show Frame Ctrl+2 · Show All Ctrl+3 [high, S1]
  - View > Pasteboard was removed on purpose in 18.0 [high, C1a].
  - │ Preview Mode ▸ [items high, S35]: Outlines Ctrl+Alt+Shift+O · Fast Ctrl+Alt+Shift+F · Anti-Alias Ctrl+Alt+Shift+A · Anti-Alias Text Ctrl+Alt+Shift+T · Full
  - │ Rulers Ctrl+Alt+Shift+R [high, S1]
  - Grid ▸: Show Grid Ctrl+' [high, S11] · Edit Grid... Ctrl+Alt+G
  - Guides ▸ [items high, S11]: Show Guides Ctrl+; · Lock Guides Ctrl+Alt+; · Edit Guides... Ctrl+Alt+Shift+G · Clear Guides
  - Snapping ▸ [items high, S11, S50]: Snap Align · Snap to Grid Ctrl+Shift+' · Snap to Guides Ctrl+Shift+; · Snap to Pixels · Snap to Objects **Ctrl+Shift+U** │ Edit Snapping... Ctrl+/. This part's draft gave Snap to Objects Ctrl+Shift+/ [med]; §6.8 cites Ctrl+Shift+U from the April 2024 shortcut export [high], which is used here (§8.1).
  - │ Hide Edges Ctrl+Shift+E [high, S1]
  - Show Shape Hints **Ctrl+Alt+I** [med, S1]. That is Adobe's own table; Flash CS-era builds used Ctrl+Alt+H.
  - Show Tab Order [med]
  - │ Screen Mode ▸: Standard Screen Mode · Full Screen Mode [high, C5]
- **Insert** [high, S6, S7, S13, S31]:
  - New Symbol... Ctrl+F8 [key med]
  - │ Motion Tween · Shape Tween · Classic Tween
  - │ Timeline ▸: Layer · Layer Folder │ Frame F5 · Keyframe F6 · Blank Keyframe F7
  - Scene
- **Modify** [items high from help-page menu paths; order and keys med]:
  - Document... Ctrl+J
  - │ Convert to Symbol... F8 · Convert to Bitmap · Break Apart Ctrl+B [high, S1]
  - │ Bitmap ▸: Swap Bitmap... · Trace Bitmap... [high, S33]
  - Symbol ▸: Swap Symbol... · Duplicate Symbol... [high, S31]
  - Shape ▸ [items high, S43, S44]: Smooth · Straighten · Advanced Smooth... Ctrl+Alt+Shift+M · Advanced Straighten... Ctrl+Alt+Shift+N · Optimize... Ctrl+Alt+Shift+C │ Convert Lines to Fills · Expand Fill... · Soften Fill Edges... │ Add Shape Hint Ctrl+Shift+H · Remove All Hints
  - Combine Objects ▸: Delete Envelope │ Union · Intersect · Punch · Crop
  - Timeline ▸ [items high, S7, S37]:
    - Distribute to Layers Ctrl+Shift+D · Distribute to Keyframes · Layer Properties... · Reverse Frames · Synchronize Symbols
    - │ Convert to Keyframes · Clear Keyframe Shift+F6 · Convert to Blank Keyframes
  - Convert to Frame-by-Frame Animation ▸: Keyframe every frame · Keyframe every other frame · Keyframe every third frame · Keyframe every fourth frame · Custom... [submenu high, P S12; placement low]. S12 only places it in the Modify menu, so whether it sits at the top level or under Timeline is unverified.
  - │ Transform ▸: Free Transform · Distort · Envelope · Scale · Rotate and Skew · Scale and Rotate... Ctrl+Alt+S [high, S1] │ Rotate 90° CW Ctrl+Shift+9 · Rotate 90° CCW Ctrl+Shift+7 [high, S1] │ Flip Vertical · Flip Horizontal │ Remove Transform Ctrl+Shift+Z [high, S1]
  - Arrange ▸: Bring to Front Ctrl+Shift+↑ · Bring Forward Ctrl+↑ · Send Backward Ctrl+↓ · Send to Back Ctrl+Shift+↓ [high, S1] │ Lock Ctrl+Alt+L · Unlock All Ctrl+Alt+Shift+L [items high, S47; keys med]
  - Align ▸ [med]: Left Ctrl+Alt+1 · Horizontal Center Ctrl+Alt+2 · Right Ctrl+Alt+3 · Top Ctrl+Alt+4 · Vertical Center Ctrl+Alt+5 · Bottom Ctrl+Alt+6 │ Distribute Widths Evenly Ctrl+Alt+7 · Distribute Heights Evenly Ctrl+Alt+9 │ Make Same Width Ctrl+Alt+Shift+7 · Make Same Height Ctrl+Alt+Shift+9 │ Align to Stage Ctrl+Alt+8
  - │ Group Ctrl+G · Ungroup Ctrl+Shift+G [high, S1]
- **Text** [med]:
  - Font ▸ [low] · Size ▸
  - Style ▸: Plain Ctrl+Shift+P · Bold Ctrl+Shift+B · Italic Ctrl+Shift+I · Faux Bold · Faux Italic [Faux high, S27] · Subscript · Superscript
  - Align ▸: Align Left Ctrl+Shift+L · Align Center Ctrl+Shift+C · Align Right Ctrl+Shift+R · Justify Ctrl+Shift+J
  - Letter Spacing ▸: Increase Ctrl+Alt+→ · Decrease Ctrl+Alt+← [high, S1] · Reset Ctrl+Alt+↑
  - Scrollable [high, S27]
  - │ Check Spelling... · Spelling Setup...
  - │ Font Embedding... (the same dialog as the Embed button) [med, S27]
- **Commands** [first 3 high, S19; the rest med]:
  - Manage Saved Commands... · Get More Commands... · Run Command... Get More Commands links to Adobe's add-ons site; minnamate should drop it or point it somewhere else.
  - │ Copy Font Names for ActionScript [low] · Copy Motion as XML [high, S7] · Export Motion XML · Import Motion XML. Help text words the last two as Export/Import Motion as XML (S7).
  - "Convert AS3 to HTML5 Canvas document" (S39) predates File > Convert To and is probably gone [low].
  - Commands the user saves (History panel menu > Save As Command) are listed here [high, S19].
- **Control** [top part high, P S12; the rest med]:
  - Play Enter · Rewind Shift+, · Go To End Shift+. [keys high, C2]
  - │ Step Forward One Frame . · Step Backward One Frame , [high, C7] · Step Forward To Next Keyframe Alt+. · Step Backward To Previous Keyframe Alt+, [high, S12]
  - │ Test Ctrl+Enter [high, S4] · Test Movie ▸: In Animate, In Browser; AIR documents add AIR Debug Launcher (Desktop/Mobile) and On Device via USB [med] · Test Scene Ctrl+Alt+Enter
  - │ Clear Publish Cache · Clear Publish Cache and Test Movie
  - │ Loop Playback · Play All Scenes [high, C1b]
  - │ Enable Simple Frame Actions Ctrl+Alt+F · Enable Simple Buttons Ctrl+Alt+B [item high, S45] · Enable Live Preview [high, S31] · Mute Sounds Ctrl+Alt+M
  - An Adobe staffer cited a "Control > Timeline > Modify Markers" submenu (onion-skin marker range) in 20.0 [low, C2].
- **Debug** [items med, S22; keys med]:
  - Debug Ctrl+Shift+Enter · Debug Movie ▸: Animate, AIR Debug Launcher (Desktop), AIR Debug Launcher (Mobile), on Device via USB [high, S22]
  - │ Continue Alt+F5 · End Debug Session Alt+F12
  - │ Step In Alt+F6 · Step Over Alt+F7 · Step Out Alt+F8
  - │ Remove All Breakpoints Ctrl+Shift+B
  - │ Begin Remote Debug Session ▸: ActionScript 3.0 [high, S22]
- **Window** [shortcuts high, S1; items as tagged; grouping and order low]:
  - Duplicate Window Ctrl+Alt+K [high, S1]
  - │ Toolbars ▸: Edit Bar; old docs also list Controller (S34) [low]
  - │ Timeline Ctrl+Alt+T · Tools Ctrl+F2 · Properties Ctrl+F3 · Library Ctrl+L
  - Assets [high, S24] · Brush Library [high, S26] · CC Libraries [high, S46] · Motion Presets [med] · Frame Picker [high, S4] · Layer Depth [high, S13]
  - │ Actions F9 [high, S2] · Code Snippets [high, S53] · Compiler Errors Alt+F2 · Debug Panels ▸ [med] · Output F2 [high, S51]
  - │ Align Ctrl+K · Color Ctrl+Shift+F9 · Info Ctrl+I · Swatches Ctrl+F9 · Transform Ctrl+T
  - │ Components Ctrl+F7 [key med] · Component Parameters [high, S2]
  - │ History Ctrl+F10 · Scene Shift+F2. Older docs put Scene and Strings under an "Other Panels" submenu (S40); whether that submenu survives in 24.0 is unverified [low].
  - Movie Explorer is deprecated; leave it out [med, S2].
  - │ Extensions ▸ [med]
  - │ Workspaces ▸: Animator · Basic · Classic · Debug · Designer · Developer · Essentials · Small Screen (plus user workspaces) │ Manage Workspaces... · Reset 'Essentials'. Resolved with §4.1: Basic is included [high, WS S4, S5] and there is no New Workspace item in 2020+ [med, WS S13]; this part's draft listed New Workspace... and left out Basic [med]. "Manage Workspaces..." replaces the draft's "Delete Workspace..."; that is the convention across Adobe's CS family (S52) [med].
  - Hide Panels F4 [high, S52]
  - │ list of open documents, with a check mark on the active one [med]
- **Help** [low except F1]:
  - <App> Help F1 [F1 high, S51] · Support Center
  - │ Updates...
  - │ Sign In / Sign Out
  - │ About <App>
  - In minnamate, use "minnamate" for the product name and drop the Adobe-account items.

### 7.5 Core Windows shortcuts
- **Tools** [high · TO S3 (April 2024 shortcut export), §6.3–§6.4]:

  | Tool | Key | Tool | Key | Tool | Key |
  |---|---|---|---|---|---|
  | Selection | V | Subselection | A | Free Transform | Q |
  | Gradient Transform | F | 3D Rotation | Shift+W | 3D Translation | G |
  | Lasso | L | Polygon | Shift+L | Magic Wand | none |
  | Pen | P | Add Anchor Point | = | Delete Anchor Point | - |
  | Convert Anchor Point | Shift+C | Text | T | Line | N |
  | Rectangle | R | Rectangle Primitive | Shift+R | Oval | O |
  | Oval Primitive | Shift+O | PolyStar | none | Pencil | Shift+Y |
  | Paint Brush | Y | Classic Brush | B | Fluid Brush | Shift+B |
  | Bone | M | Bind | Shift+M | Paint Bucket | K |
  | Ink Bottle | S | Eyedropper | I | Eraser | E |
  | Width | U | Asset Warp | W | Camera | C |
  | Hand | H | Rotation | Shift+H | Time Scrub | Alt+Shift+H |
  | Zoom | Z | | | | |

  - This part's draft (sources S1b chart, S11, S25, S26, S30, S41, R) tagged 3D Rotation, Convert Anchor, Pencil, Paint Brush, Asset Warp, Camera and Rotation [med] (S11 prints "(H)" for Rotation), Bone and Time Scrub [low], and grouped Lasso/Polygon/Magic Wand on L, Rectangle/Rectangle Primitive on R and Oval/Oval Primitive on O. §6.3 overrides that with the 2024 table.
  - No two tools share a key and no key cycles through a group [high · TO S3]. The draft said tools in a group share a key that cycles on repeat presses [med, S38]; that is dropped.
  - Hold Space for the Hand tool [high, S11]. Hold Shift+Space to rotate the stage [high, S11]. Space+T gives a temporary Time Scrub [high, S12].
  - Alt-click the stage with the Oval or Rectangle tool to open its size dialog [high, S26].
  - Up/Down arrows while dragging a rectangle change the corner radius [high, S26].
- **Frames** [high, S6, S12, C7; keys med where §7.4 tags them]:
  - F5 insert frame; Shift+F5 remove frames.
  - F6 keyframe; Shift+F6 clear keyframe.
  - F7 blank keyframe.
  - Ctrl+Alt+X / C / V cut / copy / paste frames; Ctrl+Alt+A select all frames; Alt+Backspace clear frames.
  - Alt-drag a keyframe to copy it; Ctrl-drag a span edge to change its length [high, S6].
- **Playback** [high, S12, C2, C7]:
  - Enter play/stop.
  - , and . step one frame back/forward.
  - Shift+, first frame; Shift+. last frame.
  - Alt+, and Alt+. previous/next keyframe.
  - Alt+Shift+, and Alt+Shift+. page the timeline.
  - Onion-skin toggle: Ctrl+Alt+O [med, C8; that thread is from a 2018 build]. The draft's Alt+Shift+O had no source. §5.8 found no confirmed default; make it rebindable.
  - Ctrl-drag an onion marker to resize both sides; Shift-drag to move the whole range [high, S1].
- **Zoom and view** [high, S1, S11]:
  - Ctrl+= / Ctrl+- zoom; Ctrl+1 100%; Ctrl+2 show frame; Ctrl+3 show all; Ctrl+4 400% and Ctrl+8 800% [med].
  - Ctrl+Alt+Shift+R rulers; Ctrl+' grid; Ctrl+Shift+E hide edges.
- **Panels** [high, S1, S2]: Ctrl+F3 Properties; Ctrl+L Library; Ctrl+F2 Tools; Ctrl+Alt+T Timeline; F9 Actions; F2 Output; Alt+F2 Compiler Errors; Ctrl+K Align; Ctrl+Shift+F9 Color; Ctrl+F9 Swatches; Ctrl+I Info; Ctrl+T Transform; Ctrl+F10 History; Shift+F2 Scene; Ctrl+Alt+K Duplicate Window; F4 hide panels; Ctrl+F7 Components [med].
- **Symbols and editing:**
  - F8 Convert to Symbol [high]; Ctrl+F8 New Symbol [med]; Ctrl+E edit symbol / back to document [high, S1].
  - Ctrl+G / Ctrl+Shift+G group / ungroup; Ctrl+B break apart; Ctrl+D duplicate; Ctrl+Shift+V paste in place [high, S1].
  - Ctrl+Alt+S scale and rotate; Ctrl+Shift+Z remove transform; Ctrl+Shift+7 rotate 90° CCW [high, S1]; Ctrl+Shift+9 rotate 90° CW [med].
  - Ctrl+↑ / ↓ forward / backward; add Shift for front / back [high, S1].
  - Arrows nudge 1 px; Shift+arrows nudge 10 px [high, S21].
- **Test and publish:** Ctrl+Enter test [high, S4]; Ctrl+Alt+Enter test scene; Ctrl+Shift+Enter debug; Ctrl+Shift+F12 publish settings; Shift+Alt+F12 publish; Ctrl+J document settings; Ctrl+U preferences [med].
- **Code editor** [high, S1, S49]: Ctrl+M comment; Ctrl+Shift+M uncomment; Ctrl+Shift+F auto format; Ctrl+Shift+8 show hidden characters; Ctrl+Alt+[ frame script navigator.

### 7.6 Version notes (what 2020–2023 did; target 2024)
- **20.0 = Animate 2020, released November 2019.** The draft's "20.0 (2019 release)" label was wrong.
  - Properties panel rebuilt as Tool / Object / Frame / Doc tabs [high, S2].
  - Snap to Objects moved into the Doc tab [high, C6].
  - Build 20.0.1 fixed missing "compact view" for Library items [med, S38].
- **Graphic looping:** 5 modes plus First/Last frame fields [high, S5]. The version that introduced them is unverified [low].
- **Later 2020–2022 updates** [high, S2; exact versions unverified]:
  - Quick-action toolbar, refreshed headers, draggable sections.
  - "Fill and Stroke" renamed "Color and Style"; filter cards.
  - Remove Tween button and one-click Match contents.
  - Camera gained Color Effects and Filters.
  - Modernised PolyStar, Rectangle, Oval, Text and Magic Wand tool tabs.
  - File > Export > "Export Video/Media" existed by 21.x [high, S38].
- **24.0 (Oct 2023)** [high, S17]:
  - Color Theme preference. Windows: Edit > Preferences > Edit Preferences > Interface. macOS: Animate > Settings > Edit Preferences > Interface. The draft's "Set Preferences..." Mac label has no source and was dropped.
  - Reset Warped Asset button.
  - Otherwise the visible changes look subtle [med, C3].
  - After 24.0 came maintenance mode: 24.0.x builds with fixes only, up to 24.0.15 [high, S38].
- **Still unverified** (check on a real install): Window and Help menu order; Doc-tab section order and quick-action tooltips; shape quick-action tooltips; the Text > Font submenu; filter default blur/distance; the classic-tween Rotate default; keys for Show Shape Hints, Clip to Stage, the onion-skin toggle, Bone and Time Scrub (Bone and Time Scrub are confirmed in §6); whether 24.0 still uses light native menus and the same row heights.

### Sources (panels and menus part)
- S1 https://helpx.adobe.com/animate/desktop/introduction-to-animate/animate-keyboard-shortcuts.html
- S1b: the tool-shortcut chart image embedded in S1 (not re-viewed; the image link is omitted under the clean-room rule)
- S2 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/authoring-panels.html
- S3 https://helpx.adobe.com/animate/desktop/multimedia-and-video/library.html
- S4 https://helpx.adobe.com/animate/desktop/multimedia-and-video/symbol-instances.html
- S5 https://helpx.adobe.com/animate/desktop/using/elements.html
- S6 https://helpx.adobe.com/animate/desktop/animation/frames-keyframes.html
- S7 https://helpx.adobe.com/animate/desktop/animation/classic-tween-animation.html
- S8 https://helpx.adobe.com/animate/desktop/multimedia-and-video/using-sounds.html
- S9 https://helpx.adobe.com/animate/desktop/multimedia-and-video/graphic-filters.html
- S10 https://helpx.adobe.com/animate/desktop/multimedia-and-video/applying-blend-modes.html
- S11 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/using-stage-tools-panel.html
- S12 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/timeline.html
- S13 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/timeline-layers.html
- S14 https://helpx.adobe.com/animate/desktop/introduction-to-animate/documents.html
- S15 https://helpx.adobe.com/animate/desktop/interactivity/universal-document-type-converter.html
- S16 https://helpx.adobe.com/animate/desktop/exporting-and-publishing/exporting.html
- S17 https://helpx.adobe.com/animate/using/whats-new/2024.html
- S18 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/set-preferences.html (legacy text: "Dark or Light" plus Enable Shading)
- S19 https://helpx.adobe.com/animate/desktop/multimedia-and-video/automating-tasks-commands-menu.html
- S20 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/undo-redo-history-panel.html
- S21 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/moving-copying-objects.html
- S22 https://helpx.adobe.com/animate/desktop/platforms/debugging-actionscript-3-0.html
- S23 https://helpx.adobe.com/animate/desktop/multimedia-and-video/add-video.html
- S24 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/work-with-assets-panel-animate.html
- S25 https://helpx.adobe.com/animate/desktop/animation/custom-brushes.html
- S26 https://helpx.adobe.com/animate/desktop/multimedia-and-video/draw-simple-lines-shapes.html
- S27 https://helpx.adobe.com/animate/desktop/multimedia-and-video/classic-text.html
- S28 https://helpx.adobe.com/animate/desktop/multimedia-and-video/working-with-camera-in-animate.html
- S29 https://helpx.adobe.com/animate/desktop/multimedia-and-video/strokes-fills-gradients.html
- S30 https://helpx.adobe.com/animate/desktop/multimedia-and-video/3d-graphics.html
- S31 https://helpx.adobe.com/animate/desktop/multimedia-and-video/symbols.html
- S32 https://helpx.adobe.com/animate/desktop/exporting-and-publishing/create-sprite-sheet.html
- S33 https://helpx.adobe.com/animate/desktop/multimedia-and-video/imported-bitmaps.html
- S34 https://helpx.adobe.com/animate/desktop/animation/frame-by-frame-animation.html
- S35 https://helpx.adobe.com/animate/desktop/exporting-and-publishing/publish-settings.html
- S36 https://helpx.adobe.com/animate/desktop/platforms/actionscript-publish-settings.html
- S37 https://helpx.adobe.com/animate/desktop/animation/animation-basics.html
- S38 https://helpx.adobe.com/animate/desktop/troubleshooting/fixed-issues.html
- S39 https://helpx.adobe.com/animate/desktop/interactivity/creating-publishing-html5-canvas-document.html
- S40 https://helpx.adobe.com/animate/desktop/animation/scenes.html
- S41 https://helpx.adobe.com/animate/desktop/animation/animation-guide.html
- S42 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/keyboard-shortcuts.html
- S43 https://helpx.adobe.com/animate/desktop/multimedia-and-video/reshape-lines-shapes.html
- S44 https://helpx.adobe.com/animate/desktop/animation/shape-tweening.html
- S45 https://helpx.adobe.com/animate/desktop/interactivity/creating-buttons.html
- S46 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/creative-cloud-libraries-stock.html
- S47 https://helpx.adobe.com/animate/desktop/multimedia-and-video/selecting-objects.html
- S48 https://helpx.adobe.com/animate/desktop/animation/adding-custom-eases.html
- S49 https://helpx.adobe.com/animate/desktop/platforms/writing-scripts.html
- S50 https://helpx.adobe.com/animate/desktop/animation/bone-tool-animation.html
- S51 https://helpx.adobe.com/animate/desktop/platforms/actionscript.html
- S52 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/workflow-workspace.html
- S53 https://helpx.adobe.com/animate/desktop/interactivity/add-interactivity-code-snippets.html
- S54 https://helpx.adobe.com/animate/desktop/workspace-and-workflow/find-replace.html
- C1a https://community.adobe.com/t5/animate-discussions/pasteboard/m-p/9505286
- C1b https://community.adobe.com/t5/animate/animate-cc-2018-clip-to-stage-shift-commd-w-is-broken-up-ditto-for-the-camera/m-p/9842288
- C2 https://community.adobe.com/t5/animate-discussions/why-in-newest-animate-version-select-frames-and-go-to-last-frames-it-s-not-possible-anymore/m-p/10719927
- C3 https://community.adobe.com/t5/animate-discussions/the-new-user-interface-in-animate-version-24-0/td-p/14242848
- C4 https://community.adobe.com/t5/animate-discussions/unable-to-export-preferences/m-p/15459505
- C5 https://community.adobe.com/questions-540/full-screen-mode-not-working-after-update-103314
- C6 https://community.adobe.com/questions-540/thoughts-on-animate-20-anyone-else-finding-the-new-place-for-snap-to-objects-magnet-annoying-105762
- C7 https://community.adobe.com/t5/animate-discussions/select-timeline-hotkey-or-next-prev-frame-hotkeys/m-p/9867224
- C8 https://community.adobe.com/t5/animate-discussions/onion-skin-button/m-p/10121038
- C9 https://community.adobe.com/t5/animate-discussions/adobe-animate-not-exporting-as-mp4/m-p/13708574
- C10 https://community.adobe.com/t5/animate-discussions/adobe-animate-2024-is-now-live/td-p/14149223
- L1 https://www.linkedin.com/learning/adobe-animate-cc-new-features-2/properties-panel-modernization
- R: recall of long-standing Flash/Animate behaviour where no public page lists it.

**What this part's fact-checker changed** (kept for traceability)
- §7.0: C3 only says the 24.0 theme list is still Darkest/Dark/Light, so the unsupported "mostly the start screen" claim was removed; added the macOS theme path (S17) and dropped the unsourced "Set Preferences..." label; heights from 2021–22 screenshots marked as upper bounds; added maintenance mode (S1, S38); P readings cross-checked against Spectrum because no Adobe images were downloaded; added clean-room branding rules.
- §7.1: menubar colour unified to #2F2F2F; light Windows menus and white tooltips re-tagged as 2019–22 evidence; Segoe UI loaded from the system; pre-2020 blue hot text noted as the legacy look.
- §7.2.1: the Essentials group probably includes Assets (S24); mixed selections show a count (S2); Free Transform, Zoom and Rotation in the no-Tool-tab list downgraded to [low].
- §7.2.2: Class field upgraded to [high] (S36), Target row stays [low]; the W/H toggle is called Link and is unlinked by default (S11); "More settings" was "Advanced Settings" in older builds (S39); 24 fps (S37) and 550×400 Match Default (S14) confirmed; Scale Content anchor and transparent canvas added (S11, S39).
- §7.2.3: Classic Brush "Save as preset / Remove preset" removed and replaced with S25's controls; brush mode is "Paint Fills Only" in 2021+ (S26; S38 fixes in 21.0.6/21.0.7); shape-tool ranges and behaviours added from S26; Text Behavior and Render options, orientation and points-based size added (S27); stroke cap/join/miter/hinting confirmed (S29); default fill #0066CC downgraded to [low]; Black-and-White gives black stroke and white fill (S29).
- §7.2.4: Button tracking option (S4); Graphic instances have no Display or Filters (S9, S10); colour-effect and 3D ranges (S4, S30); Swap works on several instances (S31); Bitmap and Text-field notes added; "Use Frame Picker" label (S4); filter defaults downgraded to [med]; Alt/Ctrl-click eye behaviours, presets replacing filters, and Reset Filter added (S9); Warp Options fields other than Reset Warped Asset downgraded to [low].
- §7.2.5: Sound Effect labels corrected to "Fade left to right" / "Fade right to left" (S8) and no sound effects for Canvas or WebGL; Rotate default kept as Auto with S7's None flagged; S41's checkbox labels; ease range and double-click (S7); layer-effect section named Display, needs Advanced Layers, AS3 only, not on motion-tween or camera layers (S13).
- §7.3: Export Asset... upgraded to [high] and Save As Asset... added (S24); Generate Texture Atlas... added (S32); Flash-8-era panel-menu items downgraded to [low]; Wide/Narrow State buttons legacy; compact rows from 20.0.1 (S38); dragging stage objects onto the Library is deprecated (S3).
- §7.4: removed Browse in Bridge; added a "not in 2024" list (Sync Settings, Print, Publish Preview); File Info... [low]; New opens the intent-tabbed start dialog (S14); possible novice/expert Preferences items [low]; Comment/Uncomment Selection (S1, S49); Show Shape Hints Ctrl+Alt+I (S1); Clip to Stage key [low]; 8–2000% zoom range (S11); Convert to Frame-by-Frame Animation placement [low]; legacy Convert AS3 command noted; saved commands come from History (S19); "Manage Workspaces..." replaces "Delete Workspace..."; sources added for several Window items; Movie Explorer deprecated (S2).
- §7.5: onion toggle Ctrl+Alt+O (C8); tools in a group were said to share a key (S38) — since overridden by §6.3; S11 prints "(H)" for Rotation; Alt-click shape dialog and arrow-key radius (S26), onion-marker drags (S1), code-editor keys (S1, S49) added.
- §7.6: "20.0 = Animate 2020, released Nov 2019"; the version that introduced reverse looping is unverified; 24.0 notes updated.

---

## 8. Open questions

### 8.1 Unresolved contradictions between the notes
1. **Light or dark menus in 24.x.** This reference uses light native-style menus, based on Windows screenshots of 2019–22 builds (PM P) and a 2020 context menu (TL S32). WS recalls menus that follow the dark theme. Spectrum 7 could have changed this in 24.0. Check first: it is the largest visual uncertainty left.
2. **Snap to Objects key.** Ctrl+Shift+U (TO S3, the April 2024 shortcut export [high]) or Ctrl+Shift+/ (PM [med], the Flash-era key; Edit Snapping stays Ctrl+/).
3. **Rotation tool key.** Shift+H (TO S3 + S4 [high]) or H (WS S1 / PM S11, where helpx prints "(H)").
4. **Onion-skin toggle key.** Ctrl+Alt+O (PM C8, a 2018 build [med]); TL found no confirmed default, and a 2018 claim of Alt+Shift+O conflicts. Keep it rebindable.
5. **Window > Workspaces items.** No New Workspace item and Basic present (WS S13, S4), or New Workspace... present and Basic absent (PM [med]).
6. **Panel-menu wording.** "Close Group" (PM P) or "Close Tab Group" (WS).
7. **Screenshot provenance.** TO says TechRadar's 2024 images are the reviewer's own macOS captures (Pixelmator metadata); TL calls them Adobe press screenshots credited to Adobe. Either way they show 24.x on macOS at 2×.
8. **Panel-menu glyph colour.** #A8A8A8 (TO, Tools header) or #EBEBEB (TL, Timeline strip), both 2024; and four lines (2024) versus three (2021–22 Properties).
9. **Secondary text brightness.** Measured #A1A1A1–#A8A8A8 (2024) versus Spectrum's #B9B9B9 / #D1D1D1 (WS) versus #8E8E8E (PM, 2021–22). The palette takes #A5A5A5; thin-text sampling may read dark.
10. **Layer names.** #D5D5D5 (TL) versus body text #EBEBEB; both kept.
11. **Toggled-on chip.** #494949 (TL, 2020–22), #565656 (PM, 2021–22) and #545454 (TO, 2024). The palette uses #545454.
12. **Heights.** Tab strips 22 (TL) or 24 (WS, PM); Timeline ~280 (TL) or ~290 (WS); splitters 3 (TO) or 2 (WS); dock title strip 11 (TO) or 8 (WS); stage scrollbars 7 (TL style) or 10 (WS).
13. **Properties type size.** 12 (PM P) or 11 (WS); frame numbers 11 (TL) or 10 (WS).
14. **Dock gaps.** #252525 (2024, TO/TL) or #141414–#1A1A1A (2021–22, PM P). Possibly a real 2024 change.

### 8.2 Theme and colours
- Which Spectrum gray ramp 24.x uses. The 2024 screenshots suggest classic backgrounds and newer foregrounds; confirm on Windows. It drives the frame, bar, text, label and pasteboard values; only the panel colour (#323232) is the same either way. [WS]
- The Dark pasteboard hex in 24.x (#252525 vs #1D1D1D), and whether the 1 px stage outline is visible on it by default. [WS]
- The off and on fill colours of the 24.x Properties quick-action toggles; S7 only says darker and lighter. [WS]
- Hover colours: tool button about #3F–#45, flyout row about #2C2C2C, drawer cell border about #2680EB. [TO]
- Exact hex for the shape tween, IK pose and blank-keyframe span; the loop bar was only sampled from print. [TL]
- Default onion-skin colours: S16 says blue and green, one 2020 install shows mint #99FFCD and #00FF00. The 2024 look of the onion and edit-multiple markers is unknown. [TL]
- How toggled-on timeline buttons look in 2024 (S36 says the active state is easier to see). [TL]
- What the default Color Theme is in 24.0 — answered: Dark (§4.7, WS S36 [high]). TO and PM inferred it from screenshots. [PM, TO]

### 8.3 Layout, chrome and metrics
- All pixel sizes: app bar, document tabs, edit bar, tab strips, Tools and Properties widths, Timeline height. Tools is 48 on macOS; Windows at 100% is unmeasured. [WS]
- No 2024 Windows capture at 100% was found. The header, icon bitmaps and half-pixel features may differ by about 1 px from the macOS-derived values. [TO]
- Whether the 2024 timeline toolbar and header heights differ on Windows at 100% from the macOS 2× screenshots. [TL]
- Default layer-column width (258–300 px seen) and default Timeline height at 1920×1080 on Windows. [TL]
- The order of Test Movie, Share and the workspace switcher, and where the Home button sits on Windows. [WS]
- Edit bar left group: whether Edit Scene and Edit Symbols sit before or after the breadcrumb (S12 puts Edit Symbols at the upper left in 23.0.2; S19's upper right is pre-2020 text); whether Back hides or dims at scene level; the breadcrumb separator glyph; the zoom-step arrow glyphs and whether they flank the zoom field. [WS]
- What the Essentials collapsed-icon strip contains and which side of Properties it is on. Whether the Timeline group has an Output tab by default — answered yes by TL [high existence]. [WS]
- The UI font family (Adobe Clean is unconfirmed) and the exact sizes. [WS]
- Whether the 24.x zoom presets go past 800% (1600%, 2000%). [WS]
- Whether the Share popover still says Twitter in 24.x; where "Auto-Collapse Icon Panels" and the pasteboard-colour option live in the 24.x Preferences (Properties > Doc has "Apply to pasteboard", PM [med]). [WS]
- Did 24.0's sleek UI change paddings and row heights? [PM]

### 8.4 Timeline
- The 2024 *default* toolbar set has not been seen directly. Adobe's 2024 screenshots use a customized set, so the 2023 defaults (S29) are assumed.
- Which layer height is the default (Short / Medium / Tall), and the exact px. Both CIB books show Short checked while their text implies it isn't the default. Estimates: Short about 16, Medium about 24, Tall about 32.
- Default frame-cell width (about 10 px estimated from 2020/2023 screenshots) and the slider's maximum.
- Match FPS default: on per the 2020 LinkedIn transcript, unchecked in the 2022/2023 menu screenshots.
- Whether the FPS and current-frame readouts are editable hot text (drag/type), and their tooltips.
- Tooltips for most toolbar and header buttons: Layer View, previous/next keyframe, Loop, the highlight-dot header, Delete.
- Glyphs in 2020+ for hidden and transparent layers, the outline-mode swatch, and mask, masked, guide, motion-guide, armature and camera layers.
- Whether 2024 on Windows still uses light native context menus (seen in 2020).
- What clicking the highlight-dot header or the mountain glyph, or long-pressing Layer View, does in 2024.
- The shared web-search budget ran out during the timeline check; further confirmation needs a real Animate 24 Windows screenshot.

### 8.5 Tools
- Does the 2024 drawer's ≡ menu still offer Comfortable / Compact? If a Comfortable mode exists in 2024, its metrics are unknown; all 2024 numbers come from captures with a 30 px pitch.
- The tooltip of the "…" button (maybe "Edit Toolbar…").
- When the stroke swatch comes in front of the fill: the fill is in front in four captures; the stroke is in front in S1 p.68, where stroke segments were selected.
- Smooth/Straighten disappearing when nothing on the Stage is selected rests on one capture (S2a).
- The Brush, Eraser and Pencil mode pop-ups and Lock Fill in the toolbar are verified only for 2020. Their 2024 glyphs and menu marker (check vs bullet) are unknown.
- Ctrl+Space zoom, Ctrl for a temporary Selection tool, and the Hand/Zoom double-click behaviours were not verified for 2024.
- Do toolbar buttons, and not only drawer tiles, carry the warning badge when a tool is unsupported?
- The Fluid Brush presets flyout in some 2020 builds is unconfirmed.

### 8.6 Properties and Library
- Doc tab: does Publish Settings come before Document Settings? What are the tooltips of the four Doc-tab quick actions?
- Does the Essentials Properties group show a third Assets tab by default? WS rates it [high] yes (S4, S16, S38).
- Filter defaults: is the new-filter blur/distance 4 px (the draft's screenshot reading) or 5 px (Flash 8–CS6)?
- Classic-tween Rotate default in the UI: Auto (JSFL and XFL behaviour) or None (S7 text)?
- Library panel menu in 24.0: do Shared Library Properties and the use-count items survive? How does the "Duplicated Items" folder behave?
- Fresh-install defaults: fill colour (#0066CC?), Classic Brush size and smoothing, the default text font and size.
- Exact UI details: the mixed-selection header wording, whether Swap is an icon or a text button, and the Position and Size layout (W/H left, X/Y right).
- Which tools have no Tool tab, exactly: Free Transform, Zoom, Rotation?
- The 24.x category-pill colour (#4A4A4A read on 2021–22 captures) and whether Properties glyphs moved to #EBEBEB like the toolbar.

### 8.7 Menus and shortcuts
- Window and Help menus: the exact order. Do Toolbars, Other Panels (Strings, Accessibility) and Motion Presets still exist in 24.0?
- File menu: do File Info..., Publish Preview and New from Template still exist in 24.0?
- Where Convert to Frame-by-Frame Animation sits: the top level of Modify, or under Modify > Timeline?
- Default keys in 20.0+: Show Shape Hints (Ctrl+Alt+I vs Ctrl+Alt+H), Clip to Stage, the onion-skin toggle (C8 is from 2018). Bone (M), Time Scrub (Alt+Shift+H), Rotation (Shift+H) and 3D Rotation (Shift+W) are confirmed by the April 2024 export in §6, apart from item 3 of §8.1.

### 8.8 Research limits
- The web-search budget ran out in all four checks. Later items were verified by fetching Adobe help pages, community threads and the book samples directly.
- The workspace researcher found no 2024 Classroom in a Book sample or other 24.x write-up for layout, and looked at no Adobe screenshots because of the clean-room rule.
- The panels researcher's P readings come from 2021–22 help-page screenshots and were not re-measured.
- Every visual value not tagged [high] needs measuring on a real install.

### 8.9 Measure on a real Animate 2024 install (Windows, 100%, Dark)
Measure directly; don't commit screenshots.
1. Menus: light or dark dropdowns and context menus, and their row height (§8.1 item 1).
2. Gap, bar and pasteboard colours (#252525 / #2F2F2F / #252525 expected), body text (#EBEBEB) and label text (#A5A5A5 expected). Panels should read #323232.
3. Whether the 1 px stage outline shows on the pasteboard.
4. Heights of the app bar, document tab row, edit bar, dock title strips and tab strips; widths of Tools and Properties; the Timeline height; splitter width.
5. The off and on fills of the Properties quick-action toggles and the timeline toggles.
6. The order of the app bar's right cluster (Test, Share, Workspace) and the Home button's position.
7. The edit bar's left-group order, Back at scene level, the breadcrumb separator, and the zoom-step arrows.
8. Which panels are in the Essentials icon strip, and which side of Properties it is on.
9. Timeline: default row height, cell width, layer-column width, the default toolbar set, and the shape-tween, IK, blank-span, loop and onion colours.
10. The UI font family and sizes.
11. The disputed shortcuts: Snap to Objects, Rotation, the onion-skin toggle, Show Shape Hints.
