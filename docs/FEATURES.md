# Flare features, layout and roadmap

Detail moved out of the top-level README. For the honest stub list see [STATUS.md](./STATUS.md).

Also available in Flare: Animate-style default workspace (OpenToonz and StudioGhibli rooms selectable in **Preferences > Interface > Rooms**), vector and raster drawing, xsheet/timeline, plastic/skeleton rigging, FX schematic, motion tracking and script console (inherited from OpenToonz).

## Supported file types

| Format | Extension | Status |
|--------|-----------|--------|
| Flash project (ZIP) | `.fla` | Import (XFL extraction + parse) |
| XFL project | `.xfl` | Import (directory or ZIP) |
| Compiled Flash | `.swf` | Header + embedded bitmap extraction |
| Component library | `.swc` | Catalog + embedded bitmaps |
| Flash Video | `.flv` / `.f4v` | Raster level via FFmpeg |
| ActionScript | `.as` | Imported as reference text |
| Legacy binary FLA (OLE2/CFBF, CS4 and earlier) |  `.fla`, `.fls` | Carved and read |
| Adobe AIR packages | `.air`, `.ane`, `.oam` | Unpacked, assets gathered |
| Extension packages | `.zxp`, `.mxp` | Unpacked, assets gathered |
| Shared libraries | `.sol` | Unpacked, assets gathered |
| Compressed SWF | `.swz`, `.ksk` | Same reader as `.swf` |
| ISO-BMFF video | `.f4v`, `.m4v`, `.mp4` | Detected; raster level via FFmpeg |
| Moho / Anime Studio project | `.moho`, `.mohoproj`, `.anime`, `.animeproj`, `.anme` | Structure read; see below |

Format detection is by content, not by extension, so a `.fla` that is really a
SWF — or a SWF renamed to `.zip` — still opens. The detector recognises
uncompressed `FWS`, zlib `CWS` and LZMA `ZWS`, ZIP-backed XFL, the OLE2/CFBF
signature, and the ISO-BMFF `ftyp` box.

**Moho:** Flare reads a rig's structure — layers, bones, switch layers, keyframe
tracks, and the artwork it references — and writes a manifest. It does **not**
render the rig; reproducing Moho's skeleton solve, region-weight deformation and
Smart Bones is a much larger project than reading the file, and
[`docs/MOHO_SUPPORT.md`](./MOHO_SUPPORT.md) explains why.

Complex documents will not round-trip cleanly yet. Details and limits:
[`docs/FLASH_SUPPORT.md`](./FLASH_SUPPORT.md), [`docs/how_to_import_swf.md`](./how_to_import_swf.md).
Tracking issues: [#16](https://github.com/Flare-Animate/Flare/issues/16), [#47](https://github.com/Flare-Animate/Flare/issues/47).

## Install notes

**macOS first launch.** Flare is not notarized. If macOS reports the app as
"damaged", clear the quarantine flag once:

```sh
xattr -dr com.apple.quarantine /Applications/Flare.app
```

**Linux.** If the app exits with "Could not locate Flare's stuff folder", set
`FLAREROOT` to the installed `stuff` directory, e.g.
`export FLAREROOT=/usr/share/flare/stuff`. The AppImage finds it automatically.

## Repository layout

| Path | Purpose |
|------|---------|
| `flare/sources/` | Application and libraries (C++/Qt, CMake): toonz core, UI, Flash import |
| `flare/sources/common/flash/` | Format readers: SWF, XFL/FLA, ZIP, shapes, optional AS3 bridge |
| `flare/sources/common/moho/` | Moho (Lost Marble) project reader |
| `flare/sources/common/tgame/` | Game engine: entity-component-system, fixed-timestep loop, systems |
| `flare/sources/common/*/rust/` | Rust backend (`flare_formats`): modules beside the C++ they port, manifest at `flare/sources/Cargo.toml`, |
| `flare/sources/common/flash/ac_as3_*.rs` | Native Rust AS3 core: decompile, disassemble, string / literal patch, transplant |
| `stuff/` | Runtime data: rooms, profiles, brushes, FX presets |
| `thirdparty/` | Bundled third-party libraries |
| `plugins/` | Plugin SDK and samples |
| `packaging/` | Installer / AppImage / DMG packaging |
| `ci-scripts/`, `scripts/` | CI and developer helper scripts |
| `tests/`, `tools/` | Test suites and utilities |
| `docs/` | Build guides and design docs |

Dev tip: `python scripts/log_watcher.py` tails build `*.log` files (VS Code task "watch logs", `Ctrl+Shift+B`).

## Running the tests

```sh
python tests/run_all.py          # everything: pytest, native C++, Rust, fixtures
```

Individual suites, useful while iterating:

```sh
python tests/native/run_tests.py          # native C++ readers and updater
python -m pytest tools/flash/tests -q     # Flash bridge and format fixtures
cargo test --workspace --manifest-path flare/sources/Cargo.toml
python tests/check_issue_fixes.py         # which open issues have code answering them
```

`tests/native/run_tests.py` finds Qt on its own, so `QT_BIN` need not be exported.
Pass `--no-local-src` to link the tests against the built DLLs instead of
compiling the checked-out sources — that path is what catches export/ABI mistakes,
and it needs a freshly built `tnzcore` and `flareqt`.

## Roadmap and status

Goal: a C++/Rust successor to Flash / Adobe Animate (CS6-era workflow plus later Animate features), built on OpenToonz.

Status is honest: each row states what is in the tree, not what is intended.

| Feature | Status |
|---------|--------|
| Flash/Animate import — `.fla` `.xfl` `.swf` `.swc` `.flv` `.f4v` `.as`, plus CFBF legacy `.fla`, AIR/ZXP/MXP/ANE packages | Shipped; fidelity improving |
| Moho project import | Shipped: structure, rig and asset manifest. Rendering not implemented ([doc](./MOHO_SUPPORT.md)) |
| Next2Flash SWF round-trip, AS3 decompiler | Native Rust: decompile, disassemble, string patch, class literal edit without Flex, transplant; compile needs mxmlc ([doc](./NEXT2FLASH_INTEGRATION.md)) |
| Beginner-friendly (FlipaClip-style) layout profile | Shipped: **Preferences > Interface > Rooms > FlipaClip** |
| Game-creation tooling | Scaffolded: `flare/sources/common/tgame` — ECS, fixed-timestep loop, sprite/physics/script/audio/camera/particle/UI systems. Not yet wired into the UI |
| Rust backend | Shipped in part: `flare_formats` does content-based format sniffing over a C ABI. Always built (via Corrosion) |
| Android build | Scaffolded: `flare/android` (Gradle + Qt activity). Not yet building an APK |
| Windows installer EXE | NSIS script shipped ([packaging/windows/installer.nsi](../packaging/windows/installer.nsi)) |
| Peer-to-peer remote control | Protocol and tests only (`remote_protocol_tests`); no transport |
| Auto-update | Shipped: release check, SHA-256 verification, download ([`flareupdater.cpp`](../flare/sources/flareqt/flareupdater.cpp)) |
| User-editable UI layouts, submittable as a PR | Rooms are plain `.ini` under `stuff/profiles/layouts/rooms/`; add a directory and it appears in the room list |

Reference projects: [JPEXS decompiler](https://github.com/jindrapetrik/jpexs-decompiler), [Ruffle](https://github.com/ruffle-rs/ruffle), [swf2js](https://github.com/ienaga/swf2js), [Citrus Engine](https://github.com/DaVikingCode/Citrus-Engine), [Adobe Flash Simple Game Engine](https://github.com/benjamin-stern/Adobe-Flash--Simple-Game-Engine), [as3mxml](https://github.com/BowlerHatLLC/vscode-as3mxml). Licences are preserved wherever code is ported; see [`LICENSE.txt`](../LICENSE.txt).
