//! `flare import` of Flash movies that Flash Player plays but the reader needs help with.

use super::{json, path, run};

/// A movie written by `publish-swf` with its movie End record cut off, as some files are.
fn movie_without_end(dir: &std::path::Path) -> std::path::PathBuf {
    let file = path(dir);
    let movie = dir.join("full.swf");
    json(&["new", &file, "--width", "40", "--height", "40"]);
    json(&[
        "publish-swf",
        &file,
        &movie.to_string_lossy(),
        "--no-compress",
    ]);
    let mut bytes = std::fs::read(&movie).expect("read");
    bytes.truncate(bytes.len() - 2);
    let length = u32::try_from(bytes.len()).expect("small");
    bytes[4..8].copy_from_slice(&length.to_le_bytes());
    let cut = dir.join("no-end.swf");
    std::fs::write(&cut, &bytes).expect("write");
    cut
}

#[test]
fn a_movie_without_its_end_record_imports_with_its_stage() {
    let dir = tempfile::tempdir().expect("dir");
    let movie = movie_without_end(dir.path());
    let out = dir.path().join("cut.flare").to_string_lossy().into_owned();
    let report = json(&["import", &movie.to_string_lossy(), &out]);
    assert_eq!(report["scenes"], 1);
    let info = json(&["info", &out]);
    assert_eq!(
        (
            info["stage"]["width"].as_u64(),
            info["stage"]["height"].as_u64()
        ),
        (Some(40), Some(40))
    );
}

#[test]
fn a_swf_name_on_a_picture_is_refused_and_leaves_no_project() {
    let dir = tempfile::tempdir().expect("dir");
    let fake = dir.path().join("picture.swf");
    std::fs::write(&fake, b"\x89PNG\r\n\x1a\n....").expect("write");
    let out = dir.path().join("nope.flare").to_string_lossy().into_owned();
    let result = run(&["import", &fake.to_string_lossy(), &out]);
    assert!(!result.status.success());
    assert!(!std::path::Path::new(&out).exists());
}
