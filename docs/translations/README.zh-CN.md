[English](../../README.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [日本語](README.ja.md) | **简体中文**

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare 🔥

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

一款免费、开源的动画工具,理念上接近 Adobe Animate,用 Rust 编写。面向 Windows、macOS 和 Linux,并提供供脚本和智能体使用的命令行与 MCP 服务器。

> **早期 Alpha 版本。** 请预期会有缺陷,以及不完整的 Flash/Animate 导入。请备份您的工作。

<!-- 编辑器的截图将放在这里: assets/repository/flare-app.png -->

### 功能

- 使用熟悉的工具绘图:选择、部分选取、自由变换、套索和多边形套索、钢笔、铅笔、带笔尖的画笔、橡皮擦、矩形、椭圆、多边形/星形、直线、文本、颜料桶、墨水瓶和吸管,支持填充、描边和渐变。
- 在时间轴上排布帧和图层,支持文件夹、遮罩、运动引导线、带 Ease 的补间动画和形状补间,以及绘图纸外观。
- 库中的元件(图形、影片剪辑和按钮),作为实例放置在舞台上。
- 带声音播放动画;测试影片会在 Ruffle 中运行发布的 SWF(桌面版在应用内,浏览器中在页面内)。
- 以标签页打开多个文档,可停靠的面板,自动保存和崩溃恢复。
- 打开和保存 `.flare` 项目。导入和导出 Animate 的 XFL 和 `.fla`,并报告未能带过来的内容。
- 导出 PNG、GIF 和 MP4,并发布 SWF 或 HTML5 Canvas 页面。
- 命令行(`flare`)和 MCP 服务器(`flare-mcp`),让脚本和 AI 智能体可以安全地处理同样的文档。

### 构建与运行

Rust 版本固定在 `rust-toolchain.toml` 中。Ruffle 是 git 子模块,因此请连同子模块一起克隆:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

在 Linux 上请安装 [CONTRIBUTING.md](../../CONTRIBUTING.md) 中列出的库。

发布版会为 macOS、Windows 和 Linux(x64 和 arm64)构建原生安装包:DMG、MSI 和便携 zip,以及 `.deb`、`.rpm` 和 AppImage 文件,命令行则是单独的压缩包。目前还没有发布任何版本,所以现在请从源码构建。安装包以及发布流程见 [docs/release.md](../release.md)。

### 命令行与 MCP

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

`flare-mcp project.flare --allow-write --export-dir exports` 通过 stdio 向 AI 智能体提供项目。没有 `--allow-write` 时为只读,导出只会写入导出目录,每次修改都是一个经过验证、可撤销的步骤。

### 尚未实现

尚未实现:形状提示、多于一个场景、脚本和“命令”菜单、可编辑的键盘快捷键、放置后编辑文本、合并绘图(形状是彼此独立的对象,而不是 Flash 那种相互切割合并的平面图)以及摄像机图层。已规划但未构建的功能在界面中以变暗形式显示。网页版是实验性的,尚未发布。接下来的计划见 [docs/roadmap.md](../roadmap.md)。

### 文档

[docs/README.md](../README.md) 说明了 Flare 是如何构建的。另见 [CONTRIBUTING.md](../../CONTRIBUTING.md)、工作规则 [AGENTS.md](../../AGENTS.md)、[SECURITY.md](../../SECURITY.md) 和 [CHANGELOG.md](../../CHANGELOG.md)。这些文档为英文。

### 社区

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### 许可证

BSD 3-Clause。参见 [LICENSE](../../LICENSE)。

---

*本翻译可能落后于[英文 README](../../README.md),以英文为准。*
