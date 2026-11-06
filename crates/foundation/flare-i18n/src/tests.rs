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
fn no_message_is_defined_in_two_files_of_a_language() {
    // The files of a language are one bundle, so a key in two of them would hide one meaning.
    for language in Language::ALL {
        let mut seen = std::collections::HashMap::new();
        for (index, section) in SECTIONS.iter().enumerate() {
            for key in keys_of(language.section_source(index).expect("section")) {
                if let Some(other) = seen.insert(key.clone(), *section) {
                    panic!(
                        "{} defines {key} in {other} and in {section}",
                        language.code()
                    );
                }
            }
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

/// The `$names` a message uses, sorted: all of its lines, which a message with forms spreads
/// over several.
fn placeholders_of(source: &str, key: &str) -> Vec<String> {
    let mut block = String::new();
    let mut inside = false;
    for line in source.lines() {
        if line.starts_with(&format!("{key} =")) {
            inside = true;
        } else if inside && !line.starts_with(' ') && !line.is_empty() {
            break;
        }
        if inside {
            block.push_str(line);
            block.push('\n');
        }
    }
    let mut names: Vec<String> = block
        .split('$')
        .skip(1)
        .map(|rest| {
            rest.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect::<String>()
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

#[test]
fn every_translation_uses_exactly_the_placeholders_of_its_english_message() {
    for (index, section) in SECTIONS.iter().enumerate() {
        let english = Language::English.section_source(index).expect("en");
        for key in keys_of(english) {
            let wanted = placeholders_of(english, &key);
            for language in Language::ALL {
                let source = language.section_source(index).expect("section");
                assert_eq!(
                    placeholders_of(source, &key),
                    wanted,
                    "{} {section} {key} does not use the placeholders english does",
                    language.code()
                );
            }
        }
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
        ("it-IT", Language::Italian),
        ("cs-CZ", Language::Czech),
        ("ru-RU", Language::Russian),
        ("ru_RU.UTF-8", Language::Russian),
        ("nl-NL", Language::English),
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

#[test]
fn a_count_takes_the_form_its_language_gives_it() {
    let english = Translator::new(Language::English);
    assert_eq!(english.text_count("start-ago-hour", 1, &[]), "1 hour ago");
    assert_eq!(english.text_count("start-ago-hour", 3, &[]), "3 hours ago");
    assert_eq!(english.text_count("start-ago-hour", 0, &[]), "0 hours ago");
    let french = Translator::new(Language::French);
    assert_eq!(
        french.text_count("start-ago-hour", 0, &[]),
        "il y a 0 heure",
        "French counts zero as one"
    );
    assert_eq!(
        french.text_count("start-ago-hour", 2, &[]),
        "il y a 2 heures"
    );
    let german = Translator::new(Language::German);
    assert_eq!(german.text_count("start-ago-day", 1, &[]), "vor 1 Tag");
    assert_eq!(german.text_count("start-ago-day", 5, &[]), "vor 5 Tagen");
    // Japanese has one form: the same words for any count.
    let japanese = Translator::new(Language::Japanese);
    assert_eq!(japanese.text_count("start-ago-hour", 1, &[]), "1 時間前");
    assert_eq!(japanese.text_count("start-ago-hour", 9, &[]), "9 時間前");
}

#[test]
fn every_language_can_say_how_long_ago_for_any_count() {
    for language in Language::ALL {
        let translator = Translator::new(language);
        for unit in ["minute", "hour", "day", "month", "year"] {
            for count in [0, 1, 2, 5, 21, 100] {
                let said = translator.text_count(&format!("start-ago-{unit}"), count, &[]);
                assert!(
                    said.contains(&count.to_string()) && !said.contains("start-ago"),
                    "{} {unit} {count}: {said}",
                    language.code()
                );
            }
        }
    }
}

// Checked while the tests are built, like a `tr!` is.
const _: () = assert!(has_key("close"));
const _: () = assert!(has_key("pane-tools"));
const _: () = assert!(!has_key("no-such-message"));
const _: () = assert!(!has_key("clos"), "a prefix of a key is not a key");
const _: () = assert!(
    !has_key("close-"),
    "a key with more on the end is not a key"
);

#[test]
fn every_key_english_defines_is_found_and_the_list_is_sorted() {
    assert!(
        KEYS.windows(2).all(|pair| pair[0] < pair[1]),
        "sorted and unique"
    );
    for key in KEYS {
        assert!(has_key(key), "{key}");
        assert!(Translator::new(Language::English).has(key), "{key}");
    }
    assert!(!has_key(""));
}

#[test]
fn counted_messages_say_one_message_and_two_messages() {
    let english = Translator::new(Language::English);
    assert_eq!(english.text_count("out-count", 1, &[]), "1 message");
    assert_eq!(english.text_count("out-count", 2, &[]), "2 messages");
    assert_eq!(
        english.text_count("tools-brush-limit", 1, &[]),
        "Only 1 custom brush can be kept"
    );
    assert_eq!(
        english.text_count("tools-brush-limit", 32, &[]),
        "Only 32 custom brushes can be kept"
    );
    let one = english.text_count("msg-imported", 1, &[("path", "a.fla")]);
    assert!(
        one.contains("1 note about") && one.contains("is in the"),
        "{one}"
    );
    let many = english.text_count("msg-imported", 3, &[("path", "a.fla")]);
    assert!(
        many.contains("3 notes about") && many.contains("are in the"),
        "{many}"
    );
    let german = Translator::new(Language::German);
    assert_eq!(german.text_count("out-count", 1, &[]), "1 Meldung");
    assert_eq!(german.text_count("out-count", 4, &[]), "4 Meldungen");
    let spanish = Translator::new(Language::Spanish);
    assert_eq!(spanish.text_count("out-count", 1, &[]), "1 mensaje");
    assert_eq!(spanish.text_count("out-count", 7, &[]), "7 mensajes");
}

#[test]
fn no_message_hides_a_plural_in_brackets() {
    // "note(s)" says both forms at once and is right in neither: a message with a count has one
    // form per plural category instead. (Korean's 을(를) is a particle that follows the previous
    // word, not a plural, so only Latin letters are looked for.)
    for language in Language::ALL {
        for (index, section) in SECTIONS.iter().enumerate() {
            let source = language.section_source(index).expect("section");
            for line in source.lines() {
                let chars: Vec<char> = line.chars().collect();
                for at in 1..chars.len() {
                    if chars[at] != '(' || !chars[at - 1].is_alphabetic() {
                        continue;
                    }
                    let inside: String =
                        chars[at + 1..].iter().take_while(|c| **c != ')').collect();
                    let closed = chars[at + 1..].contains(&')');
                    assert!(
                        !(closed
                            && (1..=2).contains(&inside.chars().count())
                            && inside.chars().all(|c| c.is_ascii_lowercase())),
                        "{} {section}: a plural in brackets: {line}",
                        language.code()
                    );
                }
            }
        }
    }
}
