//! The Home page, the Custom Size window, the messages the workspace says and the dialog and
//! Test Movie texts, in each language the interface speaks.

use super::*;
use flare_i18n::{Language, Translator};

fn drawn(ws: &mut Workspace) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut all = Vec::new();
    for step in 0..5 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
        all = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
    }
    all
}

fn has(texts: &[String], wanted: &str) -> bool {
    texts.iter().any(|t| t == wanted)
}

fn speaking(tag: &str) -> Workspace {
    let mut ws = workspace();
    ws.set_system_locale(tag);
    ws
}

#[test]
fn the_home_page_is_in_the_language() {
    for (tag, wanted) in [
        (
            "es-ES",
            [
                "Archivo nuevo",
                "Crear nuevo",
                "Tamaño personalizado",
                "Recientes",
            ],
        ),
        (
            "de-DE",
            [
                "Neue Datei",
                "Neu erstellen",
                "Benutzerdefinierte Größe",
                "Zuletzt verwendet",
            ],
        ),
        (
            "fr-FR",
            [
                "Nouveau fichier",
                "Créer",
                "Taille personnalisée",
                "Récents",
            ],
        ),
        (
            "pt-BR",
            [
                "Novo arquivo",
                "Criar novo",
                "Tamanho personalizado",
                "Recentes",
            ],
        ),
        (
            "ja-JP",
            [
                "新規ファイル",
                "新規作成",
                "カスタムサイズ",
                "最近使用したファイル",
            ],
        ),
        (
            "ko-KR",
            ["새 파일", "새로 만들기", "사용자 정의 크기", "최근 항목"],
        ),
        ("zh-CN", ["新建文件", "新建", "自定义尺寸", "最近"]),
    ] {
        let mut ws = speaking(tag);
        ws.set_home(true);
        let text = drawn(&mut ws);
        for word in wanted {
            assert!(has(&text, word), "{tag}: {word} missing in {text:?}");
        }
        for english in [
            "New File",
            "Create new",
            "Custom Size",
            "Recent",
            "Character Animation",
            "Home",
            "Learn",
            "Open…",
            "Import…",
            "Projects you open or save show up here.",
        ] {
            assert!(
                !has(&text, english),
                "{tag}: {english} is still English: {text:?}"
            );
        }
        // German and French spell it the same.
        assert!(
            tag.starts_with("de")
                || tag.starts_with("fr")
                || text.iter().all(|t| !t.starts_with("Version ")),
            "{tag}: the version is still English"
        );
    }
}

#[test]
fn the_custom_size_window_is_in_the_language() {
    let mut ws = speaking("es-ES");
    ws.set_home(true);
    ws.show_custom_size();
    let text = drawn(&mut ws);
    for wanted in [
        "Ancho",
        "Alto",
        "Velocidad de fotogramas",
        "Crear",
        "Cancelar",
    ] {
        assert!(has(&text, wanted), "{wanted} missing in {text:?}");
    }
    for english in ["Width", "Height", "Frame rate", "Create", "Cancel"] {
        assert!(!has(&text, english), "{english} is still English: {text:?}");
    }
}

#[test]
fn the_recent_list_is_headed_and_dated_in_the_language() {
    let mut ws = speaking("de-DE");
    ws.restore_preferences(r#"{"recent":["/no/such/folder/a.flare"]}"#)
        .expect("preferences");
    ws.set_home(true);
    let text = drawn(&mut ws);
    for wanted in ["Name", "Änderungsdatum", "Größe", "Art"] {
        assert!(has(&text, wanted), "{wanted} missing in {text:?}");
    }
    for english in ["Date modified", "Size", "Kind"] {
        assert!(!has(&text, english), "{english} is still English: {text:?}");
    }
}

#[test]
fn messages_the_workspace_says_are_in_the_language() {
    for (tag, rate, reverse) in [
        (
            "es-ES",
            "La velocidad de fotogramas debe estar entre 0,01 y 240 fps",
            "Selecciona primero los fotogramas que invertir: haz clic en uno y en otro con Mayús",
        ),
        (
            "ja-JP",
            "フレームレートは 0.01 から 240 fps の間で指定してください",
            "先に反転するフレームを選択してください: 1 つをクリックし、別の 1 つを Shift キーを押しながらクリックします",
        ),
        (
            "en-US",
            "The frame rate must be between 0.01 and 240 fps",
            "Select the frames to reverse first: click one, Shift-click another",
        ),
    ] {
        let mut ws = speaking(tag);
        ws.set_frame_rate(500.0);
        assert_eq!(ws.status(), Some(rate), "{tag}");
        ws.reverse_frames();
        assert_eq!(ws.status(), Some(reverse), "{tag}");
    }
}

#[test]
fn a_message_with_a_path_in_it_is_in_the_language_too() {
    let mut ws = speaking("fr-FR");
    ws.open(std::path::Path::new("/no/such/folder/missing.flare"));
    let status = ws.status().expect("a message");
    assert!(
        status.starts_with("Impossible d’ouvrir /no/such/folder/missing.flare"),
        "{status}"
    );
}

#[test]
fn the_history_starts_with_original_in_the_language() {
    let ws = speaking("es-ES");
    assert_eq!(ws.history_entries().0, ["Original"]);
    let ws = speaking("zh-CN");
    assert_eq!(ws.history_entries().0, ["原始"]);
}

#[test]
fn the_open_recent_and_publish_settings_windows_are_in_the_language() {
    let mut ws = speaking("es-ES");
    ws.windows.recent = true;
    ws.windows.publish_settings = true;
    let text = drawn(&mut ws);
    for wanted in [
        "Abrir reciente",
        "Todavía no se ha abierto ni guardado nada.",
        "Configuración de publicación",
    ] {
        assert!(has(&text, wanted), "{wanted} missing in {text:?}");
    }
}

/// Keys of a section's English file, in order.
fn keys(source: &str) -> Vec<&str> {
    source
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(key, _)| key))
        .collect()
}

#[test]
fn nothing_in_these_sections_is_left_in_english() {
    // Words that are the same in a language as in English, not a missing translation.
    let alike = [
        ("start-preset-web", &["de", "es", "fr", "pt-BR", "ja"][..]),
        ("start-preset-portrait", &["fr"][..]),
        ("msg-options", &["fr"][..]),
        ("start-col-name", &["de", "fr"][..]),
        ("start-kind-file", &[][..]),
        ("msg-history-original", &["de", "es", "fr", "pt-BR"][..]),
        ("movie-pause", &["de", "fr"][..]),
        ("start-version", &["de", "fr"][..]),
    ];
    let english = Translator::new(Language::English);
    let mut english_left = Vec::new();
    for source in [
        include_str!("../../../../../../localizations/en/start.ftl"),
        include_str!("../../../../../../localizations/en/messages.ftl"),
        include_str!("../../../../../../localizations/en/dialogs.ftl"),
        include_str!("../../../../../../localizations/en/movie.ftl"),
    ] {
        for language in Language::ALL {
            if language == Language::English {
                continue;
            }
            let translator = Translator::new(language);
            for key in keys(source) {
                assert!(
                    translator.has(key),
                    "{}: {key} is not translated",
                    language.code()
                );
                let is_alike = alike
                    .iter()
                    .any(|(k, codes)| *k == key && codes.contains(&language.code()));
                if !is_alike && translator.text(key) == english.text(key) {
                    english_left.push(format!("{}:{key}", language.code()));
                }
            }
        }
    }
    assert!(english_left.is_empty(), "still English: {english_left:?}");
}
