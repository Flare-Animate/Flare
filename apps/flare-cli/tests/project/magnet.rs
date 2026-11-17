//! `flare magnet`: the wiring of `flare_tools::magnet` to the command line. The bending is
//! tested in the library; here the file, the options and the refusals are checked.

use super::*;

#[test]
fn a_pull_bends_the_edge_near_it_and_writes_the_project() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    json(&["draw-rect", &file, "--rect", "0", "0", "100", "100"]);
    // Nothing is near (50, -35) yet.
    let away = [
        "magnet", &file, "--from", "50", "-35", "--to", "50", "-35", "--radius", "10",
    ];
    let before = json(&[&away[..], &["--dry-run"]].concat());
    assert_eq!(before["reached"], serde_json::json!([]));
    // Pull the top edge up from its middle.
    let pulled = json(&[
        "magnet", &file, "--from", "50", "0", "--to", "50", "-40", "--radius", "30",
    ]);
    assert_eq!(pulled["pulled"], true);
    assert_eq!(pulled["reached"].as_array().map(Vec::len), Some(1));
    let after = json(&[&away[..], &["--dry-run"]].concat());
    assert_eq!(
        after["reached"].as_array().map(Vec::len),
        Some(1),
        "the edge is up there now: {after}"
    );
}

#[test]
fn a_pull_that_reaches_nothing_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    json(&["draw-rect", &file, "--rect", "0", "0", "100", "100"]);
    let saved = std::fs::read(&file).expect("file");
    let refused = run(&[
        "magnet", &file, "--from", "400", "400", "--to", "0", "0", "--radius", "10",
    ]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("no line or shape"));
    assert_eq!(std::fs::read(&file).expect("file"), saved);
    let bad = run(&["magnet", &file, "--from", "nan", "0", "--to", "0", "0"]);
    assert!(!bad.status.success());
}
