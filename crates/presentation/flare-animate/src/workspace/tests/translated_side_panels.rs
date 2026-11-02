//! The side panels (Align, Transform, Color, Library, History, Info, Output, the Properties
//! tabs) and the status bar in each language: what they draw is the language's text, not English.

use super::*;
use flare_i18n::{Language, Translator};

const TAGS: [&str; 7] = [
    "de-DE", "es-ES", "fr-FR", "pt-BR", "ja-JP", "ko-KR", "zh-CN",
];

/// Words that are the same in a language as in English (a word both share, not a missing
/// translation).
const SHARED: [&str; 8] = [
    "hist-original",
    "col-hex",
    "inf-color",
    "lib-symbol-name",
    "prop-tween",
    "opt-vertical",
    "lib-type",
    "out-count",
];

fn speaking(tag: &str) -> Workspace {
    let mut ws = workspace();
    ws.set_system_locale(tag);
    ws
}

/// Every text one panel draws, after a few passes.
fn panel_text(
    ws: &mut Workspace,
    mut panel: impl FnMut(&mut Workspace, &mut egui::Ui),
) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut all = Vec::new();
    for step in 0..3 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| panel(ws, ui));
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

fn said(language: Language, key: &str) -> String {
    Translator::new(language).text(key)
}

fn english(key: &str) -> String {
    said(Language::English, key)
}

/// Checks that `texts` has what `keys` say in `language`, and that this is not the English.
fn assert_in_language(texts: &[String], language: Language, keys: &[&str], panel: &str) {
    for key in keys {
        let wanted = said(language, key);
        assert!(
            texts.contains(&wanted),
            "{panel} in {}: {key} = {wanted:?} is not drawn: {texts:?}",
            language.code()
        );
        if !SHARED.contains(key) {
            assert_ne!(
                wanted,
                english(key),
                "{key} is not translated in {}",
                language.code()
            );
        }
    }
}

#[test]
fn every_side_panel_draws_its_words_in_each_language() {
    for tag in TAGS {
        let mut ws = speaking(tag);
        let language = ws.language();
        assert_ne!(language, Language::English, "{tag}");

        let text = panel_text(&mut ws, |ws, ui| ws.align_panel(ui));
        assert_in_language(
            &text,
            language,
            &["al-title", "al-distribute", "al-arrange", "al-to-stage"],
            "Align",
        );
        let text = panel_text(&mut ws, |ws, ui| ws.transform_panel(ui));
        assert_in_language(
            &text,
            language,
            &[
                "tf-title",
                "tf-scale",
                "tf-rotate",
                "tf-skew",
                "tf-apply",
                "tf-reset",
                "tf-quick",
            ],
            "Transform",
        );
        let text = panel_text(&mut ws, |ws, ui| ws.color_panel(ui));
        assert_in_language(
            &text,
            language,
            &["col-fill", "col-hex", "col-swatches"],
            "Color",
        );
        let text = panel_text(&mut ws, |ws, ui| ws.library_panel(ui));
        assert_in_language(
            &text,
            language,
            &[
                "lib-none-selected",
                "lib-new-symbols",
                "lib-search",
                "lib-empty",
                "lib-empty-hint",
                "lib-kind-graphic",
            ],
            "Library",
        );
        let text = panel_text(&mut ws, |ws, ui| ws.history_panel(ui));
        assert_in_language(&text, language, &["hist-original"], "History");
        let text = panel_text(&mut ws, |ws, ui| ws.output_panel(ui));
        assert_in_language(&text, language, &["out-nothing"], "Output");
        let text = panel_text(&mut ws, |ws, ui| ws.info_panel(ui));
        assert_in_language(
            &text,
            language,
            &["inf-selection", "inf-pointer", "inf-color"],
            "Info",
        );
        let text = panel_text(&mut ws, |ws, ui| ws.frame_tab(ui));
        assert_in_language(
            &text,
            language,
            &[
                "prop-frame",
                "prop-layer",
                "prop-exposure",
                "prop-empty",
                "prop-tween",
                "prop-motion-tween",
                "prop-ease",
            ],
            "Frame",
        );
        let text = panel_text(&mut ws, |ws, ui| ws.tool_tab(ui));
        assert_in_language(
            &text,
            language,
            &["opt-fill-stroke", "opt-fill", "opt-stroke", "opt-width"],
            "Tool",
        );
    }
}

#[test]
fn a_few_words_are_what_the_adobe_terms_are() {
    // Not derived from the message files, so a wrong or missing entry cannot hide behind itself.
    let mut ws = speaking("de-DE");
    let text = panel_text(&mut ws, |ws, ui| ws.align_panel(ui));
    assert!(text.iter().any(|t| t == "Ausrichten"), "{text:?}");
    assert!(text.iter().any(|t| t == "Verteilen"), "{text:?}");
    assert!(!text.iter().any(|t| t == "Distribute"), "{text:?}");
    let mut ws = speaking("ja-JP");
    let text = panel_text(&mut ws, |ws, ui| ws.library_panel(ui));
    assert!(
        text.iter().any(|t| t == "シンボルが選択されていません"),
        "{text:?}"
    );
    let mut ws = speaking("fr-FR");
    let text = panel_text(&mut ws, |ws, ui| ws.output_panel(ui));
    assert!(text.iter().any(|t| t == "Rien à signaler"), "{text:?}");
}

#[test]
fn the_output_panel_counts_its_messages_in_the_language() {
    let mut ws = speaking("es-ES");
    ws.report_message("uno");
    ws.report_message("dos");
    let text = panel_text(&mut ws, |ws, ui| ws.output_panel(ui));
    assert!(text.iter().any(|t| t == "2 mensajes"), "{text:?}");
    assert!(!text.iter().any(|t| t.contains("messages")), "{text:?}");
}

#[test]
fn the_status_bar_says_frame_and_working_in_the_language() {
    for tag in ["de-DE", "ja-JP", "zh-CN"] {
        let mut ws = speaking(tag);
        let frame = flare_i18n::Translator::new(Language::from_system(tag))
            .text_with("bar-frame", &[("current", "1"), ("total", "1")]);
        assert!(!frame.starts_with("Frame "), "{tag}: {frame}");
        let text = panel_text(&mut ws, |ws, ui| ws.status_bar(ui));
        assert!(text.contains(&frame), "{tag}: {text:?}");
        assert!(!text.iter().any(|t| t.starts_with("Frame ")), "{tag}");
        // The size and rate are numbers and units, the same in every language.
        let stage = ws.document().stage();
        let facts = format!("{} × {} px · 24 fps", stage.width(), stage.height());
        assert!(text.contains(&facts), "{tag}: {text:?}");
    }
}

#[test]
fn symbols_are_named_and_the_library_refuses_in_the_language() {
    let mut ws = speaking("es-ES");
    ws.new_symbol();
    let names: Vec<String> = ws
        .document()
        .symbols()
        .iter()
        .map(|s| s.name().to_owned())
        .collect();
    assert_eq!(names, ["Símbolo 1"]);
    let text = panel_text(&mut ws, |ws, ui| ws.library_panel(ui));
    assert!(text.iter().any(|t| t == "Símbolo 1"), "{text:?}");
    assert!(text.iter().any(|t| t == "Gráfico"), "{text:?}");

    // Inside a symbol the library cannot change, and says so in the language.
    let id = ws.document().symbols()[0].id();
    assert!(ws.enter_symbol(id));
    ws.new_symbol();
    assert_eq!(
        ws.status(),
        Some("Sal de la edición del símbolo para modificar la biblioteca")
    );
    ws.exit_to_depth(0);

    ws.library.selected = None;
    ws.place_selected_symbol();
    assert_eq!(
        ws.status(),
        Some("Primero selecciona un símbolo en la biblioteca")
    );
}

#[test]
fn symbol_kinds_have_a_name_in_every_language() {
    for tag in TAGS {
        let ws = speaking(tag);
        for kind in flare_document::SymbolKind::ALL {
            let name = ws.kind_label(kind);
            assert_ne!(name, kind.label(), "{tag}: {kind:?}");
            assert!(!name.starts_with("lib-"), "{tag}: {name}");
        }
    }
    assert_eq!(
        speaking("de-DE").kind_label(flare_document::SymbolKind::Button),
        "Schaltfläche"
    );
    assert_eq!(
        speaking("ja-JP").kind_label(flare_document::SymbolKind::MovieClip),
        "ムービークリップ"
    );
    assert_eq!(
        workspace().kind_label(flare_document::SymbolKind::MovieClip),
        "Movie Clip"
    );
}

#[test]
fn every_history_step_has_its_name_in_each_language() {
    for tag in TAGS {
        let ws = speaking(tag);
        let language = ws.language();
        for (label, key) in Workspace::HISTORY_LABELS {
            let translated = Translator::new(language).text(key);
            assert_ne!(translated, key, "{tag}: {key} is missing");
            if !SHARED.contains(&key) {
                assert_ne!(translated, label, "{tag}: {key} is still English");
            }
            // A step that is in the list is shown by that name (the first is always the start).
            assert_eq!(ws.history_label(1, label), translated, "{tag}: {label}");
        }
        assert_eq!(ws.history_label(0, "anything"), tr!(ws, "hist-original"));
    }
}

#[test]
fn a_real_edit_shows_in_the_history_list_in_the_language() {
    let mut ws = speaking("de-DE");
    ws.new_symbol();
    let (entries, _) = ws.history_entries();
    assert_eq!(
        entries,
        ["Original", "Add the symbol"],
        "the steps keep their kept names"
    );
    assert_eq!(ws.history_label(1, &entries[1]), "Symbol hinzufügen");
    let text = panel_text(&mut ws, |ws, ui| ws.history_panel(ui));
    assert!(text.iter().any(|t| t == "Symbol hinzufügen"), "{text:?}");
    assert!(!text.iter().any(|t| t == "Add the symbol"), "{text:?}");
    // A name the table does not know is shown as it was kept; a drawing tool's is built.
    assert_eq!(ws.history_label(2, "Something else"), "Something else");
    assert_eq!(ws.history_label(2, "Draw text"), "text zeichnen");
}

#[test]
fn english_is_unchanged() {
    let mut ws = workspace();
    let text = panel_text(&mut ws, |ws, ui| ws.align_panel(ui));
    assert!(text.iter().any(|t| t == "Align to stage"), "{text:?}");
    assert_eq!(ws.history_label(1, "Draw rectangle"), "Draw rectangle");
    assert_eq!(ws.history_label(0, "x"), "Original");
}
