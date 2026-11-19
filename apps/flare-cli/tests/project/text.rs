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

/// A cache of downloaded Google Fonts holding Squada One (a small OFL font, licence text beside
/// it), so the tests set text in a Google font without the network.
fn google_cache(dir: &Path) -> std::path::PathBuf {
    let cache = dir.join("fonts");
    let family = flare_text::google::family_named("Squada One").expect("in the catalog");
    let file = cache.join(&family.cache_names()[0]);
    std::fs::create_dir_all(file.parent().expect("parent")).expect("dirs");
    std::fs::write(
        &file,
        include_bytes!("../../../../crates/foundation/flare-text/data/test/SquadaOne-Regular.ttf"),
    )
    .expect("write font");
    cache
}

fn run_with_cache(cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flare"))
        .args(args)
        .env("FLARE_FONT_CACHE", cache)
        // Anything that does reach for the network fails at once instead.
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("ALL_PROXY", "http://127.0.0.1:1")
        .output()
        .expect("run cli")
}

#[test]
fn text_in_a_google_font_is_set_in_it_and_can_be_told_not_to_download() {
    let dir = tempfile::tempdir().expect("dir");
    let cache = google_cache(dir.path());
    let drawn = |name: &str, extra: &[&str]| {
        let file = dir
            .path()
            .join(format!("{name}.flare"))
            .to_string_lossy()
            .into_owned();
        let png = dir
            .path()
            .join(format!("{name}.png"))
            .to_string_lossy()
            .into_owned();
        json(&["new", &file, "--width", "200", "--height", "60"]);
        let mut args = vec![
            "add-text",
            &file,
            "Flare",
            "--family",
            "Squada One",
            "--size",
            "40",
        ];
        args.extend(extra);
        let output = run_with_cache(&cache, &args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        json(&["export", &file, &png]);
        dark_pixels(&png)
    };
    let downloaded = drawn("google", &[]);
    let fallback = drawn("fallback", &["--no-google-fonts"]);
    assert!(downloaded > 50 && fallback > 50, "{downloaded} {fallback}");
    assert_ne!(downloaded, fallback, "the Google font was not used");

    let listed = run_with_cache(&cache, &["fonts"]);
    let listed: Value = serde_json::from_slice(&listed.stdout).expect("json");
    let google = listed["google"].as_array().expect("list");
    assert!(google.len() > 1500, "{}", google.len());
    assert!(google.iter().any(|f| f == "Roboto"));
    let off = run_with_cache(&cache, &["fonts", "--no-google-fonts"]);
    let off: Value = serde_json::from_slice(&off.stdout).expect("json");
    assert_eq!(off["google"], serde_json::json!([]));
}

#[test]
fn a_google_font_that_cannot_be_downloaded_is_an_error_naming_it() {
    let dir = tempfile::tempdir().expect("dir");
    let empty = dir.path().join("empty-cache");
    let file = path(dir.path());
    json(&["new", &file]);
    let before = std::fs::read(&file).expect("read");
    let output = run_with_cache(&empty, &["add-text", &file, "x", "--family", "Abel"]);
    assert!(!output.status.success());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.contains("Abel") && message.contains("Google Fonts"),
        "{message}"
    );
    assert_eq!(
        std::fs::read(&file).expect("read"),
        before,
        "nothing was written"
    );
}
