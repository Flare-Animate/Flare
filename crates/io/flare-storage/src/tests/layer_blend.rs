use super::records::{edit_values, set_key};
use super::*;
use flare_document::LayerBlend;

fn document_of(layers: Vec<Layer>) -> Document {
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

#[test]
fn blend_modes_and_raster_layers_are_kept_and_plain_layers_stay_plain() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let layers = vec![
        Layer::new(LayerId::new(), "Shade", vec![])
            .expect("layer")
            .with_blend(LayerBlend::Multiply)
            .with_raster(true),
        Layer::new(LayerId::new(), "Glow", vec![])
            .expect("layer")
            .with_blend(LayerBlend::Add),
        Layer::new(LayerId::new(), "Ink", vec![]).expect("layer"),
    ];
    let document = document_of(layers);
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded, document);
    let kept: Vec<(LayerBlend, bool)> = loaded.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| (l.blend(), l.is_raster()))
        .collect();
    assert_eq!(
        kept,
        vec![
            (LayerBlend::Multiply, true),
            (LayerBlend::Add, false),
            (LayerBlend::Normal, false)
        ]
    );
}

#[test]
fn a_blend_mode_this_build_does_not_know_is_a_damaged_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let layer = Layer::new(LayerId::new(), "L", vec![])
        .expect("layer")
        .with_blend(LayerBlend::Screen);
    save(&document_of(vec![layer]), &path).expect("save");
    edit_values(&path, &["scene"], |v| {
        set_key(v, "blend", &serde_json::json!("dissolve"))
    });
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}
