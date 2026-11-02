# Text objects

Text is a shape that remembers where it came from.

A text object is an ordinary `Shape` whose path is the outline of the letters, plus a
`TextSource` (`flare-document`): the words, the font family list, the size, bold and italic, and
the corner of the first line. The outline is a copy that is made from the source and only ever
replaced as a whole.

## Why not a new kind of content

A `Content::Text` would be the honest model, and every consumer of `Content` would have to learn
it: the evaluator, the renderer, hit testing, selection and bounds, transforms, tweens, the SWF
and HTML5 publishers, the XFL exporter and the stage's editing handles. Fonts differ from machine
to machine, so a document that stored only the words would draw differently where a font is
missing, and every export would need the font engine. With the outline stored, all of that keeps
working unchanged and a file draws the same everywhere, which is also what Flash does to static
text on publish. The cost is that the source can go out of step with the path, so the rules below
keep them together.

## Rules

- The path and the source change together or not at all: the only things that write a text
  object's path are `TextEngine::shape` (the font worker) and the transforms below.
- A move keeps the text and moves its corner. An even scale (the same on both axes, positive)
  keeps the text and scales its size. A turn, a shear, a stretch or a mirror makes the letters
  something a font cannot set, so the object becomes plain outlines: the source is dropped.
- Editing the points of a text object (add, delete, convert or move an anchor) rebuilds the path,
  so it also drops the source. Break Apart does the same on purpose, in one step.
- A copy of a text object, a duplicated keyframe or a pasted one, is text of its own.
- The source is limited (`MAX_TEXT_CHARS`, `MIN_TEXT_SIZE` and `MAX_TEXT_SIZE`,
  `MAX_FAMILY_CHARS`) and is validated when a file is read; a damaged one is a damaged file.
- Setting text takes the font database, which scans the machine's fonts, so it is done by the text
  worker, off the UI thread, and the newest request wins. A file written on another machine
  opens with its stored outlines; only editing it asks this machine's fonts, and a family it lacks
  falls back to the system's default, so edited text can look different from what was stored.

## Editing on the stage

The box where text is typed has a field that takes the typing and draws nothing. The font worker
sets the words as they are typed (`TextJob::Preview`, newest request wins) and the render worker
draws that in the place of the text object being edited, or on top of the scene when the text is
new (`RenderJob::text_preview`), so the stored letters and the new ones are never shown together.
The caret and the box around the text come from the same answer, so they are in the text's own font
and not the interface's. The preview stays up until the finished text is on the stage.

## Not done

Mixed styles in one text object, paragraph settings (alignment, line spacing, letter spacing),
text on a path, dynamic and input text, and fitting text to a box.
