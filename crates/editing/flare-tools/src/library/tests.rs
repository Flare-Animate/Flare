#![allow(clippy::expect_used)]

use super::*;
use crate::ops::find_span;
use crate::test_support::*;
use flare_commands::Command;
use flare_core::{InstanceId, LayerId, SymbolId, Transform2D};
use flare_document::{
    Content, Instance, Layer, PlayMode, Playback, Symbol, SymbolKind, Timeline, Tween,
};
use glamx::dvec2;
use std::num::NonZeroU32;

mod bake_transform;

/// A fixture with `sym` (symbol of a 5-frame rectangle) in the library, `outer` holding one of
/// its instances, and two scene spans holding instances of `sym` that read frame 3 and frame 8.
struct LibraryFixture {
    f: Fixture,
    sym: SymbolId,
    outer: SymbolId,
    late: u32,
}

fn playback(first_frame: u32) -> Playback {
    Playback {
        mode: PlayMode::Loop,
        first_frame,
    }
}

fn instance_span(
    start: u32,
    duration: u32,
    symbol: SymbolId,
    playback: Playback,
    transform: Transform2D,
) -> FrameSpan {
    let instance = Instance::new(InstanceId::new(), symbol, transform).with_playback(playback);
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Instance(instance),
    )
}

fn library_fixture() -> LibraryFixture {
    let mut f = fixture();
    let sym = SymbolId::new();
    let inner = Layer::new(
        LayerId::new(),
        "I",
        vec![rect_span(0, 5, 0.0, 0.0, 10.0, 10.0)],
    )
    .expect("layer");
    let outer = SymbolId::new();
    let outer_instance =
        Instance::new(InstanceId::new(), sym, Transform2D::IDENTITY).with_playback(playback(5));
    let outer_layer = Layer::new(
        LayerId::new(),
        "N",
        vec![FrameSpan::new(
            0,
            NonZeroU32::new(6).expect("nonzero"),
            Content::Instance(outer_instance),
        )],
    )
    .expect("layer");
    f.session
        .apply([
            Command::AddSymbol(
                Symbol::new(sym, "Walk", Timeline::new(vec![inner])).with_kind(SymbolKind::Graphic),
            ),
            Command::AddSymbol(Symbol::new(outer, "Rig", Timeline::new(vec![outer_layer]))),
        ])
        .expect("symbols");
    let (early, late) = (0, 6);
    let placed = Transform2D::new(glamx::DAffine2::from_translation(glamx::dvec2(5.0, 7.0)))
        .expect("finite");
    add(&mut f, instance_span(early, 6, sym, playback(3), placed));
    add(
        &mut f,
        instance_span(late, 6, sym, playback(8), Transform2D::IDENTITY),
    );
    LibraryFixture {
        f,
        sym,
        outer,
        late,
    }
}

/// The playback of the symbol instance the span at `start` holds.
fn playback_at(f: &Fixture, start: u32) -> Playback {
    match find_span(f.session.document(), f.scene, at(f, start))
        .expect("span")
        .content()
    {
        Content::Instance(instance) => instance.playback(),
        other => panic!("{other:?}"),
    }
}

/// The transform of the symbol instance the span at `start` holds.
fn transform_at(f: &Fixture, start: u32) -> Transform2D {
    match find_span(f.session.document(), f.scene, at(f, start))
        .expect("span")
        .content()
    {
        Content::Instance(instance) => instance.transform(),
        other => panic!("{other:?}"),
    }
}

/// The transform of the first instance of `symbol`'s own timeline.
fn nested(f: &Fixture, symbol: SymbolId) -> Content {
    f.session
        .document()
        .symbol(symbol)
        .expect("symbol")
        .timeline()
        .layers()[0]
        .spans()[0]
        .content()
        .clone()
}

/// The shape of `symbol`'s own timeline artwork.
fn artwork(f: &Fixture, symbol: SymbolId) -> flare_document::Shape {
    match nested(f, symbol) {
        Content::Shape(shape) => shape,
        other => panic!("{other:?}"),
    }
}

/// `a` and `b` send the same sample points to the same place.
fn same(a: Transform2D, b: Transform2D) -> bool {
    [(0.0, 0.0), (1.0, 0.0), (10.0, -3.0)]
        .iter()
        .all(|&(x, y)| {
            (a.as_affine().transform_point2(dvec2(x, y))
                - b.as_affine().transform_point2(dvec2(x, y)))
            .length()
                < 1e-9
        })
}

fn apply(
    f: &mut Fixture,
    edit: impl FnOnce(&flare_document::Document) -> Result<Vec<Command>, LibraryError>,
) {
    let commands = edit(f.session.document()).expect("an edit");
    f.session.apply(commands).expect("apply");
}

#[test]
fn increment_instances_moves_every_qualifying_instance_on_the_scene_and_in_other_symbols() {
    let LibraryFixture {
        mut f,
        sym,
        outer,
        late,
        ..
    } = library_fixture();
    apply(&mut f, |doc| increment_instances(doc, sym, 4, 2));
    assert_eq!(playback_at(&f, 0).first_frame, 3, "the early one stays");
    assert_eq!(playback_at(&f, late).first_frame, 10, "the late one moves");
    let Content::Instance(nested_instance) = nested(&f, outer) else {
        panic!("nested instance");
    };
    assert_eq!(
        nested_instance.playback().first_frame,
        7,
        "the nested one moves too"
    );
    let _ = sym;
}

#[test]
fn increment_instances_refuses_to_move_a_frame_before_the_symbols_first() {
    let LibraryFixture { f, sym, .. } = library_fixture();
    // The early instance reads frame 3; four frames back leaves the symbol's frames.
    assert_eq!(
        increment_instances(f.session.document(), sym, 0, -4),
        Err(LibraryError::BeforeFirst)
    );
    assert_eq!(
        playback_at(&f, 0).first_frame,
        3,
        "nothing was applied, so nothing moved"
    );
}

#[test]
fn increment_instances_refuses_a_no_op() {
    let LibraryFixture { f, sym, .. } = library_fixture();
    assert_eq!(
        increment_instances(f.session.document(), sym, 0, 0),
        Err(LibraryError::NoChange)
    );
    assert_eq!(
        increment_instances(f.session.document(), sym, u32::MAX, 1),
        Err(LibraryError::NoChange),
        "no instance reads that far"
    );
    assert_eq!(
        increment_instances(f.session.document(), SymbolId::new(), 0, 1),
        Err(LibraryError::NoSuchSymbol)
    );
}

#[test]
fn add_instance_to_symbol_appends_one_frame_to_the_targets_first_layer() {
    let mut f = fixture();
    let target = SymbolId::new();
    let inner = Layer::new(
        LayerId::new(),
        "T",
        vec![rect_span(0, 10, 0.0, 0.0, 10.0, 10.0)],
    )
    .expect("layer");
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            target,
            "Box",
            Timeline::new(vec![inner]),
        ))])
        .expect("symbol");
    add(&mut f, rect_span(0, 6, 40.0, 40.0, 10.0, 10.0));
    let (scene, pick) = (f.scene, at(&f, 0));
    let before = f.session.document().clone();

    apply(&mut f, |doc| {
        add_instance_to_symbol(doc, target, scene, pick)
    });

    let document = f.session.document();
    let spans = document.symbol(target).expect("target").timeline().layers()[0].spans();
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[1].start(), 10, "after the target's own frames");
    assert_eq!(spans[1].duration().get(), 1, "one frame only");
    assert!(
        matches!(spans[1].content(), Content::Shape(_)),
        "the shape was copied"
    );
    assert_eq!(
        document.scenes()[0].timeline(),
        before.scenes()[0].timeline(),
        "the scene keeps its shape"
    );
}

#[test]
fn add_instance_to_symbol_refuses_the_wrong_pick_and_a_missing_target() {
    let mut f = fixture();
    let target = SymbolId::new();
    let inner = Layer::new(
        LayerId::new(),
        "T",
        vec![rect_span(0, 10, 0.0, 0.0, 10.0, 10.0)],
    )
    .expect("layer");
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            target,
            "Box",
            Timeline::new(vec![inner]),
        ))])
        .expect("symbol");
    add(
        &mut f,
        instance_span(0, 6, target, playback(0), Transform2D::IDENTITY),
    );

    // Nothing picked: no span starts there.
    assert_eq!(
        add_instance_to_symbol(f.session.document(), target, f.scene, at(&f, 99)),
        Err(LibraryError::NothingSelected)
    );
    // An instance of the target inside the target: a cycle.
    assert_eq!(
        add_instance_to_symbol(f.session.document(), target, f.scene, at(&f, 0)),
        Err(LibraryError::Cycle)
    );
    // No target named that.
    assert_eq!(
        add_instance_to_symbol(f.session.document(), SymbolId::new(), f.scene, at(&f, 0)),
        Err(LibraryError::NoSuchSymbol)
    );
}

#[test]
fn add_instance_to_symbol_refuses_an_empty_keyframe_and_a_layerless_target() {
    let mut f = fixture();
    add(&mut f, FrameSpan::new(0, NonZeroU32::MIN, Content::Empty));
    let hollow = SymbolId::new();
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            hollow,
            "Empty",
            Timeline::new(vec![]),
        ))])
        .expect("symbol");
    let layerless = SymbolId::new();
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            layerless,
            "NoLayer",
            Timeline::new(vec![]),
        ))])
        .expect("symbol");

    assert_eq!(
        add_instance_to_symbol(f.session.document(), hollow, f.scene, at(&f, 0)),
        Err(LibraryError::NotPlaceable),
        "an empty keyframe holds nothing to copy"
    );

    // With something to copy, a target without layers still cannot take it.
    add(&mut f, rect_span(6, 4, 0.0, 0.0, 10.0, 10.0));
    assert_eq!(
        add_instance_to_symbol(f.session.document(), layerless, f.scene, at(&f, 6)),
        Err(LibraryError::NoLayer)
    );
}
