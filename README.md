<div align="center">

# Flare

<img width="663" height="400" alt="Flare banner" src="https://github.com/user-attachments/assets/73e4050f-f248-419e-8e61-fe8814ab984d" />

**A free, open-source 2D animation studio with an Adobe Animate-style workflow and native Flash/Animate file import.**

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
| `stuff/` | Runtime data: rooms, profiles, brushes, FX presets |
| `thirdparty/` | Bundled third-party libraries |
| `plugins/` | Plugin SDK and samples |
| `packaging/` | Installer / AppImage / DMG packaging |
| `ci-scripts/`, `scripts/` | CI and developer helper scripts |
| `tests/`, `tools/` | Test suites and utilities |
| `doc/` | Build guides and design docs |

Dev tip: `python scripts/log_watcher.py` tails build `*.log` files (VS Code task "watch logs", `Ctrl+Shift+B`).

## Roadmap

Goal: a C++/Rust successor to Flash / Adobe Animate (CS6-era workflow plus later Animate features), built on OpenToonz.
Status is honest: items marked Planned have no shipped code unless a doc says otherwise.

| Feature | Status |
|---------|--------|
| Flash/Animate import (`.fla` `.xfl` `.swf` `.swc`) | In progress |
| Moho project import | In progress ([doc](./doc/MOHO_SUPPORT.md)) |
| Next2Flash SWF round-trip, AS3 decompiler | In progress ([doc](./doc/NEXT2FLASH_INTEGRATION.md)) |
| Beginner-friendly (FlipaClip-style) layout profile | Planned |
| Basic game-creation tools (AS3-style scripting) | Planned |
| Incremental Rust backend (compat with OpenToonz data formats) | Planned |
| Android build ([guide](./doc/how_to_build_android.md)) | Planned |
| Windows installer EXE ([packaging/windows](./packaging/windows)) | In progress |
| Peer-to-peer remote control | Planned |
| Auto-update | Planned |

Reference projects: [JPEXS decompiler](https://github.com/jindrapetrik/jpexs-decompiler), [Ruffle](https://github.com/ruffle-rs/ruffle), [swf2js](https://github.com/ienaga/swf2js), [Citrus Engine](https://github.com/DaVikingCode/Citrus-Engine), [as3mxml](https://github.com/BowlerHatLLC/vscode-as3mxml). Licenses are preserved wherever code is ported.

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
