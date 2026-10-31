**English** | [Deutsch](docs/translations/README.de.md) | [Español](docs/translations/README.es.md) | [Français](docs/translations/README.fr.md) | [Português (Brasil)](docs/translations/README.pt-BR.md) | [日本語](docs/translations/README.ja.md) | [한국어](docs/translations/README.ko.md) | [简体中文](docs/translations/README.zh-CN.md)

<p align="center">
  <img src="assets/flare-icon-256.png" width="128" height="128" alt="The Flare flame">
</p>

# Flare Animate

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

A free, open-source animation tool in the spirit of Adobe Animate, written in Rust. Built for Windows, macOS and Linux.

> **Early alpha.** Expect bugs and incomplete Flash/Animate import fidelity. Back up your work.

<!-- A screenshot of the editor goes here: assets/repository/flare-app.png -->

### Features

- Draw with the tools Animate has, plus an extended library of tools such as Ruler (measures distance and angle) and Mirror. 
- Lay out frames and layers on a timeline, with folders, masks, motion guides, motion and shape tweens with Ease, and onion skin.
- Symbols (Graphic, Movie Clip and Button) in a library, placed as instances on the stage.
- Play the animation with sound, and Test Movie runs the published SWF in Ruffle.
- Open and save `.flare` projects. Import and export Animate's XFL and .fla
- Export PNG, GIF and MP4, and publish a SWF or an HTML5 Canvas page.
- A command line (`flare`) to create, edit, export and publish projects from scripts.

### Build and run

Rust is pinned in `rust-toolchain.toml`. Ruffle is a git submodule, so clone with submodules:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

On Linux install the libraries listed in [CONTRIBUTING.md](CONTRIBUTING.md).

Releases build native packages for macOS, Windows and Linux on x64 and arm64: a DMG, an MSI and a portable zip, and `.deb`, `.rpm` and AppImage files, plus the command line as its own archive. No version has been released yet, so for now build from source. [docs/release.md](docs/release.md) describes the packages and how a release is made.

### Command line

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Roadmap

Planned but not built yet. Most of it is shown dimmed in the interface. [docs/roadmap.md](docs/roadmap.md) has the order and the reasons.

- Merge drawing: Flash's planar-map shapes, where overlapping shapes cut and merge each other (today shapes are separate objects)
- Bones and inverse kinematics (the Bone and Bind tools)
- The rest of Animate's tools: 3D Rotation and Translation, Deco, Width, Asset Warp, Gradient Transform and Magic Wand
- Shape hints, more than one scene, and scripting with the Commands menu
- A Flash 8 interface skin, and custom themes
- A fully rearrangeable toolbar (tools can already be pinned from the "…" list) and editable keyboard shortcuts
- Editing text after it is placed
- Sharing from the Share menu to YouTube and other services
- A released web build (it is experimental today)

### Documentation

[docs/README.md](docs/README.md) explains how Flare is built. 

Contributors should look at [CONTRIBUTING.md](CONTRIBUTING.md), [AGENTS.md](AGENTS.md), [SECURITY.md](SECURITY.md) and [CHANGELOG.md](CHANGELOG.md).

### Community

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### License

BSD 3-Clause. See [LICENSE](LICENSE).
