[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | **Français** | [Português (Brasil)](README.pt-BR.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [简体中文](README.zh-CN.md)

<div align="center">

# Flare

<img src="../../assets/BannerNew.png" width="969" height="261" alt="La bannière de Flare">

**Suite d'animation libre et open source, avec continuité pour les utilisateurs d'Adobe Animate et de Flash.**

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Status](https://img.shields.io/badge/status-early%20alpha-red)

[Site web](https://flare-animate.github.io/website/) · [Version web](https://flare-animate.github.io/Flare/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [Compiler](../BUILDING.md) · [Issues](https://github.com/Flare-Animate/Flare/issues)

</div>

> **Alpha précoce.** Attendez-vous à des bugs et à un import Flash/Animate incomplet. Sauvegardez votre travail.

## Qu'est-ce que Flare ?

Flare, aussi appelé *FlareCraft*, est une suite d'animation (en cours de développement) conçue pour faciliter le passage depuis des logiciels comme *Adobe Animate*, *Adobe/Macromedia Flash* et *Moho* pour les artistes et les animateurs.

Le but est de prendre en charge les formats de ces outils, afin que les personnes puissent ouvrir et poursuivre leurs fichiers de projet existants, et de proposer une interface plus simple et plus familière, personnalisable.

Flare est en cours de réécriture en **Rust**. Il s'appuie surtout sur *OpenToonz* et *Tahoma2D*, avec des fonctions reprises de *AnimationCraft* et *Next2Flash*, qui ont été intégrées à Flare. L'ancien code C++ est conservé comme archive dans [`original/`](../../original/README.md).

## Fonctions

- Dessin avec les outils stylo, pinceau et formes, avec un outil de sélection pour déplacer, grouper et modifier les formes, et des styles de trait (plein, tireté, pointillé, stipple, hachuré, irrégulier, chevrons, corde, zigzag).
- Une chronologie avec images, calques, dossiers, masques, guides de mouvement, pelure d'oignon et un calque caméra.
- Interpolations de mouvement et de forme avec easing, et un Motion Editor pour les courbes de segments.
- Symboles (Graphique, Clip animé et Bouton) dans une bibliothèque, placés en instances sur la scène.
- Réglages des sessions de stop motion. La capture par caméra n'est pas encore construite.
- Lecture du son sur le bureau, et Tester le film, qui exécute le SWF publié dans Ruffle.
- Ouverture et enregistrement de projets `.flare`. Import et export des XFL et `.fla` d'Animate, et ouverture de films Flash (`.swf`) comme projets modifiables.
- Import de projets Moho et de mouvements BVH, de scènes `.tnz` et de niveaux `.pli` d'OpenToonz, et d'images SVG, PNG ou GIF animé, comme projets modifiables.
- Export en PNG, GIF et MP4, et publication d'un SWF ou d'une page HTML5 Canvas.
- Une ligne de commande (`flare`) et un serveur MCP (`flare-mcp`) pour les agents, pour créer, modifier, exporter et publier des projets depuis des scripts.
- Une version navigateur du même éditeur. Voir [docs/WEB.md](../WEB.md) pour ce qu'elle fait différemment.

[docs/SURFACES.md](../SURFACES.md) énumère, fonction par fonction, ce que peuvent faire l'application de bureau, la version navigateur, la ligne de commande et le serveur MCP, ainsi que les lacunes.

### Formats

- Flash/Animate : `.fla`, `.xfl`, `.swf` et `.fla` binaire ancien (importé comme squelette). Enregistrement et export en `.fla`, `.xfl` et `.flare`.
- Moho : `.moho`, `.anime`, `.mohoproj`, `.animeproj` et mouvement BVH. Calques, os, images clés et dessins vectoriels s'importent comme projets modifiables ; les images et le son sont listés, non placés.
- OpenToonz : les scènes `.tnz` et les niveaux vectoriels `.pli` s'importent (cadence, caméra, colonnes du xsheet, traits, couleurs de palette). Les niveaux raster `.tlv` sont refusés, et `.tzu` est prévu.
- Images : SVG (une image), PNG (une image) et GIF animé s'importent comme calques bitmap. JPEG, WebP et PSD ne sont pas construits.

### Plateformes

| Cible | État | Remarques |
|---|---|---|
| Windows (bureau) | Se compile depuis les sources | Chaîne MSVC et `flare-desktop`. |
| Linux (bureau) | Se compile depuis les sources | Nécessite les bibliothèques de [CONTRIBUTING.md](../../CONTRIBUTING.md). Tester le film nécessite Ruffle compilé depuis le sous-module. |
| macOS (bureau) | Se compile depuis les sources | Menu natif. La pression du stylet n'atteint pas encore le pinceau sous macOS. |
| Web (navigateur) | En ligne, précoce | La [version web](https://flare-animate.github.io/Flare/) est publiée depuis `main`. Pas d'export MP4, pas de son, et Tester le film s'exécute dans la page. |
| Android | Expérimental | Première version de `flare-android`, l'espace de travail du bureau dans une activité native. Les fichiers ne peuvent pas encore être ouverts ni enregistrés via le sélecteur du téléphone, et Tester le film est désactivé. |
| ChromeOS | Non testé | La version Android produit des APK x86_64 et arm64 qui devraient y fonctionner. Rien n'a été testé sur ChromeOS. |
| iOS | Non construit | Il n'existe pas d'application iOS. La tâche CI iOS tente seulement de compiler et ne bloque pas. |

### Téléchargements

Il n'existe pas encore de version stable. Chaque nuit, une compilation de `main` est publiée en préversion, la [version nightly](https://github.com/Flare-Animate/Flare/releases/tag/nightly) : installeurs Windows et archives ZIP portables, images disque macOS, paquets Linux `.deb`, `.rpm`, `.AppImage`, `.flatpak` et `.tar.gz`, archives de la ligne de commande, APK Android et version navigateur. La version nightly n'est recompilée que lorsque `main` a de nouveaux commits, et ne conserve que la dernière compilation. Ses paquets ne sont pas signés pour la distribution ; voir [docs/RELEASE.md](../RELEASE.md). Pour le reste, compilez depuis les sources (ci-dessous).

## Compiler et lancer

Rust est fixé dans `rust-toolchain.toml`. Ruffle est un sous-module git, donc clonez avec les sous-modules :

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Sous Linux, installez les bibliothèques listées dans [CONTRIBUTING.md](../../CONTRIBUTING.md).

[docs/BUILDING.md](../BUILDING.md) détaille les étapes pour chaque plateforme : Linux, Windows, macOS, la version navigateur et Android.

## Ligne de commande

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

## Pourquoi Flare existe

Adobe a [annoncé](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) la fin de vie d'Animate, puis a [fait marche arrière](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). Ce revirement ne change rien à la volatilité de l'industrie de l'animation ni à la part que contrôle une seule entreprise. Les animateurs ont été saisis d'une panique légitime, et la carrière de quiconque ne devrait pas dépendre de la décision d'une entreprise de continuer à vendre un produit.

Flare a commencé comme un terrain d'essai pour rendre l'animation plus facile à apprendre, puis il a été publié en open source et s'est développé à partir de là.

## Feuille de route

Les prochaines étapes, par ordre d'importance. La plupart apparaissent en grisé dans l'interface. [docs/ROADMAP.md](../ROADMAP.md) reprend cette liste et poursuit avec tout ce qui est de moindre priorité.

- Fusion du dessin : les formes à carte planaire de Flash, où les formes qui se chevauchent se coupent et fusionnent (aujourd'hui les formes sont des objets séparés)
- Qualité de dessin : dynamique de pinceau, et tracé d'une image bitmap en formes vectorielles
- SWF qui conserve davantage du document : boutons avec leurs états, interpolations de forme en morphing, masques de plusieurs formes et sons dans les symboles
- Un Motion Editor : graphiques par propriété et courbes d'easing personnalisées pour les interpolations
- Import d'images depuis des fichiers SVG et Photoshop
- Os dans les formes et outil Lier (les os qui relient des occurrences de symboles, posés par cinématique inverse avec l’outil Os, sont faits)
- Raccourcis clavier modifiables

Également prévu, en priorité moindre : ActionScript et un éditeur de scripts (l'ActionScript d'un fichier importé est conservé comme donnée et jamais exécuté), des indices de forme et plusieurs scènes, le reste des outils d'Animate, des thèmes personnalisés, une barre d'outils entièrement réorganisable et une version web publiée.

## Documentation

[docs/README.md](../README.md) explique comment Flare est construit et ce que couvre chaque document.

Les contributeurs doivent consulter [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) et [CHANGELOG.md](../../CHANGELOG.md).

## Le code d'origine

[`original/`](../../original/README.md) est le code C++ archivé, basé sur OpenToonz et Tahoma2D. Il ne sert que de référence. Rien dans Flare n'en dépend, ne le lie ni n'en copie quoi que ce soit.

## Crédits

Flare repose sur ces projets. Leurs mentions de licence sont conservées là où du code a été porté ou utilisé.

- **OpenToonz** (BSD-3-Clause, Copyright (c) DWANGO Co., Ltd. et contributeurs) : la base du code d'origine. Le lecteur PLI de `crates/io/flare-toonz` en est porté.
- **Tahoma2D** (BSD-3-Clause) : un fork d'OpenToonz et la seconde base du code d'origine. Sa licence et son README ont été lus ; ses sources d'outils n'ont été que comparées.
- **AnimationCraft** (`MIT OR Apache-2.0`) : le modèle d'écriture en Rust que Flare a absorbé.
- **Next2Flash** : ses parties MIT peuvent être portées. Son worker FFDec est en GPLv3, seul son comportement est donc utilisé.
- **jpdurigan/godot_moho_importer** (MIT, Copyright (c) 2022 Joao Pedro Durigan) : la disposition de maillage Moho utilisée par `crates/io/flare-moho`.
- **usvg** (MIT/Apache-2.0) : le lecteur SVG, utilisé comme dépendance.
- **Ruffle** (`third_party/ruffle`, un sous-module utilisé sans modification sous sa propre licence) : lecture Flash pour Tester le film, et le crate `swf` pour la publication SWF.
- **ArtCraft** (`MIT OR Apache-2.0`) et **minnamate** (pas de fichier de licence, comportement seulement) sont des sources étudiées. Leurs licences figurent dans [docs/BACKLOG.md](../BACKLOG.md).
- **nflare** et **FFDec** (GPLv3) : comportement seulement, pas de code. **SimplSam/moho-bvh-bones** (CC BY-NC-SA 4.0) : non utilisé.

## Communauté

[Discord](https://discord.com/invite/JpeScW8Awa) · [Site web](https://flare-animate.github.io/website/) · [Discussions](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## Licence

BSD 3-Clause. Voir [LICENSE](../../LICENSE). Les composants tiers conservent leurs propres licences.
