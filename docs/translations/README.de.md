# Flare

Ein freies Open-Source-Animationswerkzeug im Stil von Adobe Animate, geschrieben in Rust.

> **Frühe Alpha.** Fehler und ein unvollständiger Flash/Animate-Import sind zu erwarten. Sichern Sie Ihre Arbeit.

[English](../../README.md) · [Español](README.es.md) · [Français](README.fr.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md)

## Was Flare kann

- Mit den vertrauten Werkzeugen zeichnen, Bilder und Ebenen auf einer Zeitleiste anordnen und Tweens entlang von Bewegungsführungen erstellen.
- `.flare`-Projekte öffnen und speichern; XFL/FLA von Animate importieren und exportieren; PNG und GIF exportieren; SWF veröffentlichen.
- Mehrere Dokumente als Tabs, andockbare Panels, automatisches Speichern und Wiederherstellung nach einem unerwarteten Ende.
- Ein Kommandozeilenwerkzeug (`flare-cli`) und ein MCP-Server (`flare-mcp`), damit Skripte und KI-Agenten sicher an denselben Dokumenten arbeiten können.

## Bauen und starten

Rust ist in `rust-toolchain.toml` festgelegt. Unter Linux die in [CONTRIBUTING.md](../../CONTRIBUTING.md) genannten Bibliotheken installieren.

```
cargo run -p flare-desktop
cargo run -p flare-cli -- --help
```

Release-Builds und Paketierung sind in [docs/release.md](../release.md) beschrieben.

## Dokumentation

[docs/README.md](../README.md) erklärt, wie Flare aufgebaut ist. Siehe auch [CONTRIBUTING.md](../../CONTRIBUTING.md), [SECURITY.md](../../SECURITY.md) und [CHANGELOG.md](../../CHANGELOG.md). Diese Dokumente sind auf Englisch.

## Community

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## Lizenz

Siehe [LICENSE](../../LICENSE).

---

*Diese Übersetzung kann der [englischen README](../../README.md) hinterherhinken; maßgeblich ist das Englische.*
