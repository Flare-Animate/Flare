//! The kind of each symbol, and of the instances of it, goes out in XFL and comes back.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, Scene, Symbol, SymbolKind, Timeline,
};
use flare_export::{export_xfl, fla_bytes};
use flare_import::import_fla_bytes;
use std::num::NonZeroU32;

fn document() -> Document {
    let symbols: Vec<Symbol> = SymbolKind::ALL
        .into_iter()
        .map(|kind| Symbol::new(SymbolId::new(), kind.label(), Timeline::default()).with_kind(kind))
        .collect();
    let layers: Vec<Layer> = symbols
        .iter()
        .map(|symbol| {
            let instance = Instance::new(InstanceId::new(), symbol.id(), Transform2D::IDENTITY);
            let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Instance(instance));
            Layer::new(LayerId::new(), symbol.name(), vec![span]).expect("layer")
        })
        .collect();
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        symbols,
    )
    .expect("document")
}

fn file<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
    &files
        .iter()
        .find(|(n, _)| n.ends_with(name))
        .unwrap_or_else(|| panic!("no file {name}"))
        .1
}

#[test]
fn symbols_and_their_instances_are_written_with_the_type_xfl_uses() {
    let output = export_xfl(&document()).expect("export");
    let graphic = file(&output.files, "Graphic.xml");
    assert!(graphic.contains(r#"symbolType="graphic""#), "{graphic}");
    let button = file(&output.files, "Button.xml");
    assert!(button.contains(r#"symbolType="button""#), "{button}");
    // Animate leaves the type off a movie clip.
    let clip = file(&output.files, "Movie Clip.xml");
    assert!(!clip.contains("symbolType"), "{clip}");
    let main = file(&output.files, "DOMDocument.xml");
    assert!(
        main.contains(r#"libraryItemName="Button" symbolType="button""#),
        "{main}"
    );
    assert!(
        main.contains(r#"libraryItemName="Graphic" symbolType="graphic" loop="loop""#),
        "{main}"
    );
    let movie_clip_instance = main
        .split("<DOMSymbolInstance")
        .find(|part| part.contains(r#"libraryItemName="Movie Clip""#))
        .expect("an instance of the clip");
    let tag = movie_clip_instance.split('>').next().expect("tag");
    assert!(
        !tag.contains("symbolType") && !tag.contains("loop"),
        "{tag}"
    );
}

#[test]
fn every_kind_comes_back_from_an_exported_fla() {
    let output = export_xfl(&document()).expect("export");
    let back = import_fla_bytes(fla_bytes(&output).expect("bytes")).expect("import");
    let kinds: Vec<(String, SymbolKind)> = back
        .document
        .symbols()
        .iter()
        .map(|s| (s.name().to_owned(), s.kind()))
        .collect();
    let expected: Vec<(String, SymbolKind)> = SymbolKind::ALL
        .into_iter()
        .map(|k| (k.label().to_owned(), k))
        .collect();
    assert_eq!(kinds, expected);
}
