use super::*;

#[test]
fn layer_commands_manage_visibility_lock_order_and_deletion() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["new", &file, "--width", "10", "--height", "10"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "10",
        "10",
        "--color",
        "FF0000",
    ]);
    let second = json(&["add-layer", &file, "Second"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    let layers = |file: &str| json(&["info", file])["scenes"][0]["layers"].clone();

    json(&["layer", "hide", &file]);
    json(&["export", &file, &out]);
    assert_eq!(
        pixel(&read_png(&out), 5, 5),
        [255, 255, 255, 255],
        "hidden layers do not render"
    );
    json(&["layer", "show", &file]);
    json(&["export", &file, &out]);
    assert_eq!(pixel(&read_png(&out), 5, 5), [255, 0, 0, 255]);

    json(&[
        "layer", "rename", &file, "--layer", &second, "--name", "Backdrop",
    ]);
    json(&["layer", "lock", &file, "--layer", &second]);
    let info = layers(&file);
    assert_eq!(info[1]["name"], "Backdrop");
    assert_eq!(info[1]["locked"], true);
    assert_eq!(info[0]["locked"], false);

    json(&["layer", "up", &file, "--layer", &second]);
    assert_eq!(layers(&file)[0]["name"], "Backdrop");
    assert!(
        !run(&["layer", "up", &file, "--layer", &second])
            .status
            .success(),
        "already at the top"
    );

    let before = std::fs::read(&file).expect("read");
    assert!(
        !run(&["layer", "rename", &file]).status.success(),
        "needs --name"
    );
    assert_eq!(std::fs::read(&file).expect("read"), before);

    json(&["layer", "delete", &file, "--layer", &second]);
    assert_eq!(layers(&file).as_array().expect("array").len(), 1);
    assert!(
        !run(&["layer", "delete", &file]).status.success(),
        "the last layer stays"
    );
}

#[test]
fn import_turns_an_xfl_folder_into_a_project_and_lists_what_was_skipped() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("src");
    std::fs::create_dir_all(source.join("LIBRARY")).expect("mkdir");
    std::fs::write(
        source.join("DOMDocument.xml"),
        r##"<DOMDocument frameRate="24" width="20" height="10" backgroundColor="#FFFFFF"><timelines><DOMTimeline name="Main"><layers>
        <DOMLayer name="Box"><frames><DOMFrame index="0" duration="3"><elements><DOMShape>
          <fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
          <edges><Edge fillStyle1="1" edges="!0 0|100 0|100 100|0 100|0 0"/></edges>
        </DOMShape><DOMStaticText/></elements></DOMFrame></frames></DOMLayer>
        <DOMLayer name="Cut" layerType="mask"><frames/></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"##,
    )
    .expect("write");
    let out = dir.path().join("out.flare").to_string_lossy().into_owned();
    let source_arg = source.to_string_lossy().into_owned();

    let report = json(&["import", &source_arg, &out]);
    assert_eq!(
        (report["scenes"].as_u64(), report["symbols"].as_u64()),
        (Some(1), Some(0))
    );
    // The empty text field is only noted (Info); the mask layer is the unsupported loss.
    assert_eq!(report["unsupported"], 1, "the mask layer");
    let messages: Vec<&str> = report["diagnostics"]
        .as_array()
        .expect("list")
        .iter()
        .filter_map(|d| d["message"].as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("mask layers")),
        "{messages:?}"
    );

    // The result is a normal project: it has the imported size and renders the artwork.
    let info = json(&["info", &out]);
    assert_eq!(
        (
            info["stage"]["width"].as_u64(),
            info["stage"]["height"].as_u64()
        ),
        (Some(20), Some(10))
    );
    let png = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["export", &out, &png]);
    assert_eq!(pixel(&read_png(&png), 2, 2), [255, 0, 0, 255]);

    assert!(
        !run(&["import", &source_arg, &out]).status.success(),
        "never overwrites"
    );
    let missing = dir.path().join("nothing").to_string_lossy().into_owned();
    let other = dir.path().join("o2.flare").to_string_lossy().into_owned();
    assert!(!run(&["import", &missing, &other]).status.success());
    assert!(
        !std::path::Path::new(&other).exists(),
        "a failed import leaves no file behind"
    );
}

#[test]
fn export_xfl_round_trips_through_the_cli_as_a_folder_and_as_a_fla() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&[
        "new",
        &file,
        "--width",
        "40",
        "--height",
        "20",
        "--background",
        "102030",
    ]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "2",
        "2",
        "10",
        "10",
        "--color",
        "FF0000",
        "--duration",
        "4",
    ]);
    let symbol = json(&[
        "add-symbol",
        &file,
        "Dot",
        "--rect",
        "0",
        "0",
        "4",
        "4",
        "--color",
        "00FF00",
    ])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    let layer = json(&["add-layer", &file, "Movers"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "place",
        &file,
        &symbol,
        "--layer",
        &layer,
        "--duration",
        "4",
        "--x",
        "20",
        "--y",
        "5",
        "--tween",
    ]);
    json(&[
        "place", &file, &symbol, "--layer", &layer, "--start", "4", "--x", "30", "--y", "5",
    ]);

    let folder = dir.path().join("proj_xfl").to_string_lossy().into_owned();
    let report = json(&["export-xfl", &file, &folder]);
    assert_eq!(report["format"], "xfl");
    assert!(
        std::path::Path::new(&folder)
            .join("DOMDocument.xml")
            .is_file()
    );
    let fla = dir.path().join("proj.fla").to_string_lossy().into_owned();
    assert_eq!(json(&["export-xfl", &file, &fla])["format"], "fla");

    for (source, name) in [(&folder, "from_folder"), (&fla, "from_fla")] {
        let reimported = dir
            .path()
            .join(format!("{name}.flare"))
            .to_string_lossy()
            .into_owned();
        json(&["import", source, &reimported]);
        let info = json(&["info", &reimported]);
        assert_eq!(
            (
                info["stage"]["width"].as_u64(),
                info["stage"]["height"].as_u64()
            ),
            (Some(40), Some(20))
        );
        assert_eq!(info["symbols"].as_array().expect("symbols").len(), 1);
        // The same artwork renders the same way, including the tween halfway through.
        for frame in ["0", "2"] {
            let (a, b) = (
                dir.path()
                    .join(format!("{name}_a{frame}.png"))
                    .to_string_lossy()
                    .into_owned(),
                dir.path()
                    .join(format!("{name}_b{frame}.png"))
                    .to_string_lossy()
                    .into_owned(),
            );
            json(&["export", &file, &a, "--frame", frame]);
            json(&["export", &reimported, &b, "--frame", frame]);
            assert_eq!(
                read_png(&a).0,
                read_png(&b).0,
                "{name} frame {frame} differs"
            );
        }
    }
    assert!(
        !run(&["export-xfl", &file, &folder]).status.success(),
        "never overwrites"
    );
    assert!(!run(&["export-xfl", &file, &fla]).status.success());
}

#[test]
fn export_sprites_writes_a_sheet_and_an_atlas_and_refuses_overwrites_and_bad_ranges() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let sheet = dir.path().join("s.png").to_string_lossy().into_owned();
    json(&["new", &file, "--width", "10", "--height", "10"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "10",
        "10",
        "--color",
        "FF0000",
    ]);
    let out = json(&["export-sprites", &file, &sheet, "--columns", "1"]);
    assert_eq!(
        (out["width"].as_u64(), out["frames"].as_u64()),
        (Some(10), Some(1))
    );
    let atlas = std::fs::read_to_string(dir.path().join("s.json")).expect("atlas");
    assert!(atlas.contains("\"frame_00000\"") && atlas.contains("\"image\": \"s.png\""));
    assert_eq!(read_png(&sheet).1, 10);
    assert!(!run(&["export-sprites", &file, &sheet]).status.success());
    let other = dir.path().join("t.png").to_string_lossy().into_owned();
    assert!(
        !run(&["export-sprites", &file, &other, "--start", "3"])
            .status
            .success()
    );
}

#[test]
fn draw_rect_takes_a_stroke_style_and_a_wrong_one_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let rect = |style: &str| {
        run(&[
            "draw-rect",
            &file,
            "--rect",
            "2",
            "2",
            "20",
            "10",
            "--stroke",
            "FF0000",
            "--stroke-style",
            style,
        ])
    };
    assert!(rect("dashed").status.success());
    for style in ["hatched", "ragged", "herringbone", "rope", "zigzag"] {
        let out = rect(style);
        assert!(
            out.status.success(),
            "{style} is accepted: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let refused = rect("wavy");
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("wavy") && stderr.contains("zigzag"),
        "{stderr}"
    );
}

/// A FlipaClip project with its `meta` alone: two layers and no pictures. Its layers stay empty;
/// the test is that the command reads the `.fc` file by its extension.
fn flipaclip_meta_only() -> Vec<u8> {
    let meta = serde_json::json!({
        "format": 1, "canvasWidth": 8, "canvasHeight": 6, "fps": 12, "frameCount": 2,
        "name": "Sample", "version": 4,
        "layersState": "[{\"id\":1,\"name\":\"Capa 1\"},{\"id\":2,\"name\":\"Capa 2\"}]",
    })
    .to_string();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("meta", stored).expect("starts");
    std::io::Write::write_all(&mut zip, meta.as_bytes()).expect("writes");
    zip.finish().expect("finishes").into_inner()
}

#[test]
fn import_reads_a_flipaclip_project_by_its_extension() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("sample.fc");
    std::fs::write(&source, flipaclip_meta_only()).expect("write");
    let out = dir.path().join("out.flare").to_string_lossy().into_owned();
    let report = json(&["import", &source.to_string_lossy(), &out]);
    assert_eq!(report["scenes"].as_u64(), Some(1));
    assert_eq!(
        report["unsupported"], 0,
        "a meta-only project names nothing as skipped"
    );
    assert!(std::path::Path::new(&out).exists());
}
