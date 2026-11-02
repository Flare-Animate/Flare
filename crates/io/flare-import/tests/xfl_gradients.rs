#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_core::Rgba8;
use flare_document::{Content, Document};
use flare_import::{FileSource, Severity, import_xfl};

struct Files(HashMap<String, String>);

impl FileSource for Files {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).map(|s| s.clone().into_bytes()))
    }
}

/// A 400 by 300 stage with one rectangle over all of it, filled with `fill`.
fn stage_with(fill: &str) -> Files {
    let xml = format!(
        r##"<DOMDocument frameRate="12" width="400" height="300" backgroundColor="#FFFFFF">
<timelines><DOMTimeline name="Scene 1"><layers><DOMLayer name="L"><frames><DOMFrame index="0"><elements>
<DOMShape><fills><FillStyle index="1">{fill}</FillStyle></fills>
<edges><Edge fillStyle1="1" edges="!0 0|8000 0|8000 6000|0 6000|0 0"/></edges></DOMShape>
</elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"##
    );
    Files(HashMap::from([("DOMDocument.xml".to_owned(), xml)]))
}

fn pixel(document: &Document, x: u32, y: u32) -> Rgba8 {
    let scene = document.scenes()[0].id();
    let evaluated = flare_eval::evaluate_frame(document, scene, 0).expect("eval");
    let stage = document.stage();
    flare_render::render(
        &evaluated,
        stage.width(),
        stage.height(),
        stage.background(),
    )
    .expect("render")
    .pixel(x, y)
    .expect("pixel")
}

fn messages(imported: &flare_import::Imported, severity: Severity) -> String {
    imported
        .diagnostics
        .iter()
        .filter(|d| d.severity == severity)
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A gradient square 300 px long, top to bottom, over the whole 300 px height: the matrix scale
/// is 6000 twips over 32768 units, turned a quarter.
const VERTICAL: &str = r##"<LinearGradient>
<matrix><Matrix a="0" b="0.18310546875" c="-0.18310546875" d="0" tx="4000" ty="3000"/></matrix>
<GradientEntry color="#FF0000" ratio="0"/><GradientEntry color="#0000FF" ratio="1"/></LinearGradient>"##;

#[test]
fn a_gradient_background_is_drawn_as_a_gradient_not_left_out() {
    let imported = import_xfl(&stage_with(VERTICAL)).expect("import");
    let document = &imported.document;
    let top = pixel(document, 200, 4);
    let middle = pixel(document, 200, 150);
    let bottom = pixel(document, 200, 295);
    assert!(top.r > 230 && top.b < 25, "red at the top: {top:?}");
    assert!(
        bottom.b > 230 && bottom.r < 25,
        "blue at the bottom: {bottom:?}"
    );
    assert!(
        (100..160).contains(&middle.r) && (100..160).contains(&middle.b),
        "purple between: {middle:?}"
    );
    assert!(
        !messages(&imported, Severity::Unsupported).contains("gradient"),
        "{:?}",
        imported.diagnostics
    );
    let Content::Shape(shape) = document.scenes()[0].timeline().layers()[0].spans()[0]
        .content()
        .clone()
    else {
        panic!("a shape");
    };
    assert!(shape.gradient().is_some());
}

#[test]
fn a_gradient_that_does_not_sit_on_its_shape_is_one_color_with_a_warning() {
    // The same gradient with its numbers read in the wrong units: a square 64 px long.
    let as_pixels = VERTICAL.replace("0.18310546875", "0.0390625");
    let imported = import_xfl(&stage_with(&as_pixels)).expect("import");
    let (top, bottom) = (
        pixel(&imported.document, 200, 4),
        pixel(&imported.document, 200, 295),
    );
    assert_eq!(top, bottom, "one color all over");
    assert!(
        top.r > 100 && top.b > 100 && top.g < 20,
        "the average of the stops: {top:?}"
    );
    let warnings = messages(&imported, Severity::Warning);
    assert!(warnings.contains("does not sit on its shape"), "{warnings}");
}

#[test]
fn a_radial_gradient_is_one_color_and_the_report_says_so() {
    let radial = r##"<RadialGradient><matrix><Matrix a="0.1" d="0.1" tx="4000" ty="3000"/></matrix>
<GradientEntry color="#FFFFFF" ratio="0"/><GradientEntry color="#000000" ratio="1"/></RadialGradient>"##;
    let imported = import_xfl(&stage_with(radial)).expect("import");
    let middle = pixel(&imported.document, 200, 150);
    assert!((120..135).contains(&middle.r), "grey: {middle:?}");
    assert!(messages(&imported, Severity::Warning).contains("radial gradient"));
}

#[test]
fn a_bitmap_fill_is_still_reported_and_left_out() {
    let imported = import_xfl(&stage_with("<BitmapFill/>")).expect("import");
    assert!(messages(&imported, Severity::Unsupported).contains("bitmap fills are not imported"));
    assert_eq!(
        pixel(&imported.document, 200, 150),
        Rgba8::opaque(255, 255, 255)
    );
}
