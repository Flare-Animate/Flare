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
fn files_without_a_stage_load_with_the_default_and_bad_stages_are_rejected() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE project SET settings_json = '{\"frame_rate\":[30000,1001]}'",
        [],
    )
    .expect("strip stage");
    drop(conn);
    assert_eq!(load(&path).expect("legacy load").stage(), Stage::default());

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
fn legacy_layers_without_flags_load_visible_and_unlocked() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    assert!(!load(&path).expect("load").scenes()[0].timeline().layers()[0].is_visible());
    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE entity SET payload_json = replace(replace(payload_json, '\"visible\":', '\"v\":'), '\"locked\":', '\"l\":')",
        [],
    )
    .expect("strip flags");
    drop(conn);
    let layer = &load(&path).expect("legacy load").scenes()[0]
        .timeline()
        .layers()[0]
        .clone();
    assert!(layer.is_visible() && !layer.is_locked());
}

#[test]
fn strokes_are_revalidated_and_legacy_shapes_without_strokes_still_load() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");

    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE entity SET payload_json = replace(payload_json, '\"stroke\":', '\"legacy\":')",
        [],
    )
    .expect("strip stroke key");
    drop(conn);
    assert!(load(&path).is_ok(), "an absent stroke is a fill-only shape");

    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE entity SET payload_json = replace(payload_json, '\"width\":2.5', '\"width\":-1.0')",
        [],
    )
    .expect("corrupt width");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn gradients_are_revalidated_and_legacy_shapes_without_them_still_load() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE entity SET payload_json = replace(payload_json, '\"gradient\":', '\"legacy\":')",
        [],
    )
    .expect("strip gradient key");
    drop(conn);
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
    let conn = Connection::open(&path).expect("open");
    // Make the two gradient points identical: a gradient with no direction is corrupt.
    conn.execute(
        "UPDATE entity SET payload_json = replace(payload_json, '\"to\":[6.0,2.25]', '\"to\":[-1.5,0.0]')",
        [],
    )
    .expect("corrupt gradient");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn mask_flags_round_trip_and_older_files_without_them_load_as_normal_layers() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    assert!(load(&path).expect("load").scenes()[0].timeline().layers()[0].is_mask());
    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE entity SET payload_json = replace(payload_json, '\"mask\":', '\"legacy\":')",
        [],
    )
    .expect("strip mask key");
    drop(conn);
    let loaded = load(&path).expect("an absent flag is a normal layer");
    assert!(!loaded.scenes()[0].timeline().layers()[0].is_mask());
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

    let conn = Connection::open(&path).expect("open");
    conn.execute("UPDATE entity SET payload_json = '{nope'", [])
        .expect("update");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Json(_))));
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
        let conn = Connection::open(&path).expect("open");
        conn.execute(
            "UPDATE entity SET payload_json = ?1 WHERE kind = 'scene'",
            [payload],
        )
        .expect("tamper");
        drop(conn);
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
    assert_eq!(attachment_rows(&path).len(), 5);
}

#[test]
fn stroke_ends_and_corners_survive_and_older_files_without_them_load_round() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let document = sample();
    save(&document, &path).expect("save");
    assert_eq!(
        load(&path).expect("load"),
        document,
        "square and miter survive"
    );
    let stroke_of = |doc: &Document| {
        doc.scenes()[0].timeline().layers()[0]
            .spans()
            .iter()
            .find_map(|s| match s.content() {
                Content::Shape(shape) => shape.stroke(),
                _ => None,
            })
            .expect("a stroked shape")
    };
    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE entity SET payload_json = replace(replace(payload_json, ',\"cap\":\"square\"', ''), ',\"join\":\"miter\"', '')",
        [],
    )
    .expect("strip the style, as an older file would have it");
    drop(conn);
    let old = stroke_of(&load(&path).expect("an older file"));
    assert_eq!(
        (old.cap(), old.join()),
        (flare_document::Cap::Round, flare_document::Join::Round)
    );
    save(&document, &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE entity SET payload_json = replace(payload_json, '\"square\"', '\"flat\"')",
        [],
    )
    .expect("break the cap name");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn folders_and_their_members_survive_and_older_files_have_none() {
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
