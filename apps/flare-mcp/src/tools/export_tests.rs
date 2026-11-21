#![allow(clippy::expect_used)]

use super::*;

/// The names in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut all: Vec<String> = std::fs::read_dir(dir)
        .expect("read")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    all.sort();
    all
}

#[test]
fn a_fresh_file_is_written_whole_and_nothing_is_left_beside_it() {
    let dir = tempfile::tempdir().expect("dir");
    let out = fresh(dir.path(), "frame.png").expect("free");
    write_fresh(&out, b"picture").expect("written");
    assert_eq!(std::fs::read(&out).expect("read"), b"picture");
    assert_eq!(
        names(dir.path()),
        vec!["frame.png"],
        "no private folder left"
    );
}

#[test]
fn a_file_that_arrived_after_the_name_was_checked_is_not_written_over() {
    let dir = tempfile::tempdir().expect("dir");
    let out = fresh(dir.path(), "frame.png").expect("free when checked");
    std::fs::write(&out, b"theirs").expect("arrives meanwhile");
    let refused = write_fresh(&out, b"ours").expect_err("refused");
    assert!(refused.contains("already exists"), "{refused}");
    assert_eq!(std::fs::read(&out).expect("read"), b"theirs");
    assert_eq!(names(dir.path()), vec!["frame.png"]);
}

#[test]
fn a_writer_gets_a_path_with_the_same_name_and_a_failed_one_leaves_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("movie.webm");
    let named = keep_fresh(&out, |path| {
        std::fs::write(path, b"movie").map_err(|e| e.to_string())?;
        Ok(path.file_name().map(|n| n.to_owned()))
    })
    .expect("kept");
    assert_eq!(named.as_deref(), Some(std::ffi::OsStr::new("movie.webm")));
    let other = dir.path().join("broken.webm");
    let failed = keep_fresh(&other, |_| -> Result<(), String> {
        Err("encoder failed".into())
    });
    assert_eq!(failed, Err("encoder failed".to_owned()));
    assert_eq!(names(dir.path()), vec!["movie.webm"], "nothing half made");
}
