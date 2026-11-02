# Decision: opening a file and saving it back loses nothing

Status: accepted and built.

## The requirement

Saving a `.fla`, an XFL folder or a `.swf` that was only opened in Flare, with no change made,
must give back the file as it was. Nothing may be corrupted by opening it.

## The problem

Flare does not edit those files in place. Import converts them into a `Document`, which holds
less than the file does: bitmaps and sounds in the library, publish settings, the exact bytes
of the XML, whole parts of a SWF. Export then writes a new file from the document. Any
round trip through the document is lossy, so "unchanged in, unchanged out" cannot come from
making the document hold more; it has to come from keeping the file itself.

## The design

**The source is kept, whole.** An import that succeeds keeps what it read as an `Origin`
(`flare-document`), carried by the `Document` the way the stage and the attachments are:

- a `.fla` or `.swf`: the bytes of the file;
- an XFL folder (also reached through its `.xfl` stub or its `DOMDocument.xml`): every file and
  every empty folder, by relative name, sorted by name.

The import reads from the same bytes that are kept (a folder is read into memory once and
imported from there), so the document and the source cannot disagree, whatever happens to the
disk meanwhile. Reading never writes: the source is only opened for reading, and nothing is
created, renamed or deleted beside or inside it.

**A fingerprint says whether the document changed.** Right after the import a fingerprint of the
document is taken (`flare_storage::fingerprint`) and kept with the source as its `Baseline`. It
is a BLAKE3 over exactly the records the project file stores (settings, scenes, sounds, the
symbols' names and kinds, source attachments), in two kinds of part: one for everything outside
the symbols' timelines, and one for each symbol with the source data hung off it. A document is
"unchanged" when its current fingerprint equals the baseline. This is a content comparison, not
an edit counter, so an edit that is undone, and a project saved and reopened, are unchanged
again. It is blind to the source itself and to anything the project file does not store, which
is nothing an export could see either.

**Exact export.** An export to the source's own format of an unchanged document writes the
original:

| Source | Export | Result |
|---|---|---|
| `.fla` | `.fla` | the original file, byte for byte |
| `.fla` | folder | every entry's bytes identical, same names and order of creation |
| XFL folder | folder | every file and empty folder, byte for byte (modification times and permissions are not kept) |
| XFL folder | `.fla` | every file's bytes identical, in a new zip (the folder's `.xfl` stub is not put in an archive; `mimetype` is added if the folder had none) |
| `.swf` | publish SWF | the original file, byte for byte (the compression option does not apply) |

Anything else (a `.swf` to XFL, a `.fla` to SWF) is a conversion and is rebuilt from the
document, as before; the report says so.

**Edited export keeps everything that was not edited.** For an edited XFL/FLA import the
exporter rebuilds what the document owns and writes the rest of the original untouched:

- `DOMDocument.xml` is the original bytes if the "everything else" part of the fingerprint is
  unchanged (only symbols were edited) and every library file it lists is written under the same
  name; otherwise it is rebuilt.
- Each library item is the original bytes if that symbol's part is unchanged, else rebuilt. A
  symbol keeps the file name it came from while it keeps its name, so the main document's
  `<Include href>` stays true.
- Every other file in the source (bitmaps and sounds under `LIBRARY` and `bin`, `PublishSettings.xml`,
  `MobileSettings.xml`, `META-INF`, thumbnails, files nobody has heard of) is written back
  byte for byte, in its original order. The attachments that already carry unknown XML elements
  keep doing so; their references to those files stay valid because the files and their names
  are the same.
- A library file of a symbol that was deleted or renamed is not written (Flare owns it), and an
  original entry that would collide with one Flare writes is not written either. Both are
  reported, as is any entry that cannot be read back out of the archive (unsafe name, damaged,
  beyond the size bound).

Nothing about XFL is guessed: where Flare does not know what a file means it keeps its bytes.

**How a document is written back.** Save is an export to the file the document lives in, so the
table above is what Save does: File > Save (and `flare` / `flare-mcp` editing commands) on a document
opened from `x.fla` writes `x.fla`, the original bytes when nothing changed and otherwise the original with only
the edited parts rebuilt. The target is replaced atomically (the new file is written whole beside it, with the old
file's permissions, and moved over it; a folder is moved aside and swapped), a read-only target is refused, and
a failed save leaves the old file and the document unsaved. Only a document whose original was kept is
written back to its file (`saves_in_place`): a pre-CS5 binary `.fla`, a movie, or an Animate file over the cap has
nothing to carry the unknown files, so it opens as an unsaved import and Save asks for a new name; a pre-CS5 `.fla` is
never overwritten. The file is not re-read before it is replaced, so a change made to it by another program
after it was opened is lost by the save, as with any editor without file locking.

**The report says what happened.** An export's `Output` carries `lossless`: `exact` and a
sentence of why ("unchanged since import", "edited: N files rebuilt (...), M written as they
were", "not imported from a file", "imported from a movie"). `flare export-xfl`, the MCP
`export_xfl` and `publish_swf` print it; `flare import` says whether the original was kept.

**Bounded.** The source is capped at 256 MiB in all and 50,000 files. Beyond a cap the import
still succeeds, without a source, and says in a diagnostic that exporting it again will rebuild
it rather than copy it; the export's `lossless.reason` says the same. A folder with a symbolic
link, an unreadable file or a name that is not text is treated the same way. A legacy binary
`.fla` (read as a skeleton) keeps no source: Flare cannot write that format.

**It survives Save.** The project file gets an optional `origin` record: the fields as CBOR,
then the bytes (as sounds are), size-limited before anything is allocated, digest-checked on
load, and the source's own digest checked again after it is unframed. Projects without a source
are as small as before and files from before this exist still open; a file with a record is
refused by a build that predates it (its schema check does not know the kind), which is the
usual one-way compatibility of this format.

## What it does not do

- An edited project is not a copy: the rebuilt files are what Flare understood of them.
- Folder metadata (times, permissions, extended attributes) is not kept; a symbolic link is not
  followed and stops the source being kept.
- The project file is as big as the source plus the document; for a 100 MB `.fla` it is bigger.
  Autosave writes it too.
- Round trips through the project file, undo and the document's own validation are exact; Adobe
  Animate opening the rebuilt files is still untested.
