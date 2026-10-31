[English](../../README.md) | **Deutsch** | [Español](README.es.md) | [Français](README.fr.md) | [Português (Brasil)](README.pt-BR.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare Animate

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Ein freies Open-Source-Animationswerkzeug im Sinne von Adobe Animate, geschrieben in Rust. Gebaut für Windows, macOS und Linux.

> **Frühe Alpha.** Rechnen Sie mit Fehlern und unvollständigem Flash/Animate-Import. Sichern Sie Ihre Arbeit.

<!-- Hier kommt ein Screenshot des Editors hin: assets/repository/flare-app.png -->

### Funktionen

- Zeichnen mit den Werkzeugen, die Animate hat, dazu Flares eigenes Lineal (misst Abstand und Winkel) und Spiegel (zeichnet symmetrisch), mit Füllungen, Konturen und Verläufen.
- Bilder und Ebenen auf einer Zeitleiste anordnen, mit Ordnern, Masken, Bewegungsführungen, Bewegungs- und Formtweens mit Ease sowie Onion Skin.
- Symbole (Grafik, Movieclip und Schaltfläche) in einer Bibliothek, als Instanzen auf der Bühne platziert.
- Die Animation mit Ton abspielen; Film testen führt das veröffentlichte SWF in Ruffle aus (auf dem Desktop in der App, im Browser auf der Seite).
- Mehrere Dokumente als Tabs, andockbare Bedienfelder, automatisches Speichern und Wiederherstellung nach einem Absturz.
- `.flare`-Projekte öffnen und speichern. XFL und `.fla` von Animate importieren und exportieren, mit einem Bericht darüber, was nicht übernommen werden konnte.
- PNG, GIF und MP4 exportieren sowie ein SWF oder eine HTML5-Canvas-Seite veröffentlichen.
- Eine Kommandozeile (`flare`), um Projekte aus Skripten zu erstellen, zu bearbeiten, zu exportieren und zu veröffentlichen.

### Bauen und starten

Rust ist in `rust-toolchain.toml` festgelegt. Ruffle ist ein Git-Submodul, also mit Submodulen klonen:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Unter Linux die in [CONTRIBUTING.md](../../CONTRIBUTING.md) aufgeführten Bibliotheken installieren.

Releases erzeugen native Pakete für macOS, Windows und Linux auf x64 und arm64: ein DMG, ein MSI und ein portables Zip sowie `.deb`-, `.rpm`- und AppImage-Dateien, dazu die Kommandozeile als eigenes Archiv. Es wurde noch keine Version veröffentlicht; bauen Sie daher vorerst aus dem Quellcode. [docs/release.md](../release.md) beschreibt die Pakete und wie ein Release entsteht.

### Kommandozeile

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Roadmap

Geplant, aber noch nicht gebaut. Das meiste ist in der Oberfläche abgedunkelt zu sehen. [docs/roadmap.md](../roadmap.md) nennt Reihenfolge und Gründe.

- Zeichnen mit Verschmelzung: Flashs planare Formen, bei denen sich überlappende Formen gegenseitig schneiden und verschmelzen (heute sind Formen getrennte Objekte)
- Eine Kameraebene mit Keyframes für Position, Zoom und Drehung
- Bones und inverse Kinematik (die Werkzeuge Bone und Bind)
- Die übrigen Werkzeuge von Animate: 3D-Drehung und -Verschiebung, Deco, Breite, Asset Warp, Verlauf transformieren und Zauberstab
- Formmarken, mehr als eine Szene und Skripting mit dem Menü Befehle
- Eine Flash-8-Oberfläche und eigene Designs (Dunkel und Hell sind eingebaut)
- Eine frei anordenbare Werkzeugleiste (Werkzeuge lassen sich schon aus der „…“-Liste anheften) und bearbeitbare Tastenkürzel
- Text bearbeiten, nachdem er platziert wurde
- SWF-Veröffentlichung, die Symbole, Schaltflächen und Tweens behält, statt jedes Bild einzubrennen
- Teilen aus dem Menü Teilen zu YouTube und anderen Diensten
- Eine veröffentlichte Web-Version (heute experimentell)
- Developer-ID-Signierung und Notarisierung unter macOS sowie automatische Updates

### Dokumentation

[docs/README.md](../README.md) erklärt, wie Flare aufgebaut ist. Siehe auch [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md) mit den Arbeitsregeln, [SECURITY.md](../../SECURITY.md) und [CHANGELOG.md](../../CHANGELOG.md). Diese Dokumente sind auf Englisch.

### Community

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Lizenz

BSD 3-Clause. Siehe [LICENSE](../../LICENSE).

---

*Diese Übersetzung kann hinter dem [englischen README](../../README.md) zurückliegen, das maßgeblich ist.*
