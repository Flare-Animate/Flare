[English](../../README.md) | [Deutsch](README.de.md) | **Español** | [Français](README.fr.md) | [Português (Brasil)](README.pt-BR.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare Animate

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Una herramienta de animación libre y de código abierto, en el espíritu de Adobe Animate, escrita en Rust. Pensada para Windows, macOS y Linux.

> **Alfa temprana.** Espere errores y una importación de Flash/Animate incompleta. Haga copias de seguridad de su trabajo.

<!-- Aquí irá una captura de pantalla del editor: assets/repository/flare-app.png -->

### Características

- Dibuje con las herramientas que tiene Animate, más una biblioteca ampliada de herramientas, como la Regla (mide distancia y ángulo) y el Espejo.
- Organice fotogramas y capas en una línea de tiempo, con carpetas, máscaras, guías de movimiento, interpolaciones de movimiento y de forma con Ease, papel cebolla y una capa de cámara.
- Símbolos (Gráfico, Clip de película y Botón) en una biblioteca, colocados como instancias en el escenario.
- Reproduzca la animación con sonido; Probar película ejecuta el SWF publicado en Ruffle.
- Abra y guarde proyectos `.flare`. Importe y exporte XFL y `.fla` de Animate.
- Exporte PNG, GIF y MP4, y publique un SWF o una página HTML5 Canvas.
- Una línea de comandos (`flare`) para crear, editar, exportar y publicar proyectos desde scripts.

### Compilar y ejecutar

Rust está fijado en `rust-toolchain.toml`. Ruffle es un submódulo de git, así que clone con submódulos:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

En Linux instale las bibliotecas indicadas en [CONTRIBUTING.md](../../CONTRIBUTING.md).

Las versiones generan paquetes nativos para macOS, Windows y Linux en x64 y arm64: un DMG, un MSI y un zip portátil, y archivos `.deb`, `.rpm` y AppImage, además de la línea de comandos como archivo aparte. Todavía no se ha publicado ninguna versión, así que por ahora compile desde el código. [docs/release.md](../release.md) describe los paquetes y cómo se hace una versión.

### Línea de comandos

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Hoja de ruta

Planeado pero todavía no construido. Casi todo se muestra atenuado en la interfaz. [docs/roadmap.md](../roadmap.md) tiene el orden y los motivos.

- Dibujo de fusión: las formas de mapa planar de Flash, donde las formas superpuestas se cortan y se fusionan (hoy las formas son objetos separados)
- Huesos y cinemática inversa (las herramientas Hueso y Vincular)
- El resto de las herramientas de Animate: Rotación 3D y Traslación 3D, Deco, Anchura, Asset Warp, Transformación de degradado y Varita mágica
- Sugerencias de forma y más de una escena
- ActionScript y un editor de scripts, y los scripts del menú Comandos (el ActionScript de un archivo importado se conserva como datos y nunca se ejecuta)
- Un aspecto de interfaz como Flash 8 y temas personalizados
- Una barra de herramientas totalmente reorganizable (ya se pueden fijar herramientas desde la lista «…») y atajos de teclado editables
- Editar el texto una vez colocado
- Compartir desde el menú Compartir en YouTube y otros servicios
- Una versión web publicada (hoy es experimental)

### Documentación

[docs/README.md](../README.md) explica cómo está construido Flare.

Quien quiera contribuir debería ver [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) y [CHANGELOG.md](../../CHANGELOG.md). Estos documentos están en inglés.

### Comunidad

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Licencia

BSD de 3 cláusulas. Vea [LICENSE](../../LICENSE).

---

*Esta traducción puede ir por detrás del [README en inglés](../../README.md), que es el que manda.*
