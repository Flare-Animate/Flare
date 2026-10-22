<div align="center">

# Flare

<img width="610" height="344" alt="Flare" src="docs/images/banner.svg" />

**Free, open-source animation suite with continuity for Adobe Animate and Flash users.**

[![Discord](https://img.shields.io/discord/1500316971802296430?label=Discord&logo=discord&logoColor=white&color=5865F2)](https://discord.com/invite/JpeScW8Awa)
[![Nightly](https://img.shields.io/badge/release-nightly-brightgreen)](https://github.com/Flare-Animate/Flare/releases/tag/nightly)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Status](https://img.shields.io/badge/status-early%20alpha-red)

[Website](https://flare-animate.github.io/website/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [Download](https://github.com/Flare-Animate/Flare/releases/tag/nightly) · [Issues](https://github.com/Flare-Animate/Flare/issues) · [日本語](./doc/README_ja.md) · [简体中文](./doc/README_chs.md)

</div>

![Flare main window](<img width="1200" height="675" alt="image" src="https://github.com/user-attachments/assets/3e2053cf-74d7-4b3e-8f0b-c6194577a13d" />)

> **Early alpha.** Expect bugs and incomplete Flash/Animate import fidelity. Back up your work.

# What is Flare?

## Introduction

Flare, also referred to as *FlareCraft*, is a (Work In Progress) Animation Suite, designed to make the transition from software like *Adobe Animate*, *Adobe/Macromedia Flash* and *Moho* easier for Artists and Animators. 

We are planning to do this by implementing support for all formats used by those tools (so users can use all the same project files as if nothing has changed), and redesigning the UI to have a more simplistic and familiar layout, which can also be further customised to match exactly how you want it. 

It is currently being rewritten in **Rust**, and is based predominantly on *OpenToonz* and *Tahoma*, with many features being merged in from *AnimationCraft* and *Next2Flash*, which have been absorbed into Flare.

## Why does this exist?

Well, for a few reasons.

First and foremost, due to the [Announcement](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) by Adobe earlier this year to discontinue Adobe Animate. While this was later [u-turned on](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459), in my opinion it shows how volatile the Animation Industry is and displays how much of a monopoly certain companies have on it. There was legitimate panic over it, and I think that people shouldn't have to be concerned for their careers just because one product isn't being sold anymore.

Secondly, it was for personal use, I used Flare in the first few months as a playground to experiment with making Animation easier for me to learn, so I could basically finetune it to what I liked  best, and I decided to Open Source it, to which many people liked it, and with one Discord and the announcement of ArtCraft later, interest in contributing to Flare went up quite a bit!

I had been slowly just experimenting with Flare with minimal focus over the previous months, but now that I had finished with another side-project, I decided to put more time into fleshing out Flare.

## Features

- Animate-style workspace with Drawing / Animation / Rigging / Compositing rooms (other room layouts under **Preferences > Interface > Rooms**).
- Vector and raster drawing, xsheet/timeline, plastic rigging, FX schematic, motion tracking, script console (inherited).
- Flash/Animate File Support: `.fla`, `.xfl`, `.swf`, `.swc`, `.flv`/`.f4v`, `.as`, legacy binary `.fla`, AIR/ZXP/MXP packages. Fidelity is partial; vector shapes are not yet converted to levels ([STATUS.md](docs/STATUS.md), [FLASH_SUPPORT.md](docs/FLASH_SUPPORT.md)).
- Moho project reading: structure, rig and asset manifest, no rendering atm ([MOHO_SUPPORT.md](docs/MOHO_SUPPORT.md)).

Full format table, repository layout, tests and roadmap: [docs/FEATURES.md](docs/FEATURES.md).

## Download

Nightly builds: **[latest nightly release](https://github.com/Flare-Animate/Flare/releases/tag/nightly)**

| Platform | Asset |
|----------|-------|
| Windows 10/11 x64 | `Flare-Windows-Portable-nightly.zip` |
| Linux x86_64 (glibc 2.35+) | `Flare-x86_64.AppImage` |
| macOS 13+ | `Flare.dmg` |
| FreeBSD | `Coming Soon` |
| Android | `Coming Soon` |
| Web | `Coming Soon` |


## Build

```sh
cmake -S flare/sources -B build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build -j2   # builds are memory-heavy; use -j1 below 4 GB RAM
```

Guides: [Windows](docs/how_to_build_win.md) · [macOS](docs/how_to_build_macosx.md) · [Linux](docs/how_to_build_linux.md) · [BSD](docs/how_to_build_bsd.md) · [local builds](docs/LOCAL_BUILD.md)

## Rust Rewrite Status

The C++ code is being ported to Rust incrementally: 329 tracked `.rs` files vs 1107 remaining `.c`/`.cpp` files under `flare/sources` (counted on `Rust-Rewrite`).
Details: [RUST_REWRITE.md](docs/RUST_REWRITE.md) · [PORTING.md](docs/PORTING.md) · [REWRITE_BACKLOG.md](docs/REWRITE_BACKLOG.md

## Is this Vibecoded?

Yes and No.

While it is true that Agents such as Claude have been used during Flare's development, and at times carrying the bulk of development, the project is built ontop of a human-written codebase and is overseen and, when we are able, worked on by people.

We actively try our best to put as much human output into the project as possible and are currently looking for more developers that can donate their time to the project.

## Contributing and community

See [CONTRIBUTING.md](CONTRIBUTING.md). [Discord](https://discord.com/invite/JpeScW8Awa) · [Website](https://flare-animate.github.io/website/) · [Discussions](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## License

Modified BSD, see [LICENSE.txt](LICENSE.txt). Third-party components keep their own licenses.

Based on OpenToonz / Tahoma2D (BSD-3), see [THIRD-PARTY-NOTICES.md](flare/sources/THIRD-PARTY-NOTICES.md) and [LICENSE.txt](LICENSE.txt).
