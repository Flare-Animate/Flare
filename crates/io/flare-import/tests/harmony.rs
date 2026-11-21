//! Importing Toon Boom Harmony scenes (.xstage). Fixtures are written here, in the shape
//! Harmony 21 writes, so a test fails when the reading is wrong and not when a foreign tool
//! changes.

#![allow(clippy::expect_used)]

use flare_import::harmony::{import, looks_like_xstage};
use flare_import::{ImportError, Severity, import_bytes};

/// A scene with `columns` (`(name, displayOrder)`) on a 1920 by 1080 stage at 24 fps.
fn scene(columns: &[(&str, i32)], frames: u32) -> String {
    let columns: String = columns
        .iter()
        .enumerate()
        .map(|(index, (name, order))| {
            format!(
                "    <column type=\"0\" name=\"{name}\" displayOrder=\"{order}\" width=\"100\" id=\"{}\"/>\n",
                index + 1
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <project source=\"Harmony Premium (HarmonyPremium) version 21.0.3 build 18334\" \
         version=\"2103\" build=\"18334\" creator=\"harmony\">\n\
         <elements>\n\
         <element id=\"1\" elementName=\"Drawing\" elementFolder=\"Drawing\" pixmapFormat=\"2\"/>\n\
         </elements>\n\
         <options>\n\
         <resolution name=\"HDTV_1080p24\" size=\"1920,1080\" fovFit=\"VerticalFitFov\"/>\n\
         <framerate val=\"24\"/>\n\
         </options>\n\
         <scenes>\n\
         <scene name=\"Top\" id=\"0b0e5000b1861eda\" nbframes=\"{frames}\" startFrame=\"1\" \
         stopFrame=\"{frames}\">\n\
         <columns>\n{columns}</columns>\n\
         </scene>\n\
         </scenes>\n\
         </project>\n"
    )
}

fn layers(document: &flare_document::Document) -> Vec<(&str, u32, u32)> {
    document.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|layer| {
            let span = layer.spans().first().expect("a span");
            (layer.name(), span.start(), span.duration().get())
        })
        .collect()
}

#[test]
fn a_scene_becomes_a_project_with_its_stage_rate_and_columns() {
    let imported = import(
        "Scene.xstage",
        scene(&[("Character", 0), ("Background", 1)], 48).as_bytes(),
    )
    .expect("import");
    let document = &imported.document;
    assert_eq!(
        (document.stage().width(), document.stage().height()),
        (1920, 1080)
    );
    assert_eq!(document.frame_rate().numerator(), 24);
    assert_eq!(document.scenes()[0].name(), "Top");
    assert_eq!(
        layers(document),
        vec![("Character", 0, 48), ("Background", 0, 48)]
    );
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|note| note.severity == Severity::Unsupported
                && note.message.contains("drawings")),
        "the note says the drawings stay in Harmony: {:?}",
        imported.diagnostics
    );
}

#[test]
fn columns_are_ordered_by_display_order() {
    let imported = import(
        "Scene.xstage",
        scene(&[("Top", 2), ("Middle", 0), ("Bottom", 1)], 12).as_bytes(),
    )
    .expect("import");
    let names: Vec<&str> = imported.document.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(flare_document::Layer::name)
        .collect();
    assert_eq!(names, ["Middle", "Bottom", "Top"]);
}

#[test]
fn a_scene_without_columns_gets_one_empty_layer_and_a_note() {
    let imported = import("Scene.xstage", scene(&[], 24).as_bytes()).expect("import");
    assert_eq!(layers(&imported.document), vec![("Layer 1", 0, 24)]);
    assert_eq!(imported.diagnostics.len(), 1);
}

#[test]
fn a_scene_without_a_resolution_is_refused_by_name() {
    let text = scene(&[("Drawing", 0)], 24).replace(
        "<resolution name=\"HDTV_1080p24\" size=\"1920,1080\" fovFit=\"VerticalFitFov\"/>",
        "",
    );
    let error = import("Scene.xstage", text.as_bytes()).expect_err("refused");
    let message = error.to_string();
    assert!(message.contains("Scene.xstage"), "{message}");
    assert!(message.contains("no stage resolution"), "{message}");
}

#[test]
fn a_scene_without_a_frame_rate_is_refused() {
    let text = scene(&[("Drawing", 0)], 24).replace("<framerate val=\"24\"/>", "");
    let error = import("Scene.xstage", text.as_bytes()).expect_err("refused");
    assert!(error.to_string().contains("no frame rate"), "{error}");
}

#[test]
fn a_scene_with_no_frames_is_refused() {
    let error =
        import("Scene.xstage", scene(&[("Drawing", 0)], 0).as_bytes()).expect_err("refused");
    assert!(error.to_string().contains("no frame count"), "{error}");
}

#[test]
fn more_columns_than_the_limit_are_refused() {
    let columns: Vec<(String, i32)> = (0..1_001)
        .map(|index| (format!("Column {index}"), index))
        .collect();
    let borrowed: Vec<(&str, i32)> = columns
        .iter()
        .map(|(name, order)| (name.as_str(), *order))
        .collect();
    let error = import("Scene.xstage", scene(&borrowed, 24).as_bytes()).expect_err("refused");
    assert!(
        error.to_string().contains("more than 1000 columns"),
        "{error}"
    );
}

#[test]
fn a_file_that_is_not_xml_is_refused() {
    let error = import("Scene.xstage", b"not a scene at all").expect_err("refused");
    assert!(matches!(error, ImportError::Foreign(_)), "{error:?}");
}

#[test]
fn a_scene_read_whole_imports_the_same_as_one_from_disk() {
    let bytes = scene(&[("Character", 0)], 30).into_bytes();
    let imported = import_bytes(bytes).expect("import");
    assert_eq!(
        layers(&imported.document),
        vec![("Character", 0, 30)],
        "the browser and the MCP server have no path, but the scene is still read"
    );
}

#[test]
fn looks_like_xstage_tells_harmony_from_other_xml() {
    assert!(looks_like_xstage(scene(&[("Drawing", 0)], 1).as_bytes()));
    assert!(looks_like_xstage(b"<xstage><scene/></xstage>"));
    assert!(!looks_like_xstage(
        b"<DOMDocument><timelines/></DOMDocument>"
    ));
    assert!(!looks_like_xstage(b"PK\x03\x04"));
}
