# Flare

一款用 Rust 编写的、类似 Adobe Animate 的免费开源动画工具。

> **早期 Alpha 版本。** 可能存在错误,Flash/Animate 导入尚不完整。请备份你的工作。

[English](../../README.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [日本語](README.ja.md)

## 功能

- 使用熟悉的工具绘图,在时间轴上编排帧和图层,并沿运动引导线创建补间。
- 打开和保存 `.flare` 项目;导入和导出 Animate 的 XFL/FLA;导出 PNG 和 GIF;发布 SWF。
- 多文档标签页、可停靠面板、自动保存以及意外退出后的恢复。
- 命令行工具(`flare-cli`)和 MCP 服务器(`flare-mcp`),让脚本和 AI 代理可以安全地处理同样的文档。

## 构建与运行

Rust 版本在 `rust-toolchain.toml` 中固定。在 Linux 上请安装 [CONTRIBUTING.md](../../CONTRIBUTING.md) 中列出的库。

```
cargo run -p flare-desktop
cargo run -p flare-cli -- --help
```

发布构建和打包方式见 [docs/release.md](../release.md)。

## 文档

[docs/README.md](../README.md) 介绍了 Flare 的构建方式。另请参阅 [CONTRIBUTING.md](../../CONTRIBUTING.md)、[SECURITY.md](../../SECURITY.md) 和 [CHANGELOG.md](../../CHANGELOG.md)。这些文档为英文。

## 社区

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## 许可证

参见 [LICENSE](../../LICENSE)。

---

*本翻译可能落后于[英文 README](../../README.md),以英文版为准。*
