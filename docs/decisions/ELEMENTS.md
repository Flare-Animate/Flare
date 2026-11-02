# Decision: many elements in a keyframe

Status: accepted; stages 1 (the document model and the project file), 2 (reading everywhere) and 3 (drawing into the layer) done; 4 (import and export: the XFL parts are done with stage 2) and 5 (Merge Drawing) next.

## The problem

Flare keeps one shape or one instance in a frame span, so every shape drawn needs a layer of its
own. Animate does not: everything drawn on a layer in a frame stays on that layer. Adobe's
[Draw and create objects](https://helpx.adobe.com/animate/desktop/multimedia-and-video/drawing.html)
(section "Drawing modes and graphic objects") says it:

- **Merge Drawing**, the default: shapes drawn on one layer cut and merge each other ("the
  top-most shape cuts away the part of the shape underneath it that it overlaps"), and a shape's
  stroke and fill are separate elements.
- **Object Drawing** (the J key; Pencil, Line, Pen, Brush, Oval, Rectangle, Polygon): each shape
  is a separate drawing object on the same layer; they never merge. Modify > Combine Objects >
  Union makes a drawing object of merged shapes.
- Rectangle and Oval **primitives** keep their size and corner radius editable.

## The model

A span keeps its `content` as its first element and gains `extras`: more elements drawn above it,
in order. Everything that reads `span.content()` keeps working on the first element, so the change
reaches the other ~100 files one behaviour at a time, each with its tests, instead of in one
rewrite. A span with extras cannot carry a tween (Animate tweens one object).

Selection names an element by its span and its index in `elements()` (`SpanRef::element`, 0 for the first, so a span of one behaves as it always did). An edit rebuilds the span it is in, so it is one undo step with the commands that exist; nothing new was needed in `flare-commands`.

## Stages

1. **Model and storage**: `FrameSpan::extras`, `elements()`, validation (a camera is alone; sizes
   are bounded), the project file's optional field (older files read as before).
2. **Reading everywhere**: evaluation and drawing, hit testing, bounds, the editor's frame
   operations (insert, clear, reverse, copy, paste), the Library and the exporters.
3. **Drawing into the layer**: the tools append to the layer's keyframe at the playhead (a blank
   keyframe is made if the frame has none) instead of adding a layer; selection, move, delete,
   group and convert to symbol act on one element. Until Merge Drawing exists every shape is a
   drawing object, and the Merge/Object toggle is shown dimmed as a mock.
4. **Import and export**: XFL/FLA keep a keyframe's elements together (the importer no longer
   spreads them over extra layers); SWF import stays one layer per depth, which is how a SWF is
   built.
5. **Merge Drawing**: the planar map: overlapping fills cut each other, strokes and fills select
   apart, Union and the other Combine Objects commands.

Every stage keeps the whole workspace's checks passing, and updates the surfaces (CLI, MCP, the
browser build) with the desktop.
