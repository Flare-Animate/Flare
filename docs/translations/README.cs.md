[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [Italiano](README.it.md) | [Português (Brasil)](README.pt-BR.md) | **Čeština** | [日本語](README.ja.md) | [한국어](README.ko.md) | [Русский](README.ru.md) | [简体中文](README.zh-CN.md)

<div align="center">

# Flare

<img src="../../assets/BannerNew.png" width="969" height="261" alt="Banner Flare">

**Svobodná animační sada s otevřeným zdrojovým kódem, která navazuje na uživatele Adobe Animate a Flash.**

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Status](https://img.shields.io/badge/status-early%20alpha-red)

[Web](https://flare-animate.github.io/website/) · [Webová verze](https://flare-animate.github.io/Flare/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [Sestavení](../BUILDING.md) · [Issues](https://github.com/Flare-Animate/Flare/issues)

</div>

> **Včasná alfa.** Počítejte s chybami a neúplnou věrností importu Flash/Animate. Zálohujte si svou práci.

## Co je Flare?

Flare, také nazývaný *FlareCraft*, je animační sada ve vývoji (Work In Progress), navržená tak, aby usnadnila přechod umělců a animátorů ze softwaru jako *Adobe Animate*, *Adobe/Macromedia Flash* a *Moho*.

Cílem je podporovat formáty, které tyto nástroje používají, aby lidé mohli otevírat a dál pracovat se souborů projektů, které již mají, a uspořádat uživatelské rozhraní jednodušším a známějším způsobem, který lze přizpůsobit.

Flare se přepisuje v jazyce **Rust**. Vychází především z *OpenToonzu* a *Tahoma2D* a obsahuje funkce převzaté z *AnimationCraftu* a *Next2Flash*, které byly do Flare začleněny. Původní kód v C++ je uchován jako archiv v [`original/`](../../original/README.md).

## Funkce

- Kreslení perem, štětcem a tvarovými nástroji, s výběrovým nástrojem pro přesouvání, seskupování a úpravy tvarů a se styly tahu (plný, čárkovaný, tečkovaný, puntíkovaný, šrafovaný, roztřepený, rybí kost, lano, cik-cak).
- Časová osa s snímky, vrstvami, složkami, maskami, vodicími čárami pohybu, cibulovou slupkou a vrstvou kamery.
- Animace pohybu a tvaru se zpomalením a editor pohybu pro křivky úseků.
- Symboly (Grafika, Filmový klip a Tlačítko) v knihovně, umísťované na scénu jako instance.
- Nastavení relace stop motion. Zachycení kamerou zatím není hotové.
- Přehrávání zvuku na počítači a Testovací film, který spouští publikovaný SWF v Ruffle.
- Otevírání a ukládání projektů `.flare`. Import a export XFL a `.fla` z Animate a otevírání filmů Flash (`.swf`) jako upravitelných projektů.
- Import projektů Moho a pohybu BVH, scén OpenToonzu `.tnz` a úrovní `.pli` a také grafiky a filmů (SVG, PNG, JPEG, GIF, WebP, PSD, MP4, MOV, AVI, MKV, WebM, MPEG, FLV) jako upravitelných projektů.
- Export PNG, JPEG, GIF, APNG, animovaného WebP, SVG, PSD a MP4, ostatních formátů filmů přes ffmpeg, a publikování SWF nebo stránky HTML5 Canvas.
- Příkazový řádek (`flare`) a server MCP (`flare-mcp`) pro agenty, které umožňují vytvářet, upravovat, exportovat a publikovat projekty ze skriptů.
- Webová verze stejného editoru. Co dělá jinak, popisuje [docs/WEB.md](../WEB.md).

[docs/SURFACES.md](../SURFACES.md) uvádí funkci po funkci, co umí desktopová aplikace, webová verze, příkazový řádek a server MCP, a kde jsou mezery.

### Formáty

- Flash/Animate: `.fla`, `.xfl`, `.swf` a starší binární `.fla` (importuje se jako kostra). Ukládání a export do `.fla`, `.xfl` a `.flare`.
- Moho: `.moho`, `.anime`, `.mohoproj`, `.animeproj` a pohyb BVH. Vrstvy, kosti, klíčové snímky a vektorová grafika se importují jako upravitelné projekty; obrázky a zvuk se pouze vypisují, neumisťují se.
- OpenToonz: import scén `.tnz` a vektorových úrovní `.pli` (snímková frekvence, kamera, sloupce xsheetu, tahy, barvy palety). Rastrové úrovně `.tlv` se odmítají, `.tzu` je v plánu.
- Grafika: SVG, PNG, APNG, JPEG, GIF, WebP, TIFF, BMP, TGA, ICO, QOI, PNM, HDR, OpenEXR, DDS a PSD se importují jako bitmapové vrstvy (animované GIF, APNG a WebP jako animace); exportují se SVG a PSD. AVIF a HEIC zatím nejsou hotové.

### Platformy

| Cíl | Stav | Poznámky |
|---|---|---|
| Windows (desktop) | Sestavuje se ze zdrojových kódů | Toolchain MSVC a `flare-desktop`. |
| Linux (desktop) | Sestavuje se ze zdrojových kódů | Potřebuje knihovny uvedené v [CONTRIBUTING.md](../../CONTRIBUTING.md). Testovací film potřebuje Ruffle sestavený z podmodulu. |
| macOS (desktop) | Sestavuje se ze zdrojových kódů | Nativní nabídka. Tlak pera se zatím na macOS do štětce nedostane. |
| Web (prohlížeč) | Živé, včasná verze | [Webová verze](https://flare-animate.github.io/Flare/) se nasazuje z `main`. Bez exportu MP4, bez zvuku a Testovací film běží na stránce. |
| Android | Experimentální | První sestavení `flare-android`, desktopového pracovního prostoru v nativní aktivitě. Soubory zatím nelze otevřít ani uložit přes výběr souborů telefonu a Testovací film je vypnutý. |
| ChromeOS | Netestováno | Sestavení pro Android vytváří APK pro x86_64 a arm64, které by tam měly fungovat. Na ChromeOS se nic netestovalo. |
| iOS | Nesestavuje se | Aplikace pro iOS neexistuje. Úloha CI pro iOS pouze zkouší sestavení a nebrání ostatním krokům. |

### Ke stažení

Zatím neexistuje stabilní vydání. Každou noc se jako předběžné vydání publikuje sestavení `main`, [nightly](https://github.com/Flare-Animate/Flare/releases/tag/nightly): instalátory pro Windows a přenosné zip archivy, diskové obrazy pro macOS, balíčky pro Linux `.deb`, `.rpm`, `.AppImage`, `.flatpak` a `.tar.gz`, archivy příkazového řádku, APK pro Android a webové sestavení. Noční sestavení se přestavuje, jen když má `main` nové commity, a uchovává pouze poslední sestavení. Jeho balíčky nejsou podepsány pro distribuci; viz [docs/RELEASE.md](../RELEASE.md). Pro vše ostatní sestavte ze zdrojových kódů (viz níže).

## Sestavení a spuštění

Rust je připnutý v `rust-toolchain.toml`. Ruffle je submodul gitu, proto klonujte se submoduly:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

Na Linuxu nainstalujte knihovny uvedené v [CONTRIBUTING.md](../../CONTRIBUTING.md).

[docs/BUILDING.md](../BUILDING.md) obsahuje kroky pro každou platformu: Linux, Windows, macOS, webové sestavení a Android.

## Příkazový řádek

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

## Proč Flare existuje

Adobe [oznámila](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) konec životnosti Animate a později [změnila kurz](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). Obrat nic nemění na tom, jak nestabilní animační průmysl je ani na tom, kolik z něj ovládá jedna firma. Animátoři upadli do oprávněné paniky a kariéry lidí by neměly záviset na rozhodnutí jedné firmy dál prodávat produkt.

Flare začal jako hřiště pro snazší učení animace, později byl zveřejněn jako open source a od té doby rostl.

## Plán

Další kroky, nejdůležitější jako první. Většina je v rozhraní zobrazena zašedle. [docs/ROADMAP.md](../ROADMAP.md) tento seznam opakuje a pokračuje k vší méně důležité práci.

- Slučování kreseb: planární mapy tvarů z Flash, kde se překrývající tvary navzájem řežou a slučují (dnes jsou tvary samostatné objekty)
- Kvalita kreslení: dynamika štětce a převod bitmapy na vektorové tvary
- SWF, které zachová více z dokumentu: tlačítka se svými stavy, tvarové animace jako morfy, masky z více tvarů a zvuky uvnitř symbolů
- Editor pohybu: grafy pro jednotlivé vlastnosti a vlastní křivky zpomalení pro animace
- Import grafiky ze souborů SVG a Photoshop
- Kosti uvnitř tvarů a nástroj Bind (kosti, které propojují instance symbolů a jsou ovládány inverzní kinematikou nástrojem Kost, jsou hotové)
- Upravitelné klávesové zkratky

Také plánováno s nižší prioritou: ActionScript a editor skriptů (ActionScript v importovaném souboru se uchovává jako data a nikdy se nespouští), tvarové nápovědy a více než jedna scéna, zbylé nástroje Animate, vlastní motivy, plně přeuspořádatelný panel nástrojů a vydaná webová verze.

## Dokumentace

[docs/README.md](../README.md) vysvětluje, jak je Flare vytvářen, a popisuje, co obsahuje každý dokument.

Přispěvatelé by se měli podívat do [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) a [CHANGELOG.md](../../CHANGELOG.md).

## Původní kódová základna

[`original/`](../../original/README.md) je archivovaná kódová základna v C++, založená na OpenToonzu a Tahoma2D. Slouží pouze jako reference. Nic ve Flare na ní nezávisí ani na ni neodkazuje. Data a texty z ní převzaté se převádějí do formátů Flare a uvádějí se v [docs/ENGINEERING.md](../ENGINEERING.md).

## Poděkování

Flare stojí na těchto projektech. Jejich licenční upozornění se zachovávají tam, kde byl kód portován nebo použit.

- **OpenToonz** (BSD-3-Clause, Copyright (c) DWANGO Co., Ltd. and contributors): základ původní kódové základny. Čtečka PLI v `crates/io/flare-toonz` je z něj portována. Jeho soubory překladů byly výchozím bodem ruského, českého a italského rozhraní.
- **Tahoma2D** (BSD-3-Clause): fork OpenToonzu a druhý základ původní kódové základny. Jeho licence a README byly přečteny; zdrojové kódy nástrojů se pouze porovnávaly.
- **AnimationCraft** (`MIT OR Apache-2.0`): autorský model v Rustu, který Flare pohltil.
- **Next2Flash**: jeho části pod MIT lze portovat. Jeho pracovník FFDec je pod GPLv3, proto se používá pouze chování.
- **jpdurigan/godot_moho_importer** (MIT, Copyright (c) 2022 Joao Pedro Durigan): rozložení sítí Moho používané v `crates/io/flare-moho`.
- **usvg** (MIT/Apache-2.0): čtečka SVG, používaná jako závislost.
- **Ruffle** (`third_party/ruffle`, submodul používaný beze změn pod vlastní licencí): přehrávání Flash pro Testovací film a crate `swf` pro publikování SWF.
- **ArtCraft** (`MIT OR Apache-2.0`) a **minnamate** (bez licenčního souboru, pouze chování) jsou studované zdroje. Jejich licence jsou uvedeny v [docs/BACKLOG.md](../BACKLOG.md).
- **nflare** a **FFDec** (GPLv3): pouze chování, žádný kód. **SimplSam/moho-bvh-bones** (CC BY-NC-SA 4.0): nepoužito.

## Komunita

[Discord](https://discord.com/invite/JpeScW8Awa) · [Web](https://flare-animate.github.io/website/) · [Diskuse](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## Licence

BSD 3-Clause. Viz [LICENSE](../../LICENSE). Komponenty třetích stran si zachovávají své vlastní licence.
