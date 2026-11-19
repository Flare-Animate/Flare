#![allow(clippy::expect_used)]

use super::*;
use crate::{TextEngine, TextError, TextStyle};

/// A small Google font (Squada One, SIL Open Font License, licence text beside it).
const SQUADA: &[u8] = include_bytes!("../data/test/SquadaOne-Regular.ttf");

#[test]
fn every_line_of_the_catalog_reads() {
    let lines = CATALOG
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("commit\t"))
        .count();
    assert_eq!(families().len(), lines, "a catalog line was skipped");
    assert!(families().len() > 1500, "{}", families().len());
    assert!(is_commit(commit()), "{}", commit());
}

#[test]
fn families_are_found_whatever_the_case() {
    let roboto = family_named("roboto").expect("Roboto is a Google font");
    assert_eq!(roboto.name(), "Roboto");
    assert_eq!(roboto.category(), "Sans Serif");
    assert_eq!(
        family_named("  Open Sans ").map(GoogleFamily::name),
        Some("Open Sans")
    );
    assert!(family_named("Comic Sans MS").is_none());
    assert!(family_named("").is_none());
}

#[test]
fn addresses_name_the_commit_and_encode_the_file() {
    let roboto = family_named("Roboto").expect("Roboto");
    let urls = roboto.urls();
    assert_eq!(urls.len(), roboto.faces().count());
    let first = &urls[0];
    assert!(
        first.starts_with(&format!(
            "https://raw.githubusercontent.com/google/fonts/{}/ofl/roboto/",
            commit()
        )),
        "{first}"
    );
    assert!(first.ends_with("Roboto%5Bwdth%2Cwght%5D.ttf"), "{first}");
    assert!(urls.iter().all(|url| url.is_ascii() && !url.contains(' ')));
    assert_eq!(
        roboto.cache_names()[0],
        format!("{}/ofl/roboto/Roboto[wdth,wght].ttf", commit())
    );
}

#[test]
fn damaged_catalog_lines_are_skipped() {
    let parsed = parse(concat!(
        "commit\t5bd87dfc525c8a0d1d8529d3684716e00c3fc59a\n",
        "Good\tSerif\tofl/good\tr:Good-Regular.ttf|b:Good-Bold.ttf\n",
        "Escapes\tSerif\tofl/../etc\tr:Good-Regular.ttf\n",
        "Slash\tSerif\tofl/good\tr:../../x.ttf\n",
        "Licence\tSerif\tsecret/good\tr:Good-Regular.ttf\n",
        "Face\tSerif\tofl/good\tx:Good-Regular.ttf\n",
        "Kind\tSerif\tofl/good\tr:Good-Regular.otf\n",
        "Many\tSerif\tofl/good\tr:a.ttf|b:b.ttf|i:c.ttf|bi:d.ttf|r:e.ttf\n",
        "\tSerif\tofl/good\tr:Good-Regular.ttf\n",
        "Short\tSerif\n",
    ));
    assert_eq!(parsed.commit, "5bd87dfc525c8a0d1d8529d3684716e00c3fc59a");
    let names: Vec<&str> = parsed.families.iter().map(|f| f.name).collect();
    assert_eq!(names, ["Good"]);
    assert!(parse("commit\tnot-a-sha\n").commit.is_empty());
}

#[test]
fn only_families_before_one_that_is_here_are_missing() {
    let mut engine = TextEngine::new();
    let missing = |engine: &mut TextEngine, list: &str| -> Vec<&'static str> {
        engine
            .missing_google_families(list)
            .into_iter()
            .map(GoogleFamily::name)
            .collect()
    };
    assert_eq!(missing(&mut engine, "Squada One"), ["Squada One"]);
    assert_eq!(
        missing(
            &mut engine,
            "\"Squada One\", 'Squada One', Source Sans 3, Roboto"
        ),
        ["Squada One"]
    );
    assert!(missing(&mut engine, "sans-serif, Squada One").is_empty());
    assert!(missing(&mut engine, "Source Sans 3, Squada One").is_empty());
    assert!(missing(&mut engine, "Not A Font At All").is_empty());
    assert!(missing(&mut engine, "").is_empty());
}

#[test]
fn a_downloaded_family_sets_text_and_is_no_longer_missing() {
    let mut engine = TextEngine::new();
    let squada = family_named("Squada One").expect("in the catalog");
    let style = TextStyle {
        family: "Squada One".into(),
        size: 40.0,
        bold: false,
        italic: false,
    };
    let fallback = engine.outline("Flare", &style).expect("falls back");
    engine
        .add_google_family(squada, vec![SQUADA.to_vec()])
        .expect("the right font");
    assert!(engine.has_family("Squada One"));
    assert!(engine.missing_google_families("Squada One").is_empty());
    let set = engine.outline("Flare", &style).expect("sets");
    assert_ne!(set, fallback, "the downloaded font was not used");
}

#[test]
fn a_wrong_or_damaged_download_adds_nothing() {
    let mut engine = TextEngine::new();
    let roboto = family_named("Roboto").expect("in the catalog");
    assert_eq!(
        engine.add_google_family(roboto, vec![SQUADA.to_vec()]),
        Err(TextError::WrongFamily {
            wanted: "Roboto".into(),
            found: "Squada One".into()
        })
    );
    assert!(!engine.has_family("Squada One"), "a wrong font was kept");
    let squada = family_named("Squada One").expect("in the catalog");
    for files in [
        vec![],
        vec![b"<html>rate limited</html>".to_vec()],
        vec![SQUADA[..SQUADA.len() / 3].to_vec()],
        vec![SQUADA.to_vec(), b"junk".to_vec()],
        vec![SQUADA.to_vec(); MAX_FAMILY_FILES + 1],
    ] {
        assert_eq!(
            engine.add_google_family(squada, files),
            Err(TextError::NotAFont)
        );
    }
    assert_eq!(
        engine.add_google_family(squada, vec![vec![0; MAX_FONT_BYTES + 1]]),
        Err(TextError::FontTooLarge)
    );
    assert!(!engine.has_family("Squada One"));
}
