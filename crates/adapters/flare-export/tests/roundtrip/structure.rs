use super::*;

#[test]
fn structure_survives_a_round_trip() {
    let original = rich();
    let (_, back) = roundtrip(&original);
    let doc = &back.document;
    let stage = doc.stage();
    assert_eq!(
        (stage.width(), stage.height(), stage.background()),
        (200, 100, Rgba8::opaque(16, 32, 48))
    );
    assert_eq!(
        (doc.frame_rate().numerator(), doc.frame_rate().denominator()),
        (12, 1)
    );
    let scenes: Vec<&str> = doc.scenes().iter().map(Scene::name).collect();
    assert_eq!(scenes, vec!["Scene 1", "Second"]);
    let mut symbols: Vec<&str> = doc.symbols().iter().map(|s| s.name()).collect();
    symbols.sort_unstable();
    assert_eq!(
        symbols,
        vec!["A & \"B\" <c>", "Dot"],
        "names with XML specials survive"
    );

    let layers = doc.scenes()[0].timeline().layers();
    // Hero keeps flags, durations and the tween.
    assert_eq!(layers[0].name(), "Hero");
    assert!(layers[0].is_locked() && layers[0].is_visible());
    let hero: Vec<_> = layers[0]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.tween()))
        .collect();
    assert_eq!(hero, vec![(0, 5, Tween::Linear), (5, 5, Tween::None)]);
    // A shape with fill and stroke becomes two elements, so two layers with the stroke on top.
    let names: Vec<&str> = layers.iter().map(|l| l.name()).collect();
    assert_eq!(
        names,
        vec!["Hero", "Hidden (2)", "Hidden", "Gappy", "Blank"]
    );
    assert!(
        !layers[1].is_visible() && !layers[2].is_visible(),
        "visibility is kept on both halves"
    );
    // The gap became explicit empty frames; the empty layer got its single empty frame.
    let gappy: Vec<_> = layers[3]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get()))
        .collect();
    assert_eq!(gappy, vec![(0, 2), (2, 3), (5, 3)]);
    assert!(matches!(layers[3].spans()[1].content(), Content::Empty));
    assert!(matches!(layers[4].spans()[0].content(), Content::Empty));
    assert_eq!(layers[4].spans().len(), 1);
}

#[test]
fn rendered_artwork_matches_exactly_for_straight_edges_and_closely_for_curves() {
    let original = rich();
    let (_, back) = roundtrip(&original);
    // Rectangles via instances, tweened motion, lines and a ring with a hole: exact.
    for (scene, frame) in [(0, 0), (0, 2), (0, 6), (1, 0), (1, 2)] {
        let (max, mean) = difference(
            &render(&original, scene, frame),
            &render(&back.document, scene, frame),
        );
        // The blue ellipse symbol is only on screen from frame 5 of scene 1.
        if scene == 0 && frame >= 5 {
            assert!(
                max <= 40 && mean < 0.1,
                "curve approximation too coarse: max {max}, mean {mean}"
            );
        } else {
            assert_eq!(
                (max, mean),
                (0, 0.0),
                "scene {scene} frame {frame} should render identically"
            );
        }
    }
    // The ring really has a hole after the trip.
    let ring = render(&back.document, 1, 0);
    assert_eq!(
        ring.pixel(102, 30),
        Some(Rgba8::opaque(255, 128, 0)),
        "the band is filled"
    );
    assert_eq!(
        ring.pixel(120, 30),
        Some(Rgba8::opaque(16, 32, 48)),
        "the middle is a hole"
    );
}

#[test]
fn the_report_says_what_changed() {
    let (output, _) = roundtrip(&rich());
    let text = |severity| {
        output
            .diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .map(|d| format!("{}: {}", d.location, d.message))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let warnings = text(Severity::Warning);
    assert!(warnings.contains("gap of 3 frame(s)"), "{warnings}");
    let info = text(Severity::Info);
    assert!(info.contains("quadratic"), "{info}");
    assert!(info.contains("empty layer"), "{info}");

    // A document of straight edges needs no approximation note and has no warnings.
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene = Scene::new(
        SceneId::new(),
        "S",
        one_layer(rect(0.0, 0.0, 5.0, 5.0, RED)),
    );
    let plain = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
    let out = export_xfl(&plain).expect("export");
    assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
}

#[test]
fn a_gradient_is_written_as_its_average_color_with_a_warning() {
    let gradient = flare_document::Gradient::linear(
        pt(0.0, 0.0),
        pt(10.0, 0.0),
        Rgba8::opaque(200, 0, 0),
        Rgba8::opaque(0, 100, 0),
    )
    .expect("gradient");
    let shape = rect(0.0, 0.0, 10.0, 10.0, Rgba8::TRANSPARENT).with_gradient(gradient);
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene = Scene::new(SceneId::new(), "S", one_layer(shape));
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
    let (output, imported) = roundtrip(&doc);
    assert!(
        output
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("gradient")),
        "{:?}",
        output.diagnostics
    );
    let Content::Shape(back) = imported.document.scenes()[0].timeline().layers()[0].spans()[0]
        .content()
        .clone()
    else {
        panic!("a shape expected")
    };
    assert_eq!(
        back.fill(),
        Rgba8::opaque(100, 50, 0),
        "the average of the two stops"
    );
    assert!(back.gradient().is_none());
}

#[test]
fn mask_layers_are_exported_as_xfl_masks_that_point_at_the_layer_they_mask() {
    let mask = layer(
        "Mask",
        vec![span(0, 1, Content::Shape(rect(0.0, 0.0, 5.0, 5.0, RED)))],
    )
    .with_mask(true);
    let art = layer(
        "Art",
        vec![span(0, 1, Content::Shape(rect(0.0, 0.0, 9.0, 9.0, RED)))],
    );
    let free = layer(
        "Free",
        vec![span(0, 1, Content::Shape(rect(2.0, 2.0, 3.0, 3.0, RED)))],
    );
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![free, mask, art]));
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
    let (output, imported) = roundtrip(&doc);
    let xml = &output.files[0].1;
    // Layers are written top to bottom: Free (0), Mask (1), Art (2).
    assert!(xml.contains("<DOMLayer name=\"Free\"><frames>"), "{xml}");
    assert!(
        xml.contains("<DOMLayer name=\"Mask\" layerType=\"mask\" locked=\"true\">"),
        "{xml}"
    );
    assert!(
        xml.contains("<DOMLayer name=\"Art\" parentLayerIndex=\"1\">"),
        "the masked layer points at the mask's index: {xml}"
    );
    // The export still imports; the mask layer comes back as preserved source data.
    assert!(imported.document.scenes()[0].timeline().layers().len() >= 2);
}

#[test]
fn sounds_are_reported_as_not_exported() {
    let sound = flare_core::SoundId::new();
    let layer = layer("Sound", vec![])
        .with_cues(vec![flare_document::SoundCue { start: 0, sound }])
        .expect("cues");
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("doc")
        .with_sounds(vec![
            flare_document::Sound::new(sound, "Clap", vec![1], 8000, 1, 1).expect("sound"),
        ])
        .expect("sounds");
    let (output, _) = roundtrip(&doc);
    assert!(
        output
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("sound")),
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn awkward_names_stay_safe_and_unique() {
    let symbols: Vec<Symbol> = [
        "a",
        "A",
        "../x",
        "",
        "a/b",
        "a_b",
        &"long".repeat(100),
        "dup",
        "dup",
    ]
    .iter()
    .map(|name| {
        Symbol::new(
            SymbolId::new(),
            *name,
            one_layer(rect(0.0, 0.0, 1.0, 1.0, RED)),
        )
    })
    .collect();
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer("L", vec![])]));
    let doc = Document::new(DocumentId::new(), rate, vec![scene], symbols).expect("doc");
    let output = export_xfl(&doc).expect("export");

    let mut lower = std::collections::HashSet::new();
    for (name, _) in &output.files {
        assert!(
            !name.contains("..") && !name.starts_with('/') && !name.contains('\\'),
            "{name}"
        );
        assert!(
            name == "DOMDocument.xml" || name.starts_with("LIBRARY/"),
            "{name}"
        );
        assert!(name.len() < 120, "file names are bounded");
        assert!(
            lower.insert(name.to_lowercase()),
            "case-insensitive duplicate: {name}"
        );
    }
    assert_eq!(output.files.len(), 1 + 9);
    let warnings: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .collect();
    assert!(
        warnings.iter().any(|d| d.message.contains("renamed")),
        "duplicate library names are reported"
    );
    // And it all re-imports with every symbol present.
    let back = import_xfl(&Mem(output.files.clone())).expect("import");
    assert_eq!(back.document.symbols().len(), 9);
}

#[test]
fn frame_rates_round_trip_as_exact_rationals() {
    for (n, d) in [(24, 1), (30_000, 1001), (25, 2), (60_000, 1001), (1, 1)] {
        let rate = FrameRate::new(n, d).expect("rate");
        let scene = Scene::new(
            SceneId::new(),
            "S",
            one_layer(rect(0.0, 0.0, 1.0, 1.0, RED)),
        );
        let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
        let (_, back) = roundtrip(&doc);
        assert_eq!(back.document.frame_rate(), rate, "{n}/{d}");
    }
}
