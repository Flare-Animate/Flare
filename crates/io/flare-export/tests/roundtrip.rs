//! Export a document, import it again with `flare-import`, and compare. This shows the two
//! sides agree with each other, and quantifies what the format cannot hold (curve
//! approximation, gaps). It does not show that Adobe Animate accepts the output.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Stage, Stroke,
    Symbol, Timeline, Tween,
};
use flare_export::{Output, Severity, export_xfl, write_directory, write_fla};
use flare_import::{FileSource, import_path, import_xfl};
use glamx::{DAffine2, DVec2};

struct Mem(Vec<(String, String)>);

impl FileSource for Mem {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.clone().into_bytes()))
    }
}

fn roundtrip(document: &Document) -> (Output, flare_import::Imported) {
    let output = export_xfl(document).expect("export");
    let imported = import_xfl(&Mem(output.files.clone())).expect("the export must re-import");
    (output, imported)
}

fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
}

fn rect(x: f64, y: f64, w: f64, h: f64, fill: Rgba8) -> Shape {
    Shape::rect(ShapeId::new(), x, y, w, h, fill).expect("rect")
}

fn instance(symbol: SymbolId, x: f64, y: f64) -> Content {
    let transform = Transform2D::new(DAffine2::from_translation(DVec2::new(x, y))).expect("finite");
    Content::Instance(Instance::new(InstanceId::new(), symbol, transform))
}

fn layer(name: &str, spans: Vec<FrameSpan>) -> Layer {
    Layer::new(LayerId::new(), name, spans).expect("layer")
}

fn one_layer(shape: Shape) -> Timeline {
    Timeline::new(vec![layer("L", vec![span(0, 1, Content::Shape(shape))])])
}

fn pt(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

const RED: Rgba8 = Rgba8::opaque(255, 0, 0);

fn rich() -> Document {
    let (dot, odd) = (SymbolId::new(), SymbolId::new());
    let ellipse = Shape::ellipse(
        ShapeId::new(),
        10.0,
        10.0,
        8.0,
        6.0,
        Rgba8::opaque(0, 0, 255),
    )
    .expect("ellipse");
    let symbols = vec![
        Symbol::new(dot, "Dot", one_layer(rect(0.0, 0.0, 10.0, 10.0, RED))),
        Symbol::new(odd, "A & \"B\" <c>", one_layer(ellipse)),
    ];
    let tweened = span(0, 5, instance(dot, 10.0, 20.0)).with_tween(Tween::Linear);
    let hero = layer(
        "Hero",
        vec![tweened, span(5, 5, instance(dot, 110.0, 20.0))],
    )
    .with_locked(true);
    let disc = Shape::ellipse(
        ShapeId::new(),
        100.0,
        60.0,
        20.0,
        20.0,
        Rgba8::opaque(0, 255, 0),
    )
    .expect("ellipse")
    .with_stroke(Stroke::new(3.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    let hidden = layer("Hidden", vec![span(0, 10, Content::Shape(disc))]).with_visible(false);
    let gappy = layer(
        "Gappy",
        vec![
            span(0, 2, Content::Empty),
            span(5, 3, instance(odd, 30.0, 50.0)),
        ],
    );
    let scene_one = Scene::new(
        SceneId::new(),
        "Scene 1",
        Timeline::new(vec![hero, hidden, gappy, layer("Blank", vec![])]),
    );

    let stroke = Stroke::new(2.0, Rgba8::opaque(10, 20, 30)).expect("stroke");
    let line = Shape::line(ShapeId::new(), (5.0, 5.0), (60.0, 40.0), stroke).expect("line");
    let ring = Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(pt(100.0, 10.0)),
            PathSegment::LineTo(pt(140.0, 10.0)),
            PathSegment::LineTo(pt(140.0, 50.0)),
            PathSegment::LineTo(pt(100.0, 50.0)),
            PathSegment::Close,
            PathSegment::MoveTo(pt(110.0, 20.0)),
            PathSegment::LineTo(pt(110.0, 40.0)),
            PathSegment::LineTo(pt(130.0, 40.0)),
            PathSegment::LineTo(pt(130.0, 20.0)),
            PathSegment::Close,
        ],
        Rgba8::opaque(255, 128, 0),
    )
    .expect("ring");
    let scene_two = Scene::new(
        SceneId::new(),
        "Second",
        Timeline::new(vec![
            layer("Lines", vec![span(0, 3, Content::Shape(line))]),
            layer("Ring", vec![span(0, 3, Content::Shape(ring))]),
        ]),
    );
    let rate = FrameRate::new(12, 1).expect("rate");
    let stage = Stage::new(200, 100, Rgba8::opaque(16, 32, 48)).expect("stage");
    Document::new(DocumentId::new(), rate, vec![scene_one, scene_two], symbols)
        .expect("doc")
        .with_stage(stage)
}

fn render(document: &Document, scene: usize, frame: u32) -> flare_render::Image {
    let id = document.scenes()[scene].id();
    let evaluated = flare_eval::evaluate_frame(document, id, frame).expect("eval");
    let stage = document.stage();
    flare_render::render(
        &evaluated,
        stage.width(),
        stage.height(),
        stage.background(),
    )
    .expect("render")
}

/// (largest channel difference, mean channel difference) between two renders.
fn difference(a: &flare_render::Image, b: &flare_render::Image) -> (u8, f64) {
    let (mut max, mut sum) = (0_u8, 0_u64);
    for (x, y) in a.rgba().iter().zip(b.rgba()) {
        let d = x.abs_diff(*y);
        max = max.max(d);
        sum += u64::from(d);
    }
    (max, sum as f64 / a.rgba().len() as f64)
}

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

#[test]
fn a_translucent_background_and_invisible_shapes_are_reported() {
    let clear = Shape::rect(ShapeId::new(), 0.0, 0.0, 4.0, 4.0, Rgba8::TRANSPARENT).expect("rect");
    let scene = Scene::new(SceneId::new(), "S", one_layer(clear));
    let rate = FrameRate::new(24, 1).expect("rate");
    let stage = Stage::new(10, 10, Rgba8::new(1, 2, 3, 100)).expect("stage");
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("doc")
        .with_stage(stage);
    let out = export_xfl(&doc).expect("export");
    let all: String = out
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(all.contains("transparency"), "{all}");
    assert!(all.contains("invisible shape"), "{all}");
    assert!(import_xfl(&Mem(out.files)).is_ok());
}

#[test]
fn folders_and_zips_import_and_never_overwrite() {
    let dir = tempfile::tempdir().expect("dir");
    let output = export_xfl(&rich()).expect("export");

    let folder = dir.path().join("out").join("proj");
    write_directory(&output, &folder).expect("write");
    assert!(folder.join("DOMDocument.xml").is_file() && folder.join("LIBRARY").is_dir());
    let stub = std::fs::read_dir(&folder)
        .expect("ls")
        .filter_map(Result::ok)
        .any(|e| e.path().extension().is_some_and(|x| x == "xfl"));
    assert!(stub, "the .xfl stub marks the folder as an XFL document");
    let from_folder = import_path(&folder).expect("import folder");
    assert_eq!(from_folder.document.scenes().len(), 2);
    assert!(matches!(
        write_directory(&output, &folder),
        Err(flare_export::ExportError::AlreadyExists(_))
    ));

    let fla = dir.path().join("proj.fla");
    write_fla(&output, &fla).expect("fla");
    let from_zip = import_path(&fla).expect("import fla");
    assert_eq!(
        from_zip.document.symbols().len(),
        from_folder.document.symbols().len()
    );
    assert!(matches!(
        write_fla(&output, &fla),
        Err(flare_export::ExportError::AlreadyExists(_))
    ));

    // No staging files are left behind next to the results.
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("ls")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_failed_write_leaves_nothing_behind() {
    let dir = tempfile::tempdir().expect("dir");
    let output = export_xfl(&rich()).expect("export");
    // The parent "directory" is a file, so creating the folder must fail.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "x").expect("write");
    assert!(write_directory(&output, &blocker.join("proj")).is_err());
    assert!(write_fla(&output, &blocker.join("proj.fla")).is_err());
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("ls")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(entries.len(), 1, "only the blocker file exists");
}

#[test]
fn closed_stroked_shapes_keep_their_closing_edge() {
    // The outline's fourth side only exists as the implicit "close"; if it were dropped, the
    // left edge of this rectangle would have no stroke.
    let boxed = rect(10.0, 10.0, 40.0, 30.0, Rgba8::opaque(250, 250, 0))
        .with_stroke(Stroke::new(4.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    let scene = Scene::new(SceneId::new(), "S", one_layer(boxed));
    let rate = FrameRate::new(24, 1).expect("rate");
    let stage = Stage::new(60, 50, Rgba8::WHITE).expect("stage");
    let original = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("doc")
        .with_stage(stage);
    let (_, back) = roundtrip(&original);
    let (max, mean) = difference(&render(&original, 0, 0), &render(&back.document, 0, 0));
    assert_eq!((max, mean), (0, 0.0));
    let image = render(&back.document, 0, 0);
    assert_eq!(
        image.pixel(10, 25),
        Some(Rgba8::opaque(0, 0, 0)),
        "the closing (left) side is stroked"
    );
    assert_eq!(image.pixel(30, 25), Some(Rgba8::opaque(250, 250, 0)));
}
