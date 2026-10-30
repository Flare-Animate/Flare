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
