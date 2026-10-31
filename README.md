[**English**](README.md) | [Español](docs/translations/README.es.md) | [Français](docs/translations/README.fr.md) | [Deutsch](docs/translations/README.de.md) | [日本語](docs/translations/README.ja.md) | [简体中文](docs/translations/README.zh-CN.md)

<p align="center">
  <img src="assets/flare-icon-256.png" width="128" height="128" alt="The Flare flame">
</p>

# Flare 🔥

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

A free, open-source animation tool in the spirit of Adobe Animate, written in Rust. Built for Windows, macOS and Linux, with a command line and an MCP server for scripts and agents.

> **Early alpha.** Expect bugs and incomplete Flash/Animate import fidelity. Back up your work.

<!-- A screenshot of the editor goes here: assets/repository/flare-app.png -->

### Features

- Draw with the familiar tools: Selection, Subselection, Free Transform, Lasso and Polygon lasso, Pen, Pencil, Brush with a nib, Eraser, Rectangle, Oval, PolyStar, Line, Text, Paint Bucket, Ink Bottle and Eyedropper, with fills, strokes and gradients.
- Lay out frames and layers on a timeline, with folders, masks, motion guides, motion and shape tweens with Ease, and onion skin.
- Symbols (Graphic, Movie Clip and Button) in a library, placed as instances on the stage.
- Play the animation with sound, and Test Movie runs the published SWF in Ruffle (inside the app on the desktop, in the page in the browser).
- Several documents as tabs, dockable panels, autosave and crash recovery.
- Open and save `.flare` projects. Import and export Animate's XFL and `.fla`, with what could not be carried over reported.
- Export PNG, GIF and MP4, and publish a SWF or an HTML5 Canvas page.
- A command line (`flare`) and an MCP server (`flare-mcp`) so scripts and AI agents can work on the same documents safely.

### Build and run

Rust is pinned in `rust-toolchain.toml`. Ruffle is a git submodule, so clone with submodules:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

On Linux install the libraries listed in [CONTRIBUTING.md](CONTRIBUTING.md).

Releases build native packages for macOS, Windows and Linux on x64 and arm64: a DMG, an MSI and a portable zip, and `.deb`, `.rpm` and AppImage files, plus the command line as its own archive. No version has been released yet, so for now build from source. [docs/release.md](docs/release.md) describes the packages and how a release is made.

### Command line and MCP

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

`flare-mcp project.flare --allow-write --export-dir exports` serves a project to an AI agent over stdio. It is read-only without `--allow-write`, exports go only into the export directory, and every change is one validated, undoable step.

### Not yet

Shape hints, more than one scene, scripting and the Commands menu, editable keyboard shortcuts, editing text after it is placed, merge drawing (shapes are separate objects, not Flash's cutting and merging planar map), and a camera layer are not built. What is planned but not built is shown dimmed in the interface. The web build is experimental and not released. [docs/roadmap.md](docs/roadmap.md) says what is next.

### Documentation

[docs/README.md](docs/README.md) explains how Flare is built. See also [CONTRIBUTING.md](CONTRIBUTING.md), [AGENTS.md](AGENTS.md) for the rules of working here, [SECURITY.md](SECURITY.md) and [CHANGELOG.md](CHANGELOG.md).

### Community

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### License

BSD 3-Clause. See [LICENSE](LICENSE).
