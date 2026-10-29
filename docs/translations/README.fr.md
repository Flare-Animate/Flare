# Flare

Un outil d'animation libre et open source, dans l'esprit d'Adobe Animate, écrit en Rust.

> **Alpha précoce.** Attendez-vous à des bogues et à une importation Flash/Animate incomplète. Sauvegardez votre travail.

[English](../../README.md) · [Español](README.es.md) · [Deutsch](README.de.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md)

## Ce que fait Flare

- Dessinez avec les outils habituels, organisez images et calques sur une scénarisation et créez des interpolations le long de guides de mouvement.
- Ouvrez et enregistrez des projets `.flare` ; importez et exportez le XFL/FLA d'Animate ; exportez en PNG et GIF ; publiez en SWF.
- Plusieurs documents dans des onglets, panneaux ancrables, enregistrement automatique et récupération après une fermeture inattendue.
- Un outil en ligne de commande (`flare-cli`) et un serveur MCP (`flare-mcp`) pour que scripts et agents d'IA travaillent sur les mêmes documents en toute sécurité.

## Compiler et lancer

Rust est épinglé dans `rust-toolchain.toml`. Sous Linux, installez les bibliothèques listées dans [CONTRIBUTING.md](../../CONTRIBUTING.md).

```
cargo run -p flare-desktop
cargo run -p flare-cli -- --help
```

La construction des versions et l'empaquetage sont décrits dans [docs/release.md](../release.md).

## Documentation

[docs/README.md](../README.md) explique comment Flare est construit. Voir aussi [CONTRIBUTING.md](../../CONTRIBUTING.md), [SECURITY.md](../../SECURITY.md) et [CHANGELOG.md](../../CHANGELOG.md). Ces documents sont en anglais.

## Communauté

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## Licence

Voir [LICENSE](../../LICENSE).

---

*Cette traduction peut avoir du retard sur le [README anglais](../../README.md), qui fait foi.*
