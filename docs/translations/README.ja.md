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
- タイムライン上でフレームとレイヤーを配置できます。フォルダー、マスク、モーションガイド、Ease 付きのモーショントゥイーンとシェイプトゥイーン、オニオンスキン、カメラレイヤーに対応します。
- シンボル(グラフィック、ムービークリップ、ボタン)をライブラリに置き、インスタンスとしてステージに配置できます。
- アニメーションを音付きで再生できます。ムービープレビューは、公開した SWF を Ruffle で実行します。
- `.flare` プロジェクトを開いて保存できます。Animate の XFL と `.fla` をインポート/エクスポートできます。
- PNG、GIF、MP4 を書き出し、SWF または HTML5 Canvas ページを公開できます。
- スクリプトからプロジェクトを作成、編集、書き出し、公開できるコマンドラインインターフェース(`flare`)。

### ダウンロード

Windows、macOS、Linux 向けのダウンロードを用意する予定ですが、現時点ではまだありません。パッケージングが完了しだい、ここに追加します。それまではソースからビルドしてください(下記)。

### ビルドと実行

Rust は `rust-toolchain.toml` で固定されています。Ruffle は git サブモジュールなので、サブモジュールごとクローンしてください:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Linux では [CONTRIBUTING.md](../../CONTRIBUTING.md) に挙げたライブラリをインストールしてください。

### コマンドライン

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Flare がある理由

Adobe は Animate の販売終了を[発表](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220)し、のちに[方針を撤回](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459)しました。撤回されたとはいえ、アニメーション業界がいかに不安定で、特定の企業がどれほど市場を支配しているかが浮き彫りになりました。アニメーターたちが不安に陥ったのも当然です。ある企業が製品を売り続けるかどうかに、人々の仕事が左右されるべきではありません。

Flare は、アニメーションを学びやすくするための実験場として始まり、のちにオープンソース化されて、そこから育ってきました。

### ロードマップ

次に取り組むものを、重要な順に並べています。多くはインターフェースで薄く表示されます。[docs/ROADMAP.md](../ROADMAP.md) はこの一覧を繰り返し、優先度の低いものまで続けています。

- マージ描画: Flash の平面マップ型シェイプ(重なった図形が互いに切り合って結合する。現在、図形は別々のオブジェクトです)
- SWF ファイルを開く: 公開された SWF を編集可能なプロジェクトとして読み込む(シェイプ、シンボル、タイムライン)。`.swf` をオペレーティングシステムに登録する
- 描画品質: ブラシのダイナミクスと、ビットマップをベクターシェイプにトレースすること
- ドキュメントをより多く保持する SWF: 状態を持つボタン、モーフとしてのシェイプトゥイーン、複数のシェイプによるマスク、シンボル内のサウンド
- モーションエディター: プロパティごとのグラフと、トゥイーン用のカスタムイーズカーブ
- SVG と Photoshop ファイルからのアートワークの読み込み
- ボーンと逆運動学(ボーンツールとバインドツール)
- 配置後のテキスト編集
- 編集可能なキーボードショートカット

優先度は低めですが、次も予定しています: ActionScript とスクリプトエディター(読み込んだファイル内の ActionScript はデータとして保持され、実行されません)、シェイプヒントと複数シーン、Animate の残りのツール、カスタムテーマ、完全に並べ替えられるツールバー、公開されたウェブ版。

### ドキュメント

[docs/README.md](../README.md) は Flare の作りを説明しています。

コントリビューターは [CONTRIBUTING.md](../../CONTRIBUTING.md)、[AGENTS.md](../../AGENTS.md)、[SECURITY.md](../../SECURITY.md)、[CHANGELOG.md](../../CHANGELOG.md) を参照してください。これらは英語です。

### コミュニティ

[Discord](https://discord.com/invite/JpeScW8Awa) · [Website](https://flare-animate.github.io/website/) · [Discussions](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### ライセンス

BSD 3-Clause。[LICENSE](../../LICENSE) を参照してください。

---

*この翻訳は[英語版 README](../../README.md)より遅れることがあります。英語版が正です。*
