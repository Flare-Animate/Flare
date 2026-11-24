use super::*;

fn square(x: f64, merged: bool) -> Content {
    Content::Shape(
        Shape::rect(
            ShapeId::new(),
            x,
            0.0,
            10.0,
            10.0,
            Rgba8::new(0, 90, 200, 255),
        )
        .expect("rect")
        .merged(merged),
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

fn shapes(document: &Document) -> Vec<bool> {
    document.scenes()[0].timeline().layers()[0].spans()[0]
        .elements()
        .map(|element| matches!(element, Content::Shape(s) if s.is_merged()))
        .collect()
}

#[test]
fn merged_drawing_and_drawing_objects_stay_apart_through_saving_and_opening() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("merged.flare");
    let saved = document(
        FrameSpan::new(0, NonZeroU32::MIN, square(0.0, true))
            .with_extras(vec![square(20.0, false)])
            .expect("two"),
    );
    save(&saved, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded, saved);
    assert_eq!(shapes(&loaded), [true, false]);
}

#[test]
fn a_drawing_object_writes_nothing_new_so_older_files_read_as_drawing_objects() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("object.flare");
    let one = document(FrameSpan::new(0, NonZeroU32::MIN, square(0.0, false)));
    save(&one, &path).expect("save");
    for row in rows(&path) {
        assert!(
            !row.record.windows(6).any(|w| w == b"merged"),
            "{} record names merged",
            row.kind
        );
    }
    assert_eq!(shapes(&load(&path).expect("load")), [false]);
}
