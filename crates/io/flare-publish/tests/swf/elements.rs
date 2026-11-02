//! A layer's elements go out in stacking order, each at a depth of its own.

use super::*;

fn places(parsed: &Parsed) -> Vec<(u16, Option<u16>)> {
    parsed
        .tags
        .iter()
        .take_while(|t| !matches!(t, Tag::ShowFrame))
        .filter_map(|t| match t {
            Tag::PlaceObject(p) => Some((p.depth, p.clip_depth)),
            _ => None,
        })
        .collect()
}

fn two_in_one_layer() -> flare_document::Layer {
    let first = span(
        0,
        1,
        Content::Shape(rect(0.0, 0.0, 10.0, 10.0, Rgba8::opaque(255, 0, 0))),
    )
    .with_extras(vec![Content::Shape(rect(
        5.0,
        5.0,
        10.0,
        10.0,
        Rgba8::opaque(0, 0, 255),
    ))])
    .expect("extras");
    layer("both", vec![first])
}

#[test]
fn a_layers_elements_take_consecutive_depths_in_stacking_order() {
    let below = layer(
        "below",
        vec![span(
            0,
            1,
            Content::Shape(rect(0.0, 0.0, 20.0, 20.0, Rgba8::opaque(0, 255, 0))),
        )],
    );
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![two_in_one_layer(), below]),
    );
    let doc = document(20, 20, vec![scene], vec![]);
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    // The lower layer has depth 2; the upper layer's first element is above it, its second above that.
    assert_eq!(
        places(&parse(&published.bytes)),
        vec![(2, None), (4, None), (5, None)]
    );
}

#[test]
fn a_mask_clips_every_depth_of_the_elements_it_masks() {
    let mask = layer(
        "mask",
        vec![span(
            0,
            1,
            Content::Shape(rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)),
        )],
    )
    .with_mask(true);
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![mask, two_in_one_layer()]),
    );
    let doc = document(20, 20, vec![scene], vec![]);
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    // The clip layer at depth 1 reaches up to the last element, depth 3.
    assert_eq!(
        places(&parse(&published.bytes)),
        vec![(1, Some(3)), (2, None), (3, None)]
    );
}

#[test]
fn so_many_elements_that_depths_run_out_are_an_error() {
    // Fewer layers than the layer limit, but five elements each use up the depths.
    let five = || {
        let extras = (1..5)
            .map(|i| Content::Shape(rect(f64::from(i), 0.0, 1.0, 1.0, Rgba8::WHITE)))
            .collect();
        let first = span(0, 1, Content::Shape(rect(0.0, 0.0, 1.0, 1.0, Rgba8::WHITE)))
            .with_extras(extras)
            .expect("extras");
        layer("five", vec![first])
    };
    let layers: Vec<_> = (0..12_000).map(|_| five()).collect();
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    let doc = document(20, 20, vec![scene], vec![]);
    let result = publish_swf(&doc, &PublishOptions::default());
    assert!(matches!(result, Err(PublishError::TooManyLayers(12_000))));
}
