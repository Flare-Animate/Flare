[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [Português (Brasil)](README.pt-BR.md) | **日本語** | [한국어](README.ko.md) | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare Animate

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Adobe Animate の考え方に沿った、Rust 製の無料のオープンソース・アニメーションツールです。Windows、macOS、Linux 向けです。

> **初期アルファ版です。** 不具合や、Flash/Animate のインポートが不完全なことを想定してください。作業のバックアップを取ってください。

<!-- ここにエディターのスクリーンショットを置きます: assets/repository/flare-app.png -->

### 機能

- Animate にあるツールで描画できるほか、定規(距離と角度を測る)やミラーなど、拡張されたツールライブラリがあります。
- タイムライン上でフレームとレイヤーを配置できます。フォルダー、マスク、モーションガイド、Ease 付きのモーショントゥイーンとシェイプトゥイーン、オニオンスキンに対応します。
- シンボル(グラフィック、ムービークリップ、ボタン)をライブラリに置き、インスタンスとしてステージに配置できます。
- アニメーションを音付きで再生できます。ムービープレビューは、公開した SWF を Ruffle で実行します。
- `.flare` プロジェクトを開いて保存できます。Animate の XFL と `.fla` をインポート/エクスポートできます。
- PNG、GIF、MP4 を書き出し、SWF または HTML5 Canvas ページを公開できます。
- スクリプトからプロジェクトを作成、編集、書き出し、公開できるコマンドライン(`flare`)。

### ビルドと実行

Rust は `rust-toolchain.toml` で固定されています。Ruffle は git サブモジュールなので、サブモジュールごとクローンしてください:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Linux では [CONTRIBUTING.md](../../CONTRIBUTING.md) に挙げたライブラリをインストールしてください。

リリースでは、macOS、Windows、Linux(x64 と arm64)向けのネイティブパッケージを作ります: DMG、MSI、ポータブル zip、`.deb`、`.rpm`、AppImage、そしてコマンドラインの別アーカイブです。まだ公開されたバージョンはないため、今はソースからビルドしてください。パッケージとリリースの手順は [docs/release.md](../release.md) にあります。

### コマンドライン

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### ロードマップ

計画済みでまだ作っていないものです。多くはインターフェースで薄く表示されます。順序と理由は [docs/roadmap.md](../roadmap.md) にあります。

- マージ描画: Flash の平面マップ型シェイプ(重なった図形が互いに切り合って結合する。現在、図形は別々のオブジェクトです)
- ボーンと逆運動学(ボーンツールとバインドツール)
- Animate の残りのツール: 3D 回転と 3D 移動、Deco、幅、アセットワープ、グラデーション変形、マジックワンド
- シェイプヒント、複数のシーン、コマンドメニューによるスクリプト
- Flash 8 風のインターフェーススキンと、カスタムテーマ
- 自由に並べ替えられるツールバー(「…」の一覧からツールを固定することは既にできます)と、編集できるキーボードショートカット
- 配置後のテキスト編集
- 共有メニューから YouTube などへの共有
- 公開された Web 版(現在は実験的です)

### ドキュメント

[docs/README.md](../README.md) は Flare の作りを説明しています。

コントリビューターは [CONTRIBUTING.md](../../CONTRIBUTING.md)、[AGENTS.md](../../AGENTS.md)、[SECURITY.md](../../SECURITY.md)、[CHANGELOG.md](../../CHANGELOG.md) を参照してください。これらは英語です。

### コミュニティ

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### ライセンス

BSD 3-Clause。[LICENSE](../../LICENSE) を参照してください。

---

*この翻訳は[英語版 README](../../README.md)より遅れることがあります。英語版が正です。*
