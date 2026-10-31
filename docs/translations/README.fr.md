[English](../../README.md) | [Español](README.es.md) | **Français** | [Deutsch](README.de.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare 🔥

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Un outil d'animation libre et open source, dans l'esprit d'Adobe Animate, écrit en Rust. Conçu pour Windows, macOS et Linux, avec une ligne de commande et un serveur MCP pour les scripts et les agents.

> **Alpha précoce.** Attendez-vous à des bogues et à une importation Flash/Animate incomplète. Sauvegardez votre travail.

<!-- Une capture d'écran de l'éditeur ira ici: assets/repository/flare-app.png -->

### Fonctionnalités

- Dessinez avec les outils habituels : Sélection, Sous-sélection, Transformation libre, Lasso et Lasso polygonal, Plume, Crayon, Pinceau à pointe, Gomme, Rectangle, Ovale, PolyStar, Ligne, Texte, Pot de peinture, Encrier et Pipette, avec remplissages, contours et dégradés.
- Organisez images et calques sur une scénarisation, avec dossiers, masques, guides de mouvement, interpolations de mouvement et de forme avec Ease, et pelure d'oignon.
- Des symboles (Graphique, Clip et Bouton) dans une bibliothèque, placés comme occurrences sur la scène.
- Lisez l'animation avec le son ; Tester l'animation exécute le SWF publié dans Ruffle (dans l'application sur le bureau, dans la page dans le navigateur).
- Plusieurs documents dans des onglets, panneaux ancrables, enregistrement automatique et récupération après une fermeture inattendue.
- Ouvrez et enregistrez des projets `.flare`. Importez et exportez le XFL et le `.fla` d'Animate, avec un rapport de ce qui n'a pas pu être repris.
- Exportez en PNG, GIF et MP4, et publiez un SWF ou une page HTML5 Canvas.
- Une ligne de commande (`flare`) et un serveur MCP (`flare-mcp`) pour que scripts et agents d'IA travaillent sur les mêmes documents en toute sécurité.

### Compiler et lancer

Rust est épinglé dans `rust-toolchain.toml`. Ruffle est un sous-module git ; clonez donc avec les sous-modules :

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Sous Linux, installez les bibliothèques listées dans [CONTRIBUTING.md](../../CONTRIBUTING.md).

Les versions produisent des paquets natifs pour macOS, Windows et Linux en x64 et arm64 : un DMG, un MSI et un zip portable, et des fichiers `.deb`, `.rpm` et AppImage, plus la ligne de commande dans une archive à part. Aucune version n'est encore publiée ; compilez donc depuis les sources pour l'instant. [docs/release.md](../release.md) décrit les paquets et la façon dont une version est faite.

### Ligne de commande et MCP

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

`flare-mcp project.flare --allow-write --export-dir exports` sert un projet à un agent d'IA via stdio. Il est en lecture seule sans `--allow-write`, les exports vont uniquement dans le dossier d'export, et chaque modification est une étape validée et annulable.

### Pas encore

Ne sont pas faits : les repères de forme, plus d'une scène, les scripts et le menu Commandes, les raccourcis clavier modifiables, la modification du texte une fois placé, le dessin par fusion (les formes sont des objets séparés, pas la carte planaire de Flash qui coupe et fusionne) et un calque de caméra. Ce qui est prévu mais pas construit est affiché estompé dans l'interface. La version web est expérimentale et non publiée. [docs/roadmap.md](../roadmap.md) dit ce qui vient ensuite.

### Documentation

[docs/README.md](../README.md) explique comment Flare est construit. Voir aussi [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md) pour les règles de travail, [SECURITY.md](../../SECURITY.md) et [CHANGELOG.md](../../CHANGELOG.md). Ces documents sont en anglais.

### Communauté

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Licence

BSD à 3 clauses. Voir [LICENSE](../../LICENSE).

---

*Cette traduction peut avoir du retard sur le [README anglais](../../README.md), qui fait foi.*
