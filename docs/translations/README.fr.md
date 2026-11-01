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
- Organisez images et calques sur un scénario, avec dossiers, masques, guides de mouvement, interpolations de mouvement et de forme avec Ease, pelure d'oignon et un calque de caméra.
- Des symboles (Graphique, Clip et Bouton) dans une bibliothèque, placés comme occurrences sur la scène.
- Lisez l'animation avec le son ; Tester l'animation exécute le SWF publié dans Ruffle.
- Ouvrez et enregistrez des projets `.flare`. Importez et exportez le XFL et le `.fla` d'Animate.
- Exportez en PNG, GIF et MP4, et publiez un SWF ou une page HTML5 Canvas.
- Une interface en ligne de commande (`flare`) pour créer, modifier, exporter et publier des projets depuis des scripts.

### Téléchargements

Il y aura des téléchargements pour Windows, macOS et Linux, mais il n'y en a pas encore. Nous les ajouterons ici quand le paquetage sera terminé. D'ici là, compilez depuis les sources (voir ci-dessous).

### Compiler et lancer

Rust est épinglé dans `rust-toolchain.toml`. Ruffle est un sous-module git ; clonez donc avec les sous-modules :

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Sous Linux, installez les bibliothèques listées dans [CONTRIBUTING.md](../../CONTRIBUTING.md).

### Ligne de commande

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Pourquoi Flare existe

Adobe a [annoncé](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) la fin de vie d'Animate, puis [fait marche arrière](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). Même si elle est revenue sur sa décision, cela montre à quel point l'industrie de l'animation est instable et à quel point certaines entreprises la dominent. Les animateurs ont paniqué, et à juste titre. La carrière des gens ne devrait pas dépendre de la décision d'une entreprise de continuer à vendre un produit.

Flare a commencé comme un terrain d'essai pour rendre l'animation plus facile à apprendre, a ensuite été publié en open source et s'est développé à partir de là.

### Feuille de route

Les prochaines étapes, les plus importantes d'abord. La plupart est affiché estompé dans l'interface. [docs/ROADMAP.md](../ROADMAP.md) reprend cette liste et continue avec tout ce qui est de moindre priorité.

- Dessin par fusion : les formes à carte planaire de Flash, où les formes qui se chevauchent se découpent et fusionnent (aujourd'hui les formes sont des objets séparés)
- Ouvrir les fichiers SWF : lire un SWF publié comme projet modifiable (ses formes, symboles et scénario), avec `.swf` enregistré auprès du système d'exploitation
- Qualité du dessin : dynamique du pinceau et vectorisation d'une image bitmap en formes vectorielles
- Un SWF qui garde davantage du document : boutons avec leurs états, interpolations de forme en morphes, masques de plusieurs formes et sons dans les symboles
- Un éditeur de mouvement : graphiques par propriété et courbes d'accélération personnalisées pour les interpolations
- Importer des illustrations depuis des fichiers SVG et Photoshop
- Armatures et cinématique inverse (les outils Os et Lier)
- La modification du texte une fois placé
- Raccourcis clavier modifiables

Également prévu, à moindre priorité : ActionScript et un éditeur de scripts (l'ActionScript d'un fichier importé est conservé comme données et n'est jamais exécuté), les repères de forme et plus d'une scène, le reste des outils d'Animate, des thèmes personnalisés, une barre d'outils entièrement réorganisable et une version web publiée.

### Documentation

[docs/README.md](../README.md) explique comment Flare est construit.

Les contributeurs devraient lire [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) et [CHANGELOG.md](../../CHANGELOG.md). Ces documents sont en anglais.

### Communauté

[Discord](https://discord.com/invite/JpeScW8Awa) · [Website](https://flare-animate.github.io/website/) · [Discussions](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Licence

BSD à 3 clauses. Voir [LICENSE](../../LICENSE).

---

*Cette traduction peut avoir du retard sur le [README anglais](../../README.md), qui fait foi.*
