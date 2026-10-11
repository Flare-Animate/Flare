# Feature status

What is in the tree for the 13 requested features, and what is not.

| # | Feature | State |
|---|---------|-------|
| 1 | Moho projects | Structure, rig and asset manifest shipped; rig **rendering** not implemented (skeleton solve + region-weight deformation) |
| 2 | FlipaClip layout profile | Shipped. Selectable at Preferences > Interface > Rooms > FlipaClip |
| 3 | Next2Flash unification | Shipped as an optional sidecar: AS3 decompile, constant-string patch, compile via Flex SDK |
| 4 | Flash game creation | Scaffolded. `flare/sources/common/tgame` has ECS, a fixed-timestep loop and 7 systems; not yet reachable from the UI |
| 5 | Rust backend | Shipped in part. Format sniffing, XFL rendering, Moho, FlipaClip, frame counting, CFBF directory listing and TFilePath/TEnv helpers sit behind a C ABI, gated on `-DFLARE_WITH_RUST=ON`. Modules live beside the C++ they port (`flare/sources/common/*/rust/`); manifest at `flare/sources/Cargo.toml` |
| 6 | Android build | Scaffolded. Gradle project and Qt activity exist; no APK is produced |
| 7 | Installer EXE | NSIS script at `packaging/windows/installer.nsi` |
| 8 | Issue fixes | 9 of 13 open issues have code answering them, see `docs/ISSUE_MAP.md` and `tests/check_issue_fixes.py` |
| 9 | Universal Flash/Adobe support | Container sniffing covers SWF/CWS/ZWS, ZIP-XFL, OLE2/CFBF, ISO-BMFF; AIR/ZXP/MXP/ANE/SOL packages |
| 10 | P2P remote control | Protocol and tests only (`remote_protocol_tests`); no transport implemented |
| 11 | Auto-update | Shipped: release check, SHA-256 verification, download |
| 12 | README | Matches the tree; all relative links verified |
| 13 | Custom UI layouts | Rooms are plain `.ini` under `stuff/profiles/layouts/rooms/`; add a directory and it appears in the room list. `tools/layout_tool.py` exports, imports and validates one; submitting one upstream is a PR |

## Not claimed

- `tgame` is not wired into the application UI.
- The Android project does not produce an APK.
- P2P remote has no transport.
- Moho rig rendering is not attempted.
- The merge-drawing model (Flash's planar-map shapes: edge bending, boolean cuts) is
  not implemented. This is the largest remaining gap against Animate's drawing
  model; `docs/ANIMATIONCRAFT_ABSORPTION.md` specifies the port.

Each of these is a real gap, listed here rather than described as planned work.

## Reference projects

Code was ported or consulted from, with licences preserved:

- [JPEXS decompiler](https://github.com/jindrapetrik/jpexs-decompiler) (GPL-3.0; consulted, not vendored)
- [Ruffle](https://github.com/ruffle-rs/ruffle) (MIT/Apache-2.0; format behaviour cross-referenced in comments)
- [swf2js](https://github.com/ienaga/swf2js)
- [Citrus Engine](https://github.com/DaVikingCode/Citrus-Engine) (game-engine architecture)
- [Adobe Flash Simple Game Engine](https://github.com/benjamin-stern/Adobe-Flash--Simple-Game-Engine)
- [vscode-as3mxml](https://github.com/BowlerHatLLC/vscode-as3mxml)
- [AnimationCraft](https://github.com/KMSDax/AnimationCraft) (MIT OR Apache-2.0; per-module absorption verdicts in `docs/ANIMATIONCRAFT_ABSORPTION.md`)

## Tests

```
python tests/run_all.py
```

All 10 suites green at the time of writing: pytest (flash bridge, sync), the moho
menu wiring, the flipaclip profile, the layout tool, game export, the Rust crates
(20 unit tests), the logic-atlas checker, the open-issue map, the native C++
readers, and the flash fixtures.

`LOGIC-ATLAS.md` maps the whole system and is checked against the tree by
`scripts/atlas_check.py`, which also runs as a suite and as a pre-commit hook, so
a document claim cannot drift away from the code silently.