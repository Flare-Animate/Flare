[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | **Français** | [Português (Brasil)](README.pt-BR.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare Animate

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Un outil d'animation libre et open source, dans l'esprit d'Adobe Animate, écrit en Rust. Conçu pour Windows, macOS et Linux.

> **Alpha précoce.** Attendez-vous à des bogues et à une importation Flash/Animate incomplète. Sauvegardez votre travail.

<!-- Une capture d'écran de l'éditeur ira ici: assets/repository/flare-app.png -->

### Fonctionnalités

- Dessinez avec les outils d'Animate, plus une bibliothèque élargie d'outils, comme la Règle (mesure distance et angle) et le Miroir.
- Organisez images et calques sur une scénario, avec dossiers, masques, guides de mouvement, interpolations de mouvement et de forme avec Ease, et pelure d'oignon.
- Des symboles (Graphique, Clip et Bouton) dans une bibliothèque, placés comme occurrences sur la scène.
- Lisez l'animation avec le son ; Tester l'animation exécute le SWF publié dans Ruffle.
- Ouvrez et enregistrez des projets `.flare`. Importez et exportez le XFL et le `.fla` d'Animate.
- Exportez en PNG, GIF et MP4, et publiez un SWF ou une page HTML5 Canvas.
- Une ligne de commande (`flare`) pour créer, modifier, exporter et publier des projets depuis des scripts.

### Compiler et lancer

Rust est épinglé dans `rust-toolchain.toml`. Ruffle est un sous-module git ; clonez donc avec les sous-modules :

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Sous Linux, installez les bibliothèques listées dans [CONTRIBUTING.md](../../CONTRIBUTING.md).

Les versions produisent des paquets natifs pour macOS, Windows et Linux en x64 et arm64 : un DMG, un MSI et un zip portable, et des fichiers `.deb`, `.rpm` et AppImage, plus la ligne de commande dans une archive à part. Aucune version n'est encore publiée ; compilez donc depuis les sources pour l'instant. [docs/release.md](../release.md) décrit les paquets et la façon dont une version est faite.

### Ligne de commande

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Feuille de route

Prévu mais pas encore construit. La plupart est affiché estompé dans l'interface. [docs/roadmap.md](../roadmap.md) donne l'ordre et les raisons.

- Dessin par fusion : les formes à carte planaire de Flash, où les formes qui se chevauchent se découpent et fusionnent (aujourd'hui les formes sont des objets séparés)
- Armatures et cinématique inverse (les outils Os et Lier)
- Le reste des outils d'Animate : Rotation 3D et Translation 3D, Deco, Largeur, Asset Warp, Transformation de dégradé et Baguette magique
- Repères de forme, plus d'une scène, et les scripts avec le menu Commandes
- Un habillage d'interface Flash 8 et des thèmes personnalisés
- Une barre d'outils entièrement réorganisable (on peut déjà épingler des outils depuis la liste « … ») et des raccourcis clavier modifiables
- La modification du texte une fois placé
- Le partage depuis le menu Partager vers YouTube et d'autres services
- Une version web publiée (aujourd'hui expérimentale)

### Documentation

[docs/README.md](../README.md) explique comment Flare est construit.

Les contributeurs devraient lire [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) et [CHANGELOG.md](../../CHANGELOG.md). Ces documents sont en anglais.

### Communauté

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Licence

BSD à 3 clauses. Voir [LICENSE](../../LICENSE).

---

*Cette traduction peut avoir du retard sur le [README anglais](../../README.md), qui fait foi.*
