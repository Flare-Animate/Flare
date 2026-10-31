[English](../../README.md) | **Español** | [Français](README.fr.md) | [Deutsch](README.de.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare 🔥

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Una herramienta de animación libre y de código abierto, en el espíritu de Adobe Animate, escrita en Rust. Pensada para Windows, macOS y Linux, con una línea de comandos y un servidor MCP para scripts y agentes.

> **Alfa temprana.** Espere errores y una importación de Flash/Animate incompleta. Haga copias de seguridad de su trabajo.

<!-- Aquí irá una captura de pantalla del editor: assets/repository/flare-app.png -->

### Características

- Dibuje con las herramientas habituales: Selección, Subselección, Transformación libre, Lazo y Lazo poligonal, Pluma, Lápiz, Pincel con punta, Borrador, Rectángulo, Óvalo, PolyStar, Línea, Texto, Cubo de pintura, Tintero y Cuentagotas, con rellenos, trazos y degradados.
- Organice fotogramas y capas en una línea de tiempo, con carpetas, máscaras, guías de movimiento, interpolaciones de movimiento y de forma con Ease, y papel cebolla.
- Símbolos (Gráfico, Clip de película y Botón) en una biblioteca, colocados como instancias en el escenario.
- Reproduzca la animación con sonido; Probar película ejecuta el SWF publicado en Ruffle (dentro de la aplicación en el escritorio, en la página en el navegador).
- Varios documentos en pestañas, paneles acoplables, guardado automático y recuperación tras un cierre inesperado.
- Abra y guarde proyectos `.flare`. Importe y exporte XFL y `.fla` de Animate, con un informe de lo que no se pudo trasladar.
- Exporte PNG, GIF y MP4, y publique un SWF o una página HTML5 Canvas.
- Una línea de comandos (`flare`) y un servidor MCP (`flare-mcp`) para que scripts y agentes de IA trabajen con los mismos documentos de forma segura.

### Compilar y ejecutar

Rust está fijado en `rust-toolchain.toml`. Ruffle es un submódulo de git, así que clone con submódulos:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

En Linux instale las bibliotecas indicadas en [CONTRIBUTING.md](../../CONTRIBUTING.md).

Las versiones generan paquetes nativos para macOS, Windows y Linux en x64 y arm64: un DMG, un MSI y un zip portátil, y archivos `.deb`, `.rpm` y AppImage, además de la línea de comandos como archivo aparte. Todavía no se ha publicado ninguna versión, así que por ahora compile desde el código. [docs/release.md](../release.md) describe los paquetes y cómo se hace una versión.

### Línea de comandos y MCP

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

`flare-mcp project.flare --allow-write --export-dir exports` sirve un proyecto a un agente de IA por stdio. Es de solo lectura sin `--allow-write`, las exportaciones van solo al directorio de exportación y cada cambio es un paso validado y reversible.

### Todavía no

No están hechos: sugerencias de forma, más de una escena, scripts y el menú Comandos, atajos de teclado editables, editar el texto una vez colocado, el dibujo de fusión (las formas son objetos separados, no el mapa planar de Flash que corta y fusiona) ni una capa de cámara. Lo que está planeado pero no construido se muestra atenuado en la interfaz. La versión web es experimental y no se publica. [docs/roadmap.md](../roadmap.md) dice qué sigue.

### Documentación

[docs/README.md](../README.md) explica cómo está construido Flare. Vea también [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md) con las reglas de trabajo, [SECURITY.md](../../SECURITY.md) y [CHANGELOG.md](../../CHANGELOG.md). Estos documentos están en inglés.

### Comunidad

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Licencia

BSD de 3 cláusulas. Vea [LICENSE](../../LICENSE).

---

*Esta traducción puede ir por detrás del [README en inglés](../../README.md), que es el que manda.*
