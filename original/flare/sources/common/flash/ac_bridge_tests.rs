use super::*;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/flash_fixtures")
}
fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("flare_acbridge_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
fn rows(dir: &Path) -> Vec<Vec<String>> {
    std::fs::read_to_string(dir.join("frames.tsv")).unwrap().lines().map(|l| l.split('\t').map(String::from).collect()).collect()
}

#[test]
fn import_folder_and_zip_agree() {
    let (a, b) = (tmp("dir"), tmp("zip"));
    let la = import_to_dir(&fixtures().join("handmade_xfl"), &a).unwrap();
    let lb = import_to_dir(&fixtures().join("handmade.fla"), &b).unwrap();
    assert!(la.starts_with("ok "), "{la}");
    assert_eq!(rows(&a), rows(&b));
    let r = rows(&a);
    // the tweened ball layer yields one SVG per tween frame, so far more rows than files of a static layer
    assert!(r.len() >= 11, "{r:?}");
    let first = &r[0];
    assert_eq!(first.len(), 4);
    let svg = std::fs::read_to_string(a.join(&first[3])).unwrap();
    assert!(svg.starts_with("<svg") && !svg.contains("<rect width=\"100%\" height=\"100%\""), "{svg}");
    assert!(lb.contains("320x240") && lb.contains("@12"), "{lb}");
    assert_eq!(std::fs::read_to_string(a.join("meta.tsv")).unwrap().split('\t').next().unwrap(), "320");
}

#[test]
fn import_rejects_garbage() {
    let d = tmp("garbage");
    let f = d.join("x.fla");
    std::fs::write(&f, b"not a zip at all").unwrap();
    assert!(import_to_dir(&f, &d.join("out")).is_err());
    assert!(import_to_dir(&d.join("missing.fla"), &d.join("out2")).is_err());
}

#[test]
fn xfl_to_scene_to_xfl_round_trip() {
    let a = tmp("rt_in");
    import_to_dir(&fixtures().join("handmade.fla"), &a).unwrap();
    // Flare-side manifest: what the app writes for the xsheet it built from frames.tsv
    let r = rows(&a);
    let mut m = String::from("doc\t320\t240\t12\n");
    for row in &r {
        m += &format!("cell\t{}\t{}\t{}\t1\t{}\n", row[0], row[1], row[2], a.join(&row[3]).display());
    }
    let mf = a.join("manifest.tsv");
    std::fs::write(&mf, m).unwrap();
    let out = tmp("rt_out").join("round.fla");
    let log = export_xfl(&mf, &out).unwrap();
    assert!(log.starts_with("ok "), "{log}");
    let back = ac_xfl::open(&out).unwrap().doc;
    let layers = back.scene.layers.len();
    assert!(layers >= 1);
    assert_eq!((back.width, back.height, back.fps), (320.0, 240.0, 12.0));
    assert!(back.bitmaps.len() >= 1);
    // every imported frame row is a keyframe again
    let total: usize = back.scene.layers.iter().map(|l| l.keyframes.values().filter(|k| !k.elements.is_empty()).count()).sum();
    assert_eq!(total, r.len());
}

#[test]
fn html5_smoke() {
    let o = tmp("html5").join("movie.html");
    let log = export_html5_xfl(&fixtures().join("handmade.fla"), &o, true).unwrap();
    assert!(log.starts_with("ok "), "{log}");
    let html = std::fs::read_to_string(&o).unwrap();
    assert!(html.contains("<canvas") && html.len() > 1000);
}

#[test]
fn manifest_validation() {
    assert!(doc_from_manifest("doc\t0\t10\t24\ncell\t0\ta\t0\t1\t/x\n").is_err());
    assert!(doc_from_manifest("doc\t10\t10\t24\n").is_err());
    assert!(doc_from_manifest("doc\t10\t10\t24\ncell\t0\ta\t0\t0\t/x\n").is_err());
    assert!(doc_from_manifest("doc\t10\t10\t24\ncell\t0\ta\t0\t1\t/no/such/file.png\n").is_err());
}

#[test]
fn ffi_null_and_panic_safe() {
    unsafe {
        assert!(crate::flare_ac_xfl_import(std::ptr::null(), std::ptr::null()).is_null());
        assert!(crate::flare_ac_xfl_export(std::ptr::null(), std::ptr::null()).is_null());
        assert!(crate::flare_ac_html5_export(std::ptr::null(), std::ptr::null(), 1).is_null());
        let p = std::ffi::CString::new("/no/such").unwrap();
        let r = crate::flare_ac_xfl_import(p.as_ptr(), p.as_ptr());
        assert!(std::ffi::CStr::from_ptr(r).to_str().unwrap().starts_with("error"));
    }
    assert_eq!(crate::flare_ac_available(), 1);
}
