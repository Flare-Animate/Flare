# Flare

Rust で書かれた、Adobe Animate のような無料のオープンソース・アニメーションツールです。

> **初期アルファ版です。** 不具合や Flash/Animate インポートの不完全さがあります。作業は必ずバックアップしてください。

[English](../../README.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [简体中文](README.zh-CN.md)

## できること

- おなじみのツールで描画し、タイムライン上でフレームとレイヤーを組み立て、モーションガイドに沿ったトゥイーンを作成できます。
- `.flare` プロジェクトの読み書き、Animate の XFL/FLA のインポートとエクスポート、PNG と GIF の書き出し、SWF の公開に対応しています。
- 複数ドキュメントのタブ表示、ドッキング可能なパネル、自動保存と予期せぬ終了からの復元に対応しています。
- コマンドラインツール(`flare-cli`)と MCP サーバー(`flare-mcp`)により、スクリプトや AI エージェントが同じドキュメントを安全に扱えます。

## ビルドと実行

Rust のバージョンは `rust-toolchain.toml` で固定されています。Linux では [CONTRIBUTING.md](../../CONTRIBUTING.md) に記載のライブラリをインストールしてください。

```
cargo run -p flare-desktop
cargo run -p flare-cli -- --help
```

リリースビルドとパッケージングについては [docs/release.md](../release.md) を参照してください。

## ドキュメント

[docs/README.md](../README.md) に Flare の構造が説明されています。[CONTRIBUTING.md](../../CONTRIBUTING.md)、[SECURITY.md](../../SECURITY.md)、[CHANGELOG.md](../../CHANGELOG.md) も参照してください。これらは英語です。

## コミュニティ

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## ライセンス

[LICENSE](../../LICENSE) を参照してください。

---

*この翻訳は[英語版 README](../../README.md)より遅れる場合があります。英語版が正です。*
