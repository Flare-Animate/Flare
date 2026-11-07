//! Text fields as text objects: the words, the first run's style, the place of the field, and the
//! failure paths that keep the field as source data.
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_document::{Content, Shape};
use flare_import::{FileSource, import_xfl};

#[derive(Default)]
struct Files(HashMap<String, String>);

impl FileSource for Files {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).map(|s| s.clone().into_bytes()))
    }
}

/// A static text field with one run per `(words, attrs)` pair.
fn field(kind: &str, left: f64, top: f64, matrix: &str, runs: &[(&str, &str)]) -> String {
    let runs: String = runs
        .iter()
        .map(|(words, attrs)| {
            format!(
                r#"<DOMTextRun><characters>{words}</characters><textAttrs><DOMTextAttrs {attrs}/></textAttrs></DOMTextRun>"#
            )
        })
        .collect();
    format!(
        r#"<{kind} left="{left}" top="{top}"><matrix><Matrix {matrix}/></matrix><textRuns>{runs}</textRuns></{kind}>"#
    )
}

fn import(element: &str) -> flare_import::Imported {
    let xml = format!(
        r#"<DOMDocument frameRate="24" width="100" height="100"><timelines><DOMTimeline name="Main"><layers><DOMLayer name="L"><frames>
        <DOMFrame index="0" duration="1"><elements>{element}</elements></DOMFrame>
        </frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"#
    );
    let mut files = Files::default();
    files.0.insert("DOMDocument.xml".to_owned(), xml);
    import_xfl(&files).expect("import")
}

/// The text shapes of the first layer, in order.
fn texts(imported: &flare_import::Imported) -> Vec<Shape> {
    imported.document.scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .filter_map(|span| match span.content() {
            Content::Shape(shape) if shape.text().is_some() => Some(shape.clone()),
            _ => None,
        })
        .collect()
}

fn notes(imported: &flare_import::Imported) -> Vec<String> {
    imported
        .diagnostics
        .iter()
        .map(|d| format!("{:?}: {}", d.severity, d.message))
        .collect()
}

#[test]
fn a_static_field_becomes_a_text_object_with_its_words_style_and_place() {
    let imported = import(&field(
        "DOMStaticText",
        10.0,
        20.0,
        r#"a="1" d="1" tx="0" ty="0""#,
        &[(
            "Hello",
            r##"face="Arial" size="24" bold="true" fillColor="#FF0000""##,
        )],
    ));
    let shapes = texts(&imported);
    assert_eq!(shapes.len(), 1, "{:?}", notes(&imported));
    let source = shapes[0].text().expect("text");
    assert_eq!(source.text(), "Hello");
    assert_eq!(source.family(), "Arial");
    assert!((source.size() - 24.0).abs() < 1e-6);
    assert!(source.bold() && !source.italic());
    assert!((source.origin().as_vec().x - 10.0).abs() < 1e-9);
    assert!((source.origin().as_vec().y - 20.0).abs() < 1e-9);
    assert_eq!(shapes[0].fill().r, 255);
    assert_eq!(shapes[0].fill().g, 0);
    assert!(!shapes[0].path().is_empty(), "the letters have an outline");
}

#[test]
fn the_matrix_scales_the_size_and_places_the_corner() {
    let imported = import(&field(
        "DOMStaticText",
        10.0,
        20.0,
        r#"a="2" d="2" tx="5" ty="7""#,
        &[("Hi", r#"size="10""#)],
    ));
    let source = texts(&imported)[0].text().cloned().expect("text");
    assert!((source.size() - 20.0).abs() < 1e-6, "{}", source.size());
    assert!((source.origin().as_vec().x - 25.0).abs() < 1e-9);
    assert!((source.origin().as_vec().y - 47.0).abs() < 1e-9);
}

#[test]
fn a_dynamic_field_keeps_its_words_and_says_its_variable_is_not_carried() {
    let imported = import(&field(
        "DOMDynamicText",
        0.0,
        0.0,
        r#"a="1" d="1""#,
        &[("Score", r#"size="12""#)],
    ));
    assert_eq!(texts(&imported).len(), 1);
    let all = notes(&imported).join("\n");
    assert!(all.contains("keeps its words as static text"), "{all}");
}

#[test]
fn runs_with_different_styles_use_the_first_and_say_so() {
    let imported = import(&field(
        "DOMStaticText",
        0.0,
        0.0,
        r#"a="1" d="1""#,
        &[("A", r#"size="12""#), ("B", r#"size="30""#)],
    ));
    let shape = texts(&imported);
    assert_eq!(shape.len(), 1);
    assert_eq!(shape[0].text().expect("text").text(), "AB");
    assert!((shape[0].text().expect("text").size() - 12.0).abs() < 1e-6);
    assert!(notes(&imported).join("\n").contains("first run's style"));
}

#[test]
fn an_empty_field_makes_nothing_and_says_so() {
    let imported = import(&field(
        "DOMStaticText",
        0.0,
        0.0,
        r#"a="1" d="1""#,
        &[("  ", "")],
    ));
    assert!(texts(&imported).is_empty());
    assert!(notes(&imported).join("\n").contains("empty text field"));
}

#[test]
fn a_size_that_text_cannot_have_is_refused_and_the_field_is_kept_as_source() {
    let imported = import(&field(
        "DOMStaticText",
        0.0,
        0.0,
        r#"a="1" d="1""#,
        &[("Huge", r#"size="100000""#)],
    ));
    assert!(texts(&imported).is_empty());
    let all = notes(&imported).join("\n");
    assert!(all.contains("unusable size"), "{all}");
    assert!(
        !imported.document.attachments().is_empty(),
        "the field is kept, not dropped"
    );
}
