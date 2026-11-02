use super::*;

#[test]
fn filled_rect_is_drawn_over_background() {
    let (doc, id) = scene_with(vec![span(rect(5.0, 5.0, 10.0, 10.0, RED))], vec![]);
    let image = draw(&doc, id, Rgba8::WHITE);
    assert_eq!(image.pixel(10, 10), Some(RED));
    assert_eq!(image.pixel(1, 1), Some(Rgba8::WHITE));
    assert_eq!(image.pixel(20, 0), None);
}

#[test]
fn transparent_background_keeps_straight_alpha() {
    let half = Rgba8::new(200, 100, 50, 128);
    let (doc, id) = scene_with(vec![span(rect(0.0, 0.0, 20.0, 20.0, half))], vec![]);
    let p = draw(&doc, id, Rgba8::TRANSPARENT)
        .pixel(10, 10)
        .expect("pixel");
    assert_eq!(p.a, 128);
    // Unpremultiplied channels stay within rounding of the source color.
    assert!(
        p.r.abs_diff(200) <= 2 && p.g.abs_diff(100) <= 2 && p.b.abs_diff(50) <= 2,
        "{p:?}"
    );
    let (empty, id) = scene_with(vec![span(Content::Empty)], vec![]);
    assert_eq!(
        draw(&empty, id, Rgba8::TRANSPARENT).pixel(3, 3),
        Some(Rgba8::TRANSPARENT)
    );
}

#[test]
fn instance_transforms_move_symbol_shapes() {
    let art = SymbolId::new();
    let art_tl = Timeline::new(vec![
        Layer::new(
            LayerId::new(),
            "L",
            vec![span(rect(0.0, 0.0, 4.0, 4.0, RED))],
        )
        .expect("l"),
    ]);
    let shift =
        Transform2D::new(DAffine2::from_translation(DVec2::new(10.0, 12.0))).expect("finite");
    let instance = Instance::new(InstanceId::new(), art, shift);
    let (doc, id) = scene_with(
        vec![span(Content::Instance(instance))],
        vec![Symbol::new(art, "Art", art_tl)],
    );
    let image = draw(&doc, id, Rgba8::WHITE);
    assert_eq!(image.pixel(11, 13), Some(RED));
    assert_eq!(image.pixel(1, 1), Some(Rgba8::WHITE));
}

#[test]
fn later_layers_in_paint_order_cover_earlier_ones() {
    let blue = Rgba8::opaque(0, 0, 255);
    // First layer is on top, so it paints last.
    let spans = vec![
        span(rect(0.0, 0.0, 20.0, 20.0, RED)),
        span(rect(0.0, 0.0, 20.0, 20.0, blue)),
    ];
    let (doc, id) = scene_with(spans, vec![]);
    assert_eq!(draw(&doc, id, Rgba8::WHITE).pixel(5, 5), Some(RED));
}

#[test]
fn strokes_are_drawn_centered_on_the_path_over_the_fill() {
    use flare_document::Stroke as DocStroke;
    let blue = Rgba8::opaque(0, 0, 255);
    let shape = Shape::rect(ShapeId::new(), 5.0, 5.0, 10.0, 10.0, RED)
        .expect("rect")
        .with_stroke(DocStroke::new(4.0, blue).expect("stroke"));
    let (doc, id) = scene_with(vec![span(Content::Shape(shape))], vec![]);
    let image = draw(&doc, id, Rgba8::WHITE);
    assert_eq!(image.pixel(10, 10), Some(RED), "interior keeps the fill");
    assert_eq!(
        image.pixel(5, 10),
        Some(blue),
        "on the edge: stroke covers the fill"
    );
    assert_eq!(
        image.pixel(4, 10),
        Some(blue),
        "stroke extends half its width outside"
    );
    assert_eq!(
        image.pixel(2, 10),
        Some(Rgba8::WHITE),
        "beyond the stroke is background"
    );
    // An outline-only shape: transparent fill leaves the interior untouched.
    let outline = Shape::rect(ShapeId::new(), 5.0, 5.0, 10.0, 10.0, Rgba8::TRANSPARENT)
        .expect("rect")
        .with_stroke(DocStroke::new(2.0, blue).expect("stroke"));
    let (doc, id) = scene_with(vec![span(Content::Shape(outline))], vec![]);
    let image = draw(&doc, id, Rgba8::WHITE);
    assert_eq!(image.pixel(10, 10), Some(Rgba8::WHITE));
    assert_eq!(image.pixel(5, 10), Some(blue));
}

#[test]
fn hit_testing_finds_the_topmost_shape_through_transforms() {
    let art = SymbolId::new();
    let art_tl = Timeline::new(vec![
        Layer::new(
            LayerId::new(),
            "L",
            vec![span(rect(0.0, 0.0, 10.0, 10.0, RED))],
        )
        .expect("l"),
    ]);
    let shift =
        Transform2D::new(DAffine2::from_translation(DVec2::new(50.0, 50.0))).expect("finite");
    let instance = Instance::new(InstanceId::new(), art, shift);
    let blue = Rgba8::opaque(0, 0, 255);
    // First layer is on top: the blue rectangle covers the left half of the stage.
    let spans = vec![
        span(rect(0.0, 0.0, 55.0, 100.0, blue)),
        span(Content::Instance(instance)),
    ];
    let (doc, id) = scene_with(spans, vec![Symbol::new(art, "Art", art_tl)]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(frame.shapes.len(), 2);
    // (52, 52) is inside both; the top layer wins (paint order: it is drawn last).
    assert_eq!(hit_test(&frame, DVec2::new(52.0, 52.0)), Some(1));
    // (58, 58) is only inside the instance's translated square.
    assert_eq!(hit_test(&frame, DVec2::new(58.0, 58.0)), Some(0));
    assert_eq!(hit_test(&frame, DVec2::new(90.0, 90.0)), None);
    // The instance's local origin square (0..10) was moved away; this point is in the blue rect.
    assert_eq!(hit_test(&frame, DVec2::new(5.0, 80.0)), Some(1));
}

#[test]
fn hit_test_all_lists_every_overlapping_shape_topmost_first() {
    let blue = Rgba8::opaque(0, 0, 255);
    let spans = vec![
        span(rect(0.0, 0.0, 50.0, 50.0, blue)),
        span(rect(0.0, 0.0, 100.0, 100.0, RED)),
    ];
    let (doc, id) = scene_with(spans, vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(hit_test_all(&frame, DVec2::new(10.0, 10.0)), vec![1, 0]);
    assert_eq!(hit_test_all(&frame, DVec2::new(80.0, 80.0)), vec![0]);
    assert!(hit_test_all(&frame, DVec2::new(200.0, 200.0)).is_empty());
}

#[test]
fn transparent_fills_and_singular_transforms_are_not_hittable() {
    let clear = Rgba8::new(1, 2, 3, 0);
    let (doc, id) = scene_with(vec![span(rect(0.0, 0.0, 10.0, 10.0, clear))], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(hit_test(&frame, DVec2::new(5.0, 5.0)), None);

    let art = SymbolId::new();
    let art_tl = Timeline::new(vec![
        Layer::new(
            LayerId::new(),
            "L",
            vec![span(rect(0.0, 0.0, 10.0, 10.0, RED))],
        )
        .expect("l"),
    ]);
    let flat = Transform2D::new(DAffine2::from_scale(DVec2::new(0.0, 1.0))).expect("finite");
    let instance = Instance::new(InstanceId::new(), art, flat);
    let (doc, id) = scene_with(
        vec![span(Content::Instance(instance))],
        vec![Symbol::new(art, "Art", art_tl)],
    );
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert_eq!(hit_test(&frame, DVec2::new(0.0, 5.0)), None);
}

#[test]
fn invalid_sizes_are_rejected() {
    let (doc, id) = scene_with(vec![], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    for (w, h) in [
        (0, 10),
        (10, 0),
        (MAX_DIMENSION + 1, 1),
        (MAX_DIMENSION, MAX_DIMENSION),
    ] {
        assert!(matches!(
            render(&frame, w, h, Rgba8::WHITE),
            Err(RenderError::InvalidSize(..))
        ));
    }
}

#[test]
fn png_round_trips_pixels() {
    let (doc, id) = scene_with(vec![span(rect(5.0, 5.0, 10.0, 10.0, RED))], vec![]);
    let image = draw(&doc, id, Rgba8::WHITE);
    let mut bytes = Vec::new();
    image.write_png(&mut bytes).expect("png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes.as_slice()))
        .read_info()
        .expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    assert_eq!((info.width, info.height), (20, 20));
    assert_eq!(&buf[..info.buffer_size()], image.rgba());
}

#[test]
fn fading_scales_alpha_only_and_composition_is_source_over() {
    let (doc, id) = scene_with(vec![span(rect(0.0, 0.0, 20.0, 20.0, RED))], vec![]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    let red = render(&frame, 20, 20, Rgba8::TRANSPARENT).expect("render");
    let half = red.faded(0.5);
    assert_eq!(half.pixel(5, 5), Some(Rgba8::new(255, 0, 0, 128)));
    assert_eq!(red.faded(2.0), red, "opacity clamps to 1");
    assert_eq!(red.faded(-1.0).pixel(5, 5).map(|p| p.a), Some(0));

    // 50% red over opaque white: halfway between white and red.
    let mut canvas = render(
        &flare_eval::Frame {
            placed: vec![],
            shapes: vec![],
            clips: vec![],
        },
        20,
        20,
        Rgba8::WHITE,
    )
    .expect("bg");
    canvas.composite(&half).expect("composite");
    let p = canvas.pixel(5, 5).expect("pixel");
    assert_eq!((p.r, p.a), (255, 255));
    assert!(p.g.abs_diff(127) <= 1 && p.b.abs_diff(127) <= 1, "{p:?}");

    // Over a transparent canvas the ghost keeps its own color and alpha.
    let mut clear = render(
        &flare_eval::Frame {
            placed: vec![],
            shapes: vec![],
            clips: vec![],
        },
        20,
        20,
        Rgba8::TRANSPARENT,
    )
    .expect("clear");
    clear.composite(&half).expect("composite");
    assert_eq!(clear.pixel(5, 5), Some(Rgba8::new(255, 0, 0, 128)));
    // Transparent pixels change nothing, and mismatched sizes are refused.
    let before = canvas.clone();
    canvas.composite(&red.faded(0.0)).expect("composite");
    assert_eq!(canvas, before);
    let small = render(
        &flare_eval::Frame {
            placed: vec![],
            shapes: vec![],
            clips: vec![],
        },
        4,
        4,
        Rgba8::WHITE,
    )
    .expect("small");
    assert!(canvas.composite(&small).is_err());
}

#[test]
fn onion_frames_sit_beneath_the_current_frame() {
    let blue = Rgba8::opaque(0, 0, 255);
    // Frame 0 has a red square on the left; frame 1 has a blue square overlapping it.
    let layer = |content: (u32, Content)| {
        Layer::new(
            LayerId::new(),
            "L",
            vec![FrameSpan::new(content.0, NonZeroU32::MIN, content.1)],
        )
        .expect("layer")
    };
    let timeline = Timeline::new(vec![
        layer((1, rect(5.0, 0.0, 10.0, 10.0, blue))),
        layer((0, rect(0.0, 0.0, 10.0, 10.0, RED))),
    ]);
    let id = SceneId::new();
    let scene = Scene::new(id, "S", timeline);
    let rate = FrameRate::new(24, 1).expect("rate");
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
    let ghost = flare_eval::evaluate_frame(&doc, id, 0).expect("ghost");
    let current = flare_eval::evaluate_frame(&doc, id, 1).expect("current");

    let image = render_onion(&[(&ghost, 0.5)], &current, 20, 20, Rgba8::WHITE).expect("onion");
    // Ghost only (x 0..5): faint red. Overlap (x 5..10): the current frame's solid blue.
    let faint = image.pixel(2, 5).expect("pixel");
    assert!(faint.r == 255 && faint.g.abs_diff(127) <= 1, "{faint:?}");
    assert_eq!(
        image.pixel(7, 5),
        Some(blue),
        "the current frame covers the ghost"
    );
    assert_eq!(image.pixel(12, 5), Some(blue));
    assert_eq!(image.pixel(18, 5), Some(Rgba8::WHITE));

    // Without ghosts it is exactly the normal render.
    let plain = render(&current, 20, 20, Rgba8::WHITE).expect("plain");
    assert_eq!(
        render_onion(&[], &current, 20, 20, Rgba8::WHITE).expect("onion"),
        plain
    );
}

#[test]
fn onion_outlines_draw_the_ghost_as_a_line_and_leave_its_inside_empty() {
    let blue = Rgba8::opaque(0, 0, 255);
    let layer = |content: (u32, Content)| {
        Layer::new(
            LayerId::new(),
            "L",
            vec![FrameSpan::new(content.0, NonZeroU32::MIN, content.1)],
        )
        .expect("layer")
    };
    // Frame 0: a big red square. Frame 1: a small blue one far from it.
    let timeline = Timeline::new(vec![
        layer((1, rect(50.0, 50.0, 10.0, 10.0, blue))),
        layer((0, rect(5.0, 5.0, 30.0, 30.0, RED))),
    ]);
    let id = SceneId::new();
    let scene = Scene::new(id, "S", timeline);
    let rate = FrameRate::new(24, 1).expect("rate");
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
    let ghost = flare_eval::evaluate_frame(&doc, id, 0).expect("ghost");
    let current = flare_eval::evaluate_frame(&doc, id, 1).expect("current");
    let image = render_onion_outlines_viewport(
        &[(&ghost, 0.5)],
        &current,
        &Viewport::full(80, 80),
        80,
        80,
        Rgba8::WHITE,
        1.5,
    )
    .expect("onion");
    assert_eq!(
        image.pixel(20, 20),
        Some(Rgba8::WHITE),
        "the inside of the ghost is not painted"
    );
    let edge = image.pixel(5, 20).expect("pixel");
    assert!(
        edge != Rgba8::WHITE && edge.b > edge.r,
        "the outline is a bluish line: {edge:?}"
    );
    assert_eq!(
        image.pixel(55, 55),
        Some(blue),
        "the current frame is drawn solid"
    );
}

#[test]
fn line_ends_follow_the_stroke_cap() {
    let line = |cap: flare_document::Cap| {
        let stroke = flare_document::Stroke::new(10.0, RED)
            .expect("stroke")
            .with_cap(cap);
        let shape = Shape::line(ShapeId::new(), (20.0, 20.0), (60.0, 20.0), stroke).expect("line");
        let (doc, scene) = scene_with(vec![span(Content::Shape(shape))], vec![]);
        draw_sized(&doc, scene, Rgba8::WHITE, 80, 40)
    };
    // The line starts at x = 20 and is 10 wide, so a square end reaches back to x = 15 and a
    // round one to x = 15 only along its middle; a butt end stops at 20.
    let butt = line(flare_document::Cap::Butt);
    let square = line(flare_document::Cap::Square);
    let round = line(flare_document::Cap::Round);
    assert_eq!(
        butt.pixel(17, 20),
        Some(Rgba8::WHITE),
        "a butt end stops at the point"
    );
    assert_eq!(
        square.pixel(17, 20),
        Some(RED),
        "a square end goes half a width further"
    );
    assert_eq!(
        round.pixel(17, 20),
        Some(RED),
        "so does the middle of a round end"
    );
    assert_eq!(
        round.pixel(15, 15),
        Some(Rgba8::WHITE),
        "but the corner of a round end is cut"
    );
    assert_eq!(
        square.pixel(15, 15),
        Some(RED),
        "and the corner of a square end is not"
    );
}
