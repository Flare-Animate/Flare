# Editable vector brush strokes (issue #25)

Status: planned, not built. The issue stays open.

## What exists

Brush pressure profiles, nib settings and custom presets are in. A Pencil or Line stroke is
turned into a filled outline when it is drawn, so its centerline, its pressure samples and its
width profile are not kept.

## Decision

Keep the centerline as data. A stroke becomes a list of nodes, each with a position, a width and
an opacity; the filled outline is derived from it by the same code that makes it today. The
node edit tools (subselection, smoothing, width handles) then edit the nodes and the outline is
rebuilt.

## Schema and compatibility

- A new optional `nodes` field on the stroke content, with `width` and `opacity` per node. Old
  files have none and load as they do now; a stroke without nodes is a plain outline.
- The storage schema is bumped by one when the field is written, and the reader keeps accepting
  the previous version (see the schema notes in ARCHITECTURE.md).
- Every node count and width is bounded (AGENTS.md: everything that grows has a bound).

## Why it is not done here

It touches the document model, the storage schema, the renderer, every drawing tool and all four
surfaces (desktop, browser, `flare-cli`, `flare-mcp`). A partial version (a field with no tool
that edits it) would be a stub, so it waits for a change that can carry all of it.

## Order of work

1. Document model and storage: nodes, bounds, migration test from the previous schema.
2. Drawing tools record nodes; the outline is rebuilt from them, with a test that it matches
   today's outline.
3. Width and opacity handles on the stage, as one undo step each.
4. `flare-cli` and `flare-mcp` commands to create and set node width and opacity.
