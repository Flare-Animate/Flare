#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, Rgba8, ShapeId, SoundId};
use flare_document::{Bitmap, Scene, Shape, Sound};
use flare_history::EditorSession;

use super::*;

fn square(start: u32, duration: u32) -> FrameSpan {
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, Rgba8::WHITE).expect("rect");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

fn rate(fps: u32) -> FrameRate {
    FrameRate::new(fps, 1).expect("rate")
}

fn document(fps: u32, layers: Vec<Layer>, symbols: Vec<Symbol>) -> Document {
    Document::new(
        DocumentId::new(),
        rate(fps),
        vec![Scene::new(SceneId::new(), "Scene 1", Timeline::new(layers))],
        symbols,
    )
    .expect("doc")
}

/// A document to bring things into: one layer of 4 frames, and a symbol called "Ball".
fn target() -> Document {
    let ball = Symbol::new(
        SymbolId::new(),
        "Ball",
        Timeline::new(vec![
            Layer::new(LayerId::new(), "L", vec![square(0, 1)]).expect("l"),
        ]),
    );
    document(
        24,
        vec![Layer::new(LayerId::new(), "Layer 1", vec![square(0, 4)]).expect("layer")],
        vec![ball],
    )
}

/// A movie of 10 frames on two layers, with its own "Ball" symbol placed on one of them.
fn movie() -> Document {
    let ball = SymbolId::new();
    let symbol = Symbol::new(
        ball,
        "Ball",
        Timeline::new(vec![
            Layer::new(LayerId::new(), "L", vec![square(0, 2)]).expect("l"),
        ]),
    );
    let instance = Instance::new(InstanceId::new(), ball, Transform2D::IDENTITY);
    let one = Layer::new(LayerId::new(), "Art", vec![square(0, 10)]).expect("layer");
    let two = Layer::new(
        LayerId::new(),
        "Ball",
        vec![FrameSpan::new(
            2,
            NonZeroU32::new(3).expect("n"),
            Content::Instance(instance),
        )],
    )
    .expect("layer");
    document(24, vec![one, two], vec![symbol])
}

fn symbol_named<'a>(document: &'a Document, name: &str) -> &'a Symbol {
    document
        .symbols()
        .iter()
        .find(|s| s.name() == name)
        .unwrap_or_else(|| panic!("{name} in the library"))
}

#[test]
fn import_to_library_adds_the_movie_as_a_graphic_and_its_library_renamed_where_names_clash() {
    let mut session = EditorSession::new(target());
    let done =
        import_into(session.document(), &movie(), "intro", Destination::Library).expect("imports");
    assert_eq!(done.layer, None);
    session.apply(done.commands).expect("a valid document");
    let doc = session.document();
    let main = symbol_named(doc, "intro");
    assert_eq!(main.id(), done.symbol);
    assert_eq!(main.kind(), SymbolKind::Graphic);
    assert_eq!(main.timeline().layers().len(), 2);
    assert_eq!(main.timeline().frame_count(), 10);
    // Its Ball came in beside this document's Ball, and the movie's instance still finds it.
    let theirs = symbol_named(doc, "Ball 2");
    assert!(
        main.timeline().layers()[1]
            .spans()
            .iter()
            .flat_map(FrameSpan::elements)
            .all(|e| matches!(e, Content::Instance(i) if i.symbol() == theirs.id()))
    );
    assert!(
        done.notes
            .contains(&ImportNote::Renamed("Ball".into(), "Ball 2".into()))
    );
    // The scene is as it was.
    assert_eq!(doc.scenes()[0].timeline().layers().len(), 1);
    // One undo step takes it all back.
    assert!(session.undo());
    assert_eq!(session.document().symbols().len(), 1);
}

#[test]
fn import_to_stage_puts_the_movie_on_a_new_top_layer_from_the_playhead() {
    let mut session = EditorSession::new(target());
    let scene = session.document().scenes()[0].id();
    let done = import_into(
        session.document(),
        &movie(),
        "intro",
        Destination::Stage { scene, frame: 3 },
    )
    .expect("imports");
    session.apply(done.commands).expect("valid");
    let layers = session.document().scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 2);
    let placed = &layers[0];
    assert_eq!(Some(placed.id()), done.layer);
    assert_eq!(placed.name(), "intro");
    let spans = placed.spans();
    assert_eq!(
        spans.len(),
        2,
        "an empty stretch up to the playhead, then the movie"
    );
    assert!(matches!(spans[0].content(), Content::Empty));
    assert_eq!((spans[1].start(), spans[1].end()), (3, 13));
    let Content::Instance(instance) = spans[1].content() else {
        panic!("an instance");
    };
    assert_eq!(instance.symbol(), done.symbol);
    assert_eq!(instance.transform(), Transform2D::IDENTITY);
    // It draws: the movie's square on frame 3 of the scene.
    let drawn = flare_eval::evaluate_frame(session.document(), scene, 3).expect("evaluates");
    assert_eq!(drawn.shapes.len(), 2, "the scene's square and the movie's");
}

#[test]
fn sounds_on_the_main_timeline_play_from_the_placed_layer_or_are_noted_for_the_library() {
    let sound = Sound::new(SoundId::new(), "beep", vec![0; 64], 44_100, 1, 32).expect("sound");
    let layer = Layer::new(LayerId::new(), "Art", vec![square(0, 6)])
        .expect("layer")
        .with_cues(vec![SoundCue {
            start: 1,
            sound: sound.id(),
        }])
        .expect("cues");
    let movie = document(24, vec![layer], vec![])
        .with_sounds(vec![sound.clone()])
        .expect("sounds");
    let mut session = EditorSession::new(target());
    let scene = session.document().scenes()[0].id();
    let done = import_into(
        session.document(),
        &movie,
        "beeps",
        Destination::Stage { scene, frame: 2 },
    )
    .expect("imports");
    session.apply(done.commands).expect("valid");
    let doc = session.document();
    assert_eq!(doc.sounds().len(), 1);
    assert_eq!(
        doc.scenes()[0].timeline().layers()[0].cues(),
        &[SoundCue {
            start: 3,
            sound: sound.id()
        }]
    );
    let inside = symbol_named(doc, "beeps").timeline().layers();
    assert!(inside.iter().all(|l| l.cues().is_empty()));
    let library = import_into(&target(), &movie, "beeps", Destination::Library).expect("imports");
    assert!(library.notes.contains(&ImportNote::SoundsNotPlaced(1)));
}

#[test]
fn a_picture_this_document_has_is_shared_and_the_movie_points_at_it() {
    let rgba = vec![255; 4 * 4 * 4];
    let here = Bitmap::new(BitmapId::new(), "p", 4, 4, rgba.clone()).expect("bitmap");
    let theirs = Bitmap::new(BitmapId::new(), "p", 4, 4, rgba).expect("bitmap");
    let target = target().with_bitmaps(vec![here.clone()]).expect("bitmaps");
    let picture = BitmapInstance::new(InstanceId::new(), theirs.id(), Transform2D::IDENTITY);
    let layer = Layer::new(
        LayerId::new(),
        "Pic",
        vec![FrameSpan::new(0, NonZeroU32::MIN, Content::Bitmap(picture))],
    )
    .expect("layer");
    let movie = document(24, vec![layer], vec![])
        .with_bitmaps(vec![theirs])
        .expect("bitmaps");
    let mut session = EditorSession::new(target);
    let done = import_into(session.document(), &movie, "pic", Destination::Library).expect("ok");
    session.apply(done.commands).expect("valid");
    let doc = session.document();
    assert_eq!(doc.bitmaps().len(), 1, "one copy of the picture");
    let Content::Bitmap(placed) =
        symbol_named(doc, "pic").timeline().layers()[0].spans()[0].content()
    else {
        panic!("a picture");
    };
    assert_eq!(placed.bitmap(), here.id());
}

#[test]
fn the_camera_and_other_scenes_are_left_out_and_said_so() {
    let camera = Layer::new(LayerId::new(), "Camera", vec![])
        .expect("cam")
        .with_camera(true);
    let art = Layer::new(LayerId::new(), "Art", vec![square(0, 3)]).expect("layer");
    let movie = Document::new(
        DocumentId::new(),
        rate(12),
        vec![
            Scene::new(SceneId::new(), "One", Timeline::new(vec![camera, art])),
            Scene::new(SceneId::new(), "Two", Timeline::new(vec![])),
        ],
        vec![],
    )
    .expect("doc");
    let done = import_into(&target(), &movie, "m", Destination::Library).expect("imports");
    assert!(done.notes.contains(&ImportNote::CameraLeftOut));
    assert!(done.notes.contains(&ImportNote::ScenesLeftOut(1)));
    assert!(done.notes.contains(&ImportNote::FrameRate("12.00".into())));
    let mut session = EditorSession::new(target());
    session.apply(done.commands).expect("valid");
    let inside = symbol_named(session.document(), "m").timeline().layers();
    assert_eq!(inside.len(), 1);
    assert!(!inside[0].is_camera());
}

#[test]
fn an_empty_movie_a_missing_scene_or_the_same_library_twice_are_refused() {
    let empty = document(24, vec![], vec![]);
    assert_eq!(
        import_into(&target(), &empty, "e", Destination::Library),
        Err(ImportRefusal::NoFrames)
    );
    let elsewhere = Destination::Stage {
        scene: SceneId::new(),
        frame: 0,
    };
    assert_eq!(
        import_into(&target(), &movie(), "m", elsewhere),
        Err(ImportRefusal::NoSuchScene)
    );
    // The very same document again: its symbols are already here.
    let movie = movie();
    let mut session = EditorSession::new(target());
    let done = import_into(session.document(), &movie, "m", Destination::Library).expect("ok");
    session.apply(done.commands).expect("valid");
    assert_eq!(
        import_into(session.document(), &movie, "m", Destination::Library),
        Err(ImportRefusal::AlreadyHere("symbols"))
    );
}

#[test]
fn a_placement_past_the_last_frame_is_refused() {
    let target = target();
    let scene = target.scenes()[0].id();
    let far = Destination::Stage {
        scene,
        frame: u32::MAX - 2,
    };
    assert_eq!(
        import_into(&target, &movie(), "m", far),
        Err(ImportRefusal::FrameTooFar)
    );
}

#[test]
fn a_second_import_of_the_same_name_gets_the_next_free_name() {
    let mut session = EditorSession::new(target());
    for _ in 0..2 {
        let done = import_into(session.document(), &movie(), "intro", Destination::Library)
            .expect("imports");
        session.apply(done.commands).expect("valid");
    }
    let names: Vec<&str> = session
        .document()
        .symbols()
        .iter()
        .map(Symbol::name)
        .collect();
    for wanted in ["intro", "intro 2", "Ball 2", "Ball 3"] {
        assert!(names.contains(&wanted), "{wanted} in {names:?}");
    }
}
