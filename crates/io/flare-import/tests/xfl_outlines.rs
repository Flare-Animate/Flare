//! How a shape's fill and stroke come in from XFL.
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_document::Content;
use flare_import::{FileSource, import_xfl};

#[derive(Default)]
struct Files(HashMap<String, String>);

impl Files {
    fn with(mut self, name: &str, xml: &str) -> Self {
        self.0.insert(name.to_owned(), xml.to_owned());
        self
    }
}

impl FileSource for Files {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).map(|s| s.clone().into_bytes()))
    }
}

/// One keyframe's shapes as the layers they import to.
fn import_shapes(inner: &str) -> Vec<(bool, bool)> {
    let xml = format!(
        r##"<DOMDocument frameRate="24" width="30" height="30"><timelines><DOMTimeline name="S"><layers>
    <DOMLayer name="L"><frames><DOMFrame index="0"><elements><DOMShape>{inner}</DOMShape></elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"##
    );
    let doc = import_xfl(&Files::default().with("DOMDocument.xml", &xml))
        .expect("import")
        .document;
    doc.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .filter_map(|l| match l.spans().first()?.content() {
            Content::Shape(s) => Some((s.fill().a > 0, s.stroke().is_some())),
            _ => None,
        })
        .collect()
}

const STYLES: &str = r##"<fills><FillStyle index="1"><SolidColor color="#00FF00"/></FillStyle></fills>
      <strokes><StrokeStyle index="1"><SolidStroke weight="2"><fill><SolidColor color="#000000"/></fill></SolidStroke></StrokeStyle></strokes>"##;

#[test]
fn a_stroke_along_exactly_the_fills_edges_is_its_outline_on_one_shape() {
    let shapes = import_shapes(&format!(
        r##"{STYLES}<edges><Edge fillStyle1="1" strokeStyle="1" edges="!0 0|400 0|400 400|0 400|0 0"/></edges>"##
    ));
    assert_eq!(
        shapes,
        vec![(true, true)],
        "one shape with a fill and a stroke"
    );
    // The same outline split over several edges, drawn in the other direction, is still it.
    let split = import_shapes(&format!(
        r##"{STYLES}<edges>
        <Edge fillStyle1="1" strokeStyle="1" edges="!0 0|400 0|400 400"/>
        <Edge fillStyle1="1" strokeStyle="1" edges="!400 400|0 400|0 0"/></edges>"##
    ));
    assert_eq!(split, vec![(true, true)]);
}

#[test]
fn a_stroke_that_is_not_the_fills_boundary_stays_a_line_of_its_own() {
    // The fill is a square; the line is only one side of it.
    let shapes = import_shapes(&format!(
        r##"{STYLES}<edges>
        <Edge fillStyle1="1" edges="!0 0|400 0|400 400|0 400|0 0"/>
        <Edge strokeStyle="1" edges="!0 0|400 0"/></edges>"##
    ));
    // Layers run top first, and a line sits above the fill it was drawn with.
    assert_eq!(shapes, vec![(false, true), (true, false)]);
    // A stroke with no fill at all is a line.
    let line = import_shapes(&format!(
        r##"{STYLES}<edges><Edge strokeStyle="1" edges="!0 0|400 0"/></edges>"##
    ));
    assert_eq!(line, vec![(false, true)]);
}
