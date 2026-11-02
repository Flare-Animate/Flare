use super::*;

fn square(x: f64) -> Content {
    Content::Shape(
        Shape::rect(
            ShapeId::new(),
            x,
            0.0,
            10.0,
            10.0,
            Rgba8::new(200, 0, 0, 255),
        )
        .expect("rect"),
    )
}

fn document(span: FrameSpan) -> Document {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

fn with_elements() -> Document {
    // An instance of a symbol the document lacks is invalid, so the elements are all shapes.
    document(
        FrameSpan::new(0, NonZeroU32::new(24).expect("nonzero"), square(0.0))
            .with_extras(vec![square(20.0), square(40.0)])
            .expect("several"),
    )
}

#[test]
fn a_span_with_several_elements_keeps_them_in_order_through_saving_and_opening() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("many.flare");
    let saved = with_elements();
    save(&saved, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded, saved);
    let span = &loaded.scenes()[0].timeline().layers()[0].spans()[0];
    assert_eq!(span.element_count(), 3);
}

#[test]
fn a_span_of_one_element_writes_no_extras_so_older_readers_and_files_are_unchanged() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("one.flare");
    let one = document(FrameSpan::new(
        0,
        NonZeroU32::new(4).expect("nonzero"),
        square(0.0),
    ));
    save(&one, &path).expect("save");
    for row in rows(&path) {
        assert!(
            !row.record.windows(6).any(|w| w == b"extras"),
            "{} record names extras",
            row.kind
        );
    }
    assert_eq!(load(&path).expect("load"), one);
}

fn set_extras(value: &mut serde_json::Value, extras: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if map.contains_key("extras") {
                map.insert("extras".to_owned(), extras.clone());
            }
            for inner in map.values_mut() {
                set_extras(inner, extras);
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(|v| set_extras(v, extras)),
        _ => {}
    }
}

#[test]
fn an_element_that_is_not_a_shape_or_instance_is_a_corrupt_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("bad.flare");
    save(&with_elements(), &path).expect("save");
    edit_values(&path, &["scene"], |v| {
        set_extras(v, &serde_json::json!(["Empty"]));
    });
    assert!(
        matches!(
            load(&path),
            Err(StorageError::Timeline(
                flare_document::TimelineError::NotAnElement
            ))
        ),
        "{:?}",
        load(&path).err()
    );
}

#[test]
fn a_span_with_far_too_many_elements_is_refused_not_allocated() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("huge.flare");
    save(&with_elements(), &path).expect("save");
    let many = serde_json::Value::Array(vec![serde_json::json!("Empty"); 20_000]);
    edit_values(&path, &["scene"], |v| set_extras(v, &many));
    assert!(load(&path).is_err());
}
