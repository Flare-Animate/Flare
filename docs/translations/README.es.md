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
- Abra y guarde proyectos `.flare`. Importe y exporte XFL y `.fla` de Animate. Abra películas Flash (`.swf`) como proyectos editables.
- Exporte PNG, GIF y MP4, y publique un SWF o una página HTML5 Canvas.
- Una interfaz de línea de comandos (`flare`) para crear, editar, exportar y publicar proyectos desde scripts.

### Descargas

Habrá descargas para Windows, macOS y Linux, pero todavía no hay ninguna. Las añadiremos aquí cuando el empaquetado esté terminado. Hasta entonces, se compila desde el código fuente (abajo).

### Compilar y ejecutar

Rust está fijado en `rust-toolchain.toml`. Ruffle es un submódulo de git, así que clone con submódulos:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

En Linux instale las bibliotecas indicadas en [CONTRIBUTING.md](../../CONTRIBUTING.md).

[docs/BUILDING.md](../BUILDING.md) explica los pasos para cada plataforma: Linux, Windows, macOS, la versión para navegador y Android.

### Línea de comandos

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Por qué existe Flare

Adobe [anunció](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) el fin de vida de Animate y después [dio marcha atrás](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). Aunque rectificó, esto demuestra lo volátil que es la industria de la animación y cuánto la dominan ciertas empresas. Los animadores entraron en pánico con razón. La carrera de las personas no debería depender de que una empresa decida seguir vendiendo un producto.

Flare empezó como un espacio de pruebas para que aprender animación fuera más fácil, más tarde se publicó como código abierto y creció desde ahí.

### Hoja de ruta

Lo siguiente, lo más importante primero. Casi todo se muestra atenuado en la interfaz. [docs/ROADMAP.md](../ROADMAP.md) repite esta lista y sigue con todo lo de menor prioridad.

- Dibujo de fusión: las formas de mapa planar de Flash, donde las formas superpuestas se cortan y se fusionan (hoy las formas son objetos separados)
- Calidad de dibujo: dinámica del pincel y vectorización de un mapa de bits en formas vectoriales
- SWF que conserva más del documento: botones con sus estados, interpolaciones de forma como morfos, máscaras de varias formas y sonidos dentro de símbolos
- Un Editor de movimiento: gráficos por propiedad y curvas de suavizado personalizadas para las interpolaciones
- Importar ilustraciones desde archivos SVG y Photoshop
- Huesos y cinemática inversa (las herramientas Hueso y Vincular)
- Atajos de teclado editables

También planeado, con menor prioridad: ActionScript y un editor de scripts (el ActionScript de un archivo importado se conserva como datos y nunca se ejecuta), pistas de forma y más de una escena, el resto de las herramientas de Animate, temas personalizados, una barra de herramientas reorganizable por completo y una versión web publicada.

### Documentación

[docs/README.md](../README.md) explica cómo está construido Flare.

Quien quiera contribuir debería ver [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) y [CHANGELOG.md](../../CHANGELOG.md). Estos documentos están en inglés.

### Comunidad

[Discord](https://discord.com/invite/JpeScW8Awa) · [Website](https://flare-animate.github.io/website/) · [Discussions](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Licencia

BSD de 3 cláusulas. Vea [LICENSE](../../LICENSE).

---

*Esta traducción puede ir por detrás del [README en inglés](../../README.md), que es el que manda.*
