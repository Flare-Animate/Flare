//! What the importer cannot edit it keeps. These tests check that unsupported source text is
//! attached to the right entity, exactly, and that nothing is kept for a layer that was skipped.
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_document::{Anchor, Attachment, Document, Placement};
use flare_import::{FileSource, Severity, import_xfl};

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

pub const MAIN: &str = r##"<DOMDocument frameRate="24" width="100" height="50">
<media><DOMBitmapItem name="pic" href="pic.png"/></media>
<fonts><DOMFontItem name="Sans"/></fonts>
<symbols><Include href="Box.xml"/></symbols>
<timelines><DOMTimeline name="Scene 1"><layers>
  <DOMLayer name="Main"><frames>
    <DOMFrame index="0" duration="3">
      <Actionscript><script><![CDATA[if (a < b) { stop(); }]]></script></Actionscript>
      <elements>
        <DOMShape><fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
          <edges><Edge fillStyle1="1" edges="!0 0|200 0|200 200|0 200|0 0"/></edges></DOMShape>
        <DOMStaticText left="5"><textRuns><DOMTextRun><characters>Hello &amp; &lt;world&gt;</characters></DOMTextRun></textRuns></DOMStaticText>
      </elements>
    </DOMFrame>
    <DOMFrame index="3" duration="2"><elements><DOMBitmapInstance libraryItemName="pic"/></elements></DOMFrame>
  </frames></DOMLayer>
  <DOMLayer name="Cut" layerType="mask"><frames><DOMFrame index="0"><elements><DOMBitmapInstance libraryItemName="pic"/></elements></DOMFrame></frames></DOMLayer>
  <DOMLayer name="Broken"><frames>
    <DOMFrame index="0" duration="5"><elements><DOMStaticText/></elements></DOMFrame>
    <DOMFrame index="2" duration="5"><elements/></DOMFrame>
  </frames></DOMLayer>
</layers></DOMTimeline></timelines></DOMDocument>"##;

pub const BOX: &str = r##"<DOMSymbolItem name="Box" symbolType="graphic"><scaleGridRect left="1" right="9"/>
<timeline><DOMTimeline name="Box"><layers><DOMLayer name="L"><frames>
  <DOMFrame index="0"><elements><DOMStaticText/></elements></DOMFrame>
</frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>"##;

fn import() -> flare_import::Imported {
    import_xfl(
        &Files::default()
            .with("DOMDocument.xml", MAIN)
            .with("LIBRARY/Box.xml", BOX),
    )
    .expect("import")
}

fn layer_id(document: &Document, name: &str) -> flare_core::LayerId {
    document.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.name() == name)
        .unwrap_or_else(|| panic!("no layer {name}"))
        .id()
}

fn find(document: &Document, anchor: Anchor, placement: Placement) -> Vec<&Attachment> {
    document
        .attachments()
        .iter()
        .filter(|a| a.anchor() == anchor && a.placement() == placement)
        .collect()
}

#[test]
fn unsupported_elements_are_attached_to_the_layer_and_keyframe_they_came_from() {
    let imported = import();
    let doc = &imported.document;
    let main = layer_id(doc, "Main");

    let text = find(
        doc,
        Anchor::Layer(main),
        Placement::FrameElement { frame: 0 },
    );
    assert_eq!(text.len(), 1);
    assert_eq!(
        text[0].content(),
        "<DOMStaticText left=\"5\"><textRuns><DOMTextRun><characters>Hello &amp; &lt;world&gt;</characters></DOMTextRun></textRuns></DOMStaticText>",
        "text content survives exactly, escaped"
    );
    let bitmap = find(
        doc,
        Anchor::Layer(main),
        Placement::FrameElement { frame: 3 },
    );
    assert_eq!(
        bitmap[0].content(),
        "<DOMBitmapInstance libraryItemName=\"pic\"/>"
    );

    let script = find(doc, Anchor::Layer(main), Placement::FrameExtra { frame: 0 });
    assert_eq!(script.len(), 1);
    assert!(
        script[0].content().contains("if (a &lt; b) { stop(); }"),
        "{}",
        script[0].content()
    );
    assert_eq!(script[0].format(), "xfl");
}

#[test]
fn skipped_layers_document_children_and_symbol_data_are_kept_where_they_belong() {
    let imported = import();
    let doc = &imported.document;
    let scene = doc.scenes()[0].id();

    // The mask layer was the second source layer, so it goes back at index 1.
    let mask = find(doc, Anchor::Scene(scene), Placement::LayerAt { index: 1 });
    assert_eq!(mask.len(), 1);
    assert!(
        mask[0]
            .content()
            .starts_with("<DOMLayer name=\"Cut\" layerType=\"mask\">")
    );

    let document_level = find(doc, Anchor::Document, Placement::Child);
    let names: Vec<&str> = document_level.iter().map(|a| a.content()).collect();
    assert_eq!(names.len(), 2);
    assert!(names[0].starts_with("<media>") && names[1].starts_with("<fonts>"));

    let symbol = doc.symbols()[0].id();
    let symbol_level = find(doc, Anchor::Symbol(symbol), Placement::Child);
    assert_eq!(symbol_level.len(), 1);
    assert_eq!(
        symbol_level[0].content(),
        "<scaleGridRect left=\"1\" right=\"9\"/>"
    );
    // The symbol's own timeline also had an unsupported text element.
    let inner = doc.symbols()[0].timeline().layers()[0].id();
    assert_eq!(
        find(
            doc,
            Anchor::Layer(inner),
            Placement::FrameElement { frame: 0 }
        )
        .len(),
        1
    );
}

#[test]
fn nothing_is_kept_for_a_layer_that_was_skipped_and_the_loss_is_reported() {
    let imported = import();
    let doc = &imported.document;
    // "Broken" has overlapping frames, so the layer was skipped, and with it the text element.
    assert!(
        doc.scenes()[0]
            .timeline()
            .layers()
            .iter()
            .all(|l| l.name() != "Broken")
    );
    // `<DOMStaticText/>` appears in "Broken" (skipped) and inside the Box symbol (imported):
    // exactly the symbol's copy must have been kept.
    let bare = doc
        .attachments()
        .iter()
        .filter(|a| a.content() == "<DOMStaticText/>")
        .count();
    assert_eq!(bare, 1, "only the imported symbol's element is kept");

    let warnings: Vec<String> = imported
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .map(|d| d.message.clone())
        .collect();
    assert!(
        warnings.iter().any(|m| m.contains("could not be kept")),
        "{warnings:?}"
    );
    let info: Vec<String> = imported
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .map(|d| d.message.clone())
        .collect();
    assert!(
        info.iter()
            .any(|m| m.contains("will be written back on export")),
        "{info:?}"
    );
}

#[test]
fn every_kept_attachment_is_valid_and_anchored_to_something_that_exists() {
    let imported = import();
    // Re-applying the attachments strictly would fail on any dangling anchor or bad digest.
    let attachments = imported.document.attachments().to_vec();
    assert!(attachments.len() >= 8, "{} kept", attachments.len());
    imported
        .document
        .clone()
        .with_attachments(attachments)
        .expect("all anchors exist");
}

#[test]
fn a_document_with_nothing_unsupported_keeps_nothing() {
    let xml = r##"<DOMDocument><timelines><DOMTimeline name="S"><layers><DOMLayer name="L"><frames>
        <DOMFrame index="0"><elements><DOMShape><fills><FillStyle index="1"><SolidColor color="#000000"/></FillStyle></fills>
        <edges><Edge fillStyle1="1" edges="!0 0|20 0|20 20|0 20|0 0"/></edges></DOMShape></elements></DOMFrame>
        </frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"##;
    let imported = import_xfl(&Files::default().with("DOMDocument.xml", xml)).expect("import");
    assert!(imported.document.attachments().is_empty());
    assert!(
        imported
            .diagnostics
            .iter()
            .all(|d| !d.message.contains("source data"))
    );
}
