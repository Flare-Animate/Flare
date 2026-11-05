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

/// A version 6.6 level with no frames: the header and the end tag.
fn empty_pli() -> Vec<u8> {
    let mut f = b"PLIM".to_vec();
    f.extend([6, 6, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0]);
    f
}

const TNZ: &str = r#"<tnz version="71.0"><properties><outputs><output name="main"><fps>24</fps></output></outputs></properties>
<levelSet><levels><level id="1" name="A" type="vector"><path>+drawings/A.pli</path></level>
<level id="2" name="Evil" type="vector"><path>+../../secret.pli</path></level></levels></levelSet>
<xsheet><columns><levelColumn><cells><cell>0 1 1 1 0</cell></cells></levelColumn>
<levelColumn><cells><cell>0 1 2 1 0</cell></cells></levelColumn></columns></xsheet></tnz>"#;

#[test]
fn a_scene_finds_its_levels_beside_it_and_never_outside() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir(dir.path().join("drawings")).expect("mkdir");
    std::fs::write(dir.path().join("drawings/A.pli"), empty_pli()).expect("write");
    std::fs::write(dir.path().join("secret.pli"), empty_pli()).expect("write");
    let scene = dir.path().join("s.tnz");
    std::fs::write(&scene, TNZ).expect("write");
    let out = import_path(&scene).expect("imports");
    assert_eq!(out.document.scenes()[0].timeline().layers().len(), 2);
    assert!(
        out.diagnostics.iter().all(|d| d.location != "A"),
        "{:?}",
        out.diagnostics
    );
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.location == "Evil" && d.message.contains("not found"))
    );
}

#[test]
fn a_bare_level_palette_and_raster_are_handled_by_name() {
    let out = import_foreign_bytes(Path::new("x.pli"), empty_pli()).expect("imports");
    assert_eq!(out.document.scenes().len(), 1);
    let err = import_foreign_bytes(Path::new("x.tpl"), b"<palette><styles/></palette>".to_vec())
        .expect_err("refused");
    assert!(
        err.to_string().contains("palette is not a project"),
        "{err}"
    );
    let err =
        import_foreign_bytes(Path::new("x.tlv"), b"TLV12\0\0\0".to_vec()).expect_err("refused");
    assert!(err.to_string().contains("tlv"), "{err}");
    assert!(import_foreign_bytes(Path::new("x.tnz"), b"<nope/>".to_vec()).is_err());
}

#[test]
fn an_svg_goes_through_the_foreign_entry_point() {
    assert_eq!(foreign_kind(Path::new("a.SVG")), Some("svg"));
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="6"><rect width="4" height="4"/></svg>"#;
    let out = import_foreign_bytes(Path::new("a.svg"), svg.as_bytes().to_vec()).expect("imports");
    assert_eq!(out.document.stage().width(), 8);
    let err = import_foreign_bytes(Path::new("a.svg"), b"<svg".to_vec()).expect_err("refused");
    assert!(matches!(err, ImportError::Foreign(_)));
}
