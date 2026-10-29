use super::*;

#[test]
fn a_mask_layer_is_not_drawn_and_clips_the_layer_below_it() {
    // Top to bottom: the mask, then the layer it masks.
    let (doc, id) = masked_scene(vec![
        shape_layer("mask", 0.0, 10.0, true, true),
        shape_layer("art", 0.0, 40.0, false, true),
    ]);
    let frame = evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(frame.shapes.len(), 1, "only the masked artwork is drawn");
    assert_eq!(frame.clips.len(), 1);
    assert_eq!(frame.clips[0].len(), 1);
    assert_eq!(frame.shapes[0].clips, vec![0]);
}

#[test]
fn a_mask_only_affects_the_layer_directly_below_and_hidden_masks_stop_masking() {
    let (doc, id) = masked_scene(vec![
        shape_layer("mask", 0.0, 10.0, true, true),
        shape_layer("masked", 0.0, 40.0, false, true),
        shape_layer("free", 0.0, 40.0, false, true),
    ]);
    let frame = evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(frame.shapes.len(), 2);
    // Bottom layer paints first: "free" is unmasked, "masked" is clipped.
    assert!(frame.shapes[0].clips.is_empty());
    assert_eq!(frame.shapes[1].clips, vec![0]);

    let (doc, id) = masked_scene(vec![
        shape_layer("mask", 0.0, 10.0, true, false),
        shape_layer("art", 0.0, 40.0, false, true),
    ]);
    let frame = evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(frame.shapes.len(), 1);
    assert!(
        frame.shapes[0].clips.is_empty(),
        "a hidden mask does not clip"
    );
    assert!(frame.clips.is_empty());
}

#[test]
fn a_mask_with_no_content_at_the_frame_hides_what_it_masks() {
    let (doc, id) = masked_scene(vec![
        shape_layer("mask", 0.0, 10.0, true, true),
        shape_layer("art", 0.0, 40.0, false, true),
    ]);
    let frame = evaluate_frame(&doc, id, 9).expect("eval past the spans");
    assert!(frame.shapes.is_empty(), "the art also ends at frame 5");
    // At a frame where only the masked layer has content, the empty mask clips it all.
    let art_only = Layer::new(
        LayerId::new(),
        "art",
        vec![FrameSpan::new(
            0,
            NonZeroU32::new(9).expect("nonzero"),
            Content::Shape(
                flare_document::Shape::rect(
                    flare_core::ShapeId::new(),
                    0.0,
                    0.0,
                    5.0,
                    5.0,
                    flare_core::Rgba8::WHITE,
                )
                .expect("rect"),
            ),
        )],
    )
    .expect("layer");
    let (doc, id) = masked_scene(vec![shape_layer("mask", 0.0, 10.0, true, true), art_only]);
    let frame = evaluate_frame(&doc, id, 7).expect("eval");
    assert_eq!(frame.shapes.len(), 1);
    assert_eq!(
        frame.clips,
        vec![Vec::new()],
        "an empty region clips everything"
    );
}

#[test]
fn masks_inside_symbols_and_outside_them_both_apply() {
    let art = SymbolId::new();
    let inner = Timeline::new(vec![
        shape_layer("m", 0.0, 10.0, true, true),
        shape_layer("a", 0.0, 40.0, false, true),
    ]);
    let outer_mask = shape_layer("outer mask", 0.0, 20.0, true, true);
    let instance_layer =
        Layer::new(LayerId::new(), "inst", vec![span(0, 5, art, at(0.0))]).expect("layer");
    let (doc, id) = document(
        Timeline::new(vec![outer_mask, instance_layer]),
        vec![Symbol::new(art, "Art", inner)],
    );
    let frame = evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(frame.shapes.len(), 1);
    assert_eq!(frame.shapes[0].clips.len(), 2, "outer then inner");
}

#[test]
fn moving_a_mask_changes_the_signature_even_if_the_masked_art_stays() {
    let (doc, id) = masked_scene(vec![
        shape_layer("mask", 0.0, 10.0, true, true),
        shape_layer("art", 0.0, 40.0, false, true),
    ]);
    let frame = evaluate_frame(&doc, id, 0).expect("eval");
    let mut moved = frame.clone();
    moved.clips[0][0].transform = DAffine2::from_translation(DVec2::new(5.0, 0.0));
    assert_eq!(frame.shapes, moved.shapes, "the masked art is untouched");
    assert_ne!(frame.signature(), moved.signature());
}

#[test]
fn a_hidden_folder_hides_what_it_holds_however_deep() {
    let folder = |name: &str, visible: bool, parent: Option<LayerId>| {
        let id = LayerId::new();
        (
            id,
            Layer::new(id, name, vec![])
                .expect("layer")
                .with_folder(true)
                .with_visible(visible)
                .with_parent(parent),
        )
    };
    let (outer_id, outer) = folder("outer", true, None);
    let (inner_id, inner) = folder("inner", false, Some(outer_id));
    let child = shape_layer("child", 0.0, 10.0, false, true).with_parent(Some(inner_id));
    let loose = shape_layer("loose", 20.0, 10.0, false, true);
    let (doc, id) = masked_scene(vec![outer, inner, child, loose]);
    let frame = evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(
        frame.shapes.len(),
        1,
        "only the layer outside the hidden folder shows"
    );

    // With the inner folder shown again everything is drawn; folders draw nothing themselves.
    let (outer_id, outer) = folder("outer", true, None);
    let (inner_id, inner) = folder("inner", true, Some(outer_id));
    let child = shape_layer("child", 0.0, 10.0, false, true).with_parent(Some(inner_id));
    let (doc, id) = masked_scene(vec![outer, inner, child]);
    assert_eq!(evaluate_frame(&doc, id, 0).expect("eval").shapes.len(), 1);
    // And hiding the outer one hides the child two levels down.
    let (outer_id, outer) = folder("outer", false, None);
    let (inner_id, inner) = folder("inner", true, Some(outer_id));
    let child = shape_layer("child", 0.0, 10.0, false, true).with_parent(Some(inner_id));
    let (doc, id) = masked_scene(vec![outer, inner, child]);
    assert!(evaluate_frame(&doc, id, 0).expect("eval").shapes.is_empty());
}

fn guide_shape(path: Vec<flare_document::PathSegment>) -> FrameSpan {
    let shape = flare_document::Shape::new(
        flare_core::ShapeId::new(),
        path,
        flare_core::Rgba8::TRANSPARENT,
    )
    .expect("shape");
    FrameSpan::new(
        0,
        NonZeroU32::new(15).expect("nonzero"),
        Content::Shape(shape),
    )
}

fn corner_path() -> Vec<flare_document::PathSegment> {
    use flare_document::PathSegment::{LineTo, MoveTo};
    let p = |x, y| flare_core::DocumentPoint::new(x, y).expect("finite");
    // Down 100, then right 100: 200 long, with the corner exactly halfway.
    vec![
        MoveTo(p(0.0, 0.0)),
        LineTo(p(0.0, 100.0)),
        LineTo(p(100.0, 100.0)),
    ]
}

#[test]
fn a_motion_guide_steers_the_tween_of_the_layer_below_it_and_is_never_drawn() {
    let art = SymbolId::new();
    let mover = vec![
        span(0, 10, art, at(0.0)).with_tween(Tween::Linear),
        span(10, 5, art, at(100.0)),
    ];
    let guide_layer = Layer::new(LayerId::new(), "guide", vec![guide_shape(corner_path())])
        .expect("layer")
        .with_guide(true);
    let mover_layer = Layer::new(LayerId::new(), "mover", mover).expect("layer");
    let (doc, id) = document(
        Timeline::new(vec![guide_layer, mover_layer]),
        vec![Symbol::new(art, "Art", Timeline::default())],
    );
    let position = |frame| {
        let frame = evaluate_frame(&doc, id, frame).expect("eval");
        assert!(frame.shapes.is_empty(), "the guide is never drawn");
        frame.placed[0].transform.translation
    };
    assert!(
        (position(0) - DVec2::new(0.0, 0.0)).length() < 1e-9,
        "starts at the path's start"
    );
    // Halfway through the tween the instance is halfway along the path: at the corner.
    assert!(
        (position(5) - DVec2::new(0.0, 100.0)).length() < 1e-6,
        "{:?}",
        position(5)
    );
    // Two frames in is a fifth of the tween: a fifth of the 200 length, 40 down the first leg.
    let quarter = evaluate_frame(&doc, id, 2).expect("eval").placed[0]
        .transform
        .translation;
    assert!(
        (quarter - DVec2::new(0.0, 40.0)).length() < 1e-6,
        "{quarter:?}"
    );

    // Without the guide the same tween goes straight across.
    let straight = {
        let mover = vec![
            span(0, 10, art, at(0.0)).with_tween(Tween::Linear),
            span(10, 5, art, at(100.0)),
        ];
        let (doc, id) = document(
            Timeline::new(vec![
                Layer::new(LayerId::new(), "mover", mover).expect("layer"),
            ]),
            vec![Symbol::new(art, "Art", Timeline::default())],
        );
        evaluate_frame(&doc, id, 5).expect("eval").placed[0]
            .transform
            .translation
    };
    assert!((straight - DVec2::new(50.0, 0.0)).length() < 1e-9);
}

#[test]
fn a_hidden_guide_stops_guiding_and_paths_are_followed_by_length() {
    let art = SymbolId::new();
    let mover = vec![
        span(0, 10, art, at(0.0)).with_tween(Tween::Linear),
        span(10, 5, art, at(100.0)),
    ];
    let guide_layer = Layer::new(LayerId::new(), "guide", vec![guide_shape(corner_path())])
        .expect("layer")
        .with_guide(true)
        .with_visible(false);
    let (doc, id) = document(
        Timeline::new(vec![
            guide_layer,
            Layer::new(LayerId::new(), "mover", mover)
                .expect("layer")
                .with_visible(true),
        ]),
        vec![Symbol::new(art, "Art", Timeline::default())],
    );
    let x = evaluate_frame(&doc, id, 5).expect("eval").placed[0]
        .transform
        .translation;
    assert!(
        (x - DVec2::new(50.0, 0.0)).length() < 1e-9,
        "a hidden guide does not guide"
    );

    let path = corner_path();
    assert_eq!(point_along(&path, 0.0), Some(DVec2::new(0.0, 0.0)));
    assert_eq!(point_along(&path, 1.0), Some(DVec2::new(100.0, 100.0)));
    assert_eq!(
        point_along(&path, 2.0),
        Some(DVec2::new(100.0, 100.0)),
        "clamped"
    );
    assert_eq!(point_along(&[], 0.5), None);
    let dot = [flare_document::PathSegment::MoveTo(
        flare_core::DocumentPoint::new(3.0, 4.0).expect("finite"),
    )];
    assert_eq!(
        point_along(&dot, 0.7),
        Some(DVec2::new(3.0, 4.0)),
        "no length is just the point"
    );
}
