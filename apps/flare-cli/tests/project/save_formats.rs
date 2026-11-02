//! A script can edit a `.fla` or `.xfl` directly, and convert between every format.

use super::*;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/io/flare-import/tests/fixtures")
        .join(name)
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("list")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn layers(file: &str) -> usize {
    json(&["info", file])["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .len()
}

#[test]
fn a_command_that_edits_a_fla_writes_the_edit_back_to_the_same_fla() {
    let dir = tempfile::tempdir().expect("dir");
    let fla = dir.path().join("work.fla");
    std::fs::copy(fixture("sample.fla"), &fla).expect("copy");
    let file = fla.to_string_lossy().into_owned();
    assert_eq!(json(&["info", &file])["format"], "fla");
    let before = layers(&file);
    json(&["add-layer", &file, "Ground"]);
    assert_eq!(layers(&file), before + 1);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "1",
        "1",
        "20",
        "20",
        "--color",
        "FF0000",
    ]);
    assert_eq!(
        names(dir.path()),
        ["work.fla"],
        "no project file or leftovers"
    );
    // It is still a Flash document, not a project file.
    assert!(flare_import::import_path(&fla).is_ok());
}

#[test]
fn a_command_that_edits_an_xfl_folder_writes_the_edit_back_to_its_stub() {
    let dir = tempfile::tempdir().expect("dir");
    let folder = dir.path().join("sample");
    std::fs::create_dir(&folder).expect("dir");
    for entry in std::fs::read_dir(fixture("sample_xfl"))
        .expect("list")
        .flatten()
    {
        std::fs::copy(entry.path(), folder.join(entry.file_name())).expect("copy");
    }
    let stub = folder.join("sample.xfl").to_string_lossy().into_owned();
    let before = layers(&stub);
    json(&["add-layer", &stub, "Ground"]);
    assert_eq!(layers(&stub), before + 1);
    assert_eq!(names(dir.path()), ["sample"]);
    assert!(folder.join("DOMDocument.xml").is_file());
}

#[test]
fn convert_goes_between_every_format_and_back() {
    let dir = tempfile::tempdir().expect("dir");
    let at = |name: &str| dir.path().join(name).to_string_lossy().into_owned();
    let source = at("a.fla");
    std::fs::copy(fixture("sample.fla"), &source).expect("copy");
    let original = std::fs::read(&source).expect("read");

    let flare = json(&["convert", &source, &at("b.flare")]);
    assert_eq!(flare["format"], "flare");
    let xfl = json(&["convert", &at("b.flare"), &at("c.xfl")]);
    assert_eq!(xfl["format"], "xfl");
    let stub = xfl["converted"].as_str().expect("path").to_owned();
    assert!(
        stub.ends_with("c/sample.xfl") || stub.ends_with("c/c.xfl"),
        "{stub}"
    );
    let fla = json(&["convert", &stub, &at("d.fla")]);
    assert_eq!(fla["format"], "fla");
    assert_eq!(fla["lossless"]["exact"], true, "nothing changed on the way");
    // Through a project file and a folder and out again, the Flash document is the same bytes of
    // content: the unchanged original comes back.
    let last = flare_import::import_path(Path::new(&at("d.fla"))).expect("imports");
    let first = flare_import::import_path(Path::new(&source)).expect("imports");
    assert_eq!(last.document.scenes().len(), first.document.scenes().len());
    assert_eq!(
        std::fs::read(&source).expect("read"),
        original,
        "the source is untouched"
    );
    // Each answers from the new file too.
    assert_eq!(layers(&at("b.flare")), layers(&source));
}

#[test]
fn convert_refuses_to_replace_without_force_and_replaces_whole_with_it() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("a.fla").to_string_lossy().into_owned();
    let target = dir.path().join("b.fla").to_string_lossy().into_owned();
    std::fs::copy(fixture("sample.fla"), &source).expect("copy");
    std::fs::write(&target, b"keep me").expect("write");
    let refused = run(&["convert", &source, &target]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("--force"));
    assert_eq!(std::fs::read(&target).expect("read"), b"keep me");
    json(&["convert", &source, &target, "--force"]);
    assert_eq!(
        std::fs::read(&target).expect("read"),
        std::fs::read(&source).expect("read")
    );
}

#[test]
fn a_name_with_no_ending_becomes_a_fla_and_a_missing_source_changes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("a.fla").to_string_lossy().into_owned();
    std::fs::copy(fixture("sample.fla"), &source).expect("copy");
    let done = json(&[
        "convert",
        &source,
        &dir.path().join("plain").to_string_lossy(),
    ]);
    assert_eq!(done["format"], "fla");
    assert!(dir.path().join("plain.fla").is_file());
    let failed = run(&[
        "convert",
        &dir.path().join("missing.fla").to_string_lossy(),
        &dir.path().join("x.fla").to_string_lossy(),
    ]);
    assert!(!failed.status.success());
    assert!(!dir.path().join("x.fla").exists());
}

#[test]
fn new_makes_a_fla_when_the_name_says_so_and_a_movie_is_never_overwritten() {
    let dir = tempfile::tempdir().expect("dir");
    let fla = dir.path().join("n.fla").to_string_lossy().into_owned();
    json(&["new", &fla]);
    assert!(flare_import::import_path(Path::new(&fla)).is_ok());
    json(&["add-layer", &fla, "Two"]);
    assert_eq!(layers(&fla), 2);

    let movie = dir.path().join("m.swf").to_string_lossy().into_owned();
    json(&["publish-swf", &fla, &movie]);
    let published = std::fs::read(&movie).expect("read");
    let refused = run(&["add-layer", &movie, "X"]);
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("cannot be written back"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    assert_eq!(std::fs::read(&movie).expect("read"), published);
}
