//! `flare convert-to-bitmap`: the wiring of `flare_runtime::convert_to_bitmap` to the command
//! line. How the selection is drawn is tested in the runtime; here the options, the saved
//! project, the picture it renders and the refusals.

use super::*;

fn stage(file: &str) {
    json(&["new", file]);
    json(&[
        "set-stage",
        file,
        "--width",
        "100",
        "--height",
        "60",
        "--background",
        "ffffff",
    ]);
    json(&[
        "draw-rect",
        file,
        "--rect",
        "10",
        "10",
        "30",
        "20",
        "--color",
        "ff0000",
    ]);
}

fn rendered(file: &str, dir: &Path, name: &str) -> Vec<u8> {
    let out = dir.join(name);
    json(&["export", file, out.to_str().expect("utf-8")]);
    std::fs::read(out).expect("png")
}

#[test]
fn a_keyframe_becomes_one_bitmap_that_renders_the_same() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let before = rendered(&file, dir.path(), "before.png");
    let out = json(&["convert-to-bitmap", &file]);
    assert_eq!(out["converted"], 1);
    assert_eq!(
        (out["width"].as_u64(), out["height"].as_u64()),
        (Some(30), Some(20))
    );
    let doc = flare_storage::load(Path::new(&file)).expect("load");
    let span = doc.scenes()[0].timeline().layers()[0]
        .span_at(0)
        .expect("span");
    assert!(matches!(
        span.element(0),
        Some(flare_document::Content::Bitmap(_))
    ));
    assert_eq!(rendered(&file, dir.path(), "after.png"), before);
    let twice = json(&["convert-to-bitmap", &file, "--scale", "2", "--element", "0"]);
    assert_eq!(
        (twice["width"].as_u64(), twice["height"].as_u64()),
        (Some(60), Some(40))
    );
}

#[test]
fn every_refusal_leaves_the_file_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    // A held layer shows its last keyframe at frame 500; released, nothing does.
    json(&["layer", "release", &file]);
    let before = std::fs::read(&file).expect("read");
    let unknown = "6f1c2d3e-4a5b-4c6d-8e7f-001122334455";
    for (args, says) in [
        (vec!["--scale", "9"], "scale"),
        (vec!["--element", "4"], "not in the document"),
        (vec!["--frame", "500"], "no keyframe"),
        (vec!["--layer", unknown], "no layer"),
    ] {
        let mut all = vec!["convert-to-bitmap", file.as_str()];
        all.extend(args.iter().copied());
        let output = run(&all);
        assert!(!output.status.success(), "{args:?} was refused");
        let error = String::from_utf8_lossy(&output.stderr).to_lowercase();
        assert!(error.contains(says), "{args:?}: {error}");
        assert_eq!(std::fs::read(&file).expect("read"), before, "{args:?}");
    }
}
