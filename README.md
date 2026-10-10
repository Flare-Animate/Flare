<div align="center">

# Flare

<img width="663" height="400" alt="Flare banner" src="https://github.com/user-attachments/assets/73e4050f-f248-419e-8e61-fe8814ab984d" />

**A free, open-source Animation suite based on OpenToonz and Next2Flash. Designed with continuity for Adobe Flash/Animate users**

[![Discord](https://img.shields.io/discord/1500316971802296430?label=Discord&logo=discord&logoColor=white&color=5865F2)](https://discord.com/invite/JpeScW8Awa)
[![Website](https://img.shields.io/badge/website-flare--animate-orange)](https://flare-animate.github.io/website/)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue)](./LICENSE.txt)
[![Nightly](https://img.shields.io/badge/release-nightly-brightgreen)](https://github.com/Flare-Animate/Flare/releases/tag/nightly)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)
![Status](https://img.shields.io/badge/status-early%20alpha-red)
![C++](https://img.shields.io/badge/C%2B%2B-Qt-00599C)
![Rust](https://img.shields.io/badge/Rust-planned-orange)

[Website](https://flare-animate.github.io/website/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [Download](https://github.com/Flare-Animate/Flare/releases/tag/nightly) · [Issues](https://github.com/Flare-Animate/Flare/issues) · [日本語](./doc/README_ja.md) · [简体中文](./doc/README_chs.md)

</div>

> **Very early alpha.** Expect bugs and incomplete Flash/Animate import fidelity.
> Back up your work and report problems on [GitHub](https://github.com/Flare-Animate/Flare/issues)
> or [Discord](https://discord.com/invite/JpeScW8Awa).

## Contents

[Overview](#overview) · [Screenshots](#screenshots) · [Features](#features) · [Install](#install) · [Build](#build-from-source) · [Architecture](#architecture) · [Roadmap](#roadmap) · [Contributing](#contributing) · [License](#license)

## Overview

Flare is a community fork of [OpenToonz](https://opentoonz.github.io/) reworked to
feel like Adobe Animate. It gives the Flash/Animate community a modern,
cross-platform home that reads existing `.fla`, `.xfl` and `.swf` projects with no
subscription and no dead runtime.

- **Familiar UI** - Animate-style workspace (Drawing / Animation / Rigging / Compositing rooms) and theme.
- **Open your old work** - native C++ import for the Flash/Animate file family; no Java or external runtime.
- **OpenToonz power underneath** - vector and raster levels, xsheet/timeline, plastic rigging, FX, rendering pipeline.

## Screenshots

| Animate-style workspace | Timeline & rigging | Flash import |
|---|---|---|
| _placeholder_ | _placeholder_ | _placeholder_ |

## Features

| Area | Status |
|------|--------|
| Adobe Animate-style default workspace (OpenToonz and StudioGhibli rooms selectable in **Preferences > Interface > Rooms**) | Available |
| Vector + raster drawing, xsheet, timeline | Available (inherited) |
| Plastic/skeleton rigging, FX schematic, motion tracking, script console | Available (inherited) |
| Flash/Animate import (table below) | Work in progress |
| Next2Flash SWF round-trip + AS3 decompiler merge | In progress, see [`doc/NEXT2FLASH_INTEGRATION.md`](./doc/NEXT2FLASH_INTEGRATION.md) |
| Moho project support | In progress, see [`doc/MOHO_SUPPORT.md`](./doc/MOHO_SUPPORT.md) |

### Supported Flash file types

| Format | Extension | Status |
|--------|-----------|--------|
| Flash project (ZIP) | `.fla` | Import (XFL extraction + parse) |
| XFL project | `.xfl` | Import (directory or ZIP) |
| Compiled Flash | `.swf` | Header + embedded bitmap extraction |
| Component library | `.swc` | Catalog + embedded bitmaps |
| Flash Video | `.flv` / `.f4v` | Raster level via FFmpeg |
| ActionScript | `.as` | Imported as reference text |
| Legacy binary FLA (OLE2/CFBF, CS4 and earlier) | `.moho`-era `.fla`, `.fls` | Carved and read |
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
[`doc/MOHO_SUPPORT.md`](./doc/MOHO_SUPPORT.md) explains why.

Complex documents will not round-trip cleanly yet. Details and limits:
[`doc/FLASH_SUPPORT.md`](./doc/FLASH_SUPPORT.md), [`doc/how_to_import_swf.md`](./doc/how_to_import_swf.md).
Tracking issues: [#16](https://github.com/Flare-Animate/Flare/issues/16), [#47](https://github.com/Flare-Animate/Flare/issues/47).

## Install

Nightly builds are published on every push to `master`:
**[Latest nightly release](https://github.com/Flare-Animate/Flare/releases/tag/nightly)**

| Platform | Asset | Requirements |
|----------|-------|--------------|
| Windows | `Flare-Windows-Portable-nightly.zip` | Windows 10/11, 64-bit |
| Linux | `Flare-x86_64.AppImage` | x86_64, glibc 2.35+ (Ubuntu 22.04+) |
| macOS | `Flare.dmg` | macOS 13+ Intel (Apple Silicon via Rosetta 2) |

Pre-release builds may be unstable. Optional: install **FFmpeg** on `PATH` for
`.swf`/`.flv`/`.f4v` playback (Flash import itself needs no external tools).

**macOS first launch.** Flare is not notarized. If macOS reports the app as
"damaged", clear the quarantine flag once:

```sh
xattr -dr com.apple.quarantine /Applications/Flare.app
```

**Linux.** If the app exits with "Could not locate Flare's stuff folder", set
`FLAREROOT` to the installed `stuff` directory, e.g.
`export FLAREROOT=/usr/share/flare/stuff`. The AppImage finds it automatically.

## Build from source

Requires **CMake 3.10+**. Building is memory-intensive; limit parallel jobs on
low-RAM machines.

```sh
cmake -S flare/sources -B build -G "Ninja" -DCMAKE_BUILD_TYPE=Release

# <4GB RAM: -j1  |  4-8GB RAM: -j2  |  >8GB RAM: --parallel
cmake --build build -j2
```

Platform guides: [Windows](./doc/how_to_build_win.md) · [macOS](./doc/how_to_build_macosx.md) · [Linux](./doc/how_to_build_linux.md) · [BSD](./doc/how_to_build_bsd.md)

## Architecture

| Path | Purpose |
|------|---------|
| `flare/sources/` | Application and libraries (C++/Qt, CMake): toonz core, UI, Flash import |
| `flare/sources/common/flash/` | Format readers: SWF, XFL/FLA, ZIP, shapes, optional AS3 bridge |
| `flare/sources/common/moho/` | Moho (Lost Marble) project reader |
| `flare/sources/common/tgame/` | Game engine: entity-component-system, fixed-timestep loop, systems |
| `flare/rust/` | Rust backend (`flare_formats`): format sniffing via a C ABI, behind `-DFLARE_WITH_RUST=ON` |
| `tools/flash/next2flash/` | Optional Python sidecar for ActionScript decompile / string patch / compile |
| `stuff/` | Runtime data: rooms, profiles, brushes, FX presets |
| `thirdparty/` | Bundled third-party libraries |
| `plugins/` | Plugin SDK and samples |
| `packaging/` | Installer / AppImage / DMG packaging |
| `ci-scripts/`, `scripts/` | CI and developer helper scripts |
| `tests/`, `tools/` | Test suites and utilities |
| `doc/` | Build guides and design docs |

Dev tip: `python scripts/log_watcher.py` tails build `*.log` files (VS Code task "watch logs", `Ctrl+Shift+B`).

### Running the tests

```sh
python tests/run_all.py          # everything: pytest, native C++, Rust, fixtures
```

Individual suites, useful while iterating:

```sh
python tests/native/run_tests.py          # native C++ readers and updater
python -m pytest tools/flash/tests -q     # Flash bridge and format fixtures
cargo test --manifest-path flare/rust/Cargo.toml
python tests/check_issue_fixes.py         # which open issues have code answering them
```

`tests/native/run_tests.py` finds Qt on its own, so `QT_BIN` need not be exported.
Pass `--no-local-src` to link the tests against the built DLLs instead of
compiling the checked-out sources — that path is what catches export/ABI mistakes,
and it needs a freshly built `tnzcore` and `flareqt`.

## Roadmap

Goal: a C++/Rust successor to Flash / Adobe Animate (CS6-era workflow plus later Animate features), built on OpenToonz.
Status is honest: items marked Planned have no shipped code unless a doc says otherwise.

Status is honest: each row states what is in the tree, not what is intended.

| Feature | Status |
|---------|--------|
| Flash/Animate import — `.fla` `.xfl` `.swf` `.swc` `.flv` `.f4v` `.as`, plus CFBF legacy `.fla`, AIR/ZXP/MXP/ANE packages | Shipped; fidelity improving |
| Moho project import | Shipped: structure, rig and asset manifest. Rendering not implemented ([doc](./doc/MOHO_SUPPORT.md)) |
| Next2Flash SWF round-trip, AS3 decompiler | Shipped as an optional sidecar: decompile, string patch, compile ([doc](./doc/NEXT2FLASH_INTEGRATION.md)) |
| Beginner-friendly (FlipaClip-style) layout profile | Shipped: **Preferences > Interface > Rooms > FlipaClip** |
| Game-creation tooling | Scaffolded: `flare/sources/common/tgame` — ECS, fixed-timestep loop, sprite/physics/script/audio/camera/particle/UI systems. Not yet wired into the UI |
| Rust backend | Shipped in part: `flare_formats` does content-based format sniffing over a C ABI. Opt in with `-DFLARE_WITH_RUST=ON` |
| Android build | Scaffolded: `flare/android` (Gradle + Qt activity). Not yet building an APK |
| Windows installer EXE | NSIS script shipped ([packaging/windows/installer.nsi](./packaging/windows/installer.nsi)) |
| Peer-to-peer remote control | Protocol and tests only (`remote_protocol_tests`); no transport |
| Auto-update | Shipped: release check, SHA-256 verification, download ([`flareupdater.cpp`](./flare/sources/flareqt/flareupdater.cpp)) |
| User-editable UI layouts, submittable as a PR | Rooms are plain `.ini` under `stuff/profiles/layouts/rooms/`; add a directory and it appears in the room list |

Reference projects: [JPEXS decompiler](https://github.com/jindrapetrik/jpexs-decompiler), [Ruffle](https://github.com/ruffle-rs/ruffle), [swf2js](https://github.com/ienaga/swf2js), [Citrus Engine](https://github.com/DaVikingCode/Citrus-Engine), [Adobe Flash Simple Game Engine](https://github.com/benjamin-stern/Adobe-Flash--Simple-Game-Engine), [as3mxml](https://github.com/BowlerHatLLC/vscode-as3mxml). Licences are preserved wherever code is ported; see [`LICENSE.txt`](./LICENSE.txt).

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](./CONTRIBUTING.md).

- Discord: https://discord.com/invite/JpeScW8Awa
- Website: https://flare-animate.github.io/website/
- Discussions: [GitHub Discussions](https://github.com/orgs/Flare-Animate/discussions)
- Bugs: [GitHub Issues](https://github.com/Flare-Animate/Flare/issues)

Flare stays compatible with OpenToonz where possible; keep upstream licensing and attribution intact.

## License

Files outside `thirdparty/` and `stuff/library/mypaint brushes/` are under the
[Modified BSD License](./LICENSE.txt). Third-party components keep their original
licenses (see `thirdparty/` and `stuff/library/mypaint brushes/Licenses.txt`).
Flare is a fork of [OpenToonz](https://github.com/opentoonz/opentoonz) (DWANGO; based on Toonz by Digital Video / Studio Ghibli).

[![Discord Server](https://discord.com/api/guilds/1500316971802296430/widget.png?style=banner2)](https://discord.com/invite/JpeScW8Awa)
