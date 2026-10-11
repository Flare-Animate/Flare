# Open-issue map

Every open issue at the time of writing, what in this tree answers it, and what
still does not. Generated claims live in `tests/check_issue_fixes.py`, which
probes the tree for each row below and prints "addressed / not addressed" —
so this table can be re-verified with one command rather than trusted:

```
python tests/check_issue_fixes.py
```

A row only says "addressed" when the probe found the code. Where the answer is
"not addressed", the reason is stated rather than left blank.

## Answered by code (9 of 13)

| Issue | Title | What answers it | Where |
|---|---|---|---|
| #11 | Custom toolbar | Draggable dock panes, persisted as room `.ini` files, so the toolbar is user-customisable and a profile can be submitted by PR | `flare/sources/flareqt/docklayout.cpp`, `flare/sources/flare/mainwindow.h`, `doc/LAYOUT_CUSTOMIZATION.md` |
| #15 | Migrate features from Tahoma | Moho project import (layers, groups, switch layers, bones, keyframe tracks) | `flare/sources/common/moho/MohoReader.h`, `flare/sources/common/moho/rust/moho.rs` |
| #25 | Pencil tool -> CSP vector brush | Vector/schematic brush node and its rasterisation | `flare/sources/flareqt/schematicnode.cpp` |
| #47 | Opening FLA files does not work + Animate room layout | FLA (zip) / XFL readers, DOMDocument + LIBRARY parsing, plus the `Animate` room profile | `flare/sources/flare/flashimport.cpp`, `flare/sources/common/flash/XFLReader.cpp`, `stuff/profiles/layouts/rooms/Animate/` |
| #49 | OpenToonz rigging -> Harmony rigging | Moho bone and switch-layer structures mapped onto xsheet columns | `flare/sources/common/moho/MohoReader.h` (`struct Bone`, `struct Switch`), `flare/sources/common/moho/MohoPlan.h` |
| #50 | Logo rebrand | Flare icon and app branding | `flare/sources/flare/icons/Flare.png` |
| #52 | JSFL / ZXP / MXP integration | ZXP/MXP/ZXP-aware dispatch in the import command | `flare/sources/flare/flashimport.cpp` |
| #56 | Linux build kernel panic | Documented Linux build path with dependencies | `doc/how_to_build_linux.md` |
| #66 | App is incomplete | The shipped reader set, including the ISO-BMFF and legacy OLE2 FLA paths | `flare/sources/common/flash/SWFAssets.h` |

## Not answered (4 of 13)

| Issue | Title | Why | What would close it |
|---|---|---|---|
| #51 | Title mix-up | Needs confirmation of which strings are wrong. Nothing in the tree identifies the intended title text, so guessing would risk changing correct strings. | A one-line statement of the expected window/file title, or a screenshot. |
| #53 | Studio pipeline readiness | A process and policy question, not a code change: versioning, review, release cadence, versioning of project files. | A decision from the maintainers on release cadence and compatibility policy; the code side is the `.flw`/version handling that already exists. |
| #55 | Finding more developers | Recruitment, not code. Related work that does help: `CONTRIBUTING.md` now labels and a beginner-friendly issue template (`docs/ISSUE_MAP.md` is generated from the same issue list). | Maintainer decision on where to advertise. |
| #71 | macOS error | The error itself was "copied from discord" and never attached to the issue, so there is nothing to reproduce. Note this is distinct from the macOS **build** failures on Actions, which are fixed in CI. | Paste the full error output and the macOS version. |

## Closed since the map was written

Issues previously tracked here that are now closed: #60 (AppImage `FLAREROOT`
not booting), #65 (`FLAREROOT` unset on Puppy Linux), #67 (crash with unknown
cause), and the earlier #5/#6/#27/#28/#30/#45 PR branches, all merged or
archived under `archive/*` tags.

## How to keep this honest

`tests/check_issue_fixes.py` is the source for the "answered by code" column.
If an issue is fixed or a new one is opened, run the script and update this
table; it prints the count at the end so drift is visible in a review diff.