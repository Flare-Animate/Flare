[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [Português (Brasil)](README.pt-BR.md) | [日本語](README.ja.md) | **한국어** | [简体中文](README.zh-CN.md)

<div align="center">

# Flare

<img src="../../assets/BannerNew.png" width="969" height="261" alt="Flare 배너">

**Adobe Animate와 Flash 사용자를 위한, 작업 연속성을 갖춘 무료 오픈소스 애니메이션 스위트.**

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Status](https://img.shields.io/badge/status-early%20alpha-red)

[웹사이트](https://flare-animate.github.io/website/) · [웹 버전](https://flare-animate.github.io/Flare/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [빌드](../BUILDING.md) · [Issues](https://github.com/Flare-Animate/Flare/issues)

</div>

> **초기 알파.** 버그와 불완전한 Flash/Animate 가져오기 결과를 감수하세요. 작업물은 백업해 두세요.

## Flare란?

Flare는 *FlareCraft*라고도 불리는, (개발 중인) 애니메이션 스위트입니다. *Adobe Animate*, *Adobe/Macromedia Flash*, *Moho* 같은 소프트웨어에서 아티스트와 애니메이터가 쉽게 옮겨 올 수 있도록 설계되었습니다.

이 도구들이 쓰는 형식을 지원해, 사람들이 기존 프로젝트 파일을 그대로 열어 계속 작업할 수 있게 하고, 더 단순하고 익숙한 배치로 UI를 구성하며, 원하는 대로 사용자 지정할 수 있게 하는 것을 목표로 합니다.

Flare는 **Rust**로 다시 작성되고 있습니다. 주로 *OpenToonz*와 *Tahoma2D*를 기반으로 하며, *AnimationCraft*와 *Next2Flash*에서 가져온 기능이 Flare에 흡수되어 있습니다. 이전 C++ 코드는 [`original/`](../../original/README.md)에 보관용으로 남아 있습니다.

## 기능

- 펜, 브러시, 도형 도구로 그리기. 도형을 이동, 그룹화, 편집하는 선택 도구와 선 스타일(실선, 점선, 점묘, 빗금, 거친 선, 헤링본, 밧줄, 지그재그 등)이 있습니다.
- 프레임, 레이어, 폴더, 마스크, 모션 가이드, 어니언 스킨, 카메라 레이어를 갖춘 타임라인.
- 이징이 적용된 모션 트윈과 모양 트윈, 그리고 구간 곡선을 편집하는 Motion Editor.
- 라이브러리의 심볼(그래픽, 무비 클립, 버튼)을 무대에 인스턴스로 배치.
- 스톱모션 세션 설정. 카메라 촬영은 아직 구현되지 않았습니다.
- 데스크톱에서의 사운드 재생, 그리고 게시된 SWF를 Ruffle로 실행하는 "영화 테스트".
- `.flare` 프로젝트 열기와 저장. Animate의 XFL과 `.fla` 가져오기·내보내기, Flash 영화(`.swf`)를 편집 가능한 프로젝트로 열기.
- Moho 프로젝트와 BVH 모션, OpenToonz `.tnz` 장면과 `.pli` 레벨, SVG와 PNG·애니메이션 GIF 이미지를 편집 가능한 프로젝트로 가져오기.
- PNG, GIF, MP4 내보내기, 그리고 SWF 또는 HTML5 Canvas 페이지 게시.
- 스크립트에서 프로젝트를 만들고, 편집하고, 내보내고, 게시하는 명령줄(`flare`)과 에이전트용 MCP 서버(`flare-mcp`).
- 같은 편집기의 브라우저 버전. 차이점은 [docs/WEB.md](../WEB.md)를 보세요.

[docs/SURFACES.md](../SURFACES.md)에는 데스크톱 앱, 브라우저 버전, 명령줄, MCP 서버가 각각 무엇을 할 수 있는지 기능별로 나열하고, 빠진 부분을 적어 두었습니다.

### 형식

- Flash/Animate: `.fla`, `.xfl`, `.swf`, 구버전 바이너리 `.fla`(골격으로 가져옴). `.fla`, `.xfl`, `.flare`로 저장과 내보내기.
- Moho: `.moho`, `.anime`, `.mohoproj`, `.animeproj`, BVH 모션. 레이어, 뼈대, 키프레임, 벡터 아트는 편집 가능한 프로젝트로 가져옵니다. 이미지와 사운드는 목록으로만 표시되고 배치되지 않습니다.
- OpenToonz: `.tnz` 장면과 `.pli` 벡터 레벨 가져오기(프레임 속도, 카메라, xsheet 열, 스트로크, 팔레트 색). `.tlv` 래스터 레벨은 거부되며, `.tzu`는 계획되어 있습니다.
- 이미지: SVG(한 프레임), PNG(한 프레임), 애니메이션 GIF는 비트맵 레이어로 가져옵니다. JPEG, WebP, PSD는 구현되지 않았습니다.

### 플랫폼

| 대상 | 상태 | 비고 |
|---|---|---|
| Windows (데스크톱) | 소스에서 빌드 가능 | MSVC 도구 체인과 `flare-desktop`. |
| Linux (데스크톱) | 소스에서 빌드 가능 | [CONTRIBUTING.md](../../CONTRIBUTING.md)의 라이브러리가 필요합니다. 영화 테스트에는 서브모듈에서 빌드한 Ruffle이 필요합니다. |
| macOS (데스크톱) | 소스에서 빌드 가능 | 네이티브 메뉴. macOS에서는 펜 압력이 아직 브러시에 전달되지 않습니다. |
| 웹 (브라우저) | 서비스 중, 초기 단계 | [웹 버전](https://flare-animate.github.io/Flare/)은 `main`에서 게시됩니다. MP4 내보내기와 사운드는 없고, 영화 테스트는 페이지 안에서 실행됩니다. |
| Android | 실험적 | 네이티브 액티비티 안에서 데스크톱 작업 공간을 실행하는 `flare-android`의 첫 빌드입니다. 휴대폰의 파일 선택기로는 아직 파일을 열고 저장할 수 없으며, 영화 테스트는 꺼져 있습니다. |
| ChromeOS | 미검증 | Android 빌드는 x86_64와 arm64 APK를 만들며, 그곳에서도 실행될 것으로 예상됩니다. ChromeOS에서 검증한 것은 없습니다. |
| iOS | 미구현 | iOS 앱은 없습니다. iOS CI 작업은 컴파일만 시도하며 실패해도 막지 않습니다. |

### 다운로드

아직 안정 버전 릴리스는 없습니다. 매일 밤 `main`의 빌드가 프리릴리스로 게시됩니다([nightly 릴리스](https://github.com/Flare-Animate/Flare/releases/tag/nightly)). Windows 설치 프로그램과 휴대용 ZIP, macOS 디스크 이미지, Linux `.deb`, `.rpm`, `.AppImage`, `.flatpak`, `.tar.gz` 패키지, 명령줄 아카이브, Android APK, 브라우저 버전이 포함됩니다. nightly는 `main`에 새 커밋이 있을 때만 다시 빌드되며 최신 빌드 하나만 보관합니다. 배포용으로 서명되지 않았으므로 [docs/RELEASE.md](../RELEASE.md)를 참고하세요. 그 밖의 경우에는 소스에서 직접 빌드하세요(아래 참고).

## 빌드와 실행

Rust는 `rust-toolchain.toml`에 고정되어 있습니다. Ruffle은 git 서브모듈이므로, 서브모듈과 함께 클론하세요:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Linux에서는 [CONTRIBUTING.md](../../CONTRIBUTING.md)에 나열된 라이브러리를 설치하세요.

각 플랫폼(Linux, Windows, macOS, 브라우저 버전, Android)의 절차는 [docs/BUILDING.md](../BUILDING.md)에 있습니다.

## 명령줄

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

## Flare가 존재하는 이유

Adobe는 Animate의 지원 종료를 [발표했다가](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) 나중에 [방침을 철회했습니다](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). 철회되었다고 해서 애니메이션 업계가 얼마나 불안정한지, 그리고 한 회사가 그 업계를 얼마나 좌우하는지가 달라지지는 않습니다. 애니메이터들은 정당하게 공황에 빠졌고, 한 제품을 계속 판매할지에 대한 한 회사의 결정에 사람들의 경력이 좌우되어서는 안 됩니다.

Flare는 애니메이션을 더 쉽게 배울 수 있도록 하는 놀이터로 시작했고, 이후 오픈소스로 공개되어 거기서부터 성장해 왔습니다.

## 로드맵

다음에 할 일을 중요도 순으로 나열합니다. 대부분은 인터페이스에서 흐리게 표시됩니다. [docs/ROADMAP.md](../ROADMAP.md)는 이 목록을 그대로 반복하고, 그 이하 우선순위의 모든 항목을 이어서 다룹니다.

- 드로잉 병합: 겹치는 도형이 서로 잘려 나가고 합쳐지는 Flash의 평면 맵 도형(현재는 도형이 각각 별개의 객체입니다)
- 드로잉 품질: 브러시 동역학과 비트맵을 벡터 도형으로 추적하기
- 버튼의 상태, 모양 트윈의 모프, 여러 도형의 마스크, 심볼 안의 사운드까지 더 많은 문서를 유지하는 SWF
- Motion Editor: 속성별 그래프와 트윈의 사용자 지정 이징 곡선
- SVG 및 Photoshop 파일에서 아트워크 가져오기
- 도형 안의 본과 바인드 도구(심볼 인스턴스를 잇고 본 도구와 역운동학으로 포즈를 잡는 본은 완성됨)
- 편집 가능한 키보드 단축키

우선순위가 낮은 항목으로는 ActionScript와 스크립트 편집기(가져온 파일의 ActionScript는 데이터로만 보존되며 절대 실행되지 않습니다), 모양 힌트와 여러 장면, Animate의 나머지 도구, 사용자 지정 테마, 완전히 재배치 가능한 도구 모음, 정식 출시된 웹 버전이 계획되어 있습니다.

## 문서

[docs/README.md](../README.md)는 Flare가 어떻게 만들어지는지 설명하고, 각 문서가 무엇을 다루는지 알려 줍니다.

기여자라면 [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md), [CHANGELOG.md](../../CHANGELOG.md)를 읽어 보세요.

## 원본 코드베이스

[`original/`](../../original/README.md)은 OpenToonz와 Tahoma2D를 기반으로 한, 보관된 C++ 코드베이스입니다. 참고용으로만 보관되어 있습니다. Flare는 이것에 의존하거나, 링크하거나, 복사하지 않습니다.

## 크레딧

Flare는 다음 프로젝트들 위에 서 있습니다. 코드를 이식하거나 사용한 곳에서는 각 라이선스 고지를 유지합니다.

- **OpenToonz** (BSD-3-Clause, Copyright (c) DWANGO Co., Ltd. 및 기여자): 원본 코드베이스의 기반입니다. `crates/io/flare-toonz`의 PLI 리더는 여기서 이식되었습니다.
- **Tahoma2D** (BSD-3-Clause): OpenToonz의 포크이며, 원본 코드베이스의 또 다른 기반입니다. 라이선스와 README를 읽었고, 도구 소스는 비교만 했습니다.
- **AnimationCraft** (`MIT OR Apache-2.0`): Flare가 흡수한 Rust 저작 모델입니다.
- **Next2Flash**: 그 MIT 부분은 이식할 수 있습니다. FFDec 워커는 GPLv3이므로 동작만 사용합니다.
- **jpdurigan/godot_moho_importer** (MIT, Copyright (c) 2022 Joao Pedro Durigan): `crates/io/flare-moho`가 사용하는 Moho 메시 구조입니다.
- **usvg** (MIT/Apache-2.0): SVG 리더이며, 의존성으로 사용합니다.
- **Ruffle** (`third_party/ruffle`, 서브모듈로 수정 없이 자체 라이선스에 따라 사용): 영화 테스트를 위한 Flash 재생과, SWF 게시를 위한 `swf` 크레이트.
- **ArtCraft** (`MIT OR Apache-2.0`)와 **minnamate**(라이선스 파일 없음, 동작만 참고): 검토한 출처입니다. 라이선스는 [docs/BACKLOG.md](../BACKLOG.md)에 나열되어 있습니다.
- **nflare**와 **FFDec**(GPLv3): 동작만 참고하며 코드는 쓰지 않습니다. **SimplSam/moho-bvh-bones**(CC BY-NC-SA 4.0): 사용하지 않습니다.

## 커뮤니티

[Discord](https://discord.com/invite/JpeScW8Awa) · [웹사이트](https://flare-animate.github.io/website/) · [토론](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## 라이선스

BSD 3-Clause. [LICENSE](../../LICENSE)를 보세요. 서드파티 구성 요소는 각자의 라이선스를 유지합니다.
