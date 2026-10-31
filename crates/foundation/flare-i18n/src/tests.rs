#![allow(clippy::expect_used)]

use super::*;
use fluent_bundle::FluentResource;

fn keys_of(source: &str) -> Vec<String> {
    let resource = FluentResource::try_new(source.to_owned())
        .unwrap_or_else(|(_, errors)| panic!("does not parse: {errors:?}"));
    resource
        .entries()
        .filter_map(|entry| match entry {
            fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_owned()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_language_has_every_section_and_each_file_parses() {
    for language in Language::ALL {
        for (index, section) in SECTIONS.iter().enumerate() {
            let source = language
                .section_source(index)
                .unwrap_or_else(|| panic!("{} has no {section}", language.code()));
            assert!(
                !keys_of(source).is_empty(),
                "{} {section} is empty",
                language.code()
            );
        }
    }
}

#[test]
fn every_language_has_exactly_the_messages_english_has() {
    for (index, section) in SECTIONS.iter().enumerate() {
        let mut english = keys_of(Language::English.section_source(index).expect("en"));
        english.sort();
        let mut unique = english.clone();
        unique.dedup();
        assert_eq!(english, unique, "english {section} repeats a key");
        for language in Language::ALL {
            let mut keys = keys_of(language.section_source(index).expect("section"));
            keys.sort();
            assert_eq!(
                keys,
                english,
                "{} {section} differs from english",
                language.code()
            );
        }
    }
}

#[test]
fn a_translation_keeps_the_placeholders_the_english_has() {
    let english = Translator::new(Language::English);
    for language in Language::ALL {
        let translator = Translator::new(language);
        let said = translator.text_with("prefs-language-system", &[("language", "ZZZ")]);
        assert!(
            said.contains("ZZZ"),
            "{} drops the placeholder: {said}",
            language.code()
        );
        assert!(
            !said.contains('{'),
            "{} leaves braces: {said}",
            language.code()
        );
        assert_eq!(
            english.text_with("prefs-language-system", &[("language", "ZZZ")]),
            "System locale (ZZZ)"
        );
    }
}

#[test]
fn text_is_in_the_language_and_missing_ones_fall_back_to_english_then_to_the_key() {
    let ja = Translator::new(Language::Japanese);
    assert_eq!(ja.text("prefs-title"), "環境設定");
    assert_eq!(Translator::new(Language::German).text("close"), "Schließen");
    assert_eq!(Translator::new(Language::Spanish).text("close"), "Cerrar");
    assert_eq!(Translator::new(Language::French).text("close"), "Fermer");
    assert_eq!(
        Translator::new(Language::French).text("prefs-title"),
        "Préférences"
    );
    assert_eq!(ja.text("no-such-message"), "no-such-message");
    assert!(ja.has("prefs-title") && !ja.has("no-such-message"));
}

#[test]
fn no_text_carries_the_invisible_isolation_marks_fluent_adds_by_default() {
    for language in Language::ALL {
        let said =
            Translator::new(language).text_with("prefs-language-system", &[("language", "x")]);
        assert!(
            said.chars().all(|c| !matches!(c, '\u{2066}'..='\u{2069}')),
            "{}",
            language.code()
        );
    }
}

#[test]
fn a_system_locale_finds_the_nearest_language_and_english_when_there_is_none() {
    for (tag, wanted) in [
        ("en-US", Language::English),
        ("ja-JP", Language::Japanese),
        ("ja_JP.UTF-8", Language::Japanese),
        ("zh-CN", Language::ChineseSimplified),
        ("zh_Hans_CN", Language::ChineseSimplified),
        ("de-AT", Language::German),
        ("es-MX", Language::Spanish),
        ("ko-KR", Language::Korean),
        ("pt-BR", Language::PortugueseBrazil),
        ("pt-PT", Language::PortugueseBrazil),
        ("fr-FR", Language::French),
        ("fr_CA.UTF-8", Language::French),
        ("it-IT", Language::English),
        ("", Language::English),
        ("???", Language::English),
    ] {
        assert_eq!(Language::from_system(tag), wanted, "{tag:?}");
    }
}

#[test]
fn a_choice_is_kept_by_code_and_anything_unknown_follows_the_system() {
    for language in Language::ALL {
        assert_eq!(
            Choice::from_code(Choice::Language(language).code()),
            Choice::Language(language)
        );
    }
    assert_eq!(Choice::from_code("system"), Choice::System);
    assert_eq!(Choice::from_code("klingon"), Choice::System);
    assert_eq!(Choice::from_code(""), Choice::System);
    assert_eq!(Choice::System.resolve("de-DE"), Language::German);
    assert_eq!(
        Choice::Language(Language::Spanish).resolve("de-DE"),
        Language::Spanish
    );
    for language in Language::ALL {
        assert_eq!(Language::from_code(language.code()), Some(language));
    }
}

#[test]
fn the_language_list_is_english_then_latin_names_alphabetically_then_the_rest_by_tag() {
    assert_eq!(Language::ALL[0], Language::English);
    let latin = |l: &Language| l.native_name().chars().all(|c| (c as u32) < 0x250);
    let rest = &Language::ALL[1..];
    let split = rest.iter().take_while(|l| latin(l)).count();
    let (latin_names, others) = rest.split_at(split);
    assert!(
        others.iter().all(|l| !latin(l)),
        "no Latin name after a non-Latin one"
    );
    let names: Vec<&str> = latin_names.iter().map(|l| l.native_name()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable_by_key(|n| n.to_lowercase());
    assert_eq!(names, sorted, "Latin names alphabetical: {names:?}");
    let codes: Vec<&str> = others.iter().map(|l| l.code()).collect();
    let mut sorted_codes = codes.clone();
    sorted_codes.sort_unstable();
    assert_eq!(codes, sorted_codes, "the others by tag: {codes:?}");
    assert!(!latin_names.is_empty() && !others.is_empty());
}

#[test]
fn the_readme_is_translated_into_every_interface_language_and_each_switcher_lists_them_all() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let first_line = |path: std::path::PathBuf| {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is missing: {e}", path.display()));
        text.lines().next().unwrap_or_default().to_owned()
    };
    let mut switchers = vec![("README.md".to_owned(), first_line(root.join("README.md")))];
    for language in Language::ALL {
        if language != Language::English {
            let name = format!("docs/translations/README.{}.md", language.code());
            switchers.push((name.clone(), first_line(root.join(name))));
        }
    }
    for (file, line) in &switchers {
        for language in Language::ALL {
            assert!(
                line.contains(language.native_name()),
                "{file}'s language switcher does not list {}",
                language.native_name()
            );
        }
    }
}
