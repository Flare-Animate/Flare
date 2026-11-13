#![allow(clippy::expect_used)]

use flare_commands::Command;
use flare_core::{DocumentId, FrameRate, Rgba8, ShapeId, SoundId};
use flare_document::{Scene, Shape, Sound, SoundCue, Tween};
use flare_history::EditorSession;

use super::*;

fn square(x: f64, start: u32, duration: u32) -> FrameSpan {
    let shape = Shape::rect(ShapeId::new(), x, 0.0, 5.0, 5.0, Rgba8::WHITE).expect("rect");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

/// Top to bottom: `Over`, folder `F` holding `A` (a shape tween over 6 frames) and `B` (two
/// keyframes, 9 frames), then `Under`.
fn session() -> (EditorSession, SceneId, Vec<LayerId>) {
    let ids: Vec<LayerId> = (0..5).map(|_| LayerId::new()).collect();
    let layers = vec![
        Layer::new(ids[0], "Over", vec![square(0.0, 0, 12)]).expect("layer"),
        Layer::new(ids[1], "F", vec![])
            .expect("folder")
            .with_folder(true),
        Layer::new(
            ids[2],
            "A",
            vec![
                square(10.0, 0, 5).with_tween(Tween::Shape),
                square(40.0, 5, 1),
            ],
        )
        .expect("layer")
        .with_parent(Some(ids[1])),
        Layer::new(ids[3], "B", vec![square(20.0, 0, 4), square(30.0, 4, 5)])
            .expect("layer")
            .with_parent(Some(ids[1])),
        Layer::new(ids[4], "Under", vec![square(50.0, 0, 12)]).expect("layer"),
    ];
    let scene = SceneId::new();
    let rate = FrameRate::new(24, 1).expect("rate");
    let doc = Document::new(
        DocumentId::new(),
        rate,
        vec![Scene::new(scene, "S", Timeline::new(layers))],
        vec![],
    )
    .expect("doc");
    (EditorSession::new(doc), scene, ids)
}

/// What is drawn at `frame`: each shape, its tween step and where it lands, in paint order.
fn picture(document: &Document, scene: SceneId, frame: u32) -> Vec<(ShapeId, u64, [f64; 6])> {
    flare_eval::evaluate_frame(document, scene, frame)
        .expect("evaluates")
        .shapes
        .iter()
        .map(|s| (s.shape.id(), s.phase, s.transform.to_cols_array()))
        .collect()
}

fn names(s: &EditorSession) -> Vec<String> {
    s.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| l.name().to_owned())
        .collect()
}

#[test]
fn a_folder_becomes_a_symbol_and_every_frame_looks_the_same() {
    let (mut s, scene, ids) = session();
    let before: Vec<_> = (0..12).map(|f| picture(s.document(), scene, f)).collect();
    let done = convert_layers_to_symbol(
        s.document(),
        scene,
        &[ids[1]],
        "Flower",
        SymbolKind::Graphic,
    )
    .expect("converts");
    s.apply(done.commands).expect("a valid document");
    assert_eq!(names(&s), vec!["Over", "Flower", "Under"]);
    let symbol = s.document().symbol(done.symbol).expect("in the library");
    assert_eq!(symbol.name(), "Flower");
    assert_eq!(symbol.kind(), SymbolKind::Graphic);
    let inside: Vec<(&str, bool)> = symbol
        .timeline()
        .layers()
        .iter()
        .map(|l| (l.name(), l.parent().is_some()))
        .collect();
    assert_eq!(inside, vec![("F", false), ("A", true), ("B", true)]);
    let layer = &s.document().scenes()[0].timeline().layers()[1];
    assert_eq!(layer.id(), done.layer);
    assert_eq!(layer.spans().len(), 1);
    assert_eq!(
        layer.spans()[0].duration().get(),
        9,
        "as long as the layers were"
    );
    for (frame, was) in before.iter().enumerate().take(9) {
        assert_eq!(
            &picture(s.document(), scene, frame as u32),
            was,
            "frame {frame}"
        );
    }
    s.undo();
    assert_eq!(
        names(&s),
        vec!["Over", "F", "A", "B", "Under"],
        "one undo step"
    );
}

#[test]
fn single_layers_keep_their_order_and_their_folder() {
    let (mut s, scene, ids) = session();
    let before = picture(s.document(), scene, 3);
    let done = convert_layers_to_symbol(
        s.document(),
        scene,
        &[ids[3], ids[2]],
        "  Parts  ",
        SymbolKind::MovieClip,
    )
    .expect("converts");
    s.apply(done.commands).expect("valid");
    assert_eq!(names(&s), vec!["Over", "F", "Parts", "Under"]);
    let layer = &s.document().scenes()[0].timeline().layers()[2];
    assert_eq!(layer.parent(), Some(ids[1]), "still in the folder");
    let symbol = s.document().symbol(done.symbol).expect("made");
    assert_eq!(symbol.name(), "Parts", "trimmed");
    assert_eq!(symbol.kind(), SymbolKind::MovieClip);
    let inside: Vec<&str> = symbol.timeline().layers().iter().map(Layer::name).collect();
    assert_eq!(inside, vec!["A", "B"], "top to bottom as they were");
    assert!(
        symbol
            .timeline()
            .layers()
            .iter()
            .all(|l| l.parent().is_none())
    );
    assert_eq!(picture(s.document(), scene, 3), before);
}

#[test]
fn sounds_stay_in_the_scene_on_the_new_layer() {
    let (mut s, scene, ids) = session();
    let sound = SoundId::new();
    s.apply([
        Command::AddSound(Sound::new(sound, "pop", vec![1], 8000, 1, 8).expect("sound")),
        Command::SetCues {
            scene,
            layer: ids[3],
            cues: vec![SoundCue { start: 4, sound }],
        },
    ])
    .expect("cue");
    let done = convert_layers_to_symbol(s.document(), scene, &[ids[3]], "B", SymbolKind::Graphic)
        .expect("converts");
    s.apply(done.commands).expect("valid");
    let layer = s.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.id() == done.layer)
        .expect("new layer");
    assert_eq!(layer.cues(), &[SoundCue { start: 4, sound }]);
    let symbol = s.document().symbol(done.symbol).expect("made");
    assert!(symbol.timeline().layers()[0].cues().is_empty());
}

#[test]
fn bad_names_unknown_layers_and_the_camera_are_refused() {
    let (mut s, scene, ids) = session();
    let doc = s.document();
    let convert = |picked: &[LayerId], name: &str| {
        convert_layers_to_symbol(doc, scene, picked, name, SymbolKind::Graphic).map(|_| ())
    };
    assert_eq!(convert(&[], "X"), Err(ConvertRefusal::NoLayers));
    assert_eq!(
        convert(&[LayerId::new()], "X"),
        Err(ConvertRefusal::NoSuchLayer)
    );
    assert_eq!(convert(&[ids[0]], "  "), Err(ConvertRefusal::NameEmpty));
    assert_eq!(
        convert(&[ids[0]], &"n".repeat(MAX_NAME_CHARS + 1)),
        Err(ConvertRefusal::NameTooLong)
    );
    let made = convert_layers_to_symbol(doc, scene, &[ids[0]], "Taken", SymbolKind::Graphic)
        .expect("first");
    s.apply(made.commands).expect("valid");
    let doc = s.document();
    assert_eq!(
        convert_layers_to_symbol(doc, scene, &[ids[4]], "Taken", SymbolKind::Graphic).map(|_| ()),
        Err(ConvertRefusal::NameTaken)
    );
    let camera = LayerId::new();
    s.apply([Command::AddLayer {
        scene,
        layer: Layer::new(camera, "Camera", vec![])
            .expect("layer")
            .with_camera(true),
    }])
    .expect("camera");
    assert_eq!(
        convert_layers_to_symbol(s.document(), scene, &[camera], "Cam", SymbolKind::Graphic)
            .map(|_| ()),
        Err(ConvertRefusal::Camera)
    );
    // An empty folder has no frames to put in a symbol.
    let folder = LayerId::new();
    s.apply([Command::AddLayer {
        scene,
        layer: Layer::new(folder, "Empty", vec![])
            .expect("layer")
            .with_folder(true),
    }])
    .expect("folder");
    assert_eq!(
        convert_layers_to_symbol(s.document(), scene, &[folder], "E", SymbolKind::Graphic)
            .map(|_| ()),
        Err(ConvertRefusal::NoFrames)
    );
}

#[test]
fn a_free_name_counts_up_past_the_taken_ones() {
    let (mut s, scene, ids) = session();
    assert_eq!(free_name(s.document(), "Flower"), "Flower");
    assert_eq!(free_name(s.document(), "  "), "Symbol");
    let made = convert_layers_to_symbol(
        s.document(),
        scene,
        &[ids[0]],
        "Flower",
        SymbolKind::Graphic,
    )
    .expect("made");
    s.apply(made.commands).expect("valid");
    assert_eq!(free_name(s.document(), "Flower"), "Flower 2");
}
