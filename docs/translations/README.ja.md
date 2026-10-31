[English](../../README.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | **日本語** | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare 🔥

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Adobe Animate の考え方に沿った、Rust 製の無料のオープンソース・アニメーションツールです。Windows、macOS、Linux 向けで、スクリプトやエージェント用のコマンドラインと MCP サーバーも備えています。

> **初期アルファ版です。** 不具合や、Flash/Animate のインポートが不完全なことを想定してください。作業のバックアップを取ってください。

<!-- ここにエディターのスクリーンショットを置きます: assets/repository/flare-app.png -->

### 機能

- おなじみのツールで描画できます: 選択、部分選択、自由変形、投げ縄と多角形投げ縄、ペン、鉛筆、ペン先付きブラシ、消しゴム、長方形、楕円、多角形/星形、直線、テキスト、塗りつぶし、インク壺、スポイト。塗り、線、グラデーションに対応します。
- タイムライン上でフレームとレイヤーを配置できます。フォルダー、マスク、モーションガイド、Ease 付きのモーショントゥイーンとシェイプトゥイーン、オニオンスキンに対応します。
- シンボル(グラフィック、ムービークリップ、ボタン)をライブラリに置き、インスタンスとしてステージに配置できます。
- アニメーションを音付きで再生できます。ムービープレビューは、公開した SWF を Ruffle で実行します(デスクトップではアプリ内、ブラウザーではページ内)。
- 複数のドキュメントをタブで開き、ドッキング可能なパネル、自動保存、クラッシュ後の復元に対応します。
- `.flare` プロジェクトを開いて保存できます。Animate の XFL と `.fla` をインポート/エクスポートでき、引き継げなかったものは報告されます。
- PNG、GIF、MP4 を書き出し、SWF または HTML5 Canvas ページを公開できます。
- コマンドライン(`flare`)と MCP サーバー(`flare-mcp`)により、スクリプトや AI エージェントが同じドキュメントを安全に扱えます。

### ビルドと実行

Rust は `rust-toolchain.toml` で固定されています。Ruffle は git サブモジュールなので、サブモジュールごとクローンしてください:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Linux では [CONTRIBUTING.md](../../CONTRIBUTING.md) に挙げたライブラリをインストールしてください。

リリースでは、macOS、Windows、Linux(x64 と arm64)向けのネイティブパッケージを作ります: DMG、MSI、ポータブル zip、`.deb`、`.rpm`、AppImage、そしてコマンドラインの別アーカイブです。まだ公開されたバージョンはないため、今はソースからビルドしてください。パッケージとリリースの手順は [docs/release.md](../release.md) にあります。

### コマンドラインと MCP

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

`flare-mcp project.flare --allow-write --export-dir exports` は、プロジェクトを stdio 経由で AI エージェントに提供します。`--allow-write` がなければ読み取り専用で、書き出しはエクスポート用ディレクトリの中だけに行われ、すべての変更は検証済みで元に戻せる 1 ステップになります。

### 未対応のもの

未対応のもの: シェイプヒント、複数シーン、スクリプトとコマンドメニュー、キーボードショートカットの編集、配置後のテキスト編集、マージ描画(図形は別々のオブジェクトで、Flash のように切り合って結合する平面マップではありません)、カメラレイヤー。計画済みで未実装のものは、インターフェースで薄く表示されます。Web 版は実験的で、公開されていません。次の予定は [docs/roadmap.md](../roadmap.md) にあります。

### ドキュメント

[docs/README.md](../README.md) は Flare の作りを説明しています。[CONTRIBUTING.md](../../CONTRIBUTING.md)、作業ルールの [AGENTS.md](../../AGENTS.md)、[SECURITY.md](../../SECURITY.md)、[CHANGELOG.md](../../CHANGELOG.md) もご覧ください。これらは英語です。

### コミュニティ

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### ライセンス

BSD 3-Clause。[LICENSE](../../LICENSE) を参照してください。

---

*この翻訳は[英語版 README](../../README.md)より遅れることがあります。英語版が正です。*
