//! Names come from people and files, so whatever they hold, the exported XML stays well-formed
//! and can be imported again.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId, SymbolId};
use flare_document::{Document, Layer, Scene, Symbol, Timeline};
use flare_export::{export_xfl, fla_bytes};
use flare_import::import_fla_bytes;
use quick_xml::Reader;
use quick_xml::events::Event;

/// Every character that could break an attribute, or that XML 1.0 cannot carry at all.
const NASTY: &str =
    "a<b>&c\"d'e\tf\ng\rh\u{1}i\u{1f}j\u{fffe}k\u{ffff}l \u{1f525} \u{e9} ]]> <!-- -->";

fn document() -> Document {
    let layer = Layer::new(LayerId::new(), NASTY, vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), NASTY, Timeline::new(vec![layer]));
    let symbol = Symbol::new(SymbolId::new(), NASTY, Timeline::default());
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![symbol],
    )
    .expect("document")
}

fn is_well_formed(xml: &str) -> bool {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => return true,
            Err(_) => return false,
            Ok(_) => {}
        }
    }
}

#[test]
fn every_file_is_well_formed_whatever_the_names_hold() {
    let output = export_xfl(&document()).expect("export");
    assert!(output.files.len() >= 2, "the document and the symbol");
    for (name, body) in &output.files {
        assert!(is_well_formed(body), "{name} is not well-formed XML");
        assert!(
            !body.contains(['\u{fffe}', '\u{ffff}', '\u{1}']),
            "{name} holds a character XML 1.0 cannot carry"
        );
    }
}

#[test]
fn such_a_document_imports_again_and_keeps_what_can_be_kept() {
    let output = export_xfl(&document()).expect("export");
    let back = import_fla_bytes(fla_bytes(&output).expect("bytes")).expect("import");
    let scene = &back.document.scenes()[0];
    assert!(
        scene.name().starts_with("a<b>&c\"d'e"),
        "{:?}",
        scene.name()
    );
    assert!(scene.name().contains('\u{1f525}') && scene.name().contains('\u{e9}'));
    assert!(
        !scene.name().contains(['\u{fffe}', '\u{ffff}', '\u{1}']),
        "characters XML cannot hold became spaces"
    );
    assert_eq!(back.document.symbols().len(), 1);
}
