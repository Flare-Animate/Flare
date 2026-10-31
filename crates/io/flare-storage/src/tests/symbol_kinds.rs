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
fn a_project_from_before_kinds_opens_with_graphics() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("old.flare");
    save(&with_kinds(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    // Take the kind out of every stored symbol, as an older version wrote them.
    let rows: Vec<(String, String)> = {
        let mut statement = conn
            .prepare("SELECT id, payload_json FROM entity WHERE kind = 'symbol'")
            .expect("select");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("rows")
            .map(|r| r.expect("row"))
            .collect()
    };
    for (id, payload) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&payload).expect("json");
        value.as_object_mut().expect("object").remove("kind");
        conn.execute(
            "UPDATE entity SET payload_json = ?1 WHERE id = ?2",
            params![value.to_string(), id],
        )
        .expect("update");
    }
    drop(conn);
    let back = load(&path).expect("load");
    assert!(
        back.symbols()
            .iter()
            .all(|s| s.kind() == flare_document::SymbolKind::Graphic)
    );
}

#[test]
fn an_unknown_kind_is_a_damaged_file_not_a_guess() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("bad.flare");
    save(&with_kinds(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    let (id, payload): (String, String) = conn
        .query_row(
            "SELECT id, payload_json FROM entity WHERE kind = 'symbol' LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("a symbol");
    let mut value: serde_json::Value = serde_json::from_str(&payload).expect("json");
    value["kind"] = serde_json::Value::String("sprite".into());
    conn.execute(
        "UPDATE entity SET payload_json = ?1 WHERE id = ?2",
        params![value.to_string(), id],
    )
    .expect("update");
    drop(conn);
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

#[test]
fn a_project_from_before_easing_opens_with_none() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("old.flare");
    save(&tweened(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    let rows: Vec<(String, String)> = {
        let mut statement = conn
            .prepare("SELECT id, payload_json FROM entity WHERE kind = 'scene'")
            .expect("select");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("rows")
            .map(|r| r.expect("row"))
            .collect()
    };
    assert!(!rows.is_empty());
    // Take `ease` out of every span, as an older version wrote them.
    fn strip(value: &mut serde_json::Value, found: &mut usize) {
        match value {
            serde_json::Value::Object(map) => {
                if map.remove("ease").is_some() {
                    *found += 1;
                }
                map.values_mut().for_each(|v| strip(v, found));
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|v| strip(v, found)),
            _ => {}
        }
    }
    let mut found = 0;
    for (id, payload) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&payload).expect("json");
        strip(&mut value, &mut found);
        conn.execute(
            "UPDATE entity SET payload_json = ?1 WHERE id = ?2",
            params![value.to_string(), id],
        )
        .expect("update");
    }
    assert!(found >= 2, "the ease was stored on each span ({found})");
    drop(conn);
    let back = load(&path).expect("an older file still opens");
    let spans = back.scenes()[0].timeline().layers()[0].spans();
    assert_eq!((spans[0].tween(), spans[0].ease()), (Tween::Shape, 0));
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

/// Rewrites every stored payload with `change`, as an older or odd file would have it.
fn rewrite_payloads(path: &std::path::Path, change: impl Fn(&mut serde_json::Value)) {
    let conn = Connection::open(path).expect("open");
    let rows: Vec<(String, String)> = {
        let mut statement = conn
            .prepare("SELECT id, payload_json FROM entity")
            .expect("select");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("rows")
            .map(|r| r.expect("row"))
            .collect()
    };
    for (id, payload) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&payload).expect("json");
        change(&mut value);
        conn.execute(
            "UPDATE entity SET payload_json = ?1 WHERE id = ?2",
            params![value.to_string(), id],
        )
        .expect("update");
    }
}

fn visit(
    value: &mut serde_json::Value,
    change: &dyn Fn(&mut serde_json::Map<String, serde_json::Value>),
) {
    match value {
        serde_json::Value::Object(map) => {
            change(map);
            for v in map.values_mut() {
                visit(v, change);
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(|v| visit(v, change)),
        _ => {}
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
fn a_project_from_before_playback_loops_from_the_first_frame_and_an_unknown_mode_does_too() {
    use flare_document::{PlayMode, Playback};
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("old.flare");
    save(&with_playback(PlayMode::PlayOnce, 5), &path).expect("save");
    // Take both settings out, as a file from before they existed.
    rewrite_payloads(&path, |value| {
        visit(value, &|map| {
            map.remove("play");
            map.remove("first_frame");
        });
    });
    assert_eq!(
        playback_of(&load(&path).expect("load")),
        Playback::default()
    );
    // A mode this version does not know plays as the default; the first frame is kept.
    save(
        &with_playback(PlayMode::SingleFrame, 2),
        &dir.path().join("odd.flare"),
    )
    .expect("save");
    rewrite_payloads(&dir.path().join("odd.flare"), |value| {
        visit(value, &|map| {
            if map.contains_key("play") {
                map.insert("play".into(), "bounce".into());
            }
        });
    });
    let odd = load(&dir.path().join("odd.flare")).expect("load");
    assert_eq!(
        playback_of(&odd),
        Playback {
            mode: PlayMode::Loop,
            first_frame: 2
        }
    );
}
