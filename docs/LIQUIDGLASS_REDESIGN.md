# LiquidGlass redesign

## Problems found (before)
- Theme was Dark plus a 40-line override that put `padding`/`min-height: 22px`/alpha backgrounds on every QPushButton/QToolButton/QSpinBox/QLineEdit: toolbar buttons clipped top and bottom, spin boxes clipped, "Reset Positior", "Automatically Save Every", startup bar labels cut (TEST_REPORT_FULL D2/D3).
- Fake glass: rgba surfaces on a flat navy gave muddy, inconsistent greys, mismatched with Dark's untouched #303030/#4a4a4a surfaces underneath (two palettes in one UI).
- Harsh white group-box borders, unchecked checkbox border nearly invisible, low-contrast inactive tabs and panel titles, disabled text alpha-blended.

## What changed
- `scripts/dev/gen_liquidglass.py` regenerates `stuff/config/qss/LiquidGlass/LiquidGlass.qss`: full Dark.qss (same 600 selectors) with every neutral grey remapped on one blue-slate ramp and accents remapped, plus `scripts/dev/liquidglass_overrides.qss`. No more `@import`.
- Overrides: solid layered surfaces, subtle top-to-bottom gradients on buttons/combos/selected tabs, 1px light edges, radii 4/6/8, single accent (#2a6fb0 fill, #5fb4f0 focus, #8fd0ff checked), solid disabled colour, visible checkbox borders. No padding or size changes, so Dark's compact metrics hold. Qt QSS cannot blur, so no alpha faking.
- C++: `startuppopup.cpp` bottom bar fixed height -> minimum height; `tooloptions.cpp` Reset Position/Zoom/Rotation/Pick Screen buttons `setFixedWidth` -> `setMinimumWidth` (theme padding exceeded the text-only width).
- Other themes untouched. Tools: `scripts/dev/gui_theme_session.sh`, `gui_lg_prep.sh` (Xvfb screenshots; 125% via additionalStyleSheet font-size 15px), `contrast_check.py`.

## Contrast (WCAG, `python3 scripts/dev/contrast_check.py`)
```
PASS 13.72:1 (need 4.5)  text primary / base  #e6edf6 on #172131
PASS 11.41:1 (need 4.5)  text primary / raised btn  #e6edf6 on #222f45
PASS 16.85:1 (need 4.5)  text / input well  #f2f7fd on #0f1621
PASS  7.33:1 (need 4.5)  secondary text / base  #9fb0c8 on #172131
PASS  9.53:1 (need 4.5)  group title / base  #b9c8de on #172131
PASS  9.12:1 (need 4.5)  inactive tab text / tab  #c3d0e4 on #1f2b3f
PASS  5.25:1 (need 4.5)  selected tab/menu white / accent  #ffffff on #2a6fb0
PASS  5.25:1 (need 4.5)  white / accent gradient end  #ffffff on #2a6fb0
PASS  4.40:1 (need 3.0)  disabled text / base  #7886a0 on #172131
PASS  5.56:1 (need 3.0)  checkbox border / well  #7d8fb0 on #0f1621
PASS  2.47:1 (need 1.0)  button border / base  #4a5e80 on #172131
PASS  8.01:1 (need 3.0)  focus ring / well  #5fb4f0 on #0f1621
PASS  9.73:1 (need 4.5)  checked text / pressed  #8fd0ff on #172131
PASS 10.28:1 (need 3.0)  icon base / base  #cecece on #172131
PASS 13.21:1 (need 4.5)  tooltip text / tooltip  #f2f7fd on #1f2b3f
```
Before (measured): white on old accent #3c88ad 3.95:1 (fails 4.5); unchecked checkbox border #070707 on #131313 ~1.1:1 (fails 3). tools/themes/check-themes.mjs reports 0/10 themes complete on origin too (12 uncovered selectors, unchanged).

## Verification
Screenshots in `docs/images/qa/liquidglass-{before,after}-*.png` (100% and 125% font). No clipped labels in Draw/Rigging rooms, startup dialog, Preferences at 100% and 125%.

## Style Editor "My Favorites" overlap (fixed)
`StyleEditor::createStylePages` (flareqt/styleeditor.cpp) builds a label, toggle button and menu checkbox per style set as children of the editor. In builds without `HAVE_MYPaint` the Raster case is compiled out, so those three widgets were never placed in a layout or menu and were painted at (0,0) over the tab bar, in every theme. They are now deleted in that case. Before/after crops: `liquidglass-{before,after}-styleeditor-tabs-{100,125}.png`.

## Other rooms
Animate, Timing, Cleanup, Library at 100% and 125% font: no clipped labels found (Timing/Cleanup 125% shots included).

## Known remaining
- Dialog/menu corner radii are not visible without a compositing window manager.
