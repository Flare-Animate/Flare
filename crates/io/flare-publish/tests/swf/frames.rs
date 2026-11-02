use super::*;

#[test]
fn the_played_movie_matches_the_evaluator_frame_by_frame() {
    let doc = animated();
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let played = play(&parsed);
    let scene = doc.scenes()[0].id();
    assert_eq!(played.len(), 10);
    for (frame, flat) in played.iter().enumerate() {
        let evaluated = flare_eval::evaluate_frame(&doc, scene, frame as u32).expect("eval");
        assert_eq!(
            flat.shapes.len(),
            evaluated.shapes.len(),
            "frame {frame}: same number of shapes"
        );
        for (placed, drawn) in flat.shapes.iter().zip(&evaluated.shapes) {
            let (got, want) = (
                placed.transform.to_cols_array(),
                drawn.transform.to_cols_array(),
            );
            for (g, w) in got.iter().zip(want) {
                assert!(
                    (g - w).abs() < 0.06,
                    "frame {frame}: matrix {got:?} vs {want:?}"
                );
            }
        }
    }
}

#[test]
fn shared_shapes_are_defined_once_and_hidden_layers_are_not_published() {
    let doc = animated();
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let defined: Vec<u16> = parsed
        .tags
        .iter()
        .filter_map(|t| {
            if let Tag::DefineShape(s) = t {
                Some(s.id)
            } else {
                None
            }
        })
        .collect();
    // The red square (used by two instances over ten frames) and the disc: two definitions.
    assert_eq!(defined.len(), 2, "{defined:?}");
    let mut unique = defined.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), defined.len(), "character ids are unique");
}

#[test]
fn rendering_the_decoded_swf_matches_rendering_the_document() {
    let doc = animated();
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let background = doc.stage().background();
    for frame in 0..10 {
        let (max, mean) = difference(
            &render_original(&doc, 0, frame),
            &render_swf_frame(&parsed, frame as usize, background),
        );
        // The disc is made of curves, so allow a little edge difference; it must stay tiny.
        assert!(
            max <= 60 && mean < 0.15,
            "frame {frame}: max {max}, mean {mean}"
        );
    }
    // A frame with only straight edges and whole-pixel positions would be exact; the tween
    // frames move the red square by whole pixels here, so check one precisely: the square.
    let img = render_swf_frame(&parsed, 0, background);
    assert_eq!(img.pixel(15, 25), Some(Rgba8::opaque(255, 0, 0)));
    assert_eq!(img.pixel(150, 90), Some(background));
}

#[test]
fn holes_stay_holes_whichever_way_the_contours_are_wound() {
    for flip in [false, true] {
        let doc = document(
            80,
            60,
            vec![one_frame_scene("S", vec![ring(flip)], 1)],
            vec![],
        );
        let parsed = parse(
            &publish_swf(&doc, &PublishOptions::default())
                .expect("publish")
                .bytes,
        );
        let image = render_swf_frame(&parsed, 0, doc.stage().background());
        assert_eq!(
            image.pixel(14, 30),
            Some(Rgba8::opaque(255, 128, 0)),
            "flip={flip}: the band is filled"
        );
        assert_eq!(
            image.pixel(30, 30),
            Some(doc.stage().background()),
            "flip={flip}: the middle is a hole"
        );
        let (max, mean) = difference(&render_original(&doc, 0, 0), &image);
        assert_eq!(
            (max, mean),
            (0, 0.0),
            "flip={flip}: identical to the original"
        );
    }
}

#[test]
fn stroked_closed_shapes_keep_all_four_sides_and_open_filled_shapes_are_closed() {
    let boxed = rect(10.0, 10.0, 40.0, 30.0, Rgba8::opaque(250, 250, 0))
        .with_stroke(Stroke::new(4.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    // An open triangle with a fill but no Close: the fill must still be a solid triangle.
    let open = Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(pt(60.0, 10.0)),
            PathSegment::LineTo(pt(90.0, 10.0)),
            PathSegment::LineTo(pt(90.0, 40.0)),
        ],
        Rgba8::opaque(0, 200, 200),
    )
    .expect("open");
    let doc = document(
        100,
        60,
        vec![one_frame_scene("S", vec![boxed, open], 1)],
        vec![],
    );
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let image = render_swf_frame(&parsed, 0, doc.stage().background());
    assert_eq!(
        image.pixel(10, 25),
        Some(Rgba8::opaque(0, 0, 0)),
        "left side (the closing edge) is stroked"
    );
    assert_eq!(
        image.pixel(30, 10),
        Some(Rgba8::opaque(0, 0, 0)),
        "top side"
    );
    assert_eq!(
        image.pixel(30, 25),
        Some(Rgba8::opaque(250, 250, 0)),
        "the inside"
    );
    let (max, mean) = difference(&render_original(&doc, 0, 0), &image);
    assert_eq!((max, mean), (0, 0.0));
}

#[test]
fn running_out_of_shape_ids_is_an_error_not_a_corrupt_file() {
    // Ids run 1..=65 534; one more shape than that cannot be numbered.
    let result = publish_swf(
        &document_of_distinct_shapes(65_535),
        &PublishOptions::default(),
    );
    assert!(
        matches!(result, Err(PublishError::TooManyShapes)),
        "{:?}",
        result.map(|p| p.frames)
    );
    let at_the_limit = publish_swf(
        &document_of_distinct_shapes(65_534),
        &PublishOptions::default(),
    );
    assert!(at_the_limit.is_ok(), "exactly the limit still publishes");
}

#[test]
fn a_gradient_is_written_with_its_colors_and_a_matrix_that_puts_them_at_its_ends() {
    let gradient = flare_document::Gradient::linear(
        pt(10.0, 20.0),
        pt(10.0, 120.0),
        Rgba8::opaque(200, 0, 0),
        Rgba8::new(0, 100, 0, 128),
    )
    .expect("gradient");
    let shape = rect(0.0, 0.0, 20.0, 140.0, Rgba8::TRANSPARENT).with_gradient(gradient);
    let doc = document(20, 140, vec![one_frame_scene("A", vec![shape], 1)], vec![]);
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    assert!(
        published
            .diagnostics
            .iter()
            .all(|d| !d.message.contains("gradient")),
        "{:?}",
        published.diagnostics
    );
    let parsed = parse(&published.bytes);
    let fills: Vec<&swf::FillStyle> = parsed
        .tags
        .iter()
        .filter_map(|t| match t {
            Tag::DefineShape(s) => Some(&s.styles.fill_styles),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(fills.len(), 1);
    let swf::FillStyle::LinearGradient(g) = fills[0] else {
        panic!("not a linear gradient: {:?}", fills[0]);
    };
    assert_eq!(g.records.len(), 2);
    assert_eq!((g.records[0].ratio, g.records[1].ratio), (0, 255));
    assert_eq!(
        (
            g.records[0].color.r,
            g.records[0].color.a,
            g.records[1].color.g,
            g.records[1].color.a
        ),
        (200, 255, 100, 128)
    );
    assert_eq!(g.spread, swf::GradientSpread::Pad);
    // The gradient's x axis (-16384..16384 twips) must land on the line, in pixels.
    let at = |x: f64| {
        let m = &g.matrix;
        (
            (m.a.to_f64() * x + m.tx.get() as f64) / 20.0,
            (m.b.to_f64() * x + m.ty.get() as f64) / 20.0,
        )
    };
    let (start, end) = (at(-16384.0), at(16384.0));
    assert!(
        (start.0 - 10.0).abs() < 0.1 && (start.1 - 20.0).abs() < 0.1,
        "{start:?}"
    );
    assert!(
        (end.0 - 10.0).abs() < 0.1 && (end.1 - 120.0).abs() < 0.1,
        "{end:?}"
    );
}

#[test]
fn a_fully_transparent_gradient_draws_nothing() {
    let gradient = flare_document::Gradient::linear(
        pt(0.0, 0.0),
        pt(20.0, 0.0),
        Rgba8::TRANSPARENT,
        Rgba8::TRANSPARENT,
    )
    .expect("gradient");
    let shape = rect(0.0, 0.0, 20.0, 20.0, Rgba8::TRANSPARENT).with_gradient(gradient);
    let doc = document(20, 20, vec![one_frame_scene("A", vec![shape], 1)], vec![]);
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    assert!(!parsed.tags.iter().any(|t| matches!(t, Tag::DefineShape(_))));
}

/// Every PlaceObject of the first frame as (depth, clip depth).
fn first_frame_places(parsed: &Parsed) -> Vec<(u16, Option<u16>)> {
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

/// A scene whose top layer masks the one below with `mask` shapes.
fn masked(mask: Vec<Shape>, art: Vec<Shape>) -> Document {
    let spans = |shapes: Vec<Shape>| -> Vec<flare_document::FrameSpan> {
        shapes
            .into_iter()
            .map(|s| span(0, 1, Content::Shape(s)))
            .collect()
    };
    let mask_layer = layer("mask", spans(mask)).with_mask(true);
    let art_layers: Vec<_> = art
        .into_iter()
        .map(|s| layer("art", vec![span(0, 1, Content::Shape(s))]))
        .collect();
    let mut layers = vec![mask_layer];
    layers.extend(art_layers);
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(layers));
    document(20, 20, vec![scene], vec![])
}

#[test]
fn a_mask_is_a_clip_layer_under_the_shapes_it_masks() {
    let doc = masked(
        vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)],
        vec![rect(0.0, 0.0, 20.0, 20.0, Rgba8::opaque(255, 0, 0))],
    );
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    assert!(
        published
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Warning),
        "{:?}",
        published.diagnostics
    );
    // The mask at depth 1 clips depth 2, the masked shape.
    assert_eq!(
        first_frame_places(&parse(&published.bytes)),
        vec![(1, Some(2)), (2, None)]
    );
}

#[test]
fn a_mask_with_nothing_to_mask_or_nothing_drawn_writes_no_clip_layer() {
    let nothing = masked(
        vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)],
        vec![rect(0.0, 0.0, 20.0, 20.0, Rgba8::TRANSPARENT)],
    );
    let parsed = parse(
        &publish_swf(&nothing, &PublishOptions::default())
            .expect("p")
            .bytes,
    );
    assert!(first_frame_places(&parsed).is_empty());
    // A mask that draws nothing hides what is under it.
    let empty_mask = masked(
        vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::TRANSPARENT)],
        vec![rect(0.0, 0.0, 20.0, 20.0, Rgba8::opaque(255, 0, 0))],
    );
    let parsed = parse(
        &publish_swf(&empty_mask, &PublishOptions::default())
            .expect("p")
            .bytes,
    );
    assert!(first_frame_places(&parsed).is_empty());
}

#[test]
fn a_mask_of_several_shapes_is_reported_and_leaves_the_layer_unmasked() {
    // A symbol of two shapes makes a mask whose region is the union of both.
    let pair = flare_core::SymbolId::new();
    let shape_layer = |x: f64| {
        layer(
            "L",
            vec![span(
                0,
                1,
                Content::Shape(rect(x, 0.0, 5.0, 5.0, Rgba8::WHITE)),
            )],
        )
    };
    let symbol = flare_document::Symbol::new(
        pair,
        "Pair",
        Timeline::new(vec![shape_layer(0.0), shape_layer(10.0)]),
    );
    let mask = layer("mask", vec![span(0, 1, instance(pair, 0.0, 0.0))]).with_mask(true);
    let art = layer(
        "art",
        vec![span(
            0,
            1,
            Content::Shape(rect(0.0, 0.0, 20.0, 20.0, Rgba8::opaque(255, 0, 0))),
        )],
    );
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![mask, art]));
    let doc = document(20, 20, vec![scene], vec![symbol]);
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    assert!(
        published
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("masks")),
        "{:?}",
        published.diagnostics
    );
    // The art is the lower of two layers, so it is at depth 2; nothing clips it.
    assert_eq!(
        first_frame_places(&parse(&published.bytes)),
        vec![(2, None)]
    );
}

#[test]
fn scenes_play_in_sequence_with_a_warning() {
    let a = one_frame_scene(
        "A",
        vec![rect(0.0, 0.0, 10.0, 10.0, Rgba8::opaque(255, 0, 0))],
        3,
    );
    let b = one_frame_scene(
        "B",
        vec![rect(20.0, 0.0, 10.0, 10.0, Rgba8::opaque(0, 0, 255))],
        2,
    );
    let doc = document(40, 20, vec![a, b], vec![]);
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    assert_eq!(published.frames, 5);
    let parsed = parse(&published.bytes);
    assert_eq!(play(&parsed).len(), 5);
    let background = doc.stage().background();
    assert_eq!(
        render_swf_frame(&parsed, 1, background).pixel(5, 5),
        Some(Rgba8::opaque(255, 0, 0))
    );
    assert_eq!(
        render_swf_frame(&parsed, 3, background).pixel(25, 5),
        Some(Rgba8::opaque(0, 0, 255))
    );
    assert_eq!(
        render_swf_frame(&parsed, 3, background).pixel(5, 5),
        Some(background),
        "scene A is gone"
    );
    assert!(
        published
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("one after another"))
    );
}

#[test]
fn empty_documents_publish_one_blank_frame_and_extremes_are_refused() {
    let layer_only = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer("L", vec![])]));
    let blank = document(20, 20, vec![layer_only], vec![]);
    let published = publish_swf(&blank, &PublishOptions::default()).expect("publish");
    assert_eq!(published.frames, 1);
    assert_eq!(parse(&published.bytes).declared_frames, 1);

    let long = one_frame_scene("S", vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)], 70_000);
    assert!(matches!(
        publish_swf(
            &document(10, 10, vec![long], vec![]),
            &PublishOptions::default()
        ),
        Err(PublishError::TooManyFrames(_))
    ));
    let huge = one_frame_scene("S", vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)], 1);
    assert!(matches!(
        publish_swf(
            &document(9_000, 100, vec![huge], vec![]),
            &PublishOptions::default()
        ),
        Err(PublishError::StageTooLarge)
    ));
}

#[test]
fn compressed_and_plain_files_agree_and_publishing_is_deterministic() {
    let doc = animated();
    let plain = publish_swf(&doc, &PublishOptions { compress: false }).expect("plain");
    let packed = publish_swf(&doc, &PublishOptions { compress: true }).expect("packed");
    assert_eq!(&plain.bytes[..3], b"FWS");
    assert_eq!(&packed.bytes[..3], b"CWS");
    assert!(packed.bytes.len() < plain.bytes.len());
    assert_eq!(
        parse(&plain.bytes).tags.len(),
        parse(&packed.bytes).tags.len()
    );
    let again = publish_swf(&doc, &PublishOptions { compress: true }).expect("again");
    assert_eq!(again.bytes, packed.bytes, "same document, same bytes");
}

#[test]
fn lossy_choices_are_reported() {
    let clear_bg = Stage::new(10, 10, Rgba8::new(1, 2, 3, 100)).expect("stage");
    let fast = FrameRate::new(1000, 1).expect("rate");
    let scene = one_frame_scene("S", vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)], 1);
    let doc = Document::new(DocumentId::new(), fast, vec![scene], vec![])
        .expect("doc")
        .with_stage(clear_bg);
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    let text: String = published
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("transparency"), "{text}");
    assert!(text.contains("capped"), "{text}");
    assert!(text.contains("baked"), "{text}");
    let parsed = parse(&published.bytes);
    assert!(parsed.frame_rate <= 255.99, "{}", parsed.frame_rate);
}
