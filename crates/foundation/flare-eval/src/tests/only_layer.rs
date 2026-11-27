use super::*;

/// The names of the layers whose shapes the frame drew, by the shape each `shape_layer` holds.
fn drawn(frame: &Frame<'_>, layers: &[&Layer]) -> Vec<String> {
    let shape_of = |layer: &Layer| match layer.spans().first().map(FrameSpan::content) {
        Some(Content::Shape(shape)) => Some(shape.id()),
        _ => None,
    };
    let mut names: Vec<String> = frame
        .shapes
        .iter()
        .filter_map(|s| {
            layers
                .iter()
                .find(|l| shape_of(l) == Some(s.shape.id()))
                .map(|l| l.name().to_owned())
        })
        .collect();
    names.sort();
    names
}

#[test]
fn one_layer_draws_only_its_own_shapes() {
    let top = shape_layer("top", 0.0, 10.0, false, true);
    let bottom = shape_layer("bottom", 20.0, 30.0, false, true);
    let (doc, id) = masked_scene(vec![top.clone(), bottom.clone()]);
    let layers = [&top, &bottom];
    assert_eq!(
        drawn(&evaluate_frame(&doc, id, 0).expect("eval"), &layers),
        vec!["bottom", "top"]
    );
    let only_top = evaluate_frame_of_layer_without_camera(&doc, id, 0, top.id()).expect("eval");
    assert_eq!(drawn(&only_top, &layers), vec!["top"]);
    let only_bottom = evaluate_frame_of_layer(&doc, id, 0, bottom.id()).expect("eval");
    assert_eq!(drawn(&only_bottom, &layers), vec!["bottom"]);
    // A layer the scene does not have draws nothing rather than everything.
    let stranger = evaluate_frame_of_layer(&doc, id, 0, LayerId::new()).expect("eval");
    assert!(stranger.shapes.is_empty());
}

#[test]
fn one_layer_is_still_masked_and_a_folder_brings_what_it_holds() {
    // A mask above the layer still clips it though the mask is not the layer asked for.
    let mask = shape_layer("mask", 0.0, 5.0, true, true);
    let art = shape_layer("art", 0.0, 40.0, false, true);
    let (doc, id) = masked_scene(vec![mask.clone(), art.clone()]);
    let frame = evaluate_frame_of_layer(&doc, id, 0, art.id()).expect("eval");
    assert_eq!(drawn(&frame, &[&mask, &art]), vec!["art"]);
    assert_eq!(frame.shapes[0].clips, vec![0]);

    let folder_id = LayerId::new();
    let folder = Layer::new(folder_id, "folder", vec![])
        .expect("layer")
        .with_folder(true);
    let inner = shape_layer("inner", 0.0, 10.0, false, true).with_parent(Some(folder_id));
    let deeper_id = LayerId::new();
    let deeper = Layer::new(deeper_id, "deeper", vec![])
        .expect("layer")
        .with_folder(true)
        .with_parent(Some(folder_id));
    let deepest = shape_layer("deepest", 0.0, 20.0, false, true).with_parent(Some(deeper_id));
    let loose = shape_layer("loose", 0.0, 30.0, false, true);
    let layers = [&inner, &deepest, &loose];
    let (doc, id) = masked_scene(vec![
        folder,
        inner.clone(),
        deeper,
        deepest.clone(),
        loose.clone(),
    ]);
    assert_eq!(
        drawn(
            &evaluate_frame_of_layer(&doc, id, 0, folder_id).expect("eval"),
            &layers
        ),
        vec!["deepest", "inner"],
        "the folder's layers, however deep"
    );
    assert_eq!(
        drawn(
            &evaluate_frame_of_layer(&doc, id, 0, deeper_id).expect("eval"),
            &layers
        ),
        vec!["deepest"]
    );
}
