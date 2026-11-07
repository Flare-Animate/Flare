[English](../../README.md) | [Deutsch](README.de.md) | [Español](README.es.md) | [Français](README.fr.md) | [Italiano](README.it.md) | [Português (Brasil)](README.pt-BR.md) | [Čeština](README.cs.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | **Русский** | [简体中文](README.zh-CN.md)

<div align="center">

# Flare

<img src="../../assets/BannerNew.png" width="969" height="261" alt="Баннер Flare">

**Свободный анимационный пакет с открытым исходным кодом, который сохраняет преемственность для пользователей Adobe Animate и Flash.**

[![CI](https://img.shields.io/github/actions/workflow/status/Flare-Animate/Flare/ci.yml?branch=main&label=CI)](https://github.com/Flare-Animate/Flare/actions/workflows/ci.yml)
[![License: BSD 3-Clause](https://img.shields.io/badge/license-BSD%203--Clause-blue)](../../LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/JpeScW8Awa)
[![Issues](https://img.shields.io/github/issues/Flare-Animate/Flare)](https://github.com/Flare-Animate/Flare/issues)
![Status](https://img.shields.io/badge/status-early%20alpha-red)

[Сайт](https://flare-animate.github.io/website/) · [Веб-версия](https://flare-animate.github.io/Flare/) · [Discord](https://discord.com/invite/JpeScW8Awa) · [Сборка](../BUILDING.md) · [Issues](https://github.com/Flare-Animate/Flare/issues)

</div>

> **Ранняя альфа-версия.** Ожидайте ошибок и неполной точности импорта Flash/Animate. Сохраняйте резервные копии своей работы.

## Что такое Flare?

Flare, также называемый *FlareCraft*, — это анимационный пакет (находится в разработке), созданный, чтобы упростить переход художников и аниматоров с таких программ, как *Adobe Animate*, *Adobe/Macromedia Flash* и *Moho*.

Цель — поддерживать форматы, которые используют эти инструменты, чтобы люди могли открывать и продолжать работать с уже имеющимися файлами проектов, а интерфейс строить проще и привычнее, с возможностью настройки.

Flare переписывается на **Rust**. В основе в первую очередь *OpenToonz* и *Tahoma2D*, а также функции из *AnimationCraft* и *Next2Flash*, которые вошли во Flare. Старый код на C++ сохранён как архив в [`original/`](../../original/README.md).

## Возможности

- Рисование пером, кистью и фигурами, инструмент выделения для перемещения, группировки и редактирования фигур, а также стили обводки (сплошная, штриховая, пунктирная, точечная, штриховка, неровная, «ёлочка», верёвка, зигзаг).
- Временная шкала с кадрами, слоями, папками, масками, направляющими движения, луковой кожей и слоем камеры.
- Анимации движения и формы со сглаживанием и редактор движения для кривых отрезков.
- Символы (графика, клип и кнопка) в библиотеке, размещаемые на сцене как экземпляры.
- Настройки сеанса покадровой анимации. Съёмка камерой пока не реализована.
- Воспроизведение звука на настольных системах и «Тестовый ролик», который запускает опубликованный SWF в Ruffle.
- Открытие и сохранение проектов `.flare`. Импорт и экспорт XFL и `.fla` из Animate, а также открытие фильмов Flash (`.swf`) как редактируемых проектов.
- Импорт проектов Moho и движения BVH, сцен OpenToonz `.tnz` и уровней `.pli`, а также иллюстраций и роликов (SVG, PNG, JPEG, GIF, WebP, PSD, MP4, MOV, AVI, MKV, WebM, MPEG, FLV) как редактируемых проектов.
- Экспорт в PNG, JPEG, GIF, APNG, анимированный WebP, SVG, PSD и MP4, другие форматы роликов через ffmpeg, а также публикация SWF или страницы HTML5 Canvas.
- Командная строка (`flare`) и сервер MCP (`flare-mcp`) для агентов: создание, правка, экспорт и публикация проектов из скриптов.
- Браузерная версия того же редактора. См. [docs/WEB.md](../WEB.md), чем она отличается.

[docs/SURFACES.md](../SURFACES.md) перечисляет по каждой функции, что умеют настольное приложение, браузерная версия, командная строка и сервер MCP, и где есть пробелы.

### Форматы

- Flash/Animate: `.fla`, `.xfl`, `.swf` и устаревший двоичный `.fla` (импортируется как скелет). Сохранение и экспорт в `.fla`, `.xfl` и `.flare`.
- Moho: `.moho`, `.anime`, `.mohoproj`, `.animeproj` и движение BVH. Слои, кости, ключевые кадры и векторная графика импортируются как редактируемые проекты; изображения и звук перечисляются, но не размещаются.
- OpenToonz: импорт сцен `.tnz` и векторных уровней `.pli` (частота кадров, камера, столбцы xsheet, штрихи, цвета палитры). Растровые уровни `.tlv` не принимаются, `.tzu` запланирован.
- Иллюстрации: SVG, PNG, APNG, JPEG, GIF, WebP, TIFF, BMP, TGA, ICO, QOI, PNM, HDR, OpenEXR, DDS и PSD импортируются как растровые слои (анимированные GIF, APNG и WebP как анимации); экспортируются SVG и PSD. AVIF и HEIC не реализованы.

### Платформы

| Платформа | Состояние | Примечания |
|---|---|---|
| Windows (настольная) | Собирается из исходников | Набор инструментов MSVC и `flare-desktop`. |
| Linux (настольная) | Собирается из исходников | Нужны библиотеки из [CONTRIBUTING.md](../../CONTRIBUTING.md). Для «Тестового ролика» нужен Ruffle, собранный из подмодуля. |
| macOS (настольная) | Собирается из исходников | Собственное меню. Нажим пера пока не передаётся кисти на macOS. |
| Web (браузер) | Работает, ранняя версия | [Веб-версия](https://flare-animate.github.io/Flare/) публикуется из `main`. Нет экспорта MP4 и звука; «Тестовый ролик» работает на странице. |
| Android | Экспериментально | Первая сборка `flare-android`: настольное рабочее пространство в нативной активности. Файлы пока нельзя открыть или сохранить через выбор файлов телефона, «Тестовый ролик» отключён. |
| ChromeOS | Не проверялось | Сборка Android создаёт APK для x86_64 и arm64, которые должны работать и там. На ChromeOS ничего не тестировалось. |
| iOS | Не собирается | Приложения для iOS нет. Задача CI для iOS только пытается собрать проект и не блокирует сборку. |

### Загрузки

Стабильного выпуска пока нет. Каждую ночь сборка `main` публикуется как предварительный выпуск, [nightly](https://github.com/Flare-Animate/Flare/releases/tag/nightly): установщики для Windows и переносимые zip-архивы, образы дисков для macOS, пакеты Linux `.deb`, `.rpm`, `.AppImage`, `.flatpak` и `.tar.gz`, архивы командной строки, APK для Android и браузерная сборка. Ночная сборка пересобирается, только если в `main` есть новые коммиты, и хранит лишь последнюю сборку. Её пакеты не подписаны для распространения; см. [docs/RELEASE.md](../RELEASE.md). Для всего остального соберите из исходников (см. ниже).

## Сборка и запуск

Rust закреплён в `rust-toolchain.toml`. Ruffle — подмодуль git, поэтому клонируйте репозиторий вместе с подмодулями:

    git clone --recurse-submodules https://github.com/Flare-Animate/Flare.git
    cd Flare
    cargo run --release -p flare-desktop

На Linux установите библиотеки, перечисленные в [CONTRIBUTING.md](../../CONTRIBUTING.md).

[docs/BUILDING.md](../BUILDING.md) содержит шаги для каждой платформы: Linux, Windows, macOS, браузерная версия и Android.

## Командная строка

    flare new project.flare --width 1280 --height 720
    flare draw-rect project.flare --rect 100 100 300 200 --color FF6600
    flare export project.flare frame.png
    flare export project.flare loop.gif --format gif
    flare publish-html5 project.flare page.html
    flare --help

## Зачем нужен Flare

Adobe [объявила](https://community.adobe.com/announcements-539/adobe-animate-end-of-life-and-support-timeline-1548220) о прекращении поддержки Animate, а затем [изменила решение](https://community.adobe.com/announcements-539/update-on-the-status-of-adobe-animate-1548459). Этот поворот не меняет того, насколько неустойчива анимационная отрасль и сколько её контролирует одна компания. Аниматоры оказались в законной панике, а карьера людей не должна зависеть от решения одной компании продолжать продавать продукт.

Flare начинался как площадка для того, чтобы анимацию было легче освоить; позднее его сделали открытым, и он вырос из этого.

## Дорожная карта

Ближайшее, по порядку важности. Большая часть показана в интерфейсе затемнённой. [docs/ROADMAP.md](../ROADMAP.md) повторяет этот список и переходит к остальному, менее приоритетному.

- Объединение рисунков: планарная карта фигур Flash, где пересекающиеся фигуры обрезают и объединяют друг друга (сейчас фигуры — отдельные объекты)
- Качество рисования: динамика кисти и трассировка растрового изображения в векторные фигуры
- SWF, который сохраняет больше документа: кнопки с их состояниями, анимации формы как морфинг, маски из нескольких фигур и звуки внутри символов
- Редактор движения: графики по отдельным свойствам и пользовательские кривые сглаживания для анимаций
- Импорт иллюстраций из файлов SVG и Photoshop
- Кости внутри фигур и инструмент Bind (кости, связывающие экземпляры символов и позируемые обратной кинематикой инструментом «Кость», уже реализованы)
- Редактируемые сочетания клавиш

Также запланировано с меньшим приоритетом: ActionScript и редактор скриптов (ActionScript в импортированном файле хранится как данные и никогда не выполняется), подсказки формы и больше одной сцены, остальные инструменты Animate, пользовательские темы, полностью перестраиваемая панель инструментов и выпущенная веб-сборка.

## Документация

[docs/README.md](../README.md) объясняет, как устроен Flare, и говорит, что содержит каждый документ.

Участникам стоит посмотреть [CONTRIBUTING.md](../../CONTRIBUTING.md), [AGENTS.md](../../AGENTS.md), [SECURITY.md](../../SECURITY.md) и [CHANGELOG.md](../../CHANGELOG.md).

## Исходный код проекта

[`original/`](../../original/README.md) — архив кодовой базы на C++, основанной на OpenToonz и Tahoma2D. Он сохранён как справочный материал. Ничто во Flare не зависит от него и не ссылается на него. Данные и тексты, взятые из него, переводятся в форматы Flare и указываются в [docs/ENGINEERING.md](../ENGINEERING.md).

## Благодарности

Flare опирается на эти проекты. Их уведомления о лицензиях сохраняются там, где код был портирован или использован.

- **OpenToonz** (BSD-3-Clause, Copyright (c) DWANGO Co., Ltd. and contributors): основа исходной кодовой базы. Читатель PLI в `crates/io/flare-toonz` портирован из него. Его файлы переводов послужили основой русского, чешского и итальянского интерфейсов.
- **Tahoma2D** (BSD-3-Clause): форк OpenToonz и вторая основа исходной кодовой базы. Его лицензия и README прочитаны; исходники инструментов только сравнивались.
- **AnimationCraft** (`MIT OR Apache-2.0`): модель авторинга на Rust, которую Flare поглотил.
- **Next2Flash**: его части под MIT могут быть портированы. Его модуль FFDec под GPLv3, поэтому используется только поведение.
- **jpdurigan/godot_moho_importer** (MIT, Copyright (c) 2022 Joao Pedro Durigan): раскладка мешей Moho, используемая `crates/io/flare-moho`.
- **usvg** (MIT/Apache-2.0): чтение SVG, используется как зависимость.
- **Ruffle** (`third_party/ruffle`, подмодуль без изменений, по его собственной лицензии): воспроизведение Flash для «Тестового ролика» и крейт `swf` для публикации SWF.
- **ArtCraft** (`MIT OR Apache-2.0`) и **minnamate** (файла лицензии нет, только поведение) — изучаемые источники. Их лицензии перечислены в [docs/BACKLOG.md](../BACKLOG.md).
- **nflare** и **FFDec** (GPLv3): только поведение, без кода. **SimplSam/moho-bvh-bones** (CC BY-NC-SA 4.0): не используется.

## Сообщество

[Discord](https://discord.com/invite/JpeScW8Awa) · [Сайт](https://flare-animate.github.io/website/) · [Обсуждения](https://github.com/orgs/Flare-Animate/discussions) · [Issues](https://github.com/Flare-Animate/Flare/issues)

## Лицензия

BSD 3-Clause. См. [LICENSE](../../LICENSE). Сторонние компоненты сохраняют собственные лицензии.
