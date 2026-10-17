# Stub audit status

Scope: flare/sources/{flare_formats.rs,common/*/rust,remote}, tools/flash, flare/sources/flare/{flashimport,mohoimport,castviewer}.cpp.
Method: rg for todo!/unimplemented!/"not yet"/"not implemented"/stub/placeholder.

## Result

| Area | Item | Status |
|------|------|--------|
| flare/sources/*/rust | no todo!/unimplemented!/stub markers | clean |
| flashimport.cpp | vector shape import (XFL/FLA shapes) | NOT implemented; reported as "Not converted to levels" |
| flashimport.cpp | shape text, static/dynamic text | NOT implemented |
| flashimport.cpp | morph shapes, sounds, video, components | NOT implemented |
| flashimport.cpp | SWF vector art, text, embedded fonts, video, nested sprite timelines | NOT implemented |
| flashimport.cpp | legacy binary FLA (OLE2/CS4-) timeline + vectors | NOT implemented (bitmaps recovered only) |
| flashimport.cpp | XFL/FLA advanced timeline/vector/ActionScript | experimental |
| tools/flash/ext2flash compile | mxmlc/Flex SDK toolchain missing (bridge in abc_patcher/swf_patcher exists) | stub |
| castviewer.cpp | frame count not shown in file explorer | NOT implemented (user-reported; root cause: importer creates no level for vector-only FLA) |

## Notes

- User report "import shows nothing" = vector-only FLA. Importer extracts bitmaps only;
  vector content is named in the dialog, not drawn. Fix = vector shape import (large, not done).
- Small items deferred; none were trivially fixable without the vector importer.
