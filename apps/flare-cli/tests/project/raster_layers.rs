//! Raster layers, blend modes, the Brush's presets and the pixel eraser from the command line:
//! the wiring of `flare_commands::layers` and `flare_tools::blend`. What they do is tested in
//! the libraries; here the options, what `info` reports and the refusals.

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

fn layer(file: &str, id: &str) -> Value {
    let info = json(&["info", file]);
    info["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .find(|l| l["id"] == id)
        .cloned()
        .expect("the layer")
}

/// The alpha of the painting on layer `id` under stage point `(x, y)`.
fn alpha(file: &str, id: &str, x: usize, y: usize) -> u8 {
    let doc = flare_storage::load(Path::new(file)).expect("load");
    let layer = doc.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.id().as_uuid().to_string() == id)
        .expect("layer");
    let span = layer.span_at(0).expect("a keyframe");
    let Some(flare_document::Content::Bitmap(placed)) = span.element(0) else {
        panic!("not a bitmap");
    };
    let bitmap = doc.bitmap(placed.bitmap()).expect("bitmap");
    let d = bitmap.width() as usize / 100;
    bitmap.pixels()[((y * d) * bitmap.width() as usize + x * d) * 4 + 3]
}

#[test]
fn a_raster_layer_is_made_reported_and_switched_back() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let made = json(&["add-layer", &file, "Paint", "--raster"]);
    let id = made["layer"].as_str().expect("id").to_owned();
    assert_eq!(layer(&file, &id)["raster"], true);
    assert_eq!(layer(&file, &id)["blend"], "normal");
    let plain = json(&["add-layer", &file, "Ink"]);
    let ink = plain["layer"].as_str().expect("id");
    assert_eq!(layer(&file, ink)["raster"], false, "vector unless asked");
    json(&["layer", "vector", &file, "--layer", &id]);
    assert_eq!(layer(&file, &id)["raster"], false);
    json(&["layer", "raster", &file, "--layer", &id]);
    assert_eq!(layer(&file, &id)["raster"], true);
}

#[test]
fn a_blend_mode_is_set_by_name_and_an_unknown_one_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let id = json(&["add-layer", &file, "Shade"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    let set = json(&[
        "layer",
        "blend",
        &file,
        "--layer",
        &id,
        "--mode",
        "Color Dodge",
    ]);
    assert_eq!(set["mode"], "color-dodge");
    assert_eq!(layer(&file, &id)["blend"], "color-dodge");
    let before = std::fs::read(&file).expect("read");
    for (args, says) in [
        (vec!["--mode", "dissolve"], "unknown blend mode"),
        (vec![], "--mode"),
    ] {
        let mut all = vec!["layer", "blend", file.as_str(), "--layer", id.as_str()];
        all.extend(args.iter().copied());
        let output = run(&all);
        assert!(!output.status.success(), "{args:?} was refused");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(says), "{args:?}: {error}");
        assert_eq!(std::fs::read(&file).expect("read"), before);
    }
}

#[test]
fn a_preset_paints_and_the_eraser_rubs_out() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let id = json(&["add-layer", &file, "Paint", "--raster"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    let stroke = ["--along", "5", "30", "--along", "95", "30"];
    let mut paint = vec!["raster-brush", file.as_str(), "--layer", id.as_str()];
    paint.extend(stroke);
    paint.extend(["--preset", "marker", "--color", "0000ff"]);
    json(&paint);
    let marker = alpha(&file, &id, 50, 30);
    assert!((170..=185).contains(&marker), "the marker's 70%: {marker}");

    let erase = json(&[
        "raster-erase",
        &file,
        "--layer",
        &id,
        "--along",
        "50",
        "10",
        "--along",
        "50",
        "50",
        "--size",
        "10",
    ]);
    assert_eq!(erase["tool"], "erase");
    assert_eq!(alpha(&file, &id, 50, 30), 0, "rubbed out");
    assert_eq!(alpha(&file, &id, 20, 30), marker, "the rest stays");

    let output = run(&[
        "raster-brush",
        &file,
        "--along",
        "5",
        "5",
        "--preset",
        "crayon",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown preset"));
}

#[test]
fn the_vector_brush_sends_a_raster_layer_to_raster_brush() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let id = json(&["add-layer", &file, "Paint", "--raster"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    let output = run(&[
        "brush", &file, "--layer", &id, "--along", "5", "5", "--along", "50", "50",
    ]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("raster-brush"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// What the first keyframe of layer `id` holds: (pictures, shapes).
fn held(file: &str, id: &str) -> (usize, usize) {
    let doc = flare_storage::load(Path::new(file)).expect("load");
    let layer = doc.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.id().as_uuid().to_string() == id)
        .expect("layer");
    let span = layer.span_at(0).expect("a keyframe");
    let count = |f: fn(&flare_document::Content) -> bool| span.elements().filter(|c| f(c)).count();
    (
        count(|c| matches!(c, flare_document::Content::Bitmap(_))),
        count(|c| matches!(c, flare_document::Content::Shape(_))),
    )
}

#[test]
fn converting_a_layer_draws_its_shapes_into_pixels_and_traces_them_back() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let id = json(&["add-layer", &file, "Ink"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "10",
        "10",
        "40",
        "20",
        "--color",
        "ff0000",
        "--layer",
        &id,
    ]);
    assert_eq!(held(&file, &id), (0, 1));
    let raster = json(&["layer", "raster", &file, "--layer", &id]);
    assert_eq!(raster["changed"], true);
    assert_eq!(held(&file, &id), (1, 0), "a painting");
    assert_eq!(layer(&file, &id)["raster"], true);
    json(&["layer", "vector", &file, "--layer", &id]);
    let (pictures, shapes) = held(&file, &id);
    assert_eq!((pictures, shapes > 0), (0, true), "traced into shapes");
    assert_eq!(layer(&file, &id)["raster"], false);
}
