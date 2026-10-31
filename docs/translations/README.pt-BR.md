[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | **Português (Brasil)** | [日本語](README.ja.md) | [한국어](README.ko.md) | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare 🔥

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Uma ferramenta de animação livre e de código aberto, no espírito do Adobe Animate, escrita em Rust. Feita para Windows, macOS e Linux, com uma linha de comando e um servidor MCP para scripts e agentes.

> **Alfa inicial.** Espere erros e uma importação de Flash/Animate incompleta. Faça backup do seu trabalho.

<!-- Aqui entrará uma captura de tela do editor: assets/repository/flare-app.png -->

### Recursos

- Desenhe com as ferramentas de sempre: Seleção, Subseleção, Transformação livre, Laço e Laço poligonal, Caneta, Lápis, Pincel com ponta, Borracha, Retângulo, Oval, PolyStar, Linha, Texto, Balde de tinta, Tinteiro e Conta-gotas, com preenchimentos, traços e gradientes.
- Organize quadros e camadas em uma linha do tempo, com pastas, máscaras, guias de movimento, interpolações de movimento e de forma com Ease e papel de cebola.
- Símbolos (Gráfico, Clipe de filme e Botão) em uma biblioteca, colocados como instâncias no palco.
- Reproduza a animação com som; Testar filme executa o SWF publicado no Ruffle (dentro do aplicativo no desktop, na página no navegador).
- Vários documentos em abas, painéis acopláveis, salvamento automático e recuperação após um fechamento inesperado.
- Abra e salve projetos `.flare`. Importe e exporte XFL e `.fla` do Animate, com um relatório do que não pôde ser trazido.
- Exporte PNG, GIF e MP4 e publique um SWF ou uma página HTML5 Canvas.
- Uma linha de comando (`flare`) e um servidor MCP (`flare-mcp`) para que scripts e agentes de IA trabalhem com os mesmos documentos com segurança.

### Compilar e executar

O Rust está fixado em `rust-toolchain.toml`. O Ruffle é um submódulo do git, então clone com os submódulos:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

No Linux, instale as bibliotecas listadas em [CONTRIBUTING.md](../../CONTRIBUTING.md).

As versões geram pacotes nativos para macOS, Windows e Linux em x64 e arm64: um DMG, um MSI e um zip portátil, e arquivos `.deb`, `.rpm` e AppImage, além da linha de comando como um arquivo separado. Nenhuma versão foi publicada ainda, então por enquanto compile a partir do código-fonte. [docs/release.md](../release.md) descreve os pacotes e como uma versão é feita.

### Linha de comando e MCP

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

`flare-mcp project.flare --allow-write --export-dir exports` serve um projeto a um agente de IA por stdio. É somente leitura sem `--allow-write`, as exportações vão apenas para o diretório de exportação e cada alteração é um passo validado e reversível.

### Ainda não

Não estão prontos: dicas de forma, mais de uma cena, scripts e o menu Comandos, atalhos de teclado editáveis, edição do texto depois de colocado, o desenho por mesclagem (as formas são objetos separados, não o mapa planar do Flash que corta e mescla) e uma camada de câmera. O que está planejado mas não construído aparece esmaecido na interface. A versão web é experimental e não foi publicada. [docs/roadmap.md](../roadmap.md) diz o que vem a seguir.

### Documentação

[docs/README.md](../README.md) explica como o Flare é construído. Veja também [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md) com as regras de trabalho, [SECURITY.md](../../SECURITY.md) e [CHANGELOG.md](../../CHANGELOG.md). Esses documentos estão em inglês.

### Comunidade

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Licença

BSD de 3 cláusulas. Veja [LICENSE](../../LICENSE).

---

*Esta tradução pode estar atrás do [README em inglês](../../README.md), que é a versão de referência.*
