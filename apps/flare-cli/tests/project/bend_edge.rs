//! `flare bend-edge`: the wiring of `flare_tools::reshape` to the command line. The geometry is
//! tested in the library; here the file, the options, merged against object and the refusals.

use super::*;

fn square(file: &str, merge: bool) {
    json(&["new", file]);
    let mut args = vec!["draw-rect", file, "--rect", "0", "0", "100", "100"];
    if merge {
        args.push("--merge");
    }
    json(&args);
}

#[test]
fn dragging_an_edge_of_merged_drawing_curves_it_and_writes_the_project() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    square(&file, true);
    let plan = json(&[
        "bend-edge",
        &file,
        "--from",
        "50",
        "1",
        "--to",
        "50",
        "-40",
        "--dry-run",
    ]);
    assert_eq!(plan["bent"], false);
    let saved = std::fs::read(&file).expect("file");
    let bent = json(&["bend-edge", &file, "--from", "50", "1", "--to", "50", "-40"]);
    assert_eq!(bent["bent"], true);
    assert_ne!(
        std::fs::read(&file).expect("file"),
        saved,
        "the project changed"
    );
    // The top edge is up there now.
    let again = json(&[
        "bend-edge",
        &file,
        "--from",
        "50",
        "-40",
        "--to",
        "50",
        "-40",
        "--dry-run",
    ]);
    assert_eq!(again["bent"], false, "{again}");
}

#[test]
fn an_object_a_miss_and_bad_numbers_are_refused_and_change_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    square(&file, false);
    let saved = std::fs::read(&file).expect("file");
    let object = run(&["bend-edge", &file, "--from", "50", "0", "--to", "50", "-40"]);
    assert!(
        !object.status.success(),
        "a drawing object is not reshaped by its edge"
    );
    assert!(String::from_utf8_lossy(&object.stderr).contains("no merged line or shape"));
    let miss = run(&["bend-edge", &file, "--from", "400", "400", "--to", "0", "0"]);
    assert!(!miss.status.success());
    let bad = run(&["bend-edge", &file, "--from", "nan", "0", "--to", "0", "0"]);
    assert!(!bad.status.success());
    assert_eq!(std::fs::read(&file).expect("file"), saved);
}
