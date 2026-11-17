# Shader graphs

Flare has a bounded, typed CPU shader graph (`flare_core::ShaderGraph`) and an RGBA image
renderer (`flare_render::{render_shader, apply_shader}`). Graphs evaluate deterministically on
the CPU, including in the browser-compatible libraries. `apply_shader` runs one graph on every
source pixel; a graph reads the pixel with `SourceColor` and `Channel`, as well as normalized
pixel-center `UvX`/`UvY` coordinates and time in seconds.

The current interchange file is versioned JSON. `flare shader import PROJECT NAME GRAPH.json`
adds or replaces an asset by name, `flare shader export PROJECT NAME GRAPH.json` writes an asset
back to that format, and `flare shader list PROJECT` lists saved assets.
`flare shader render PROJECT OUTPUT.png --shader NAME` renders a project frame through an ordered
stack of up to 16 named graphs (`--frame`, `--width`, `--height` and `--transparent` are available).
Each graph receives the preceding graph's output as its source color. MCP exposes the same graph
library through `list_shader_graphs`, `get_shader_graph` and `set_shader_graph`; `export_png` accepts
the optional `shaders` list. The JSON shape is:

```json
{
  "version": 1,
  "nodes": [
    { "op": "constant", "args": 0.0 },
    { "op": "constant", "args": 0.0 },
    { "op": "constant", "args": 0.0 },
    { "op": "constant", "args": 1.0 },
    { "op": "color", "args": [0, 1, 2, 3] }
  ],
  "output": 4
}
```

Node indices are zero-based. Edges must point to an earlier node, so cycles are impossible.
Supported operations are `constant`, `uv_x`, `uv_y`, `time`, `source_color`, `channel`,
`add`, `multiply`, `sin`, `fract`, `noise2_d` and `color`; every graph ends at a color node.
Channels and output colors are normalized to 0..1. Graphs are limited to 256 nodes, 64 named
assets per document, and 1 MiB per imported file. `.flare` schema version 5 stores shader graph
records; loading validates the graph again.

The CLI and MCP server can list, export and edit these portable graph assets, and both can render a
frame through a shader stack. This is the foundation, not full source-format compatibility. Effects
are not yet attached to timeline objects or automatically applied by regular project export, and
there is no desktop or browser authoring panel. OpenToonz
Shader Fx XML descriptors and GLSL programs are not executed. Unity `.shadergraph` files and
Blender `.blend` node graphs are not imported.
