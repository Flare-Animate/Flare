**English** | [Deutsch](docs/translations/README.de.md) | [Español](docs/translations/README.es.md) | [Français](docs/translations/README.fr.md) | [Português (Brasil)](docs/translations/README.pt-BR.md) | [日本語](docs/translations/README.ja.md) | [한국어](docs/translations/README.ko.md) | [简体中文](docs/translations/README.zh-CN.md)

<div align="center">

# Flare

<img src="assets/BannerNew.png" width="969" height="261" alt="The Flare Banner">

**Free, open-source animation suite with continuity for Adobe Animate and Flash users.**

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Status](https://img.shields.io/badge/status-early%20alpha-red)

[Website](https://flare-animate.github.io/website/) · [Web version](https://flare-animate.github.io/Flare/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [Build](docs/BUILDING.md) · [Issues](https://github.com/Flare-Animate/Flare/issues)

</div>

> **Early alpha.** Expect bugs and incomplete Flash/Animate import fidelity. Back up your work.

## What is Flare?

Flare, also referred to as *FlareCraft*, is a (Work In Progress) Animation Suite, designed to make the transition from software like *Adobe Animate*, *Adobe/Macromedia Flash* and *Moho* easier for Artists and Animators.

The aim is to support the formats those tools use, so people can open and keep working on the project files they already have, and to lay out the UI in a simpler, more familiar way that can be customised.

It is being rewritten in **Rust**. It is based predominantly on *OpenToonz* and *Tahoma2D*, with features merged in from *AnimationCraft* and *Next2Flash*, which have been absorbed into Flare. The old C++ code is kept in [`original/`](original/README.md) as an archive.

## Features

- Drawing with pen, brush and shape tools, with a selection tool for moving, grouping and editing shapes, and stroke styles (solid, dashed, dotted, stipple, hatched, ragged, herringbone, rope, zigzag).
- A timeline with frames, layers, folders, masks, motion guides, onion skin and a camera layer.
- Motion and shape tweens with easing, and a Motion Editor for the segment curves.
- Symbols (Graphic, Movie Clip and Button) in a library, placed as instances on the stage.
- Stop motion session settings. Camera capture is not built yet.
- Sound playback on desktop, and Test Movie, which runs the published SWF in Ruffle.
- Open and save `.flare` projects. Import and export Animate's XFL and `.fla`, and open Flash movies (`.swf`) as editable projects.
- Import Moho projects and BVH motion, OpenToonz `.tnz` scenes and `.pli` levels, and SVG and PNG or animated GIF artwork, as editable projects.
- Export PNG, GIF and MP4, and publish a SWF or an HTML5 Canvas page.
- A command line (`flare`) and an MCP server (`flare-mcp`) for agents, to create, edit, export and publish projects from scripts.
- A browser build of the same editor. See [docs/WEB.md](docs/WEB.md) for what it does differently.

[docs/SURFACES.md](docs/SURFACES.md) lists, feature by feature, what each of the desktop app, the browser build, the command line and the MCP server can do, and the gaps.

### Formats

- Flash/Animate: `.fla`, `.xfl`, `.swf`, and legacy binary `.fla` (imported as a skeleton). Save and export to `.fla`, `.xfl` and `.flare`.
- Moho: `.moho`, `.anime`, `.mohoproj`, `.animeproj` and BVH motion. Layers, bones, keyframes and vector art import as editable projects; images and sound are listed, not placed.
- OpenToonz: `.tnz` scenes and `.pli` vector levels import (frame rate, camera, xsheet columns, strokes, palette colours). `.tlv` raster levels are refused, and `.tzu` is planned.
- Artwork: SVG (one frame), PNG (one frame) and animated GIF import as bitmap layers. JPEG, WebP and PSD are not built.

### Platforms

| Target | State | Notes |
|---|---|---|
| Windows (desktop) | Builds from source | MSVC toolchain and `flare-desktop`. |
| Linux (desktop) | Builds from source | Needs the libraries in [CONTRIBUTING.md](CONTRIBUTING.md). Test Movie needs Ruffle built from the submodule. |
| macOS (desktop) | Builds from source | Native menu. Pen pressure does not reach the Brush on macOS yet. |
| Web (browser) | Live, early | The [web version](https://flare-animate.github.io/Flare/) is deployed from `main`. No MP4 export, no sound, and Test Movie runs in the page. |
| Android | Experimental | First build of `flare-android`, the desktop workspace in a native activity. Files cannot yet be opened or saved through the phone's picker, and Test Movie is off. |
| ChromeOS | Untested | The Android build makes x86_64 and arm64 APKs that should run there. Nothing has been tested on ChromeOS. |
| iOS | Not built | There is no iOS app. The iOS CI job only tries to compile and does not block. |

### Downloads

There is no stable release yet. Each night a build of `main` is published as a pre-release, the [nightly](https://github.com/Flare-Animate/Flare/releases/tag/nightly): Windows installers and portable zips, macOS disk images, Linux `.deb`, `.rpm`, `.AppImage`, `.flatpak` and `.tar.gz` packages, the command line archives, the Android APKs and the browser build. The nightly is rebuilt only when `main` has new commits, and keeps only the latest build. Its packages are not signed for distribution; see [docs/RELEASE.md](docs/RELEASE.md). Build from source (below) for anything else.

## Build and run

Rust is pinned in `rust-toolchain.toml`. Ruffle is a git submodule, so clone with submodules:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

On Linux install the libraries listed in [CONTRIBUTING.md](CONTRIBUTING.md).

[docs/BUILDING.md](docs/BUILDING.md) has the steps for each platform: Linux, Windows, macOS, the browser build and Android.

## Command line

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

## Why Flare exists

Adobe [announced](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) the end of life of Animate and later [reversed course](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). The reversal does not change how volatile the animation industry is, or how much of it one company controls. Animators were thrown into legitimate panic, and people's careers should not depend on one company's decision to keep selling a product.

Flare began as a playground for making animation easier to learn, was later open-sourced, and grew from there.

## Roadmap

The next things, most important first. Most of it is shown dimmed in the interface. [docs/ROADMAP.md](docs/ROADMAP.md) repeats this list and goes on to everything of lower priority.

- Merge drawing: Flash's planar-map shapes, where overlapping shapes cut and merge each other (today shapes are separate objects)
- Drawing quality: brush dynamics, and tracing a bitmap into vector shapes
- SWF that keeps more of the document: buttons with their states, shape tweens as morphs, masks of several shapes and sounds inside symbols
- A Motion Editor: per-property graphs and custom ease curves for tweens
- Importing artwork from SVG and Photoshop files
- Bones inside shapes and the Bind tool (bones that link symbol instances, posed by inverse kinematics with the Bone tool, are built)
- Editable keyboard shortcuts

Also planned, at lower priority: ActionScript and a script editor (ActionScript in an imported file is kept as data and never run), shape hints and more than one scene, the rest of Animate's tools, custom themes, a fully rearrangeable toolbar and a released web build.

## Documentation

[docs/README.md](docs/README.md) explains how Flare is built and says what each document covers.

Contributors should look at [CONTRIBUTING.md](CONTRIBUTING.md), [AGENTS.md](AGENTS.md), [SECURITY.md](SECURITY.md) and [CHANGELOG.md](CHANGELOG.md).

## The original codebase

[`original/`](original/README.md) is the archived C++ codebase, based on OpenToonz and Tahoma2D. It is kept as a reference only. Nothing in Flare depends on it, links to it or copies from it.

## Credits

Flare stands on these projects. Their licence notices are kept where code was ported or used.

- **OpenToonz** (BSD-3-Clause, Copyright (c) DWANGO Co., Ltd. and contributors): the base of the original codebase. The PLI reader in `crates/io/flare-toonz` is ported from it.
- **Tahoma2D** (BSD-3-Clause): an OpenToonz fork and the other base of the original codebase. Its licence and README were read; its tool sources were only compared.
- **AnimationCraft** (`MIT OR Apache-2.0`): the Rust authoring model Flare absorbed.
- **Next2Flash**: its MIT parts may be ported. Its FFDec worker is GPLv3, so only its behaviour is used.
- **jpdurigan/godot_moho_importer** (MIT, Copyright (c) 2022 Joao Pedro Durigan): the Moho mesh layout used by `crates/io/flare-moho`.
- **usvg** (MIT/Apache-2.0): the SVG reader, used as a dependency.
- **Ruffle** (`third_party/ruffle`, a submodule used unmodified under its own licence): Flash playback for Test Movie, and the `swf` crate for SWF publishing.
- **ArtCraft** (`MIT OR Apache-2.0`) and **minnamate** (no licence file, behaviour only) are studied sources. Their licences are listed in [docs/BACKLOG.md](docs/BACKLOG.md).
- **nflare** and **FFDec** (GPLv3): behaviour only, no code. **SimplSam/moho-bvh-bones** (CC BY-NC-SA 4.0): not used.

## Community

[Discord](https://discord.com/invite/JpeScW8Awa) · [Website](https://flare-animate.github.io/website/) · [Discussions](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## License

BSD 3-Clause. See [LICENSE](LICENSE). Third-party components keep their own licences.
