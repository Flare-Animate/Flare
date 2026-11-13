use super::*;

#[test]
fn a_transparent_layer_keeps_its_opacity_and_an_opaque_one_stays_opaque() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let layers = [50, 0, 100]
        .into_iter()
        .map(|percent| {
            Layer::new(LayerId::new(), format!("{percent}"), vec![])
                .expect("layer")
                .with_view_opacity(percent)
        })
        .collect();
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded, document);
    let kept: Vec<u8> = loaded.scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(Layer::view_opacity)
        .collect();
    assert_eq!(kept, vec![50, 0, 100]);
}

#[test]
fn view_opacity_is_held_at_100() {
    let layer = Layer::new(LayerId::new(), "L", vec![])
        .expect("layer")
        .with_view_opacity(250);
    assert_eq!(layer.view_opacity(), 100);
}
