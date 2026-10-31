//! The CJK fonts the interface carries cover every character its translations use.
#![allow(clippy::expect_used)]

use egui::FontId;
use flare_i18n::{Language, SECTIONS};

/// Characters from the CJK and Hangul blocks and the punctuation of the CJK languages: what
/// Source Sans cannot draw and a CJK font has to.
fn needs_cjk(c: char) -> bool {
    matches!(c as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7A3 | 0xFF00..=0xFFEF)
}

#[test]
fn every_cjk_character_in_a_translation_is_in_that_languages_embedded_font() {
    let ctx = egui::Context::default();
    flare_ui::theme::apply(&ctx);
    for step in 0..2 {
        let input = egui::RawInput {
            time: Some(f64::from(step)),
            ..egui::RawInput::default()
        };
        ctx.run_ui(input, |_| {}).textures_delta.clear();
    }
    let mut checked = 0;
    for language in Language::ALL {
        let Some(family) = flare_ui::theme::cjk_family(language.code()) else {
            continue;
        };
        let font = FontId::new(14.0, family);
        for index in 0..SECTIONS.len() {
            let source = language.section_source(index).expect("section");
            for c in source.chars().filter(|c| needs_cjk(*c)) {
                checked += 1;
                assert!(
                    ctx.fonts_mut(|f| f.has_glyph(&font, c)),
                    "{} uses {c:?} (U+{:04X}), which its font lacks: run scripts/fonts/subset_cjk.py",
                    language.code(),
                    c as u32
                );
            }
        }
    }
    assert!(
        checked > 100,
        "the test looked at the translations: {checked}"
    );
}

#[test]
fn the_names_in_the_language_list_are_in_their_own_fonts() {
    let ctx = egui::Context::default();
    flare_ui::theme::apply(&ctx);
    for step in 0..2 {
        let input = egui::RawInput {
            time: Some(f64::from(step)),
            ..egui::RawInput::default()
        };
        ctx.run_ui(input, |_| {}).textures_delta.clear();
    }
    for language in Language::ALL {
        if let Some(family) = flare_ui::theme::cjk_family(language.code()) {
            let font = FontId::new(14.0, family);
            assert!(
                ctx.fonts_mut(|f| f.has_glyphs(&font, language.native_name())),
                "{}",
                language.code()
            );
        }
    }
}
