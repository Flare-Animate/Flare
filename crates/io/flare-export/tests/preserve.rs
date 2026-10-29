//! Source preservation across import, edit, save and export. The headline check is a fixed
//! point: importing an exported document yields the same preserved data as the first import.
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Anchor, Attachment, Document, Layer, Placement, Scene, Timeline};
use flare_edit::{Command, EditorSession};
use flare_export::{Severity, export_xfl};
use flare_import::{FileSource, import_xfl};

struct Mem(HashMap<String, String>);

impl FileSource for Mem {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).map(|s| s.clone().into_bytes()))
    }
}

const MAIN: &str = r##"<DOMDocument frameRate="24" width="100" height="50">
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
        <DOMStaticText left="5"><textRuns><DOMTextRun><characters>  Hello &amp; &lt;world&gt;  </characters></DOMTextRun></textRuns></DOMStaticText>
      </elements>
    </DOMFrame>
    <DOMFrame index="3" duration="2"><elements><DOMBitmapInstance libraryItemName="pic"/></elements></DOMFrame>
  </frames></DOMLayer>
  <DOMLayer name="Cut" layerType="mask"><frames><DOMFrame index="0"><elements><DOMBitmapInstance libraryItemName="pic"/></elements></DOMFrame></frames></DOMLayer>
</layers></DOMTimeline></timelines></DOMDocument>"##;

const BOX: &str = r##"<DOMSymbolItem name="Box" symbolType="graphic"><scaleGridRect left="1" right="9"/>
<timeline><DOMTimeline name="Box"><layers><DOMLayer name="L"><frames>
  <DOMFrame index="0"><elements><DOMStaticText/></elements></DOMFrame>
</frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>"##;

fn source() -> Mem {
    Mem(HashMap::from([
        ("DOMDocument.xml".to_owned(), MAIN.to_owned()),
        ("LIBRARY/Box.xml".to_owned(), BOX.to_owned()),
    ]))
}

/// A description of an attachment that does not depend on generated IDs.
fn describe(document: &Document) -> Vec<String> {
    let layer_name = |id: LayerId| {
        let mut all: Vec<&Layer> = document
            .scenes()
            .iter()
            .flat_map(|s| s.timeline().layers())
            .collect();
        all.extend(
            document
                .symbols()
                .iter()
                .flat_map(|s| s.timeline().layers()),
        );
        all.into_iter()
            .find(|l| l.id() == id)
            .map(|l| l.name().to_owned())
            .expect("layer exists")
    };
    let mut out: Vec<String> = document
        .attachments()
        .iter()
        .map(|a| {
            let anchor = match a.anchor() {
                Anchor::Document => "document".to_owned(),
                Anchor::Scene(_) => "scene".to_owned(),
                Anchor::Symbol(id) => {
                    format!("symbol {}", document.symbol(id).expect("symbol").name())
                }
                Anchor::Layer(id) => format!("layer {}", layer_name(id)),
            };
            format!("{anchor} | {:?} | {}", a.placement(), a.content())
        })
        .collect();
    out.sort();
    out
}

fn export_then_import(document: &Document) -> Document {
    let output = export_xfl(document).expect("export");
    import_xfl(&Mem(output.files.into_iter().collect()))
        .expect("re-import")
        .document
}

#[test]
fn preserved_data_is_a_fixed_point_of_export_then_import() {
    let first = import_xfl(&source()).expect("import").document;
    let before = describe(&first);
    assert!(
        before.len() >= 8,
        "a meaningful amount was kept: {before:#?}"
    );

    let second = export_then_import(&first);
    assert_eq!(
        describe(&second),
        before,
        "nothing was lost or changed by the round trip"
    );
    // And a third generation is stable too.
    assert_eq!(describe(&export_then_import(&second)), before);
}

#[test]
fn kept_text_appears_in_the_export_exactly() {
    let doc = import_xfl(&source()).expect("import").document;
    let output = export_xfl(&doc).expect("export");
    let all: String = output.files.iter().map(|(_, body)| body.as_str()).collect();
    for snippet in [
        "<characters>  Hello &amp; &lt;world&gt;  </characters>",
        "<DOMBitmapInstance libraryItemName=\"pic\"/>",
        "<script>if (a &lt; b) { stop(); }</script>",
        "<media><DOMBitmapItem name=\"pic\" href=\"pic.png\"/></media>",
        "<scaleGridRect left=\"1\" right=\"9\"/>",
        "<DOMLayer name=\"Cut\" layerType=\"mask\">",
    ] {
        assert!(all.contains(snippet), "missing {snippet}");
    }
    assert!(
        !output
            .diagnostics
            .iter()
            .any(|d| d.message.contains("could not be placed")),
        "{:?}",
        output.diagnostics
    );
}

fn layer_named(document: &Document, name: &str) -> LayerId {
    document.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.name() == name)
        .expect("layer")
        .id()
}

#[test]
fn deleting_a_layer_removes_its_kept_data_and_undo_restores_it() {
    let imported = import_xfl(&source()).expect("import").document;
    let scene = imported.scenes()[0].id();
    let main = layer_named(&imported, "Main");
    let before = imported.attachments().len();
    let mut session = EditorSession::new(imported);

    session
        .apply([Command::RemoveLayer { scene, layer: main }])
        .expect("remove");
    let left = describe(session.document());
    assert!(
        left.iter().all(|d| !d.starts_with("layer Main")),
        "{left:#?}"
    );
    assert!(left.len() < before);
    let output = export_xfl(session.document()).expect("export");
    let all: String = output.files.iter().map(|(_, b)| b.as_str()).collect();
    assert!(
        !all.contains("Hello"),
        "the deleted layer's text is not resurrected"
    );
    assert!(
        !output
            .diagnostics
            .iter()
            .any(|d| d.message.contains("could not be placed")),
        "removal is clean, not a warning"
    );

    session.undo();
    assert_eq!(session.document().attachments().len(), before);
}

#[test]
fn data_whose_keyframe_was_edited_away_is_reported_not_guessed_at() {
    let imported = import_xfl(&source()).expect("import").document;
    let scene = imported.scenes()[0].id();
    let main = layer_named(&imported, "Main");
    let mut session = EditorSession::new(imported);

    // The bitmap element belonged to the keyframe at frame 3. Remove that keyframe.
    session
        .apply([Command::RemoveSpan {
            scene,
            layer: main,
            start: 3,
        }])
        .expect("remove span");
    let output = export_xfl(session.document()).expect("export");
    let all: String = output.files.iter().map(|(_, b)| b.as_str()).collect();
    // The Main layer no longer has a keyframe at 3, so its bitmap element is not written. (The
    // kept mask layer holds a similar element of its own, which is a different attachment.)
    let start = all.find("<DOMLayer name=\"Main\">").expect("Main layer");
    let end = all.find("<DOMLayer name=\"Cut\"").expect("kept mask layer");
    let main_layer = &all[start..end];
    assert!(!main_layer.contains("index=\"3\""), "{main_layer}");
    assert!(!main_layer.contains("DOMBitmapInstance"), "{main_layer}");
    let warnings: Vec<&str> = output
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        warnings
            .iter()
            .any(|m| m.contains("could not be placed") && m.contains("FrameElement")),
        "{warnings:?}"
    );
    // The data that still has its keyframe is untouched.
    assert!(all.contains("Hello &amp; &lt;world&gt;"));
}

fn doc_with(attachments: Vec<Attachment>) -> Document {
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("doc")
        .with_attachments(attachments)
        .expect("attachments")
}

#[test]
fn malformed_or_hostile_preserved_text_is_never_written_into_the_output() {
    let bad = [
        "not xml at all",
        "<a><b></a>",
        "<a/><evil/>",
        "</close><a/>",
        "<!DOCTYPE a [<!ENTITY x \"y\">]><a/>",
        "<?pi evil?><a/>",
        "stray text <a/>",
        "<a/> trailing <b/>",
        "",
    ];
    for content in bad {
        let note = Attachment::new("xfl", Anchor::Document, Placement::Child, content)
            .expect("attachment");
        let output = export_xfl(&doc_with(vec![note])).expect("export");
        let main = &output.files[0].1;
        assert!(
            main.ends_with("</timelines></DOMDocument>\n"),
            "{content:?} must not be written: {main}"
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|d| d.message.contains("not well-formed")),
            "{content:?}: {:?}",
            output.diagnostics
        );
        // The result is still a document the importer accepts.
        import_xfl(&Mem(output.files.into_iter().collect())).expect("still importable");
    }
    // Well-formed content is written; an unknown format is ignored without noise.
    let good = Attachment::new(
        "xfl",
        Anchor::Document,
        Placement::Child,
        "<ok a=\"1\">text</ok>",
    )
    .expect("good");
    let other =
        Attachment::new("other", Anchor::Document, Placement::Child, "<ignored/>").expect("other");
    let output = export_xfl(&doc_with(vec![good, other])).expect("export");
    assert!(output.files[0].1.contains("<ok a=\"1\">text</ok>"));
    assert!(!output.files[0].1.contains("ignored"));
}

#[test]
fn kept_layers_return_to_their_original_position_among_the_layers() {
    let first = Layer::new(LayerId::new(), "First", vec![]).expect("layer");
    let second = Layer::new(LayerId::new(), "Second", vec![]).expect("layer");
    let scene_id = SceneId::new();
    let scene = Scene::new(scene_id, "S", Timeline::new(vec![first, second]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let notes = vec![
        Attachment::new(
            "xfl",
            Anchor::Scene(scene_id),
            Placement::LayerAt { index: 0 },
            "<DOMLayer name=\"Top\"/>",
        )
        .expect("a"),
        Attachment::new(
            "xfl",
            Anchor::Scene(scene_id),
            Placement::LayerAt { index: 1 },
            "<DOMLayer name=\"Mid\"/>",
        )
        .expect("b"),
        Attachment::new(
            "xfl",
            Anchor::Scene(scene_id),
            Placement::LayerAt { index: 40 },
            "<DOMLayer name=\"Far\"/>",
        )
        .expect("c"),
    ];
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("doc")
        .with_attachments(notes)
        .expect("attach");
    let main = &export_xfl(&doc).expect("export").files[0].1;
    let order: Vec<usize> = [
        "name=\"Top\"",
        "name=\"First\"",
        "name=\"Mid\"",
        "name=\"Second\"",
        "name=\"Far\"",
    ]
    .iter()
    .map(|needle| {
        main.find(needle)
            .unwrap_or_else(|| panic!("missing {needle}"))
    })
    .collect();
    assert!(
        order.windows(2).all(|w| w[0] < w[1]),
        "layers are in source order, extras last: {order:?}"
    );
}

#[test]
fn saving_and_loading_the_project_does_not_change_what_is_exported() {
    let doc = import_xfl(&source()).expect("import").document;
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    flare_storage::save(&doc, &path).expect("save");
    let loaded = flare_storage::load(&path).expect("load");
    assert_eq!(loaded.attachments(), doc.attachments());
    assert_eq!(
        export_xfl(&loaded).expect("export").files,
        export_xfl(&doc).expect("export").files
    );
}
