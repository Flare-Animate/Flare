//! `/` in an XFL edge string is a line, as `|` is: the way Animate writes some shapes.

#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_core::Rgba8;
use flare_document::Document;
use flare_import::{FileSource, Severity, import_xfl};

struct Files(HashMap<String, String>);

impl FileSource for Files {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).map(|s| s.clone().into_bytes()))
    }
}

/// A 400 by 300 stage with a red 400 by 300 square drawn with `edges`.
fn square(edges: &str) -> Files {
    let xml = format!(
        r##"<DOMDocument frameRate="12" width="400" height="300" backgroundColor="#FFFFFF">
<timelines><DOMTimeline name="Scene 1"><layers><DOMLayer name="L"><frames><DOMFrame index="0"><elements>
<DOMShape><fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
<edges><Edge fillStyle1="1" edges="{edges}"/></edges></DOMShape>
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

#[test]
fn slash_lines_draw_the_same_square_as_bar_lines() {
    let bars = import_xfl(&square("!0 0|8000 0|8000 6000|0 6000|0 0")).expect("import");
    let slashes = import_xfl(&square("!0 0/8000 0/8000 6000/0 6000/0 0")).expect("import");
    for imported in [&bars, &slashes] {
        assert!(
            imported
                .diagnostics
                .iter()
                .all(|d| d.severity != Severity::Unsupported),
            "{:?}",
            imported.diagnostics
        );
        assert_eq!(
            pixel(&imported.document, 200, 150),
            Rgba8::new(255, 0, 0, 255)
        );
    }
}

#[test]
fn a_slash_joined_to_a_number_is_read_as_a_line_break_not_a_bad_number() {
    // The shape of the real case: `-235/-33` is a line to (-33, -1), not one number.
    let imported = import_xfl(&square("!0 0/-33 -1/8000 0")).expect("import");
    assert!(
        imported
            .diagnostics
            .iter()
            .all(|d| !d.message.contains("bad number")),
        "{:?}",
        imported.diagnostics
    );
}
