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

- Zeichnen mit den Werkzeugen, die Animate hat, dazu eine erweiterte Werkzeugbibliothek, etwa Lineal (misst Abstand und Winkel) und Spiegel.
- Bilder und Ebenen auf einer Zeitleiste anordnen, mit Ordnern, Masken, Bewegungsführungen, Bewegungs- und Formtweens mit Ease, Onion Skin und einer Kameraebene.
- Symbole (Grafik, Movieclip und Schaltfläche) in einer Bibliothek, als Instanzen auf der Bühne platziert.
- Die Animation mit Ton abspielen; Film testen führt das veröffentlichte SWF in Ruffle aus.
- `.flare`-Projekte öffnen und speichern. XFL und `.fla` von Animate importieren und exportieren. SWF-Filme (`.swf`) als bearbeitbare Projekte öffnen.
- PNG, GIF und MP4 exportieren sowie ein SWF oder eine HTML5-Canvas-Seite veröffentlichen.
- Eine Kommandozeilenschnittstelle (`flare`), um Projekte aus Skripten zu erstellen, zu bearbeiten, zu exportieren und zu veröffentlichen.

### Downloads

Es wird Downloads für Windows, macOS und Linux geben, aber es gibt noch keine. Wir tragen sie hier ein, sobald das Packaging fertig ist. Bis dahin aus dem Quellcode bauen (siehe unten).

### Bauen und starten

Rust ist in `rust-toolchain.toml` festgelegt. Ruffle ist ein Git-Submodul, also mit Submodulen klonen:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Unter Linux die in [CONTRIBUTING.md](../../CONTRIBUTING.md) aufgeführten Bibliotheken installieren.

[docs/BUILDING.md](../BUILDING.md) beschreibt die Schritte für jede Plattform: Linux, Windows, macOS, den Browser-Build und Android.

### Kommandozeile

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Warum es Flare gibt

Adobe hat das Ende von Animate [angekündigt](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) und später [zurückgenommen](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). Auch wenn die Entscheidung zurückgenommen wurde, zeigt sie, wie unbeständig die Animationsbranche ist und wie sehr bestimmte Unternehmen sie beherrschen. Animatoren gerieten zu Recht in Panik. Die Laufbahn von Menschen sollte nicht davon abhängen, dass ein Unternehmen ein Produkt weiterverkauft.

Flare begann als Spielwiese, um Animation leichter erlernbar zu machen, wurde später als Open Source veröffentlicht und wuchs von dort aus.

### Roadmap

Als Nächstes, das Wichtigste zuerst. Das meiste ist in der Oberfläche abgedunkelt zu sehen. [docs/ROADMAP.md](../ROADMAP.md) wiederholt diese Liste und führt alles mit niedrigerer Priorität auf.

- Zeichnen mit Verschmelzung: Flashs planare Formen, bei denen sich überlappende Formen gegenseitig schneiden und verschmelzen (heute sind Formen getrennte Objekte)
- Zeichenqualität: Pinseldynamik und das Nachzeichnen einer Bitmap in Vektorformen
- SWF, das mehr vom Dokument behält: Schaltflächen mit ihren Zuständen, Formtweens als Morphs, Masken aus mehreren Formen und Töne in Symbolen
- Ein Motion Editor: Diagramme je Eigenschaft und eigene Ease-Kurven für Tweens
- Grafiken aus SVG- und Photoshop-Dateien importieren
- Bones und inverse Kinematik (die Werkzeuge Bone und Bind)
- Bearbeitbare Tastenkürzel

Ebenfalls geplant, mit niedrigerer Priorität: ActionScript und ein Skripteditor (ActionScript in einer importierten Datei bleibt als Daten erhalten und wird nie ausgeführt), Formmarken und mehrere Szenen, die übrigen Werkzeuge von Animate, eigene Designs, eine frei anordenbare Werkzeugleiste und eine veröffentlichte Web-Version.

### Dokumentation

[docs/README.md](../README.md) erklärt, wie Flare aufgebaut ist.

Mitwirkende sollten [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) und [CHANGELOG.md](../../CHANGELOG.md) lesen. Diese Dokumente sind auf Englisch.

### Community

[Discord](https://discord.com/invite/JpeScW8Awa) · [Website](https://flare-animate.github.io/website/) · [Discussions](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Lizenz

BSD 3-Clause. Siehe [LICENSE](../../LICENSE).

---

*Diese Übersetzung kann hinter dem [englischen README](../../README.md) zurückliegen, das maßgeblich ist.*
