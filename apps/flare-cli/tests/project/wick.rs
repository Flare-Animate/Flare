//! Wick Editor projects through the command line: a `.wick` imports as a project by its extension,
//! and a gzip file named `.wick` is refused and leaves no project behind.

use super::*;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/io/flare-import/tests/fixtures/wick")
        .join(name)
}

#[test]
fn a_wick_project_imports_by_its_extension() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir
        .path()
        .join("synthetic.wick")
        .to_string_lossy()
        .into_owned();
    std::fs::copy(fixture("synthetic.wick"), &source).expect("copy");
    let project = dir.path().join("p.flare").to_string_lossy().into_owned();
    let report = json(&["import", &source, &project]);
    assert_eq!(report["scenes"].as_u64(), Some(1), "{report}");
    assert!(report.to_string().contains("Wick project"), "{report}");
    assert!(Path::new(&project).exists());
}

#[test]
fn a_gzip_file_named_wick_is_refused_and_leaves_no_project() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("packed.wick");
    std::fs::write(&source, [0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3]).expect("write");
    let project = dir.path().join("p.flare");
    let output = run(&[
        "import",
        &source.to_string_lossy(),
        &project.to_string_lossy(),
    ]);
    assert!(!output.status.success());
    assert!(!project.exists());
}
