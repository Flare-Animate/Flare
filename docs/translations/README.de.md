[English](../../README.md) | **Deutsch** | [Español](README.es.md) | [Français](README.fr.md) | [Italiano](README.it.md) | [Português (Brasil)](README.pt-BR.md) | [Čeština](README.cs.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Русский](README.ru.md) | [简体中文](README.zh-CN.md)

<div align="center">

# Flare

<img src="../../assets/BannerNew.png" width="969" height="261" alt="Das Flare-Banner">

**Freie Open-Source-Animationssuite mit Kontinuität für Nutzer von Adobe Animate und Flash.**

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Status](https://img.shields.io/badge/status-early%20alpha-red)

[Website](https://flare-animate.github.io/website/) · [Webversion](https://flare-animate.github.io/Flare/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [Bauen](../BUILDING.md) · [Issues](https://github.com/Flare-Animate/Flare/issues)

</div>

> **Frühe Alpha.** Rechnen Sie mit Fehlern und unvollständigem Flash/Animate-Import. Sichern Sie Ihre Arbeit.

## Was ist Flare?

Flare, auch *FlareCraft* genannt, ist eine (in Arbeit befindliche) Animationssuite, die den Wechsel von Software wie *Adobe Animate*, *Adobe/Macromedia Flash* und *Moho* für Zeichner und Animatoren erleichtern soll.

Ziel ist, die Formate dieser Werkzeuge zu unterstützen, damit Menschen ihre vorhandenen Projektdateien öffnen und weiterarbeiten können, und die Oberfläche einfacher und vertrauter aufzubauen, die sich anpassen lässt.

Flare wird in **Rust** neu geschrieben. Die Basis bilden überwiegend *OpenToonz* und *Tahoma2D*, mit Funktionen aus *AnimationCraft* und *Next2Flash*, die in Flare aufgegangen sind. Der alte C++-Code liegt als Archiv in [`original/`](../../original/README.md).

## Funktionen

- Zeichnen mit Stift-, Pinsel- und Formwerkzeugen, mit einem Auswahlwerkzeug zum Verschieben, Gruppieren und Bearbeiten von Formen und mit Strichstilen (durchgezogen, gestrichelt, gepunktet, Stipple, schraffiert, zerfranst, Fischgrät, Seil, Zickzack).
- Eine Zeitleiste mit Bildern, Ebenen, Ordnern, Masken, Bewegungsführungen, Onion Skin und einer Kameraebene.
- Bewegungs- und Formtweens mit Ease, und ein Motion Editor für die Segmentkurven.
- Symbole (Grafik, Movieclip und Schaltfläche) in einer Bibliothek, als Instanzen auf der Bühne platziert.
- Einstellungen für Stop-Motion-Sitzungen. Die Kameraaufnahme ist noch nicht gebaut.
- Tonwiedergabe auf dem Desktop und Film testen, das das veröffentlichte SWF in Ruffle ausführt.
- `.flare`-Projekte öffnen und speichern. XFL und `.fla` von Animate importieren und exportieren, und Flash-Filme (`.swf`) als bearbeitbare Projekte öffnen.
- Moho-Projekte und BVH-Bewegungen, OpenToonz-Szenen `.tnz` und -Level `.pli` sowie Grafiken und Filme (SVG, PNG, JPEG, GIF, WebP, PSD, MP4, MOV, AVI, MKV, WebM, MPEG, FLV) als bearbeitbare Projekte importieren.
- PNG, JPEG, GIF, APNG, animiertes WebP, SVG, PSD und MP4 exportieren, weitere Filmformate über ffmpeg, sowie ein SWF oder eine HTML5-Canvas-Seite veröffentlichen.
- Eine Kommandozeile (`flare`) und ein MCP-Server (`flare-mcp`) für Agenten, um Projekte aus Skripten zu erstellen, zu bearbeiten, zu exportieren und zu veröffentlichen.
- Eine Browserversion desselben Editors. Was sie anders macht, steht in [docs/WEB.md](../WEB.md).

[docs/SURFACES.md](../SURFACES.md) listet Funktion für Funktion auf, was die Desktop-App, die Browserversion, die Kommandozeile und der MCP-Server können, und wo die Lücken liegen.

### Formate

- Flash/Animate: `.fla`, `.xfl`, `.swf` und das binäre `.fla` der alten Version (als Gerüst importiert). Speichern und Exportieren als `.fla`, `.xfl` und `.flare`.
- Moho: `.moho`, `.anime`, `.mohoproj`, `.animeproj` und BVH-Bewegung. Ebenen, Knochen, Keyframes und Vektorgrafik werden als bearbeitbare Projekte importiert; Bilder und Ton werden aufgelistet, nicht platziert.
- OpenToonz: `.tnz`-Szenen und `.pli`-Vektorlevel werden importiert (Bildrate, Kamera, Xsheet-Spalten, Striche, Palettenfarben). `.tlv`-Rasterlevel werden abgelehnt, `.tzu` ist geplant.
- Grafiken: SVG, PNG, APNG, JPEG, GIF, WebP, TIFF, BMP, TGA, ICO, QOI, PNM, HDR, OpenEXR, DDS und PSD werden als Pixelebenen importiert (animiertes GIF, APNG und WebP als Animationen); SVG und PSD werden exportiert. AVIF und HEIC sind nicht gebaut.

### Plattformen

| Ziel | Stand | Hinweise |
|---|---|---|
| Windows (Desktop) | Aus dem Quellcode baubar | MSVC-Toolchain und `flare-desktop`. |
| Linux (Desktop) | Aus dem Quellcode baubar | Braucht die Bibliotheken aus [CONTRIBUTING.md](../../CONTRIBUTING.md). Film testen braucht Ruffle aus dem Submodul. |
| macOS (Desktop) | Aus dem Quellcode baubar | Natives Menü. Der Stiftdruck erreicht den Pinsel unter macOS noch nicht. |
| Web (Browser) | Live, früh | Die [Webversion](https://flare-animate.github.io/Flare/) wird aus `main` veröffentlicht. Kein MP4-Export, kein Ton, und Film testen läuft in der Seite. |
| Android | Experimentell | Erster Build von `flare-android`, der Desktop-Arbeitsbereich in einer nativen Activity. Dateien lassen sich über den Dateiwähler des Telefons noch nicht öffnen oder speichern, und Film testen ist aus. |
| ChromeOS | Ungetestet | Der Android-Build erzeugt APKs für x86_64 und arm64, die dort laufen sollten. Auf ChromeOS wurde nichts getestet. |
| iOS | Nicht gebaut | Es gibt keine iOS-App. Der iOS-CI-Job versucht nur das Kompilieren und blockiert nicht. |

### Downloads

Es gibt noch kein stabiles Release. Jede Nacht wird ein Build von `main` als Vorabversion veröffentlicht, das [Nightly-Release](https://github.com/Flare-Animate/Flare/releases/tag/nightly): Windows-Installer und portable ZIPs, macOS-Disk-Images, Linux-Pakete `.deb`, `.rpm`, `.AppImage`, `.flatpak` und `.tar.gz`, die Kommandozeilen-Archive, die Android-APKs und der Browser-Build. Das Nightly wird nur neu gebaut, wenn `main` neue Commits hat, und es behält nur den neuesten Build. Seine Pakete sind nicht für die Verteilung signiert; siehe [docs/RELEASE.md](../RELEASE.md). Für alles andere bauen Sie aus dem Quellcode (unten).

## Bauen und starten

Rust ist in `rust-toolchain.toml` festgelegt. Ruffle ist ein Git-Submodul, daher mit Submodulen klonen:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Unter Linux installieren Sie die Bibliotheken aus [CONTRIBUTING.md](../../CONTRIBUTING.md).

[docs/BUILDING.md](../BUILDING.md) enthält die Schritte für jede Plattform: Linux, Windows, macOS, die Browserversion und Android.

## Kommandozeile

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

## Warum es Flare gibt

Adobe hat das Ende von Animate [angekündigt](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) und später [zurückgenommen](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). Die Umkehr ändert nichts daran, wie unbeständig die Animationsbranche ist und wie viel davon ein Konzern kontrolliert. Animatoren gerieten in berechtigte Panik, und die Karriere von Menschen sollte nicht von der Entscheidung eines Unternehmens abhängen, ein Produkt weiter zu verkaufen.

Flare begann als Spielwiese, um Animation leichter lernbar zu machen, wurde später quelloffen und wuchs von dort.

## Roadmap

Die nächsten Punkte, das Wichtigste zuerst. Vieles davon ist in der Oberfläche abgeblendet zu sehen. [docs/ROADMAP.md](../ROADMAP.md) wiederholt diese Liste und behandelt danach alles mit geringerer Priorität.

- Zusammenführen beim Zeichnen: Flash-Planarkarten, bei denen sich überlappende Formen gegenseitig schneiden und verschmelzen (heute sind Formen getrennte Objekte)
- Zeichenqualität: Pinseldynamik und das Nachzeichnen einer Pixelgrafik zu Vektorformen
- SWF, das mehr vom Dokument erhält: Schaltflächen mit ihren Zuständen, Formtweens als Morphs, Masken aus mehreren Formen und Töne in Symbolen
- Ein Motion Editor: Graphen je Eigenschaft und eigene Ease-Kurven für Tweens
- Import von Grafiken aus SVG- und Photoshop-Dateien
- Bones in Formen und das Werkzeug Bind (Bones, die Symbolinstanzen verbinden und mit dem Werkzeug Bone über inverse Kinematik posiert werden, sind gebaut)
- Bearbeitbare Tastaturkürzel

Außerdem, mit geringerer Priorität geplant: ActionScript und ein Skripteditor (ActionScript in einer importierten Datei wird als Daten behalten und nie ausgeführt), Formhinweise und mehr als eine Szene, der Rest der Werkzeuge von Animate, eigene Designs, eine vollständig frei anordenbare Werkzeugleiste und eine veröffentlichte Webversion.

## Dokumentation

[docs/README.md](../README.md) erklärt, wie Flare gebaut ist, und sagt, was jedes Dokument abdeckt.

Mitwirkende sollten [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) und [CHANGELOG.md](../../CHANGELOG.md) lesen.

## Das ursprüngliche Projekt

[`original/`](../../original/README.md) ist der archivierte C++-Code, basierend auf OpenToonz und Tahoma2D. Er dient nur als Referenz. Nichts in Flare hängt davon ab oder verlinkt darauf. Daten und Texte daraus werden in Flares eigene Formate überführt und in [docs/ENGINEERING.md](../ENGINEERING.md) genannt.

## Danksagung

Flare steht auf den Schultern dieser Projekte. Ihre Lizenzhinweise bleiben dort erhalten, wo Code übernommen oder portiert wurde.

- **OpenToonz** (BSD-3-Clause, Copyright (c) DWANGO Co., Ltd. und Mitwirkende): die Basis des ursprünglichen Projekts. Der PLI-Leser in `crates/io/flare-toonz` ist daraus portiert. Seine Übersetzungsdateien bilden den Ausgangspunkt für die russische, tschechische und italienische Oberfläche.
- **Tahoma2D** (BSD-3-Clause): ein Fork von OpenToonz und die zweite Basis des ursprünglichen Projekts. Lizenz und README wurden gelesen; die Werkzeugquellen wurden nur verglichen.
- **AnimationCraft** (`MIT OR Apache-2.0`): das Rust-Autorenmodell, das Flare aufgenommen hat.
- **Next2Flash**: seine MIT-Teile dürfen portiert werden. Sein FFDec-Worker steht unter GPLv3, daher wird nur sein Verhalten übernommen.
- **jpdurigan/godot_moho_importer** (MIT, Copyright (c) 2022 Joao Pedro Durigan): das Moho-Netzlayout, das `crates/io/flare-moho` verwendet.
- **usvg** (MIT/Apache-2.0): der SVG-Leser, als Abhängigkeit verwendet.
- **Ruffle** (`third_party/ruffle`, ein Submodul, unverändert unter der eigenen Lizenz verwendet): Flash-Wiedergabe für Film testen und das Crate `swf` für die SWF-Veröffentlichung.
- **ArtCraft** (`MIT OR Apache-2.0`) und **minnamate** (keine Lizenzdatei, nur Verhalten) sind untersuchte Quellen. Ihre Lizenzen stehen in [docs/BACKLOG.md](../BACKLOG.md).
- **nflare** und **FFDec** (GPLv3): nur Verhalten, kein Code. **SimplSam/moho-bvh-bones** (CC BY-NC-SA 4.0): nicht verwendet.

## Community

[Discord](https://discord.com/invite/JpeScW8Awa) · [Website](https://flare-animate.github.io/website/) · [Diskussionen](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## Lizenz

BSD 3-Clause. Siehe [LICENSE](../../LICENSE). Drittkomponenten behalten ihre eigenen Lizenzen.
