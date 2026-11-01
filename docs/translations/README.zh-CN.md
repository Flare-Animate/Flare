[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [Português (Brasil)](README.pt-BR.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | **简体中文**

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare Animate

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

一款免费、开源的动画工具,理念上接近 Adobe Animate,用 Rust 编写。面向 Windows、macOS 和 Linux。

> **早期 Alpha 版本。** 请预期会有缺陷,以及不完整的 Flash/Animate 导入。请备份您的工作。

<!-- 编辑器的截图将放在这里: assets/repository/flare-app.png -->

### 功能

- 使用 Animate 具备的工具绘图,另有扩展的工具库,例如标尺(测量距离和角度)和镜像。
- 在时间轴上排布帧和图层,支持文件夹、遮罩、运动引导线、带 Ease 的补间动画和形状补间、绘图纸外观以及摄像机图层。
- 库中的元件(图形、影片剪辑和按钮),作为实例放置在舞台上。
- 带声音播放动画;测试影片会在 Ruffle 中运行发布的 SWF。
- 打开和保存 `.flare` 项目。导入和导出 Animate 的 XFL 和 `.fla`。
- 导出 PNG、GIF 和 MP4,并发布 SWF 或 HTML5 Canvas 页面。
- 命令行(`flare`),可在脚本中创建、编辑、导出和发布项目。

### 构建与运行

Rust 版本固定在 `rust-toolchain.toml` 中。Ruffle 是 git 子模块,因此请连同子模块一起克隆:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

在 Linux 上请安装 [CONTRIBUTING.md](../../CONTRIBUTING.md) 中列出的库。

发布版会为 macOS、Windows 和 Linux(x64 和 arm64)构建原生安装包:DMG、MSI 和便携 zip,以及 `.deb`、`.rpm` 和 AppImage 文件,命令行则是单独的压缩包。目前还没有发布任何版本,所以现在请从源码构建。安装包以及发布流程见 [docs/release.md](../release.md)。

### 命令行

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### 路线图

接下来要做的事,按重要程度排列。大部分在界面中以变暗形式显示。[docs/roadmap.md](../roadmap.md) 重复这份清单,并继续列出优先级较低的内容。

- 合并绘图:Flash 的平面图形,重叠的形状会相互切割并合并(目前形状是彼此独立的对象)
- 绘图质量:画笔动态,以及把位图描摹成矢量形状
- 保留更多文档内容的 SWF:带状态的按钮、以变形形式输出的形状补间、由多个形状组成的遮罩,以及元件内的声音
- 动画编辑器:按属性的曲线图,以及补间的自定义缓动曲线
- 从 SVG 和 Photoshop 文件导入图稿
- 骨骼与反向运动学(骨骼工具和绑定工具)
- 放置后编辑文本
- 可编辑的键盘快捷键

同样已规划但优先级较低:ActionScript 和脚本编辑器(导入文件中的 ActionScript 作为数据保留,从不运行)、形状提示和多个场景、Animate 其余的工具、自定义主题、可完全重新排列的工具栏,以及正式发布的网页版。

### 文档

[docs/README.md](../README.md) 说明了 Flare 是如何构建的。

贡献者请查看 [CONTRIBUTING.md](../../CONTRIBUTING.md)、[AGENTS.md](../../AGENTS.md)、[SECURITY.md](../../SECURITY.md) 和 [CHANGELOG.md](../../CHANGELOG.md)。这些文档为英文。

### 社区

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### 许可证

BSD 3-Clause。参见 [LICENSE](../../LICENSE)。

---

*本翻译可能落后于[英文 README](../../README.md),以英文为准。*
