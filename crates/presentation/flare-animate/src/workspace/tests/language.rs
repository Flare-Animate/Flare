//! The interface's language: the system's, a chosen one, and what the dialogs say in it.

use super::*;
use flare_i18n::{Choice, Language};

fn texts(ws: &mut Workspace, ctx: &egui::Context, time: f64) -> Vec<String> {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect()
}

/// Everything drawn in a few passes with the Preferences dialog open.
fn preferences_text(ws: &mut Workspace) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    ws.windows.preferences = true;
    let mut all = Vec::new();
    for step in 0..5 {
        all = texts(ws, &ctx, f64::from(step) * 0.1);
    }
    all
}

fn has(texts: &[String], wanted: &str) -> bool {
    texts.iter().any(|t| t == wanted)
}

#[test]
fn it_starts_in_english_following_the_system_and_the_dialog_says_so() {
    let mut ws = workspace();
    assert_eq!(ws.language(), Language::English);
    assert_eq!(ws.language_choice(), Choice::System);
    let text = preferences_text(&mut ws);
    for wanted in [
        "Preferences",
        "General",
        "Language",
        "System locale (English)",
        "Restore defaults",
    ] {
        assert!(has(&text, wanted), "{wanted}: {text:?}");
    }
}

#[test]
fn following_the_system_speaks_its_language_and_the_list_names_it() {
    let mut ws = workspace();
    ws.set_system_locale("de-DE");
    assert_eq!(ws.language(), Language::German);
    let text = preferences_text(&mut ws);
    assert!(has(&text, "Einstellungen"), "{text:?}");
    assert!(has(&text, "Systemsprache (Deutsch)"), "{text:?}");
    assert!(!has(&text, "Preferences"));
    // A system language there is no translation for is English, and the list says English.
    ws.set_system_locale("fr-FR");
    assert_eq!(ws.language(), Language::English);
    let text = preferences_text(&mut ws);
    assert!(has(&text, "System locale (English)"), "{text:?}");
}

#[test]
fn a_chosen_language_wins_over_the_system_and_every_language_draws_its_dialog() {
    for language in Language::ALL {
        let mut ws = workspace();
        ws.set_system_locale("de-DE");
        let mut prefs = ws.preferences();
        prefs.language = language.code().to_owned();
        ws.apply_preferences(&prefs);
        assert_eq!(ws.language(), language);
        let text = preferences_text(&mut ws);
        let title = flare_i18n::Translator::new(language).text("prefs-title");
        assert!(has(&text, &title), "{}: {text:?}", language.code());
    }
}

#[test]
fn the_choice_is_kept_with_the_preferences_and_an_unknown_one_follows_the_system() {
    let mut ws = workspace();
    let mut prefs = ws.preferences();
    assert_eq!(prefs.language, "system");
    prefs.language = "pt-BR".to_owned();
    ws.apply_preferences(&prefs);
    let saved = ws.preferences_json().expect("json");
    let mut fresh = workspace();
    fresh.restore_preferences(&saved).expect("restored");
    assert_eq!(fresh.language(), Language::PortugueseBrazil);
    assert_eq!(
        fresh.language_choice(),
        Choice::Language(Language::PortugueseBrazil)
    );
    fresh
        .restore_preferences(r#"{"language": "klingon"}"#)
        .expect("odd value");
    assert_eq!(fresh.language_choice(), Choice::System);
    // A file from before the setting existed has none.
    fresh.restore_preferences("{}").expect("old file");
    assert_eq!(fresh.language_choice(), Choice::System);
    // Restoring defaults goes back to following the system.
    ws.apply_preferences(&Preferences::default());
    assert_eq!(ws.language_choice(), Choice::System);
}

#[test]
fn the_publish_popup_and_its_format_names_follow_the_language() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let mut prefs = ws.preferences();
    prefs.language = "es".to_owned();
    ws.apply_preferences(&prefs);
    ws.perform(MenuAction::Publish);
    let mut all = Vec::new();
    for step in 0..5 {
        all = texts(&mut ws, &ctx, f64::from(step) * 0.1);
    }
    for wanted in [
        "Publicar",
        "Formatos",
        "Compartir",
        "GIF animado (.gif)",
        "Cerrar",
    ] {
        assert!(has(&all, wanted), "{wanted}: {all:?}");
    }
    // The places to share are names, the same in every language.
    assert!(has(&all, "Twitter") && has(&all, "Facebook"));
}
