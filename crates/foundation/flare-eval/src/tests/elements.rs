use super::*;

fn rect(x: f64) -> Content {
    Content::Shape(
        flare_document::Shape::rect(
            flare_core::ShapeId::new(),
            x,
            0.0,
            10.0,
            10.0,
            flare_core::Rgba8::WHITE,
        )
        .expect("rect"),
    )
}

fn many_layer(contents: Vec<Content>, mask: bool) -> Layer {
    let mut contents = contents.into_iter();
    let first = contents.next().expect("one");
    let span = FrameSpan::new(0, NonZeroU32::new(5).expect("nonzero"), first)
        .with_extras(contents.collect())
        .expect("extras");
    Layer::new(LayerId::new(), "L", vec![span])
        .expect("layer")
        .with_mask(mask)
}

#[test]
fn every_element_of_a_span_is_drawn_bottom_to_top_in_one_layer() {
    let (a, b, c) = (rect(0.0), rect(20.0), rect(40.0));
    let scene = Timeline::new(vec![many_layer(
        vec![a.clone(), b.clone(), c.clone()],
        false,
    )]);
    let (doc, id) = document(scene, vec![]);
    let frame = evaluate_frame(&doc, id, 2).expect("eval");
    let drawn: Vec<_> = frame
        .shapes
        .iter()
        .map(|d| Content::Shape(d.shape.clone().into_owned()))
        .collect();
    assert_eq!(drawn, vec![a, b, c]);
}

#[test]
fn a_masks_extra_elements_all_clip_and_instances_among_shapes_are_placed() {
    let scene = Timeline::new(vec![
        many_layer(vec![rect(0.0), rect(50.0)], true),
        many_layer(vec![rect(0.0)], false),
    ]);
    let (doc, id) = document(scene, vec![]);
    let frame = evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(frame.clips.len(), 1);
    assert_eq!(
        frame.clips[0].len(),
        2,
        "the mask region is every shape of the mask"
    );

    let art = SymbolId::new();
    let instance = Content::Instance(Instance::new(InstanceId::new(), art, at(7.0)));
    let scene = Timeline::new(vec![many_layer(vec![rect(0.0), instance], false)]);
    let (doc, id) = document(scene, vec![Symbol::new(art, "Art", Timeline::default())]);
    let frame = evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(frame.shapes.len(), 1);
    assert_eq!(frame.placed.len(), 1);
}

#[test]
fn a_level_lists_each_element_of_a_layer_with_its_index() {
    let scene = Timeline::new(vec![many_layer(vec![rect(0.0), rect(20.0)], false)]);
    let (doc, id) = document(scene, vec![]);
    let timeline = doc
        .scenes()
        .iter()
        .find(|s| s.id() == id)
        .expect("scene")
        .timeline();
    let level = evaluate_level(&doc, timeline, 0).expect("level");
    let seen: Vec<_> = level.items.iter().map(|i| (i.layer, i.element)).collect();
    assert_eq!(seen, vec![(0, 0), (0, 1)]);
}
