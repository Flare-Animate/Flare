//! `flare raster-brush`: the wiring of `flare_tools::blend::lay_paint` to the command line. How
//! the paint mixes is tested in the library; here the options, the saved picture, the layer and
//! the refusals.

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
}

/// The pixel at `(x, y)` of the topmost picture on the first layer at frame 0, with how many
/// pictures the project holds.
fn painted(file: &str, x: usize, y: usize) -> ([u8; 4], usize) {
    let doc = flare_storage::load(Path::new(file)).expect("load");
    let span = doc.scenes()[0].timeline().layers()[0]
        .span_at(0)
        .expect("a keyframe");
    let top = span.element_count() - 1;
    let Some(flare_document::Content::Bitmap(placed)) = span.element(top) else {
        panic!("not a bitmap");
    };
    let bitmap = doc.bitmap(placed.bitmap()).expect("bitmap");
    let i = (y * bitmap.width() as usize + x) * 4;
    let p = &bitmap.pixels()[i..i + 4];
    ([p[0], p[1], p[2], p[3]], doc.bitmaps().len())
}

#[test]
fn a_stroke_paints_its_color_and_a_second_one_mixes_with_it() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let first = json(&[
        "raster-brush",
        &file,
        "--along",
        "5",
        "30",
        "--along",
        "95",
        "30",
        "--size",
        "20",
        "--color",
        "ff0000",
        "--mix",
        "0",
    ]);
    assert_eq!(first["tool"], "raster-brush");
    assert_eq!(first["points"], 2);
    assert_eq!(painted(&file, 50, 30), ([255, 0, 0, 255], 1));
    json(&[
        "raster-brush",
        &file,
        "--along",
        "10",
        "30",
        "--along",
        "90",
        "30",
        "--size",
        "20",
        "--color",
        "0000ff",
        "--mix",
        "50",
        "--layer",
        first["layer"].as_str().expect("layer id"),
    ]);
    let (mixed, pictures) = painted(&file, 50, 30);
    assert_eq!(pictures, 1, "painted into the same picture");
    assert!(
        (100..=155).contains(&mixed[0]) && (100..=155).contains(&mixed[2]),
        "half mixed: {mixed:?}"
    );
}

#[test]
fn opacity_and_pressure_are_taken() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    json(&[
        "raster-brush",
        &file,
        "--along",
        "5",
        "30",
        "--along",
        "95",
        "30",
        "--pressure",
        "1",
        "1",
        "--size",
        "20",
        "--color",
        "0000ff",
        "--strength",
        "50",
    ]);
    let (p, _) = painted(&file, 50, 30);
    assert!((120..=136).contains(&p[3]), "half opaque: {p:?}");
}

#[test]
fn every_refusal_leaves_the_file_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let before = std::fs::read(&file).expect("read");
    let unknown = "6f1c2d3e-4a5b-4c6d-8e7f-001122334455".to_string();
    for (args, says) in [
        (vec!["--mix", "150"], "color mix"),
        (vec!["--color", "nope"], "color"),
        (vec!["--layer", unknown.as_str()], "no such layer"),
        (vec!["--size", "0"], "size"),
        (vec!["--pressure", "2"], "pressure"),
        (vec!["--color", "00000000"], "did not change"),
    ] {
        let mut all = vec!["raster-brush", file.as_str(), "--along", "10", "10"];
        all.extend(args.iter().copied());
        let output = run(&all);
        assert!(!output.status.success(), "{args:?} was refused");
        let error = String::from_utf8_lossy(&output.stderr).to_lowercase();
        assert!(error.contains(says), "{args:?}: {error}");
        assert_eq!(std::fs::read(&file).expect("read"), before, "{args:?}");
    }
}
