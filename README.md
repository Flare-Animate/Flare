# Flare

<img width="663" height="400" alt="FlareBanner" src="https://github.com/user-attachments/assets/73e4050f-f248-419e-8e61-fe8814ab984d" />


A fork of OpenToonz rebranded as Flare — focused on providing an Adobe Animate-like
user experience and improving interoperability with Flash assets (.swf/.fla).

This repository is a fork of OpenToonz and retains the original licensing and
attribution. See the Licensing section below for details.

[日本語](./doc/README_ja.md) [简体中文](./doc/README_chs.md)

## What is Flare?

Flare is a community-driven fork of OpenToonz that ships a revamped UI layout
inspired by Adobe Animate and introduces native import support for SWF files
(when FFmpeg is available) and experimental helpers for FLA projects.

For the original Flare project and its history, see the Flare website:
https://Flare.github.io/e/index.html

## Program Requirements

To enable SWF import features, install FFmpeg and ensure it is available on your PATH.

Flare uses CMake as its build system.  You must have CMake (version 3.10 or later)
installed and available on your PATH before attempting to configure or build the
project.  On Windows you can install CMake via the official installer or
[Chocolatey](https://chocolatey.org/).  On macOS use Homebrew (`brew install cmake`),
and on Linux use your distribution's package manager (`apt`, `dnf`, etc.).

## Installation

Please see the `doc/` folder for platform-specific build and installation
instructions.

## How to Build Locally

You can configure a build directory from the repository root with a command such as:

```sh
cmake -S flare/sources -B build -G "Ninja" -DCMAKE_BUILD_TYPE=Release
```

and then build:

```sh
cmake --build build --parallel
```

For more detailed, platform‑specific guidance follow the links below:

- [Windows](./doc/how_to_build_win.md)
- [macOS](./doc/how_to_build_macosx.md)
- [Linux](./doc/how_to_build_linux.md)
- [BSD](./doc/how_to_build_bsd.md)

## Community & Contribution

This fork aims to stay compatible with OpenToonz where possible while
introducing new features. When contributing, please keep the original project's
licensing and attribution in mind.

## Licensing

Files outside of the `thirdparty` and `stuff/library/mypaint brushes`
directories are based on the Modified BSD License.
- [modified BSD license](./LICENSE.txt).

Third-party components retain their original licenses. See the relevant
documentation in `thirdparty/` and `stuff/library/mypaint brushes/Licenses.txt`.

### Adobe Animate-style Workspace & Theme

Flare includes an "Adobe Animate" workspace and an Adobe-like color theme by
default. To switch to the Adobe Animate workspace, open the Room (Workspace)
menu and choose "Adobe Animate".

### Importing SWF files

Flare uses FFmpeg to import `.swf` files as raster frame sequences when
FFmpeg supports the format on your system. Install FFmpeg and ensure it is
available on your PATH (or configure it via Preferences) to enable SWF import
capabilities.

Flare also provides a Vector Flash import workflow that leverages third-party
Flash decompilers (e.g., JPEXS). Use File → Import → Import Flash (Vector via
External Decompiler)... to run a helper script that exports SVG/image sequences
from Flash files for importing into Flare. See `doc/how_to_import_swf.md` for
details.

## Development helper scripts

To make it easier to follow build and test output you can run the included
`log_watcher.py` script.  It watches `*.log` files underneath the build
directory and prints the last few lines whenever they are modified.  This is
also the script that the autonomous chat mode will launch automatically:

```sh
python scripts/log_watcher.py    # defaults to flare/build
```

You can run the same command via the "watch logs" task in VS Code (`Ctrl+Shift+B`).

