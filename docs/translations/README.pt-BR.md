[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | **Português (Brasil)** | [日本語](README.ja.md) | [한국어](README.ko.md) | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare Animate

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Uma ferramenta de animação livre e de código aberto, no espírito do Adobe Animate, escrita em Rust. Feita para Windows, macOS e Linux.

> **Alfa inicial.** Espere erros e uma importação de Flash/Animate incompleta. Faça backup do seu trabalho.

<!-- Aqui entrará uma captura de tela do editor: assets/repository/flare-app.png -->

### Recursos

- Desenhe com as ferramentas que o Animate tem, mais uma biblioteca ampliada de ferramentas, como a Régua (mede distância e ângulo) e o Espelho.
- Organize quadros e camadas em uma linha do tempo, com pastas, máscaras, guias de movimento, interpolações de movimento e de forma com Ease e papel de cebola.
- Símbolos (Gráfico, Clipe de filme e Botão) em uma biblioteca, colocados como instâncias no palco.
- Reproduza a animação com som; Testar filme executa o SWF publicado no Ruffle.
- Abra e salve projetos `.flare`. Importe e exporte XFL e `.fla` do Animate.
- Exporte PNG, GIF e MP4 e publique um SWF ou uma página HTML5 Canvas.
- Uma linha de comando (`flare`) para criar, editar, exportar e publicar projetos a partir de scripts.

### Compilar e executar

O Rust está fixado em `rust-toolchain.toml`. O Ruffle é um submódulo do git, então clone com os submódulos:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

No Linux, instale as bibliotecas listadas em [CONTRIBUTING.md](../../CONTRIBUTING.md).

As versões geram pacotes nativos para macOS, Windows e Linux em x64 e arm64: um DMG, um MSI e um zip portátil, e arquivos `.deb`, `.rpm` e AppImage, além da linha de comando como um arquivo separado. Nenhuma versão foi publicada ainda, então por enquanto compile a partir do código-fonte. [docs/release.md](../release.md) descreve os pacotes e como uma versão é feita.

### Linha de comando

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Roteiro

Planejado, mas ainda não construído. Quase tudo aparece esmaecido na interface. [docs/roadmap.md](../roadmap.md) tem a ordem e os motivos.

- Desenho por mesclagem: as formas de mapa planar do Flash, em que formas sobrepostas se cortam e se mesclam (hoje as formas são objetos separados)
- Ossos e cinemática inversa (as ferramentas Osso e Vincular)
- O restante das ferramentas do Animate: Rotação 3D e Translação 3D, Deco, Largura, Asset Warp, Transformação de gradiente e Varinha mágica
- Dicas de forma e mais de uma cena
- ActionScript e um editor de scripts, e os scripts do menu Comandos (o ActionScript de um arquivo importado é mantido como dados e nunca é executado)
- Uma aparência de interface do Flash 8 e temas personalizados
- Uma barra de ferramentas totalmente reorganizável (já dá para fixar ferramentas pela lista "…") e atalhos de teclado editáveis
- Editar o texto depois de colocado
- Compartilhar pelo menu Compartilhar no YouTube e em outros serviços
- Uma versão web publicada (hoje é experimental)

### Documentação

[docs/README.md](../README.md) explica como o Flare é construído.

Quem for contribuir deve ver [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) e [CHANGELOG.md](../../CHANGELOG.md). Esses documentos estão em inglês.

### Comunidade

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Licença

BSD de 3 cláusulas. Veja [LICENSE](../../LICENSE).

---

*Esta tradução pode estar atrás do [README em inglês](../../README.md), que é a versão de referência.*
