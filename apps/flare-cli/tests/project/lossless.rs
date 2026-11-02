//! Opening a file and exporting it again through the command line gives the file back.

use super::*;
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/io/flare-import/tests/fixtures")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("dir");
    for entry in std::fs::read_dir(from).expect("list") {
        let entry = entry.expect("entry");
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).expect("copy");
        }
    }
}

fn tree(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(at).expect("list") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let name = path
                    .strip_prefix(root)
                    .expect("in")
                    .to_string_lossy()
                    .into_owned();
                out.insert(name, std::fs::read(&path).expect("read"));
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn an_imported_fla_exports_as_the_same_file_until_it_is_edited() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("in.fla");
    std::fs::copy(fixtures().join("sample.fla"), &source).expect("copy");
    let original = std::fs::read(&source).expect("read");
    let stamp = std::fs::metadata(&source)
        .expect("meta")
        .modified()
        .expect("time");
    let project = dir.path().join("p.flare").to_string_lossy().into_owned();

    let report = json(&["import", &source.to_string_lossy(), &project]);
    assert_eq!(report["original_kept"]["kept"], true);
    assert_eq!(report["original_kept"]["format"], "fla");
    assert_eq!(report["original_kept"]["bytes"], original.len());
    // The source is exactly as it was, and nothing was written beside it.
    assert_eq!(std::fs::read(&source).expect("read"), original);
    assert_eq!(
        std::fs::metadata(&source)
            .expect("meta")
            .modified()
            .expect("time"),
        stamp
    );

    let out = dir.path().join("out.fla").to_string_lossy().into_owned();
    let exported = json(&["export-xfl", &project, &out]);
    assert_eq!(exported["lossless"]["exact"], true, "{exported}");
    assert_eq!(
        std::fs::read(&out).expect("read"),
        original,
        "byte for byte"
    );

    // An edit rebuilds only what it must, and the report says it is no longer the original.
    json(&["add-layer", &project, "Extra"]);
    let edited_out = dir.path().join("edited.fla").to_string_lossy().into_owned();
    let edited = json(&["export-xfl", &project, &edited_out]);
    assert_eq!(edited["lossless"]["exact"], false);
    assert!(
        edited["lossless"]["reason"]
            .as_str()
            .expect("r")
            .contains("edited since")
    );
    assert_ne!(std::fs::read(&edited_out).expect("read"), original);

    // Undoing it in a later session is not possible from the command line, so an unchanged
    // copy made before the edit stays the reference.
    assert_eq!(std::fs::read(&out).expect("read"), original);
}

#[test]
fn an_imported_xfl_folder_exports_as_the_same_folder() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("src");
    copy_tree(&fixtures().join("handmade_xfl"), &source);
    std::fs::create_dir_all(source.join("empty")).expect("empty");
    let before = tree(&source);
    let project = dir.path().join("p.flare").to_string_lossy().into_owned();
    let report = json(&["import", &source.to_string_lossy(), &project]);
    assert_eq!(report["original_kept"]["format"], "xfl");
    assert_eq!(tree(&source), before, "the folder is untouched");
    assert!(
        source.join("empty").is_dir()
            && std::fs::read_dir(source.join("empty")).expect("l").count() == 0
    );

    let out = dir.path().join("out");
    let exported = json(&["export-xfl", &project, &out.to_string_lossy()]);
    assert_eq!(exported["lossless"]["exact"], true, "{exported}");
    assert_eq!(tree(&out), before);
    assert!(out.join("empty").is_dir(), "an empty folder is kept too");
}

#[test]
fn an_imported_movie_publishes_as_the_same_file_and_an_edited_one_is_rebuilt() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let movie = dir.path().join("m.swf").to_string_lossy().into_owned();
    json(&["new", &file, "--width", "30", "--height", "30"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "5",
        "5",
        "20",
        "20",
        "--color",
        "FF0000",
    ]);
    json(&["publish-swf", &file, &movie]);
    let original = std::fs::read(&movie).expect("read");

    let back = dir.path().join("back.flare").to_string_lossy().into_owned();
    assert_eq!(
        json(&["import", &movie, &back])["original_kept"]["format"],
        "swf"
    );
    let again = dir.path().join("again.swf").to_string_lossy().into_owned();
    let published = json(&["publish-swf", &back, &again]);
    assert_eq!(published["original"], true);
    assert_eq!(std::fs::read(&again).expect("read"), original);
    // `--no-compress` does not apply to a movie that is written back as it was.
    let plain = dir.path().join("plain.swf").to_string_lossy().into_owned();
    json(&["publish-swf", &back, &plain, "--no-compress"]);
    assert_eq!(std::fs::read(&plain).expect("read"), original);

    json(&["set-stage", &back, "--width", "31"]);
    let edited = dir.path().join("edited.swf").to_string_lossy().into_owned();
    let published = json(&["publish-swf", &back, &edited]);
    assert_eq!(published["original"], false);
    assert_ne!(std::fs::read(&edited).expect("read"), original);
}

#[test]
fn a_project_that_was_not_imported_reports_that_its_export_is_rebuilt() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let out = dir.path().join("out.fla").to_string_lossy().into_owned();
    let exported = json(&["export-xfl", &file, &out]);
    assert_eq!(exported["lossless"]["exact"], false);
    assert!(
        exported["lossless"]["reason"]
            .as_str()
            .expect("r")
            .contains("not imported")
    );
}

#[test]
fn a_file_that_cannot_be_imported_is_refused_and_left_alone() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("broken.fla");
    let bytes = b"PK\x03\x04 truncated".to_vec();
    std::fs::write(&source, &bytes).expect("write");
    let out = dir.path().join("p.flare").to_string_lossy().into_owned();
    assert!(
        !run(&["import", &source.to_string_lossy(), &out])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&source).expect("read"), bytes);
    assert!(!Path::new(&out).exists());
}
