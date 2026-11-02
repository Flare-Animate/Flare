use super::*;

#[test]
fn a_translucent_background_and_invisible_shapes_are_reported() {
    let clear = Shape::rect(ShapeId::new(), 0.0, 0.0, 4.0, 4.0, Rgba8::TRANSPARENT).expect("rect");
    let scene = Scene::new(SceneId::new(), "S", one_layer(clear));
    let rate = FrameRate::new(24, 1).expect("rate");
    let stage = Stage::new(10, 10, Rgba8::new(1, 2, 3, 100)).expect("stage");
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("doc")
        .with_stage(stage);
    let out = export_xfl(&doc).expect("export");
    let all: String = out
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(all.contains("transparency"), "{all}");
    assert!(all.contains("invisible shape"), "{all}");
    assert!(import_xfl(&Mem(out.files)).is_ok());
}

#[test]
fn folders_and_zips_import_and_never_overwrite() {
    let dir = tempfile::tempdir().expect("dir");
    let output = export_xfl(&rich()).expect("export");

    let folder = dir.path().join("out").join("proj");
    write_directory(&output, &folder).expect("write");
    assert!(folder.join("DOMDocument.xml").is_file() && folder.join("LIBRARY").is_dir());
    let stub = std::fs::read_dir(&folder)
        .expect("ls")
        .filter_map(Result::ok)
        .any(|e| e.path().extension().is_some_and(|x| x == "xfl"));
    assert!(stub, "the .xfl stub marks the folder as an XFL document");
    let from_folder = import_path(&folder).expect("import folder");
    assert_eq!(from_folder.document.scenes().len(), 2);
    assert!(matches!(
        write_directory(&output, &folder),
        Err(flare_export::ExportError::AlreadyExists(_))
    ));

    let fla = dir.path().join("proj.fla");
    write_fla(&output, &fla).expect("fla");
    let from_zip = import_path(&fla).expect("import fla");
    assert_eq!(
        from_zip.document.symbols().len(),
        from_folder.document.symbols().len()
    );
    assert!(matches!(
        write_fla(&output, &fla),
        Err(flare_export::ExportError::AlreadyExists(_))
    ));

    // No staging files are left behind next to the results.
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("ls")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_failed_write_leaves_nothing_behind() {
    let dir = tempfile::tempdir().expect("dir");
    let output = export_xfl(&rich()).expect("export");
    // The parent "directory" is a file, so creating the folder must fail.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "x").expect("write");
    assert!(write_directory(&output, &blocker.join("proj")).is_err());
    assert!(write_fla(&output, &blocker.join("proj.fla")).is_err());
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("ls")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(entries.len(), 1, "only the blocker file exists");
}

#[test]
fn closed_stroked_shapes_keep_their_closing_edge() {
    // The outline's fourth side only exists as the implicit "close"; if it were dropped, the
    // left edge of this rectangle would have no stroke.
    let boxed = rect(10.0, 10.0, 40.0, 30.0, Rgba8::opaque(250, 250, 0))
        .with_stroke(Stroke::new(4.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    let scene = Scene::new(SceneId::new(), "S", one_layer(boxed));
    let rate = FrameRate::new(24, 1).expect("rate");
    let stage = Stage::new(60, 50, Rgba8::WHITE).expect("stage");
    let original = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("doc")
        .with_stage(stage);
    let (_, back) = roundtrip(&original);
    let (max, mean) = difference(&render(&original, 0, 0), &render(&back.document, 0, 0));
    assert_eq!((max, mean), (0, 0.0));
    let image = render(&back.document, 0, 0);
    assert_eq!(
        image.pixel(10, 25),
        Some(Rgba8::opaque(0, 0, 0)),
        "the closing (left) side is stroked"
    );
    assert_eq!(image.pixel(30, 25), Some(Rgba8::opaque(250, 250, 0)));
}
