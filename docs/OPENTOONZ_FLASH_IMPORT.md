# OpenToonz FLA/XFL import (C++ merge)

Upstream: OpenAnimationLibrary/opentoonz-dev, branch `feature/fla-xfl-import`,
commits `be0c87f1`..`48eddb42` (7 commits). Licence: OpenToonz BSD-3-Clause,
(c) 2016 DWANGO Co., Ltd. and OpenToonz contributors; see
`flare/sources/THIRD-PARTY-NOTICES.md`.

Each upstream commit is one Flare commit on branch `merge-xfl`. Paths were mapped
`toonz/sources/toonzlib` -> `flare/sources/flarelib`, `toonz/sources/toonz` ->
`flare/sources/flare`, `toonz/sources/toonzqt` -> `flare/sources/flareqt`,
`include/toonz` -> `include/flare`, `include/toonzqt` -> `include/flareqt`.

## What it adds
- `flare/sources/flash/` (static lib `tnzflash`, namespace `Flash`): Qt-based
  FLA/XFL reader (archive, document, timeline, geometry, fills, media, shape tweens).
- `flarelib/flash*.cpp`: scene importer, preflight, provenance, source bundle,
  field bindings, handoff, text, vector, effects. Scene/xsheet gain Flash
  instances and frame labels (`flashinstance.h`, `TXsheet::m_flashFrameLabels`,
  `ToonzScene::flashSourceBundle`).
- App: `File > Import > Import FLA/XFL as Editable Scene...` (`MI_ImportFlash`),
  `Windows > Flash Instance` (`MI_OpenFlashInstancePanel`).
- FXs: Color Matrix, Drop Shadow, Bevel; Iwa_TextFx layout extensions.
- `ToonzScene::save` takes an optional `saveSceneIcon` (default true).

## Relationship to Flare's existing Flash support
Flare's Rust FLA/XFL/SWF stack (`common/flash/ac_*.rs`, `flarelib/ac_*`, see
FLASH_SUPPORT.md) and the app command `MI_ImportFlashVector`
(`flare/flashimport.cpp`, common/flash `XFLReader`) are kept unchanged. The merged
C++ importer is a separate, editable-scene path (PLI vector levels + Flash instances),
exposed under a different command id and menu label. Nothing was removed or replaced.
Both readers parse the same formats; consolidating them is follow-up work.
