[English](../../README.md) | [Deutsch](README.de.md) | **Español** | [Français](README.fr.md) | [Português (Brasil)](README.pt-BR.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [简体中文](README.zh-CN.md)

<div align="center">

# Flare

<img src="../../assets/BannerNew.png" width="969" height="261" alt="El banner de Flare">

**Suite de animación libre y de código abierto con continuidad para usuarios de Adobe Animate y Flash.**

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Status](https://img.shields.io/badge/status-early%20alpha-red)

[Sitio web](https://flare-animate.github.io/website/) · [Versión web](https://flare-animate.github.io/Flare/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [Compilar](../BUILDING.md) · [Issues](https://github.com/Flare-Animate/Flare/issues)

</div>

> **Alfa temprana.** Espere errores e importación incompleta de Flash/Animate. Guarde su trabajo con copias de seguridad.

## ¿Qué es Flare?

Flare, también llamado *FlareCraft*, es una suite de animación (en desarrollo) pensada para facilitar el paso desde programas como *Adobe Animate*, *Adobe/Macromedia Flash* y *Moho* a artistas y animadores.

La idea es admitir los formatos que usan esas herramientas, para que las personas puedan abrir y seguir trabajando con los archivos de proyecto que ya tienen, y dar a la interfaz un diseño más sencillo y familiar, que se pueda personalizar.

Flare se está reescribiendo en **Rust**. Se basa sobre todo en *OpenToonz* y *Tahoma2D*, con funciones incorporadas de *AnimationCraft* y *Next2Flash*, que ya forman parte de Flare. El código C++ antiguo se conserva como archivo en [`original/`](../../original/README.md).

## Funciones

- Dibujo con herramientas de lápiz, pincel y formas, con una herramienta de selección para mover, agrupar y editar formas, y estilos de trazo (sólido, discontinuo, punteado, stipple).
- Una línea de tiempo con fotogramas, capas, carpetas, máscaras, guías de movimiento, papel cebolla y una capa de cámara.
- Interpolaciones de movimiento y de forma con easing, y un Motion Editor para las curvas de los segmentos.
- Símbolos (Gráfico, Clip de película y Botón) en una biblioteca, colocados como instancias en el escenario.
- Ajustes de sesiones de stop motion. La captura con cámara todavía no está construida.
- Reproducción de sonido en el escritorio, y Probar película, que ejecuta el SWF publicado en Ruffle.
- Abrir y guardar proyectos `.flare`. Importar y exportar XFL y `.fla` de Animate, y abrir películas Flash (`.swf`) como proyectos editables.
- Importar proyectos de Moho y movimiento BVH, escenas `.tnz` y niveles `.pli` de OpenToonz, y obras en SVG, PNG o GIF animado, como proyectos editables.
- Exportar PNG, GIF y MP4, y publicar un SWF o una página HTML5 Canvas.
- Una línea de comandos (`flare`) y un servidor MCP (`flare-mcp`) para agentes, para crear, editar, exportar y publicar proyectos desde scripts.
- Una versión para navegador del mismo editor. Vea [docs/WEB.md](../WEB.md) para lo que hace distinto.

[docs/SURFACES.md](../SURFACES.md) enumera, función por función, lo que pueden hacer la aplicación de escritorio, la versión para navegador, la línea de comandos y el servidor MCP, y las carencias.

### Formatos

- Flash/Animate: `.fla`, `.xfl`, `.swf` y `.fla` binario heredado (importado como esqueleto). Guardar y exportar a `.fla`, `.xfl` y `.flare`.
- Moho: `.moho`, `.anime`, `.mohoproj`, `.animeproj` y movimiento BVH. Capas, huesos, fotogramas clave y arte vectorial se importan como proyectos editables; las imágenes y el sonido se listan, no se colocan.
- OpenToonz: se importan escenas `.tnz` y niveles vectoriales `.pli` (velocidad de fotogramas, cámara, columnas de xsheet, trazos, colores de paleta). Los niveles raster `.tlv` se rechazan, y `.tzu` está previsto.
- Obras: SVG (un fotograma), PNG (un fotograma) y GIF animado se importan como capas de mapa de bits. JPEG, WebP y PSD no están construidos.

### Plataformas

| Destino | Estado | Notas |
|---|---|---|
| Windows (escritorio) | Se compila desde el código | Cadena de herramientas MSVC y `flare-desktop`. Todavía sin descargas. |
| Linux (escritorio) | Se compila desde el código | Necesita las bibliotecas de [CONTRIBUTING.md](../../CONTRIBUTING.md). Probar película necesita Ruffle compilado desde el submódulo. |
| macOS (escritorio) | Se compila desde el código | Menú nativo. La presión del lápiz todavía no llega al pincel en macOS. |
| Web (navegador) | En vivo, temprana | La [versión web](https://flare-animate.github.io/Flare/) se publica desde `main`. Sin exportación MP4, sin sonido, y Probar película se ejecuta en la página. |
| Android | Experimental | Primera versión de `flare-android`, el espacio de trabajo de escritorio dentro de una actividad nativa. Todavía no se pueden abrir ni guardar archivos con el selector del teléfono, y Probar película está desactivado. |
| ChromeOS | Sin probar | La versión Android genera APK para x86_64 y arm64 que deberían funcionar allí. No se ha probado nada en ChromeOS. |
| iOS | No construido | No hay aplicación para iOS. El trabajo de CI para iOS solo intenta compilar y no bloquea. |

Todavía no se ha publicado ninguna versión, así que no hay descargas. Compile desde el código (abajo).

## Compilar y ejecutar

Rust está fijado en `rust-toolchain.toml`. Ruffle es un submódulo de git, así que clone con los submódulos:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

En Linux instale las bibliotecas que aparecen en [CONTRIBUTING.md](../../CONTRIBUTING.md).

[docs/BUILDING.md](../BUILDING.md) tiene los pasos para cada plataforma: Linux, Windows, macOS, la versión para navegador y Android.

## Línea de comandos

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

## Por qué existe Flare

Adobe [anunció](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) el fin de vida de Animate y después [dio marcha atrás](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). La marcha atrás no cambia lo volátil que es la industria de la animación ni cuánto la controla una sola empresa. Los animadores entraron en pánico, con razón, y la carrera de una persona no debería depender de que una empresa siga vendiendo un producto.

Flare empezó como un espacio de pruebas para que la animación fuera más fácil de aprender, después se publicó como código abierto y creció desde ahí.

## Hoja de ruta

Lo siguiente, por orden de importancia. La mayor parte aparece atenuada en la interfaz. [docs/ROADMAP.md](../ROADMAP.md) repite esta lista y continúa con todo lo de menor prioridad.

- Fusión de dibujo: las formas de mapa plano de Flash, donde las formas que se solapan se cortan y se fusionan (hoy las formas son objetos separados)
- Calidad de dibujo: dinámica de pincel, y trazar un mapa de bits a formas vectoriales
- SWF que conserve más del documento: botones con sus estados, interpolaciones de forma como morphs, máscaras de varias formas y sonidos dentro de símbolos
- Un Motion Editor: gráficos por propiedad y curvas de easing personalizadas para las interpolaciones
- Importar arte desde archivos SVG y de Photoshop
- Huesos y cinemática inversa (las herramientas Bone y Bind)
- Atajos de teclado editables

Además, con menor prioridad: ActionScript y un editor de scripts (el ActionScript de un archivo importado se conserva como datos y nunca se ejecuta), puntos de forma y más de una escena, el resto de herramientas de Animate, temas personalizados, una barra de herramientas totalmente reordenable y una versión web publicada.

## Documentación

[docs/README.md](../README.md) explica cómo se construye Flare y dice qué cubre cada documento.

Quienes contribuyan deben leer [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) y [CHANGELOG.md](../../CHANGELOG.md).

## El código original

[`original/`](../../original/README.md) es el código C++ archivado, basado en OpenToonz y Tahoma2D. Se conserva solo como referencia. Nada de Flare depende de él, lo enlaza o copia de él.

## Créditos

Flare se apoya en estos proyectos. Sus avisos de licencia se conservan donde se portó o usó código.

- **OpenToonz** (BSD-3-Clause, Copyright (c) DWANGO Co., Ltd. y colaboradores): la base del código original. El lector PLI de `crates/io/flare-toonz` está portado desde él.
- **Tahoma2D** (BSD-3-Clause): una bifurcación de OpenToonz y la otra base del código original. Se leyeron su licencia y su README; sus fuentes de herramientas solo se compararon.
- **AnimationCraft** (`MIT OR Apache-2.0`): el modelo de autoría en Rust que Flare absorbió.
- **Next2Flash**: sus partes MIT pueden portarse. Su worker FFDec es GPLv3, así que solo se usa su comportamiento.
- **jpdurigan/godot_moho_importer** (MIT, Copyright (c) 2022 Joao Pedro Durigan): la disposición de malla de Moho que usa `crates/io/flare-moho`.
- **usvg** (MIT/Apache-2.0): el lector SVG, usado como dependencia.
- **Ruffle** (`third_party/ruffle`, un submódulo usado sin cambios bajo su propia licencia): reproducción de Flash para Probar película, y el crate `swf` para publicar SWF.
- **ArtCraft** (`MIT OR Apache-2.0`) y **minnamate** (sin archivo de licencia, solo comportamiento) son fuentes estudiadas. Sus licencias figuran en [docs/BACKLOG.md](../BACKLOG.md).
- **nflare** y **FFDec** (GPLv3): solo comportamiento, sin código. **SimplSam/moho-bvh-bones** (CC BY-NC-SA 4.0): no usado.

## Comunidad

[Discord](https://discord.com/invite/JpeScW8Awa) · [Sitio web](https://flare-animate.github.io/website/) · [Debates](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## Licencia

BSD 3-Clause. Vea [LICENSE](../../LICENSE). Los componentes de terceros conservan sus propias licencias.
