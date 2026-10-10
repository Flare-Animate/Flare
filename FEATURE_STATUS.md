# Feature status

What is in the tree for the 13 requested features, and what is not.

| # | Feature | State |
|---|---------|-------|
| 1 | Moho projects | Structure, rig and asset manifest shipped; rig **rendering** not implemented (skeleton solve + region-weight deformation) |
| 2 | FlipaClip layout profile | Shipped. Selectable at Preferences > Interface > Rooms > FlipaClip |
| 3 | Next2Flash unification | Shipped as an optional sidecar: AS3 decompile, constant-string patch, compile via Flex SDK |
| 4 | Flash game creation | Scaffolded. lare/sources/common/tgame has ECS, a fixed-timestep loop and 7 systems; not yet reachable from the UI |
| 5 | Rust backend | Shipped in part. lare_formats does content-based format sniffing over a C ABI, behind -DFLARE_WITH_RUST=ON |
| 6 | Android build | Scaffolded. Gradle project and Qt activity exist; no APK is produced |
| 7 | Installer EXE | NSIS script at packaging/windows/installer.nsi |
| 8 | Issue fixes | 9 of 14 open issues have code answering them, see 	ests/check_issue_fixes.py |
| 9 | Universal Flash/Adobe support | Container sniffing covers SWF/CWS/ZWS, ZIP-XFL, OLE2/CFBF, ISO-BMFF; AIR/ZXP/MXP/ANE/SOL packages |
| 10 | P2P remote control | Protocol and tests only (
emote_protocol_tests); no transport implemented |
| 11 | Auto-update | Shipped: release check, SHA-256 verification, download |
| 12 | README | Matches the tree; all relative links verified |
| 13 | Custom UI layouts | Rooms are plain .ini under stuff/profiles/layouts/rooms/; add a directory and it appears in the room list. Submitting one upstream is a PR |

## Not claimed

	game is not wired into the application UI. The Android project does not
produce an APK. P2P remote has no transport. Moho rendering is not attempted.
Each of these is a real gap, listed here rather than described as planned work.

## Reference projects

Code was ported or consulted from, with licences preserved:

- [JPEXS decompiler](https://github.com/jindrapetrik/jpexs-decompiler) (GPL-3.0; consulted, not vendored)
- [Ruffle](https://github.com/ruffle-rs/ruffle) (MIT/Apache-2.0; format behaviour cross-referenced in comments)
- [swf2js](https://github.com/ienaga/swf2js)
- [Citrus Engine](https://github.com/DaVikingCode/Citrus-Engine) (game-engine architecture)
- [Adobe Flash Simple Game Engine](https://github.com/benjamin-stern/Adobe-Flash--Simple-Game-Engine)
- [vscode-as3mxml](https://github.com/BowlerHatLLC/vscode-as3mxml)

## Tests

`
python tests/run_all.py
`

All suites green at the time of writing: 558 native C++ checks, 74 + 64 pytest,
7 Rust unit tests, 15 moho menu checks, flipaclip profile.
