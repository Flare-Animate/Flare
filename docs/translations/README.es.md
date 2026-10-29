# Flare

Una herramienta de animación libre y de código abierto, al estilo de Adobe Animate, escrita en Rust.

> **Alfa temprana.** Habrá errores y la importación de Flash/Animate es incompleta. Haz copias de seguridad de tu trabajo.

[English](../../README.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md)

## Qué hace

- Dibuja con las herramientas habituales, organiza fotogramas y capas en una línea de tiempo y crea interpolaciones siguiendo guías de movimiento.
- Abre y guarda proyectos `.flare`; importa y exporta XFL/FLA de Animate; exporta PNG y GIF; publica SWF.
- Varios documentos en pestañas, paneles acoplables, autoguardado y recuperación tras un cierre inesperado.
- Una herramienta de línea de comandos (`flare-cli`) y un servidor MCP (`flare-mcp`) para que scripts y agentes de IA trabajen con los mismos documentos de forma segura.

## Compilar y ejecutar

Rust está fijado en `rust-toolchain.toml`. En Linux instala las bibliotecas indicadas en [CONTRIBUTING.md](../../CONTRIBUTING.md).

```
cargo run -p flare-desktop
cargo run -p flare-cli -- --help
```

La compilación de versiones y el empaquetado se describen en [docs/release.md](../release.md).

## Documentación

[docs/README.md](../README.md) explica cómo está construido Flare. Consulta también [CONTRIBUTING.md](../../CONTRIBUTING.md), [SECURITY.md](../../SECURITY.md) y [CHANGELOG.md](../../CHANGELOG.md). Estos documentos están en inglés.

## Comunidad

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## Licencia

Consulta [LICENSE](../../LICENSE).

---

*Esta traducción puede ir por detrás del [README en inglés](../../README.md), que es la referencia.*
