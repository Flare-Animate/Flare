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
- Organize quadros e camadas em uma linha do tempo, com pastas, máscaras, guias de movimento, interpolações de movimento e de forma com Ease, papel de cebola e uma camada de câmera.
- Símbolos (Gráfico, Clipe de filme e Botão) em uma biblioteca, colocados como instâncias no palco.
- Reproduza a animação com som; Testar filme executa o SWF publicado no Ruffle.
- Abra e salve projetos `.flare`. Importe e exporte XFL e `.fla` do Animate.
- Exporte PNG, GIF e MP4 e publique um SWF ou uma página HTML5 Canvas.
- Uma interface de linha de comando (`flare`) para criar, editar, exportar e publicar projetos a partir de scripts.

### Downloads

Haverá downloads para Windows, macOS e Linux, mas ainda não há nenhum. Vamos adicioná-los aqui quando o empacotamento estiver concluído. Até lá, compile a partir do código-fonte (abaixo).

### Compilar e executar

O Rust está fixado em `rust-toolchain.toml`. O Ruffle é um submódulo do git, então clone com os submódulos:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

No Linux, instale as bibliotecas listadas em [CONTRIBUTING.md](../../CONTRIBUTING.md).

### Linha de comando

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Por que o Flare existe

A Adobe [anunciou](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) o fim da vida do Animate e depois [voltou atrás](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). Mesmo tendo voltado atrás, isso mostra como a indústria da animação é instável e o quanto certas empresas a dominam. Os animadores entraram em pânico, com razão. A carreira das pessoas não deveria depender da decisão de uma empresa de continuar vendendo um produto.

O Flare começou como um campo de testes para tornar a animação mais fácil de aprender, foi aberto ao público mais tarde e cresceu a partir daí.

### Roteiro

O que vem a seguir, o mais importante primeiro. Quase tudo aparece esmaecido na interface. [docs/ROADMAP.md](../ROADMAP.md) repete esta lista e segue com tudo de menor prioridade.

- Desenho por mesclagem: as formas de mapa planar do Flash, em que formas sobrepostas se cortam e se mesclam (hoje as formas são objetos separados)
- Qualidade do desenho: dinâmica do pincel e vetorização de um bitmap em formas vetoriais
- SWF que preserva mais do documento: botões com seus estados, interpolações de forma como morphs, máscaras de várias formas e sons dentro de símbolos
- Um Editor de movimento: gráficos por propriedade e curvas de suavização personalizadas para as interpolações
- Importar arte de arquivos SVG e Photoshop
- Ossos e cinemática inversa (as ferramentas Osso e Vincular)
- Editar o texto depois de colocado
- Atalhos de teclado editáveis

Também planejado, com menor prioridade: ActionScript e um editor de scripts (o ActionScript de um arquivo importado é mantido como dados e nunca é executado), dicas de forma e mais de uma cena, o restante das ferramentas do Animate, temas personalizados, uma barra de ferramentas totalmente reorganizável e uma versão web publicada.

### Documentação

[docs/README.md](../README.md) explica como o Flare é construído.

Quem for contribuir deve ver [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) e [CHANGELOG.md](../../CHANGELOG.md). Esses documentos estão em inglês.

### Comunidade

[Discord](https://discord.com/invite/JpeScW8Awa) · [Website](https://flare-animate.github.io/website/) · [Discussions](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### Licença

BSD de 3 cláusulas. Veja [LICENSE](../../LICENSE).

---

*Esta tradução pode estar atrás do [README em inglês](../../README.md), que é a versão de referência.*
