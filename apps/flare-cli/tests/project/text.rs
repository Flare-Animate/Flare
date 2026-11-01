use super::*;

/// A font file the test brings, so it does not depend on the fonts a machine has installed.
fn hack(dir: &Path) -> String {
    let file = dir.join("Hack.ttf");
    std::fs::write(&file, epaint_default_fonts::HACK_REGULAR).expect("write font");
    file.to_string_lossy().into_owned()
}

fn dark_pixels(png: &str) -> usize {
    let image = read_png(png);
    (0..image.1 * image.2)
        .filter(|i| pixel(&image, i % image.1, i / image.1)[0] < 128)
        .count()
}

#[test]
fn text_is_added_drawn_changed_and_taken_apart() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let font = hack(dir.path());
    let png = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["new", &file, "--width", "200", "--height", "60"]);

    let added = json(&[
        "add-text",
        &file,
        "Hello",
        "--x",
        "5",
        "--y",
        "5",
        "--family",
        "Hack",
        "--size",
        "30",
        "--font-file",
        &font,
    ]);
    let shape = added["shape"].as_str().expect("id").to_owned();
    json(&["export", &file, &png]);
    let small = dark_pixels(&png);
    assert!(small > 100, "the letters are drawn: {small}");

    // The same text, bigger, covers more of the picture, and it is still the same object.
    json(&[
        "set-text",
        &file,
        &shape,
        "--size",
        "45",
        "--font-file",
        &font,
    ]);
    json(&["export", &file, &png]);
    assert!(dark_pixels(&png) > small, "bigger letters cover more");

    // The words change; the text can be a different color.
    json(&[
        "set-text",
        &file,
        &shape,
        "--text",
        "Hi",
        "--color",
        "FF0000",
        "--font-file",
        &font,
    ]);
    json(&["export", &file, &png]);
    let image = read_png(&png);
    let reds = (0..image.1 * image.2)
        .filter(|i| pixel(&image, i % image.1, i / image.1) == [255, 0, 0, 255])
        .count();
    assert!(reds > 50, "the letters are red: {reds}");

    // Taken apart, the letters stay and the words are forgotten.
    let apart = json(&["break-text", &file, &shape]);
    assert_eq!(apart["text"], false);
    let refused = run(&[
        "set-text",
        &file,
        &shape,
        "--size",
        "20",
        "--font-file",
        &font,
    ]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("not text"));
    json(&["export", &file, &png]);
    let image2 = read_png(&png);
    let reds2 = (0..image2.1 * image2.2)
        .filter(|i| pixel(&image2, i % image2.1, i / image2.1) == [255, 0, 0, 255])
        .count();
    assert_eq!(reds, reds2, "the same letters");
}

#[test]
fn text_that_cannot_be_set_is_refused_without_writing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let font = hack(dir.path());
    json(&["new", &file]);
    let before = std::fs::read(&file).expect("read");
    for args in [
        vec!["add-text", &file, "   ", "--font-file", &font],
        vec!["add-text", &file, "x", "--size", "0", "--font-file", &font],
        vec![
            "add-text",
            &file,
            "x",
            "--size",
            "99999",
            "--font-file",
            &font,
        ],
        vec![
            "set-text",
            &file,
            "00000000-0000-0000-0000-000000000000",
            "--size",
            "9",
        ],
        vec!["break-text", &file, "00000000-0000-0000-0000-000000000000"],
    ] {
        assert!(!run(&args).status.success(), "{args:?}");
    }
    let missing = dir.path().join("none.ttf").to_string_lossy().into_owned();
    assert!(
        !run(&["add-text", &file, "x", "--font-file", &missing])
            .status
            .success()
    );
    assert_eq!(
        std::fs::read(&file).expect("read"),
        before,
        "nothing was written"
    );
}

#[test]
fn the_fonts_command_lists_families_including_one_that_was_brought() {
    let dir = tempfile::tempdir().expect("dir");
    let font = hack(dir.path());
    let listed = json(&["fonts", "--font-file", &font]);
    let families: Vec<&str> = listed["families"]
        .as_array()
        .expect("list")
        .iter()
        .filter_map(|f| f.as_str())
        .collect();
    assert!(families.contains(&"Hack"), "{families:?}");
}
