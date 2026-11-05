//! Flipnote Studio files through the command line: a `.ppm` and a `.kwz` import as projects, and a
//! damaged one is refused and leaves no project behind.

use super::*;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/io/flare-import/tests/fixtures/flipnote")
        .join(name)
}

fn layer_names(project: &str) -> Vec<String> {
    json(&["info", project])["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .map(|l| l["name"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn a_ppm_imports_as_two_layers_and_paper() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("blank.ppm").to_string_lossy().into_owned();
    std::fs::copy(fixture("blank.ppm"), &source).expect("copy");
    let project = dir.path().join("p.flare").to_string_lossy().into_owned();
    let report = json(&["import", &source, &project]);
    assert!(
        report.to_string().contains("1 frames at 12 a second"),
        "{report}"
    );
    assert_eq!(layer_names(&project), ["Layer 1", "Layer 2", "Paper"]);
}

#[test]
fn a_kwz_imports_as_three_layers_and_paper_and_names_its_sound() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("blank.kwz").to_string_lossy().into_owned();
    std::fs::copy(fixture("blank.kwz"), &source).expect("copy");
    let project = dir.path().join("p.flare").to_string_lossy().into_owned();
    let report = json(&["import", &source, &project]);
    assert_eq!(
        layer_names(&project),
        ["Layer 1", "Layer 2", "Layer 3", "Paper"]
    );
    assert!(
        report.to_string().contains("sound is not imported"),
        "{report}"
    );
}

#[test]
fn a_damaged_ppm_is_refused_and_leaves_no_project() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("broken.ppm");
    let bytes = std::fs::read(fixture("blank.ppm")).expect("read");
    std::fs::write(&source, &bytes[..200]).expect("write");
    let project = dir.path().join("p.flare");
    let output = run(&[
        "import",
        &source.to_string_lossy(),
        &project.to_string_lossy(),
    ]);
    assert!(!output.status.success());
    assert!(!project.exists());
}
