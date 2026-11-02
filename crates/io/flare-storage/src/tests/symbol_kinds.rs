use super::*;

fn with_kinds() -> Document {
    let symbols: Vec<Symbol> = flare_document::SymbolKind::ALL
        .into_iter()
        .map(|kind| Symbol::new(SymbolId::new(), kind.label(), Timeline::default()).with_kind(kind))
        .collect();
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        symbols,
    )
    .expect("document")
}

#[test]
fn each_kind_of_symbol_survives_saving_and_opening() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("kinds.flare");
    let document = with_kinds();
    save(&document, &path).expect("save");
    let back = load(&path).expect("load");
    assert_eq!(back, document);
    let kinds: Vec<_> = back.symbols().iter().map(|s| s.kind()).collect();
    assert_eq!(kinds, flare_document::SymbolKind::ALL);
}

#[test]
fn an_unknown_kind_is_a_damaged_file_not_a_guess() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("bad.flare");
    save(&with_kinds(), &path).expect("save");
    edit_values(&path, &["symbol"], |v| {
        set_key(v, "kind", &serde_json::json!("sprite"))
    });
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

fn tweened() -> Document {
    let shape = |x: f64| {
        Content::Shape(Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, Rgba8::WHITE).expect("rect"))
    };
    let span =
        |start, content| FrameSpan::new(start, NonZeroU32::new(4).expect("nonzero"), content);
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![
            span(0, shape(0.0)).with_tween(Tween::Shape).with_ease(-35),
            span(4, shape(50.0)),
        ],
    )
    .expect("layer");
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
fn a_shape_tween_and_its_ease_survive_saving_and_opening() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("tween.flare");
    let document = tweened();
    save(&document, &path).expect("save");
    let back = load(&path).expect("load");
    assert_eq!(back, document);
    let spans = back.scenes()[0].timeline().layers()[0].spans();
    assert_eq!((spans[0].tween(), spans[0].ease()), (Tween::Shape, -35));
}

fn with_playback(mode: flare_document::PlayMode, first_frame: u32) -> Document {
    let symbol = Symbol::new(SymbolId::new(), "Walk", Timeline::default());
    let instance = flare_document::Instance::new(
        flare_core::InstanceId::new(),
        symbol.id(),
        flare_core::Transform2D::IDENTITY,
    )
    .with_playback(flare_document::Playback { mode, first_frame });
    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Instance(instance));
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![symbol],
    )
    .expect("document")
}

fn playback_of(document: &Document) -> flare_document::Playback {
    match document.scenes()[0].timeline().layers()[0].spans()[0].content() {
        Content::Instance(instance) => instance.playback(),
        other => panic!("not an instance: {other:?}"),
    }
}

#[test]
fn an_instances_playback_survives_saving_and_opening() {
    use flare_document::{PlayMode, Playback};
    let dir = tempfile::tempdir().expect("dir");
    for (mode, first) in [
        (PlayMode::PlayOnce, 3),
        (PlayMode::SingleFrame, 7),
        (PlayMode::Loop, 0),
    ] {
        let path = dir
            .path()
            .join(format!("{}.flare", mode.name().replace(' ', "-")));
        let document = with_playback(mode, first);
        save(&document, &path).expect("save");
        let back = load(&path).expect("load");
        assert_eq!(back, document);
        assert_eq!(
            playback_of(&back),
            Playback {
                mode,
                first_frame: first
            }
        );
    }
}

#[test]
fn an_unknown_play_mode_is_refused_and_an_absent_one_loops_from_where_it_was_set() {
    use flare_document::{PlayMode, Playback};
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("odd.flare");
    save(&with_playback(PlayMode::SingleFrame, 2), &path).expect("save");
    edit_values(&path, &["scene"], |v| {
        set_key(v, "play", &serde_json::json!("bounce"))
    });
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
    save(&with_playback(PlayMode::SingleFrame, 2), &path).expect("save");
    edit_values(&path, &["scene"], |v| strip_key(v, "play"));
    assert_eq!(
        playback_of(&load(&path).expect("load")),
        Playback {
            mode: PlayMode::Loop,
            first_frame: 2
        },
        "no mode named: the default, with the first frame kept"
    );
}
