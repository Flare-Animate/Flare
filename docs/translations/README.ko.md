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

- Animate에 있는 도구로 그리며, 눈금자(거리와 각도를 잽니다)와 미러 같은 확장된 도구 라이브러리도 있습니다.
- 타임라인에서 프레임과 레이어를 배치합니다. 폴더, 마스크, 모션 가이드, Ease가 있는 모션 트윈과 셰이프 트윈, 어니언 스킨, 카메라 레이어를 지원합니다.
- 라이브러리의 심볼(그래픽, 무비 클립, 버튼)을 인스턴스로 무대에 배치합니다.
- 소리와 함께 애니메이션을 재생합니다. 무비 테스트는 게시한 SWF를 Ruffle에서 실행합니다.
- `.flare` 프로젝트를 열고 저장합니다. Animate의 XFL과 `.fla`를 가져오고 내보냅니다. Flash 무비(`.swf`)를 편집 가능한 프로젝트로 엽니다.
- PNG, GIF, MP4를 내보내고 SWF 또는 HTML5 Canvas 페이지를 게시합니다.
- 스크립트에서 프로젝트를 만들고, 편집하고, 내보내고, 게시하는 명령줄 인터페이스(`flare`).

### 다운로드

Windows, macOS, Linux용 다운로드를 제공할 예정이지만 아직은 없습니다. 패키징이 끝나면 여기에 추가하겠습니다. 그때까지는 소스에서 빌드해 주세요(아래 참고).

### 빌드 및 실행

Rust는 `rust-toolchain.toml`에 고정되어 있습니다. Ruffle은 git 서브모듈이므로 서브모듈과 함께 복제하세요:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Linux에서는 [CONTRIBUTING.md](../../CONTRIBUTING.md)에 나열된 라이브러리를 설치하세요.

### 명령줄

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

### Flare가 존재하는 이유

Adobe는 Animate의 서비스 종료를 [발표](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220)했다가 나중에 [방침을 철회](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459)했습니다. 철회했더라도 이 일은 애니메이션 업계가 얼마나 불안정한지, 특정 기업이 업계를 얼마나 장악하고 있는지를 보여 줍니다. 애니메이터들이 공황에 빠진 것은 당연했습니다. 한 회사가 제품을 계속 판매할지 말지에 사람들의 직업이 좌우되어서는 안 됩니다.

Flare는 애니메이션을 더 쉽게 배울 수 있도록 실험하는 놀이터로 시작해 나중에 오픈 소스로 공개되었고, 거기서 자라났습니다.

### 로드맵

다음에 할 일을 중요한 순서대로 나열했습니다. 대부분 인터페이스에 흐리게 표시됩니다. [docs/ROADMAP.md](../ROADMAP.md)는 이 목록을 그대로 담고 우선순위가 낮은 항목까지 이어서 설명합니다.

- 병합 그리기: Flash의 평면 맵 도형으로, 겹치는 도형이 서로 자르고 합쳐집니다(지금은 도형이 분리된 객체입니다)
- 그리기 품질: 브러시 다이내믹스와 비트맵을 벡터 도형으로 추적하기
- 문서를 더 많이 보존하는 SWF: 상태를 가진 버튼, 모프로서의 모양 트윈, 여러 도형으로 된 마스크, 심볼 안의 사운드
- 모션 편집기: 속성별 그래프와 트윈용 사용자 지정 이즈 곡선
- SVG와 Photoshop 파일에서 아트워크 가져오기
- 도형 안의 본과 바인드 도구(심볼 인스턴스를 잇고 본 도구와 역운동학으로 포즈를 잡는 본은 완성됨)
- 편집 가능한 키보드 단축키

우선순위는 낮지만 다음도 계획되어 있습니다: ActionScript와 스크립트 편집기(가져온 파일의 ActionScript는 데이터로 보존되며 실행되지 않습니다), 모양 힌트와 둘 이상의 장면, Animate의 나머지 도구, 사용자 지정 테마, 완전히 재배치할 수 있는 도구 모음, 공개된 웹 버전.

### 문서

[docs/README.md](../README.md)는 Flare가 어떻게 만들어지는지 설명합니다.

기여자는 [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md), [CHANGELOG.md](../../CHANGELOG.md)를 참고하세요. 이 문서들은 영어입니다.

### 커뮤니티

[Discord](https://discord.com/invite/JpeScW8Awa) · [Website](https://flare-animate.github.io/website/) · [Discussions](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

### 라이선스

BSD 3-Clause. [LICENSE](../../LICENSE)를 참고하세요.

---

*이 번역은 [영어 README](../../README.md)보다 늦을 수 있으며, 영어판이 기준입니다.*
