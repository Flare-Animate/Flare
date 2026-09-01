# Flare

<img width="663" height="400" alt="FlareBanner" src="https://github.com/user-attachments/assets/73e4050f-f248-419e-8e61-fe8814ab984d" />

**Flare** is a free, open-source 2D animation studio — a community-driven fork of
[OpenToonz](https://opentoonz.github.io/) reworked to feel like **Adobe Animate**
and to open the Flash/Animate file formats Adobe left behind (`.fla`, `.xfl`,
`.swf`, and friends).

The goal is simple: give the Flash/Animate community a modern, actively
maintained, cross-platform home that reads their existing projects — without a
subscription and without a dead runtime.

[![Discord](https://img.shields.io/discord/1500316971802296430?label=Discord&logo=discord&logoColor=white&color=5865F2)](https://discord.com/invite/JpeScW8Awa)
[![Website](https://img.shields.io/badge/website-flare--animate-orange)](https://flare-animate.github.io/website/)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue)](./LICENSE.txt)

➡️ **Website:** https://flare-animate.github.io/website/ · **Discord:** https://discord.com/invite/JpeScW8Awa

> ⚠️ **Very early alpha.** Flare is under active, community-driven development.
> Expect bugs, missing polish, and incomplete Flash/Animate import fidelity.
> Not yet production-ready — back up your work and report issues on
> [GitHub](https://github.com/Flare-Animate/Flare/issues) or
> [Discord](https://discord.com/invite/JpeScW8Awa).

[日本語](./doc/README_ja.md) · [简体中文](./doc/README_chs.md)

---

## Why Flare?

Adobe Animate is proprietary, subscription-only, and Flash is end-of-life — yet
huge amounts of animation, games, and educational content still live in `.fla`
and `.swf` files. OpenToonz is a powerful production tool but its UI is
unfamiliar to Flash/Animate artists. Flare bridges the two:

- **Familiar UI** — the default workspace is laid out like Adobe Animate
  (Drawing / Animation / Rigging / Compositing rooms), with an Adobe-style theme.
- **Open your old work** — built-in import for the Flash/Animate file family.
- **OpenToonz power underneath** — vector + raster levels, a full xsheet/timeline,
  plastic/skeleton rigging, FX, and the OpenToonz rendering pipeline.

## Features

### Adobe Animate-style workspace (default)
Flare ships with the **Adobe Animate** workspace selected out of the box — no
configuration needed. Prefer the classic layout? Open **Preferences → Interface →
Rooms** and switch to **OpenToonz** (or **StudioGhibli**).

### Flash / Adobe Animate file support *(work in progress)*
Built-in, native C++ import — no Java, JPEXS, or external runtime required.

| Format | Extension | Status |
|--------|-----------|--------|
| Flash project (ZIP) | `.fla` | Import (XFL extraction + parse) |
| XFL project | `.xfl` | Import (directory or ZIP) |
| Compiled Flash | `.swf` | Header + embedded bitmap extraction |
| Component library | `.swc` | Catalog + embedded bitmaps |
| Flash Video | `.flv` / `.f4v` | Raster level via FFmpeg |
| ActionScript | `.as` | Imported as reference text |

> ⚠️ Flash import is actively being developed and **not yet production-ready** —
> complex documents will not round-trip cleanly. See [`doc/FLASH_SUPPORT.md`](./doc/FLASH_SUPPORT.md)
> for the architecture and current limitations, and please file bugs with a sample
> file. Tracking issues: [#16](https://github.com/Flare-Animate/Flare/issues/16),
> [#47](https://github.com/Flare-Animate/Flare/issues/47).

### Inherited from OpenToonz / Tahoma2D
Flare keeps the OpenToonz feature set and continues to pull improvements from
OpenToonz and the [Tahoma2D](https://tahoma2d.org/) community fork:
vector & raster drawing, the xsheet and timeline, plastic & skeleton rigging,
the GTS scanning tools, the effects (FX) schematic, motion tracking, and the
script console.

### Merging with Next2Flash
The Flare-Animate org is consolidating its Flash tooling by merging
[Next2Flash](https://github.com/SSF2-Mods-Official/Next2Flash) — an MIT-licensed
SWF round-trip editor with a native AS3 decompiler — into Flare. The native SWF
reader/writer pieces are being ported, while AS3 round-tripping is bridged as an
optional helper. See [`doc/NEXT2FLASH_INTEGRATION.md`](./doc/NEXT2FLASH_INTEGRATION.md).

## Download

Pre-built nightly binaries for **Windows**, **Linux** (AppImage + tarball), and
**macOS** (DMG) are published automatically on every push to `master`:

➡️ **[Latest nightly release](https://github.com/Flare-Animate/Flare/releases/tag/nightly)**

| Platform | Asset | Requirements |
|----------|-------|--------------|
| Windows | `Flare-Windows-Portable-nightly.zip` | Windows 10/11, 64-bit |
| Linux | `Flare-x86_64.AppImage` | x86_64, glibc 2.35+ (Ubuntu 22.04+) |
| macOS | `Flare.dmg` | macOS 13+ Intel (Apple Silicon via Rosetta 2) |

These are pre-release builds and may be unstable.

## Program Requirements

To enable FFmpeg-based `.swf`/`.flv`/`.f4v` playback, install **FFmpeg** and make
sure it is on your `PATH`. The Flash *import* commands themselves need no external
tools.

Building requires **CMake 3.10+** on your `PATH`. Install it via the official
installer or [Chocolatey](https://chocolatey.org/) on Windows, Homebrew
(`brew install cmake`) on macOS, or your package manager on Linux.

## How to Build Locally

> ⚠️ Building Flare is memory-intensive. On systems with < 8 GB RAM, limit
> parallel jobs to avoid freezes.

```sh
cmake -S flare/sources -B build -G "Ninja" -DCMAKE_BUILD_TYPE=Release

# < 4GB RAM: -j1   |   4-8GB RAM: -j2   |   > 8GB RAM: --parallel
cmake --build build -j2
```

Platform-specific guides:

- [Windows](./doc/how_to_build_win.md)
- [macOS](./doc/how_to_build_macosx.md)
- [Linux](./doc/how_to_build_linux.md)
- [BSD](./doc/how_to_build_bsd.md)

## Community & Contribution

- 💬 **Discord:** https://discord.com/invite/JpeScW8Awa — the fastest way to ask
  questions, report bugs, and follow development.
- 🌐 **Website:** https://flare-animate.github.io/website/
- 🗳️ **Discussions:** [GitHub Discussions](https://github.com/orgs/Flare-Animate/discussions)
  for proposals and ideas.
- 🐛 **Issues:** [GitHub Issues](https://github.com/Flare-Animate/Flare/issues).

Contributions are welcome — see [CONTRIBUTING.md](./CONTRIBUTING.md). Flare aims to
stay compatible with OpenToonz where possible; please keep the original project's
licensing and attribution in mind.

## Licensing

Files outside `thirdparty/` and `stuff/library/mypaint brushes/` are under the
[Modified BSD License](./LICENSE.txt). Third-party components retain their
original licenses — see `thirdparty/` and
`stuff/library/mypaint brushes/Licenses.txt`.

Flare is a fork of [OpenToonz](https://github.com/opentoonz/opentoonz) (© DWANGO,
based on Toonz © Digital Video / Studio Ghibli) and retains its licensing and
attribution.

## Development helper scripts

`scripts/log_watcher.py` watches `*.log` files under the build directory and
prints the tail whenever they change (also wired to the "watch logs" task in
VS Code, `Ctrl+Shift+B`):

```sh
python scripts/log_watcher.py    # defaults to flare/build
```
