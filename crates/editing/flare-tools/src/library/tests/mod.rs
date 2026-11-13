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

mod add_to_symbol;
mod bake;
mod increment;
mod symbols;
