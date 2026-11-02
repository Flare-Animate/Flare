use super::*;
use flare_document::TextSource;

fn with_text() -> Document {
    let source = TextSource::new(
        "Hello\nthere",
        "Helvetica, sans-serif",
        32.0,
        true,
        false,
        DocumentPoint::new(12.5, 40.0).expect("point"),
    )
    .expect("source");
    let shape = Shape::rect(ShapeId::new(), 12.5, 40.0, 80.0, 30.0, Rgba8::WHITE)
        .expect("rect")
        .with_text(source);
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(4).expect("nonzero"),
        Content::Shape(shape),
    );
    let layer = Layer::new(LayerId::new(), "Text", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

#[test]
fn text_keeps_its_words_and_font_through_saving_and_opening() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("text.flare");
    let document = with_text();
    save(&document, &path).expect("save");
    assert_eq!(load(&path).expect("load"), document);
}

#[test]
fn a_file_without_the_text_record_opens_as_plain_outlines() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("text.flare");
    save(&with_text(), &path).expect("save");
    edit_values(&path, &["scene"], |v| strip_key(v, "text"));
    let loaded = load(&path).expect("an absent text record is a plain shape");
    let Content::Shape(shape) = loaded.scenes()[0].timeline().layers()[0].spans()[0].content()
    else {
        panic!("a shape");
    };
    assert!(shape.text().is_none());
}

#[test]
fn a_text_record_that_could_not_be_text_is_a_damaged_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("text.flare");
    for (key, value) in [
        ("size", serde_json::json!(0.0)),
        ("size", serde_json::json!(1.0e9)),
        ("text", serde_json::json!("   ")),
        ("origin", serde_json::json!([null, 1.0])),
    ] {
        save(&with_text(), &path).expect("save");
        edit_values(&path, &["scene"], |v| set_key(v, key, &value));
        assert!(
            matches!(load(&path), Err(StorageError::Corrupt(_))),
            "{key} = {value}"
        );
        assert!(load(&path).is_err(), "{key} = {value}");
    }
}
