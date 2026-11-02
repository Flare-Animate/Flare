# Flare tool icons

`tools/*.svg` are Flare's tool icons, copied unchanged from
`Flare-Animate/flare`: `flare/sources/flare/icons/dark/tools/20x20/` (21 tool
icons) and, for Lasso, Polygon Lasso, Line, Rectangle, Oval, PolyStar and Camera,
`icons/dark/actions/20x20/` (`selection_freehand`, `selection_polyline`,
`geometric_*`) and `actions/16x16/camera.svg`. Checked against Flare's git
history: they come from OpenToonz/Tahoma2D or Flare's rebrand, none from Adobe. They come
from OpenToonz's icon set (Flare is an OpenToonz fork): BSD-3-Clause,
© DWANGO Co., Ltd., OpenToonz contributors and Flare contributors. They are
black on transparent; the toolbar renders them as masks tinted with the theme
colour (`crates/ac-app/src/toolbar.rs`, `flare_icon_svg`). Tools Flare has no
icon for use this project's own icons in `assets/icons/tools` (same style).

Not used: icons or theme materials Flare derived from studying Adobe software.
