use super::*;

#[test]
fn round_trip_preserves_identity_order_and_matrices() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let document = sample();
    save(&document, &path).expect("save");
    assert_eq!(load(&path).expect("load"), document);
}

#[test]
fn resaving_replaces_the_file_atomically() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("first");
    let second = sample();
    save(&second, &path).expect("second");
    assert_eq!(load(&path).expect("load"), second);
    assert_eq!(
        fs::read_dir(dir.path()).expect("list").count(),
        1,
        "no stray temp files"
    );
}

#[test]
fn failed_save_leaves_previous_file_intact() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let original = sample();
    save(&original, &path).expect("save");
    let missing = dir.path().join("no-such-dir").join("b.flare");
    assert!(matches!(
        save(&sample(), &missing),
        Err(StorageError::Io(_))
    ));
    assert_eq!(load(&path).expect("load"), original);
}

#[test]
fn non_flare_and_truncated_files_are_rejected() {
    let dir = tempfile::tempdir().expect("dir");
    let text = dir.path().join("t.flare");
    fs::write(
        &text,
        b"this is definitely not sqlite, just some long enough text....",
    )
    .expect("w");
    assert!(matches!(load(&text), Err(StorageError::NotFlareFile)));

    let plain = dir.path().join("p.db");
    Connection::open(&plain)
        .expect("open")
        .execute_batch("CREATE TABLE x(a);")
        .expect("x");
    assert!(matches!(load(&plain), Err(StorageError::NotFlareFile)));

    let good = dir.path().join("g.flare");
    save(&sample(), &good).expect("save");
    let bytes = fs::read(&good).expect("read");
    fs::write(&good, &bytes[..bytes.len() / 2]).expect("truncate");
    assert!(load(&good).is_err());
    assert!(load(&dir.path().join("missing.flare")).is_err());
}

#[test]
fn a_stage_that_cannot_exist_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE project SET settings_json = '{\"frame_rate\":[24,1],\"stage\":{\"width\":0,\"height\":5,\"background\":[0,0,0,0]}}'",
        [],
    )
    .expect("bad stage");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn strokes_are_revalidated_and_a_shape_may_have_none() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    edit_values(&path, &["scene"], |v| strip_key(v, "stroke"));
    assert!(load(&path).is_ok(), "an absent stroke is a fill-only shape");

    save(&sample(), &path).expect("save");
    edit_values(&path, &["scene"], |v| {
        set_key(v, "width", &serde_json::json!(-1.0))
    });
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn gradients_are_revalidated_and_a_shape_may_have_none() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    edit_values(&path, &["scene"], |v| strip_key(v, "gradient"));
    let loaded = load(&path).expect("an absent gradient is a solid shape");
    let shape = loaded.scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .find_map(|s| match s.content() {
            Content::Shape(shape) => Some(shape.clone()),
            _ => None,
        })
        .expect("shape");
    assert!(shape.gradient().is_none());

    save(&sample(), &path).expect("save");
    // Make the two gradient points identical: a gradient with no direction is corrupt.
    edit_values(&path, &["scene"], |v| {
        set_key(v, "to", &serde_json::json!([-1.5, 0.0]))
    });
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn mask_flags_round_trip() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    assert!(load(&path).expect("load").scenes()[0].timeline().layers()[0].is_mask());
}

#[test]
fn newer_schema_versions_are_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    Connection::open(&path)
        .expect("open")
        .pragma_update(None, "user_version", 2)
        .expect("set");
    assert!(matches!(
        load(&path),
        Err(StorageError::UnsupportedVersion(2))
    ));
}

#[test]
fn tampered_content_is_revalidated_on_load() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute("DELETE FROM entity WHERE kind = 'symbol'", [])
        .expect("delete");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Invalid(_))));

    // A record that is not CBOR of anything, stored as properly as any other.
    save(&sample(), &path).expect("save");
    set_raw(&path, "scene", b"{nope");
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn corrupted_project_files_never_panic_and_corruption_is_usually_detected() {
    let dir = tempfile::tempdir().expect("dir");
    let good = dir.path().join("good.flare");
    save(&sample(), &good).expect("save");
    let original = fs::read(&good).expect("read");
    let mutant = dir.path().join("mutant.flare");

    let (mut errors, mut accepted) = (0, 0);
    let mut state = 0x5EED_u64;
    // Flip a run of bytes at many positions: header, schema pages and payloads alike.
    for _ in 0..400 {
        let mut bytes = original.clone();
        let at = (lcg(&mut state) as usize) % bytes.len();
        let run = 1 + (lcg(&mut state) as usize) % 8;
        for i in at..(at + run).min(bytes.len()) {
            bytes[i] ^= 0xFF;
        }
        fs::write(&mutant, &bytes).expect("write");
        match load(&mutant) {
            Ok(_) => accepted += 1,
            Err(_) => errors += 1,
        }
    }
    // Truncations at every kind of boundary.
    for keep in (0..original.len()).step_by(original.len() / 40 + 1) {
        fs::write(&mutant, &original[..keep]).expect("write");
        if load(&mutant).is_ok() {
            accepted += 1;
        } else {
            errors += 1;
        }
    }
    // Zeroed pages and garbage appended.
    let mut zeroed = original.clone();
    zeroed[100..original.len().min(4096)].fill(0);
    fs::write(&mutant, &zeroed).expect("write");
    assert!(
        load(&mutant).is_err(),
        "a zeroed first page is not a project"
    );
    let mut extended = original.clone();
    extended.extend(std::iter::repeat_n(0xAB, 10_000));
    fs::write(&mutant, &extended).expect("write");
    let _ = load(&mutant); // trailing garbage may or may not matter; it must not panic

    assert!(
        errors > 50,
        "corruption was caught often enough to mean something ({errors} vs {accepted})"
    );
    // Whatever was accepted is still a valid document, by construction of the loader.
}

#[test]
fn a_project_with_hostile_payloads_is_refused_not_trusted() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    for payload in [
        "null",
        "[]",
        "{\"name\":1}",
        "{\"name\":\"x\",\"timeline\":{\"layers\":[{\"id\":\"nope\",\"name\":\"l\",\"spans\":[]}]}}",
        "{\"name\":\"x\",\"timeline\":{\"layers\":[{\"id\":\"00000000-0000-0000-0000-000000000001\",\"name\":\"l\",\"spans\":[{\"start\":4294967295,\"duration\":4294967295,\"content\":\"Empty\"}]}]}}",
        "{\"name\":\"x\",\"timeline\":{\"layers\":[{\"id\":\"00000000-0000-0000-0000-000000000001\",\"name\":\"l\",\"spans\":[{\"start\":0,\"duration\":0,\"content\":\"Empty\"}]}]}}",
    ] {
        save(&sample(), &path).expect("save");
        set_json(&path, "scene", payload);
        // Some of these are well-formed but extreme (a span ending past u32); those load as
        // valid documents because frame arithmetic is overflow-safe. None may panic.
        let result = load(&path);
        if payload.contains("\"duration\":0")
            || payload == "null"
            || payload == "[]"
            || payload.contains("\"name\":1")
            || payload.contains("\"id\":\"nope\"")
        {
            assert!(result.is_err(), "{payload}");
        }
    }
}

#[test]
fn attachments_round_trip_exactly_including_awkward_text() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let document = sample();
    assert_eq!(document.attachments().len(), 5);
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(
        loaded.attachments(),
        document.attachments(),
        "same order, same bytes"
    );
    assert_eq!(json(&path, "attachment").len(), 5);
}

#[test]
fn stroke_ends_and_corners_survive_and_a_name_that_is_not_one_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let document = sample();
    save(&document, &path).expect("save");
    assert_eq!(
        load(&path).expect("load"),
        document,
        "square and miter survive"
    );
    edit_values(&path, &["scene"], |v| {
        set_key(v, "cap", &serde_json::json!("flat"))
    });
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn stroke_style_survives_and_an_unknown_one_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let again = dir.path().join("b.flare");
    let document = sample();
    save(&document, &path).expect("save");
    // Strokes in the sample have a "cap" (they are not round), so that is where a style goes.
    fn add_style(v: &mut serde_json::Value, style: &str) {
        match v {
            serde_json::Value::Object(map) => {
                if map.contains_key("cap") {
                    map.insert("style".to_owned(), serde_json::json!(style));
                }
                map.values_mut().for_each(|v| add_style(v, style));
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|v| add_style(v, style)),
            _ => {}
        }
    }
    edit_values(&path, &["scene"], |v| add_style(v, "dotted"));
    let dotted = load(&path).expect("a known style loads");
    assert_ne!(dotted, document, "the style was read");
    save(&dotted, &again).expect("save again");
    assert_eq!(load(&again).expect("load"), dotted, "and written back");
    edit_values(&path, &["scene"], |v| add_style(v, "wavy"));
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn folders_and_their_members_survive() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let folder = Layer::new(LayerId::new(), "Folder", vec![])
        .expect("layer")
        .with_folder(true);
    let child = Layer::new(LayerId::new(), "Child", vec![])
        .expect("layer")
        .with_parent(Some(folder.id()));
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![folder.clone(), child.clone()]),
    );
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded, document);
    let layers = loaded.scenes()[0].timeline().layers();
    assert!(layers[0].is_folder() && layers[1].parent() == Some(layers[0].id()));
    // A file from before folders existed has neither key and loads as plain layers.
    let plain = sample();
    save(&plain, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert!(
        loaded.scenes()[0]
            .timeline()
            .layers()
            .iter()
            .all(|l| !l.is_folder() && l.parent().is_none())
    );
}

#[test]
fn motion_guides_survive() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let guide = Layer::new(LayerId::new(), "Guide", vec![])
        .expect("layer")
        .with_guide(true);
    let below = Layer::new(LayerId::new(), "Below", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![guide, below]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded, document);
    assert!(loaded.scenes()[0].timeline().layers()[0].is_guide());
    save(&sample(), &path).expect("save");
    assert!(
        load(&path).expect("load").scenes()[0]
            .timeline()
            .layers()
            .iter()
            .all(|l| !l.is_guide())
    );
}

#[test]
fn every_coordinate_comes_back_to_the_last_bit() {
    // serde_json's default reader can be one unit in the last place off for some numbers; a
    // saved drawing must not drift. This height was found by the property test.
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let height = 217.852_648_013_951_6_f64;
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 1.0, height, Rgba8::WHITE).expect("rect");
    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    save(&document, &path).expect("save");
    assert_eq!(load(&path).expect("load"), document);
}

#[test]
fn outline_colors_survive() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let colored = Layer::new(LayerId::new(), "Colored", vec![])
        .expect("layer")
        .with_outline(Some(flare_core::Rgba8::opaque(10, 200, 30)));
    let plain = Layer::new(LayerId::new(), "Plain", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![colored, plain]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded, document);
    let layers = loaded.scenes()[0].timeline().layers();
    assert_eq!(
        layers[0].outline(),
        Some(flare_core::Rgba8::opaque(10, 200, 30))
    );
    assert_eq!(layers[1].outline(), None);
}

#[test]
fn a_camera_layer_and_its_keyframes_survive_to_the_last_bit() {
    use flare_core::DocumentPoint;
    use flare_document::{Camera, Content, FrameSpan, Tween};
    let camera = |x: f64, zoom: f64, rotation: f64| {
        Camera::new(
            DocumentPoint::new(x, 0.1 + x / 3.0).expect("finite"),
            zoom,
            rotation,
        )
        .expect("camera")
    };
    let spans = vec![
        FrameSpan::new(
            0,
            std::num::NonZeroU32::new(6).expect("nonzero"),
            Content::Camera(camera(1.0, 1.5, 0.3)),
        )
        .with_tween(Tween::Linear)
        .with_ease(-40),
        FrameSpan::new(
            6,
            std::num::NonZeroU32::new(6).expect("nonzero"),
            Content::Camera(camera(7.0, 0.25, -2.0)),
        ),
    ];
    let layer = Layer::new(LayerId::new(), "Camera", spans)
        .expect("layer")
        .with_camera(true);
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("camera.flare");
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(
        loaded.scenes()[0].timeline(),
        document.scenes()[0].timeline()
    );
    assert!(loaded.scenes()[0].timeline().layers()[0].is_camera());
}

#[test]
fn a_keyframe_interpolation_survives_and_a_plain_tween_stays_plain() {
    use flare_document::SpanInterp;
    use flare_tween::{Easing, HandleKind, Interpolation, Params};
    let interp = SpanInterp::new(
        Interpolation::Elastic,
        Easing::Out,
        HandleKind::Free,
        [0.25, 0.1, -0.25, 0.0],
        Params::default(),
    )
    .expect("interp");
    let span = |start: u32, interp: Option<SpanInterp>| {
        FrameSpan::new(start, NonZeroU32::new(2).expect("nonzero"), Content::Empty)
            .with_tween(Tween::Linear)
            .with_ease(30)
            .with_interp(interp)
    };
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![span(0, Some(interp)), span(2, None)],
    )
    .expect("layer");
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document");
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("interp.flare");
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    let spans = loaded.scenes()[0].timeline().layers()[0].spans().to_vec();
    assert_eq!(spans[0].interp(), Some(interp));
    assert_eq!(spans[1].interp(), None);
    assert_eq!(spans[1].ease(), 30);
    assert_eq!(loaded, document);
}
