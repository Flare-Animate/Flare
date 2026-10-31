[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [Português (Brasil)](README.pt-BR.md) | [日本語](README.ja.md) | **한국어** | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="../../assets/flare-icon-256.png" width="128" height="128" alt="Flare">
</p>

# Flare Animate

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)

Adobe Animate의 취지를 따른, Rust로 작성한 무료 오픈 소스 애니메이션 도구입니다. Windows, macOS, Linux용입니다.

> **초기 알파 버전입니다.** 버그와 불완전한 Flash/Animate 가져오기를 예상하세요. 작업을 백업하세요.

<!-- 여기에 편집기 스크린샷이 들어갑니다: assets/repository/flare-app.png -->

### 기능

- Animate에 있는 도구로 그리며, Flare만의 눈금자(거리와 각도를 잽니다)와 미러(대칭으로 그립니다)도 있습니다. 채우기, 선, 그라데이션을 지원합니다.
- 타임라인에서 프레임과 레이어를 배치합니다. 폴더, 마스크, 모션 가이드, Ease가 있는 모션 트윈과 셰이프 트윈, 어니언 스킨을 지원합니다.
- 라이브러리의 심볼(그래픽, 무비 클립, 버튼)을 인스턴스로 무대에 배치합니다.
- 소리와 함께 애니메이션을 재생합니다. 무비 테스트는 게시한 SWF를 Ruffle에서 실행합니다(데스크톱에서는 앱 안에서, 브라우저에서는 페이지 안에서).
- 여러 문서를 탭으로 열고, 도킹 가능한 패널, 자동 저장, 비정상 종료 후 복구를 지원합니다.
- `.flare` 프로젝트를 열고 저장합니다. Animate의 XFL과 `.fla`를 가져오고 내보내며, 옮기지 못한 내용은 보고합니다.
- PNG, GIF, MP4를 내보내고 SWF 또는 HTML5 Canvas 페이지를 게시합니다.
- 스크립트에서 프로젝트를 만들고, 편집하고, 내보내고, 게시하는 명령줄(`flare`).

### 빌드 및 실행

Rust는 `rust-toolchain.toml`에 고정되어 있습니다. Ruffle은 git 서브모듈이므로 서브모듈과 함께 복제하세요:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Linux에서는 [CONTRIBUTING.md](../../CONTRIBUTING.md)에 나열된 라이브러리를 설치하세요.

릴리스는 macOS, Windows, Linux(x64 및 arm64)용 네이티브 패키지를 만듭니다: DMG, MSI, 휴대용 zip, `.deb`, `.rpm`, AppImage 파일, 그리고 별도 압축 파일로 된 명령줄입니다. 아직 공개된 버전이 없으므로 지금은 소스에서 빌드하세요. 패키지와 릴리스 방법은 [docs/release.md](../release.md)에 있습니다.

### 명령줄

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### 로드맵

계획되어 있지만 아직 만들지 않은 것입니다. 대부분 인터페이스에 흐리게 표시됩니다. 순서와 이유는 [docs/roadmap.md](../roadmap.md)에 있습니다.

- 병합 그리기: Flash의 평면 맵 도형으로, 겹치는 도형이 서로 자르고 합쳐집니다(지금은 도형이 분리된 객체입니다)
- 위치, 확대/축소, 회전을 키프레임으로 움직이는 카메라 레이어
- 본과 역운동학(본 도구와 바인드 도구)
- Animate의 나머지 도구: 3D 회전과 3D 이동, 데코, 폭, 애셋 워프, 그라디언트 변형, 마술봉
- 셰이프 힌트, 둘 이상의 장면, 명령 메뉴를 통한 스크립트
- Flash 8 스타일의 인터페이스 스킨과 사용자 지정 테마(어두운 테마와 밝은 테마는 기본 제공)
- 자유롭게 재배치할 수 있는 도구 모음("…" 목록에서 도구를 고정하는 것은 이미 가능합니다)과 편집 가능한 키보드 단축키
- 배치한 뒤의 텍스트 편집
- 모든 프레임을 고정하지 않고 심볼, 버튼, 트윈을 유지하는 SWF 게시
- 공유 메뉴에서 YouTube 등으로 공유
- 공개된 웹 버전(지금은 실험적입니다)
- macOS의 Developer ID 서명과 공증, 그리고 자동 업데이트

### 문서

[docs/README.md](../README.md)는 Flare가 어떻게 만들어지는지 설명합니다. [CONTRIBUTING.md](../../CONTRIBUTING.md), 작업 규칙인 [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md), [CHANGELOG.md](../../CHANGELOG.md)도 참고하세요. 이 문서들은 영어입니다.

### 커뮤니티

[Discord](https://discord.com/invite/JpeScW8Awa) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### 라이선스

BSD 3-Clause. [LICENSE](../../LICENSE)를 참고하세요.

---

*이 번역은 [영어 README](../../README.md)보다 늦을 수 있으며, 영어판이 기준입니다.*
