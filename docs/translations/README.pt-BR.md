[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [Italiano](README.it.md) | **Português (Brasil)** | [Čeština](README.cs.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [Русский](README.ru.md) | [简体中文](README.zh-CN.md)

<div align="center">

# Flare

<img src="../../assets/BannerNew.png" width="969" height="261" alt="O banner do Flare">

**Suíte de animação gratuita e de código aberto, com continuidade para usuários do Adobe Animate e do Flash.**

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Status](https://img.shields.io/badge/status-early%20alpha-red)

[Site](https://flare-animate.github.io/website/) · [Versão web](https://flare-animate.github.io/Flare/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [Compilar](../BUILDING.md) · [Issues](https://github.com/Flare-Animate/Flare/issues)

</div>

> **Alpha inicial.** Espere erros e uma importação incompleta de Flash/Animate. Faça backup do seu trabalho.

## O que é o Flare?

O Flare, também chamado de *FlareCraft*, é uma suíte de animação (em desenvolvimento) feita para facilitar a transição de programas como *Adobe Animate*, *Adobe/Macromedia Flash* e *Moho* para artistas e animadores.

A proposta é oferecer suporte aos formatos dessas ferramentas, para que as pessoas possam abrir e continuar trabalhando nos arquivos de projeto que já têm, e apresentar uma interface mais simples e familiar, que pode ser personalizada.

O Flare está sendo reescrito em **Rust**. Ele se baseia principalmente em *OpenToonz* e *Tahoma2D*, com recursos incorporados de *AnimationCraft* e *Next2Flash*, que foram absorvidos pelo Flare. O código C++ antigo fica guardado como arquivo em [`original/`](../../original/README.md).

## Recursos

- Desenho com as ferramentas de caneta, pincel e formas, com uma ferramenta de seleção para mover, agrupar e editar formas, e estilos de traço (sólido, tracejado, pontilhado, stipple, hachurado, irregular, espinha de peixe, corda, ziguezague).
- Uma linha do tempo com quadros, camadas, pastas, máscaras, guias de movimento, papel cebola e uma camada de câmera.
- Interpolações de movimento e de forma com easing, e um Motion Editor para as curvas dos segmentos.
- Símbolos (Gráfico, Clipe de Filme e Botão) em uma biblioteca, colocados como instâncias no palco.
- Configurações de sessões de stop motion. A captura por câmera ainda não foi construída.
- Reprodução de som no desktop, e Testar Filme, que executa o SWF publicado no Ruffle.
- Abrir e salvar projetos `.flare`. Importar e exportar XFL e `.fla` do Animate, e abrir filmes Flash (`.swf`) como projetos editáveis.
- Importar projetos Moho e movimentos BVH, cenas `.tnz` e níveis `.pli` do OpenToonz, e arte em SVG, PNG ou GIF animado, como projetos editáveis.
- Exportar PNG, GIF e MP4, e publicar um SWF ou uma página HTML5 Canvas.
- Uma linha de comando (`flare`) e um servidor MCP (`flare-mcp`) para agentes, para criar, editar, exportar e publicar projetos a partir de scripts.
- Uma versão para navegador do mesmo editor. Veja [docs/WEB.md](../WEB.md) para saber o que ela faz de forma diferente.

O [docs/SURFACES.md](../SURFACES.md) lista, recurso por recurso, o que cada um do aplicativo de desktop, da versão para navegador, da linha de comando e do servidor MCP consegue fazer, e as lacunas.

### Formatos

- Flash/Animate: `.fla`, `.xfl`, `.swf` e `.fla` binário legado (importado como esqueleto). Salvar e exportar em `.fla`, `.xfl` e `.flare`.
- Moho: `.moho`, `.anime`, `.mohoproj`, `.animeproj` e movimento BVH. Camadas, ossos, quadros-chave e arte vetorial são importados como projetos editáveis; imagens e som são listados, não colocados.
- OpenToonz: cenas `.tnz` e níveis vetoriais `.pli` são importados (taxa de quadros, câmera, colunas do xsheet, traços, cores de paleta). Níveis raster `.tlv` são recusados, e `.tzu` está previsto.
- Arte: SVG (um quadro), PNG (um quadro) e GIF animado são importados como camadas de bitmap. JPEG, WebP e PSD não foram construídos.

### Plataformas

| Destino | Situação | Observações |
|---|---|---|
| Windows (desktop) | Compila a partir do código-fonte | Toolchain MSVC e `flare-desktop`. |
| Linux (desktop) | Compila a partir do código-fonte | Precisa das bibliotecas listadas em [CONTRIBUTING.md](../../CONTRIBUTING.md). Testar Filme precisa do Ruffle compilado a partir do submódulo. |
| macOS (desktop) | Compila a partir do código-fonte | Menu nativo. A pressão da caneta ainda não chega ao Pincel no macOS. |
| Web (navegador) | No ar, inicial | A [versão web](https://flare-animate.github.io/Flare/) é publicada a partir de `main`. Sem exportação MP4, sem som, e Testar Filme roda na própria página. |
| Android | Experimental | Primeira versão do `flare-android`, o espaço de trabalho do desktop em uma atividade nativa. Ainda não é possível abrir nem salvar arquivos pelo seletor do telefone, e Testar Filme está desligado. |
| ChromeOS | Não testado | A versão Android gera APKs para x86_64 e arm64 que deveriam rodar ali. Nada foi testado no ChromeOS. |
| iOS | Não construído | Não há aplicativo para iOS. A tarefa de CI do iOS só tenta compilar e não bloqueia. |

### Downloads

Ainda não há uma versão estável. Todas as noites, uma compilação de `main` é publicada como pré-lançamento, a [nightly](https://github.com/Flare-Animate/Flare/releases/tag/nightly): instaladores e ZIPs portáteis para Windows, imagens de disco para macOS, pacotes Linux `.deb`, `.rpm`, `.AppImage`, `.flatpak` e `.tar.gz`, os arquivos da linha de comando, os APKs do Android e a versão para navegador. A nightly só é recompilada quando `main` tem commits novos e mantém apenas a compilação mais recente. Os pacotes não são assinados para distribuição; veja [docs/RELEASE.md](../RELEASE.md). Para o resto, compile a partir do código-fonte (abaixo).

## Compilar e executar

O Rust está fixado em `rust-toolchain.toml`. O Ruffle é um submódulo do git, então clone com os submódulos:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

No Linux, instale as bibliotecas listadas em [CONTRIBUTING.md](../../CONTRIBUTING.md).

Em [docs/BUILDING.md](../BUILDING.md) estão os passos de cada plataforma: Linux, Windows, macOS, a versão para navegador e Android.

## Linha de comando

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

## Por que o Flare existe

A Adobe [anunciou](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) o fim de vida do Animate e depois [voltou atrás](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). A mudança de rumo não altera o quanto a indústria da animação é instável nem o quanto uma única empresa a controla. Animadores entraram em pânico, com razão, e a carreira de ninguém deveria depender de uma empresa continuar vendendo um produto.

O Flare começou como um espaço de experimentação para tornar a animação mais fácil de aprender, depois foi aberto como código aberto e cresceu a partir daí.

## Roteiro

Os próximos itens, do mais importante para o menos. A maior parte aparece esmaecida na interface. O [docs/ROADMAP.md](../ROADMAP.md) repete esta lista e segue com tudo o que tem prioridade menor.

- Mesclagem de desenho: formas de mapa planar do Flash, em que formas sobrepostas se cortam e se mesclam (hoje as formas são objetos separados)
- Qualidade de desenho: dinâmica de pincel, e converter um bitmap em formas vetoriais
- SWF que mantenha mais do documento: botões com seus estados, interpolações de forma como morphs, máscaras de várias formas e sons dentro de símbolos
- Um Motion Editor: gráficos por propriedade e curvas de easing personalizadas para as interpolações
- Importar arte de arquivos SVG e do Photoshop
- Ossos dentro de formas e a ferramenta Vincular (os ossos que ligam instâncias de símbolos, posados por cinemática inversa com a ferramenta Osso, já estão prontos)
- Atalhos de teclado editáveis

Também previsto, com prioridade menor: ActionScript e um editor de scripts (o ActionScript de um arquivo importado é mantido como dado e nunca é executado), formas de referência e mais de uma cena, o restante das ferramentas do Animate, temas personalizados, uma barra de ferramentas totalmente reorganizável e uma versão web lançada.

## Documentação

O [docs/README.md](../README.md) explica como o Flare é construído e diz o que cada documento cobre.

Quem contribui deve ler [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) e [CHANGELOG.md](../../CHANGELOG.md).

## O código original

O [`original/`](../../original/README.md) é o código C++ arquivado, baseado em OpenToonz e Tahoma2D. Ele serve apenas como referência. Nada no Flare depende dele nem o liga. Dados e textos retirados dele são convertidos para os formatos do Flare e creditados em [docs/ENGINEERING.md](../ENGINEERING.md).

## Créditos

O Flare se apoia nestes projetos. Os avisos de licença são mantidos onde código foi portado ou usado.

- **OpenToonz** (BSD-3-Clause, Copyright (c) DWANGO Co., Ltd. e colaboradores): a base do código original. O leitor PLI de `crates/io/flare-toonz` foi portado a partir dele. Os arquivos de tradução dele são o ponto de partida das interfaces russa, tcheca e italiana.
- **Tahoma2D** (BSD-3-Clause): um fork do OpenToonz e a outra base do código original. Foram lidos a licença e o README; as fontes das ferramentas foram apenas comparadas.
- **AnimationCraft** (`MIT OR Apache-2.0`): o modelo de autoria em Rust que o Flare absorveu.
- **Next2Flash**: suas partes MIT podem ser portadas. Seu worker FFDec é GPLv3, então só o comportamento é usado.
- **jpdurigan/godot_moho_importer** (MIT, Copyright (c) 2022 Joao Pedro Durigan): o layout de malha do Moho usado por `crates/io/flare-moho`.
- **usvg** (MIT/Apache-2.0): o leitor SVG, usado como dependência.
- **Ruffle** (`third_party/ruffle`, um submódulo usado sem alterações sob a própria licença): reprodução Flash para Testar Filme, e o crate `swf` para publicar SWF.
- **ArtCraft** (`MIT OR Apache-2.0`) e **minnamate** (sem arquivo de licença, só comportamento) são fontes estudadas. As licenças estão listadas em [docs/BACKLOG.md](../BACKLOG.md).
- **nflare** e **FFDec** (GPLv3): só comportamento, sem código. **SimplSam/moho-bvh-bones** (CC BY-NC-SA 4.0): não usado.

## Comunidade

[Discord](https://discord.com/invite/JpeScW8Awa) · [Site](https://flare-animate.github.io/website/) · [Discussões](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## Licença

BSD 3-Clause. Veja [LICENSE](../../LICENSE). Componentes de terceiros mantêm suas próprias licenças.
