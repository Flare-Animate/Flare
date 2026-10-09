# Moho Project Import in Flare

## What this does

**File → Import → Import Moho Project** reads a Moho (Lost Marble) project,
reports what the rig contains, and writes a manifest of its structure beside
the project, gathering whatever bitmaps it can find.

It then builds the scene (`common/moho/MohoPlan.h`, executed by
`flare/mohoimport.cpp`):

| Moho                     | Flare                                                      |
|--------------------------|------------------------------------------------------------|
| Image layer              | column + raster level loaded from `image_path`             |
| Vector / mesh layer      | column + empty vector level named `Group/Layer`            |
| Group / Bone layer       | flattened; path kept in the column name, bone inherited    |
| Switch layer             | one column per child; cells only on frames its key selects |
| Bones (`skeleton.bones`) | pegbars, parent hierarchy kept; bound layers parented      |
| Hidden layer             | column with camstand visibility off                        |
| Frame range              | xsheet rows `start..end` (frame 0 = rest pose, skipped)    |

Not solved (see [Why not rendering](#why-not-rendering)): mesh deformation,
region/flexible binding (-1), Smart Bones, per-layer transform channels,
layer effects. Vector geometry arrives as empty named levels to redraw/trace.
Missing bitmaps fall back to an empty vector level so the column still exists.

Tested by `tests/native/moho_plan_tests.cpp` (flatten, bone inherit, switch
gating, keyless switch).

## Formats

| Extension | Container | Status |
|-----------|-----------|--------|
| `.moho` | ZIP around `Project.mohoproj` | Read |
| `.anime` | ZIP around `Project.animeproj` (Anime Studio 11) | Read |
| `.mohoproj` / `.animeproj` | The bare JSON document; Moho 13+ opens these directly | Read |
| `.anme` | Pre-11 plain text | Recognised, and reported as unsupported — Moho itself only reads it |

The container is identified from the **leading bytes**, not the extension, so a
`.moho` that is really a bare document still opens, and a ZIP that is not a
Moho project is rejected with a reason rather than mis-parsed.

Both container forms yield the same document; that equivalence is covered by a
regression test.

### Rejecting a non-Moho file without guessing

The pre-11 `.anme` format is plain text beginning with the literal header
`Anime Studio Project`, and that header is what identifies it. It used to be
inferred instead — "the first non-whitespace byte is not `{`, so this is a legacy
project" — which is true of every SWF, PNG, PDF and ELF, so any binary file
dropped on the Moho import was reported as

> pre-11 .anme project: a plain-text format Moho itself only reads. Re-save it
> from Moho as a .moho project.

That is the worst answer available for a user holding a Flash movie: it names a
real format, says it is old, and points at software that cannot help. It was
found by running the reader over real files — `mario.ssf`, a 3.5 MB uncompressed
SWF that Moho exported — rather than by any test, because no test fed the reader a
SWF. `tests/native/probe_samples` now exists so that a format nobody thought to
test reports as a number rather than as a pass.

The same function now also skips a UTF-8 BOM before the JSON brace, which the
JSON grammar permits and editors emit, and peeks 64 bytes rather than 8 so a
header after leading whitespace is still seen. Both are covered by
`moho_reader_tests`, and `mutation_check.py` carries controls that reintroduce
each of the three behaviours.

## What is extracted

- Canvas size, frame rate, frame range, format version
- The full layer tree: name, type, nesting, visibility, draw order
  (Moho stores draw order as array order, back to front — there is no z-index)
- Per-layer bone binding: rigid (`parent_bone >= 0`), flexible/region
  (`parent_bone == -1`, with its `flexi_bone_subset`), or unbound
- The bone skeleton: names, parent indices, lengths, strengths, dynamics flags
  (kept as a flat array with parent indices, exactly as Moho stores them)
- Switch layers and their alternatives, including which child is active
- Animated values: every keyframe track and its keys
- Smart Bone / timeline action names
- Mesh geometry counts, document style count
- Referenced image paths, and how many were actually found

Channels are found **structurally** — any object with parallel `when`/`val`
arrays is one — rather than from a list of key names. The real key set is large
(40+ distinct names in an ordinary 23-bone rig) and channels nest several
levels deep, under `transforms`, inside `mesh.points[]` and
`mesh.curves[].points[]`, and on `skeleton.bones[]`. A name list would be wrong
on the first real document.

Every field is read with a default, because Moho documents drop fields between
format generations: the 1021 generation omits `doc_uuid`, `action_refs`,
`modified_date` and the bezier handle weights entirely. A reader that assumes
presence crashes on older files; `tests/flash_fixtures/generate_moho_fixtures.py`
generates a 1021 fixture that would catch that.

## The manifest

`writeManifest()` writes `moho_manifest.json` into `<project>_flare_import/`,
containing the census, the bone array, switch layers, the nested layer tree and
the image accounting. It is a stable, machine-readable description of the rig,
so the structure is usable by other tools and not only displayable in a dialog.

Any referenced images that exist beside the project are copied to
`<project>_flare_import/images/`, preserving sub-paths.

## Images are not in the container

This is the single most important thing to know about a `.moho` file: **it does
not contain the artwork.** Moho references bitmaps by relative path from a
sibling folder (normally `images/`), which `File → Gather Media` populates. A
`.moho` handed over on its own is therefore an incomplete project.

Flare reports the referenced, found and missing counts rather than quietly
producing an empty import, and the manifest records the caveat.

## Why not rendering

Reproducing a Moho render means, in order:

1. Solve the skeleton.
2. Deform each layer. There is **no per-point weight table** in the normal
   case: a region-bound layer (`parent_bone == -1`, which is 779 of 842 layers
   in the reference corpus) is a distance-weighted blend of *all* bones, with
   the weights computed at run time from bone length and strength.
3. Reconstruct beziers, including tapered strokes.
4. Apply masking, layer effects and the seven style variants.
5. Run bone dynamics — a spring/damping solve layered on the keyed pose, active
   on a large fraction of real bones.

Each step is individually understood; the composition is where the work is.
`moho2svg`, the most advanced implementation in existence, needed months of
pixel-diffing against Moho's own SVG export to get there, and its code is not
open source. The interpolation semantics (`im` modes, the decomposed bezier
handles in `b[{ai,pi,ao,po}]`) are still listed as an open unknown in the most
thorough public reverse-engineering writeup.

An approximate renderer would be worse than none here: a rig that looks nearly
right and deforms subtly wrongly is very hard to diagnose, and a user has no
way to tell it apart from a correct one.

## Tests

```sh
# fixtures (a .moho/.mohoproj pair per case, plus rejection cases)
python tests/flash_fixtures/generate_moho_fixtures.py /tmp/mohofx

# menu wiring — a command that compiles but is never added to a menu is invisible
python tests/moho/test_moho_menu.py

# reader behaviour, against the fixtures and against a real project
moho_tests /tmp/mohofx
moho <path-to-project.moho> <out-dir>
```

`moho_tests` covers container detection, the equivalence of the two container
forms, the minimal rig, switch layers, the 1021 generation, nesting and bone
binding, a dangling bone parent, every rejection path, and the manifest. The
C++ harnesses resolve the reader from the built `tnzcore.dll` at run time rather
than importing it, so they exercise the shipped code without a link-time
dependency on the module under test.

## Sources

- Smith Micro / Lost Marble, *Anime Studio Document File Format* — the original
  20-page spec; still the only first-party documentation of the top-level
  sections, styles and the layer `type` enum.
- `vinhio/moho2svg` — JSON Schemas validated against 46 real project files
  spanning versions 1021–1045. Not open source, and nothing is copied from it;
  its schemas are the most complete public description of the format.
- `jpdurigan/godot_moho_importer` (MIT) and `mohokit` — container layout and the
  rigging model.
- `CharWalk.moho`, from the godot_moho_importer examples, is the real project
  used to verify the reader.

OpenToonz, Tahoma2D, Krita and Synfig have no Moho import of any kind.
