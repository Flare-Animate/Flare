use super::*;

#[test]
fn publish_swf_writes_a_file_the_ruffle_reader_accepts_and_never_overwrites() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&[
        "new",
        &file,
        "--width",
        "64",
        "--height",
        "32",
        "--background",
        "102030",
    ]);
    let symbol = json(&[
        "add-symbol",
        &file,
        "Dot",
        "--rect",
        "0",
        "0",
        "8",
        "8",
        "--color",
        "FF0000",
    ])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "place",
        &file,
        &symbol,
        "--duration",
        "6",
        "--x",
        "0",
        "--y",
        "4",
        "--tween",
    ]);
    json(&[
        "place", &file, &symbol, "--start", "6", "--x", "40", "--y", "4",
    ]);

    let out = dir.path().join("movie.swf").to_string_lossy().into_owned();
    let report = json(&["publish-swf", &file, &out]);
    assert_eq!(report["frames"], 7);
    assert!(
        report["diagnostics"]
            .as_array()
            .expect("list")
            .iter()
            .any(|d| d["message"].as_str().is_some_and(|m| m.contains("baked")))
    );

    let bytes = std::fs::read(&out).expect("read");
    assert_eq!(&bytes[..3], b"CWS");
    let buf = swf::decompress_swf(&bytes[..]).expect("decompress");
    let parsed = swf::parse_swf(&buf).expect("the SWF parses");
    assert_eq!(parsed.header.num_frames(), 7);
    assert_eq!(
        (
            parsed.header.stage_size().x_max.to_pixels(),
            parsed.header.stage_size().y_max.to_pixels()
        ),
        (64.0, 32.0)
    );
    assert_eq!(
        parsed
            .tags
            .iter()
            .filter(|t| matches!(t, swf::Tag::ShowFrame))
            .count(),
        7
    );

    let plain = dir.path().join("plain.swf").to_string_lossy().into_owned();
    json(&["publish-swf", &file, &plain, "--no-compress"]);
    assert_eq!(&std::fs::read(&plain).expect("read")[..3], b"FWS");
    assert!(
        !run(&["publish-swf", &file, &out]).status.success(),
        "never overwrites"
    );
    let missing = dir.path().join("none.flare").to_string_lossy().into_owned();
    let other = dir.path().join("o.swf").to_string_lossy().into_owned();
    assert!(!run(&["publish-swf", &missing, &other]).status.success());
    assert!(
        !std::path::Path::new(&other).exists(),
        "a failed publish leaves no file"
    );
}

#[test]
fn source_data_survives_import_save_reopen_and_export_through_the_cli() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("src");
    std::fs::create_dir_all(&source).expect("mkdir");
    std::fs::write(
        source.join("DOMDocument.xml"),
        r##"<DOMDocument frameRate="24" width="20" height="10"><media><DOMBitmapItem name="pic"/></media><timelines><DOMTimeline name="Main"><layers>
        <DOMLayer name="Box"><frames><DOMFrame index="0" duration="3"><elements><DOMShape>
          <fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
          <edges><Edge fillStyle1="1" edges="!0 0|100 0|100 100|0 100|0 0"/></edges>
        </DOMShape><DOMStaticText><textRuns><DOMTextRun><characters>Keep me &amp; my text</characters></DOMTextRun></textRuns></DOMStaticText></elements></DOMFrame></frames></DOMLayer>
        <DOMLayer name="Cut" layerType="mask"><frames/></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"##,
    )
    .expect("write");
    let project = dir.path().join("p.flare").to_string_lossy().into_owned();
    let report = json(&["import", &source.to_string_lossy(), &project]);
    assert_eq!(
        report["kept_source_data"], 3,
        "the media list, the text object and the mask layer"
    );

    // Editing does not lose it: change the stage and add a layer, then look again.
    json(&["set-stage", &project, "--width", "30"]);
    json(&["add-layer", &project, "Extra"]);
    assert_eq!(json(&["info", &project])["attachments"], 3);

    let out = dir.path().join("out").to_string_lossy().into_owned();
    json(&["export-xfl", &project, &out]);
    let written =
        std::fs::read_to_string(std::path::Path::new(&out).join("DOMDocument.xml")).expect("read");
    for snippet in [
        "<characters>Keep me &amp; my text</characters>",
        "<media><DOMBitmapItem name=\"pic\"/></media>",
        "<DOMLayer name=\"Cut\" layerType=\"mask\"><frames/></DOMLayer>",
    ] {
        assert!(written.contains(snippet), "missing {snippet}");
    }
}

#[cfg(unix)]
#[test]
fn test_movie_hands_a_valid_swf_to_the_player_and_reports_how_it_exited() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "40", "--height", "20"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "10",
        "10",
        "--duration",
        "4",
    ]);

    let player = dir.path().join("fake-ruffle");
    std::fs::write(
        &player,
        "#!/bin/sh\nhere=$(dirname \"$0\")\ncp \"$1\" \"$here/received.swf\"\nexit 5\n",
    )
    .expect("write");
    std::fs::set_permissions(&player, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let report = json(&["test-movie", &file, "--ruffle", &player.to_string_lossy()]);
    assert_eq!(
        report["exit_code"], 5,
        "the player's exit status is reported, not hidden"
    );
    let bytes = std::fs::read(dir.path().join("received.swf")).expect("the player received a file");
    let buf = swf::decompress_swf(&bytes[..]).expect("decompress");
    assert_eq!(swf::parse_swf(&buf).expect("parses").header.num_frames(), 4);

    let missing = dir
        .path()
        .join("nothing-here")
        .to_string_lossy()
        .into_owned();
    let output = run(&["test-movie", &file, "--ruffle", &missing]);
    assert!(!output.status.success());
    // With no player given and none on PATH, the error says how to fix it.
    let none = Command::new(env!("CARGO_BIN_EXE_flare-cli"))
        .args(["test-movie", &file])
        .env("PATH", "/nonexistent")
        .env_remove("FLARE_RUFFLE")
        .output()
        .expect("run");
    assert!(!none.status.success());
    assert!(
        String::from_utf8_lossy(&none.stderr).contains("Ruffle was not found"),
        "{}",
        String::from_utf8_lossy(&none.stderr)
    );
}
