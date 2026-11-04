#![allow(clippy::expect_used)]
//! Moho, BVH and Toonz files go through the same entry points as Flash documents.

use std::path::Path;

use flare_import::{ImportError, Severity, foreign_kind, import_foreign_bytes, import_path};

const MOHO: &str = r#"{"project_data": {"width": 320, "height": 240, "fps": 12},
  "layers": [{"name": "Photo", "type": "ImageLayer", "image_path": "a.png"}]}"#;
const BVH: &str = "HIERARCHY\nROOT Hips\n{\n OFFSET 0 0 0\n CHANNELS 1 Zrotation\n End Site\n {\n  OFFSET 0 5 0\n }\n}\nMOTION\nFrames: 2\nFrame Time: 0.1\n0\n10\n";

#[test]
fn names_pick_the_reader() {
    assert_eq!(foreign_kind(Path::new("a.MOHO")), Some("moho"));
    assert_eq!(foreign_kind(Path::new("a.bvh")), Some("bvh"));
    assert_eq!(foreign_kind(Path::new("a.tnz")), Some("toonz"));
    assert_eq!(foreign_kind(Path::new("a.fla")), None);
}

#[test]
fn a_moho_file_imports_with_unsupported_parts_named() {
    let out =
        import_foreign_bytes(Path::new("x.mohoproj"), MOHO.as_bytes().to_vec()).expect("imports");
    assert_eq!(out.document.stage().width(), 320);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Unsupported)
    );
}

#[test]
fn bvh_imports_from_a_path() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("walk.bvh");
    std::fs::write(&path, BVH).expect("write");
    let out = import_path(&path).expect("imports");
    assert_eq!(out.document.frame_rate().numerator(), 10);
}

#[test]
fn broken_files_are_refused_by_name() {
    let err = import_foreign_bytes(Path::new("x.moho"), b"not json".to_vec()).expect_err("refused");
    assert!(
        matches!(err, ImportError::Foreign(ref m) if m.contains("x.moho")),
        "{err}"
    );
    let err = import_foreign_bytes(Path::new("x.bvh"), vec![0xff, 0xfe]).expect_err("refused");
    assert!(matches!(err, ImportError::Foreign(_)));
}
