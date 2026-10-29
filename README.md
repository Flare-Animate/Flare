# Flare

A free, open-source animation tool in the spirit of Adobe Animate, written in Rust.

> **Early alpha.** Expect bugs and incomplete Flash/Animate import fidelity. Back up your work.

Translations: [Español](docs/translations/README.es.md) · [Français](docs/translations/README.fr.md) · [Deutsch](docs/translations/README.de.md) · [日本語](docs/translations/README.ja.md) · [简体中文](docs/translations/README.zh-CN.md)

## What it does

- Draw with the familiar tools, lay out frames and layers on a timeline, tween along motion guides.
- Open and save `.flare` projects; import and export Animate's XFL/FLA; export PNG and GIF; publish SWF.
- Several documents as tabs, dockable panels, autosave and crash recovery.
- A command-line tool (`flare-cli`) and an MCP server (`flare-mcp`) so scripts and AI agents can work on the same documents safely.

## Build and run

Rust is pinned in `rust-toolchain.toml`. On Linux install the libraries listed in
[CONTRIBUTING.md](CONTRIBUTING.md).

```
cargo run -p flare-desktop
cargo run -p flare-cli -- --help
```

Packaging is described in [docs/release.md](docs/release.md).

## Documentation

[docs/README.md](docs/README.md) explains how Flare is built. See also
[CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md) and [CHANGELOG.md](CHANGELOG.md).

## Community

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## License

See [LICENSE](LICENSE).
