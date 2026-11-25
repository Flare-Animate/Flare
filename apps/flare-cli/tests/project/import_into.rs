//! `flare import-into`: a Flash movie brought into an existing project, as File > Import >
//! Import to Stage and Import to Library.

use super::*;

/// A movie of one rectangle held for 6 frames, as `publish-swf` writes it.
fn movie(dir: &Path) -> String {
    let file = dir.join("source.flare").to_string_lossy().into_owned();
    json(&["new", &file]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "10",
        "10",
        "50",
        "50",
        "--duration",
        "6",
    ]);
    let movie = dir.join("intro.swf").to_string_lossy().into_owned();
    json(&["publish-swf", &file, &movie]);
    movie
}

#[test]
fn import_into_puts_a_movie_on_a_new_layer_and_in_the_library() {
    let dir = tempfile::tempdir().expect("dir");
    let movie = movie(dir.path());
    let file = path(dir.path());
    json(&["new", &file]);
    let report = json(&["import-into", &file, &movie, "--frame", "2"]);
    assert_eq!(report["name"], "intro", "{report}");
    assert!(report["layer"].is_string(), "{report}");
    let info = json(&["info", &file]);
    let layers = info["scenes"][0]["layers"].as_array().expect("layers");
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0]["name"], "intro");
    assert_eq!(layers[0]["id"], report["layer"]);
    let names: Vec<&str> = info["symbols"]
        .as_array()
        .expect("symbols")
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(names.contains(&"intro"), "{names:?}");
    // The movie's rectangle draws on the stage from frame 2.
    let drawn = json(&["eval", &file, "--frame", "2"]);
    assert!(
        !drawn["placed"].as_array().expect("placed").is_empty(),
        "{drawn}"
    );
    let before = json(&["eval", &file, "--frame", "1"]);
    assert_eq!(
        before["placed"].as_array().map(Vec::len),
        Some(0),
        "{before}"
    );
}

#[test]
fn import_into_the_library_leaves_the_stage_alone_and_a_second_copy_is_renamed() {
    let dir = tempfile::tempdir().expect("dir");
    let movie = movie(dir.path());
    let file = path(dir.path());
    json(&["new", &file]);
    let first = json(&["import-into", &file, &movie, "--library"]);
    assert!(first["layer"].is_null(), "{first}");
    let second = json(&["import-into", &file, &movie, "--library"]);
    assert_eq!(second["name"], "intro 2", "{second}");
    let info = json(&["info", &file]);
    assert_eq!(
        info["scenes"][0]["layers"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn a_file_that_is_not_a_movie_is_refused_and_the_project_is_untouched() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let before = std::fs::read(&file).expect("read");
    let fake = dir.path().join("fake.swf");
    std::fs::write(&fake, b"FWS not really").expect("write");
    let result = run(&["import-into", &file, &fake.to_string_lossy()]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("cannot import"));
    assert_eq!(std::fs::read(&file).expect("read"), before);
}
