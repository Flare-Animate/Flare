//! Symbols stay symbols: each one is a sprite, defined once, with the clock its playback says.

use flare_core::SymbolId;
use flare_document::{Instance, Layer, PlayMode, Playback, Symbol, SymbolKind, Tween};

use super::*;

pub(super) fn sprites(parsed: &Parsed) -> Vec<(u16, u16, usize)> {
    parsed
        .tags
        .iter()
        .filter_map(|t| match t {
            Tag::DefineSprite(s) => Some((
                s.id,
                s.num_frames,
                s.tags
                    .iter()
                    .filter(|t| matches!(t, Tag::DoAction(_)))
                    .count(),
            )),
            _ => None,
        })
        .collect()
}

pub(super) fn playing(symbol: SymbolId, x: f64, y: f64, playback: Playback) -> Content {
    let transform =
        flare_core::Transform2D::new(glamx::DAffine2::from_translation(glamx::DVec2::new(x, y)))
            .expect("finite");
    Content::Instance(
        Instance::new(flare_core::InstanceId::new(), symbol, transform).with_playback(playback),
    )
}

/// Three frames, each a square of its own color further right.
pub(super) fn steps() -> Timeline {
    let square = |x: f64, c: Rgba8| Content::Shape(rect(x, 0.0, 8.0, 8.0, c));
    Timeline::new(vec![layer(
        "L",
        vec![
            span(0, 1, square(0.0, Rgba8::opaque(255, 0, 0))),
            span(1, 1, square(10.0, Rgba8::opaque(0, 255, 0))),
            span(2, 1, square(20.0, Rgba8::opaque(0, 0, 255))),
        ],
    )])
}

pub(super) fn playback(mode: PlayMode, first_frame: u32) -> Playback {
    Playback { mode, first_frame }
}

/// Every kind of symbol and way of playing one, each on a layer of its own, with the same
/// instance starting again after a gap, one tweened, and a symbol inside a symbol.
fn kinds() -> Document {
    let (graphic, clip, button, outer) = (
        SymbolId::new(),
        SymbolId::new(),
        SymbolId::new(),
        SymbolId::new(),
    );
    let nested = Timeline::new(vec![
        layer(
            "in",
            vec![
                span(
                    0,
                    3,
                    playing(graphic, 0.0, 12.0, playback(PlayMode::Loop, 0)),
                )
                .with_tween(Tween::Linear),
                span(
                    3,
                    3,
                    playing(graphic, 30.0, 12.0, playback(PlayMode::Loop, 0)),
                ),
            ],
        ),
        layer(
            "dot",
            vec![span(
                0,
                6,
                Content::Shape(rect(0.0, 0.0, 4.0, 4.0, Rgba8::WHITE)),
            )],
        ),
    ]);
    let symbols = vec![
        Symbol::new(graphic, "Graphic", steps()),
        Symbol::new(clip, "Clip", steps()).with_kind(SymbolKind::MovieClip),
        Symbol::new(button, "Button", steps()).with_kind(SymbolKind::Button),
        Symbol::new(outer, "Outer", nested).with_kind(SymbolKind::MovieClip),
    ];
    let row = |y: f64, content: &dyn Fn(f64) -> Content| {
        layer("row", vec![span(0, 7, content(y)), span(9, 7, content(y))])
    };
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![
            row(0.0, &|y| {
                playing(graphic, 0.0, y, playback(PlayMode::Loop, 0))
            }),
            row(14.0, &|y| {
                playing(graphic, 0.0, y, playback(PlayMode::Loop, 1))
            }),
            row(28.0, &|y| {
                playing(graphic, 0.0, y, playback(PlayMode::PlayOnce, 1))
            }),
            row(42.0, &|y| {
                playing(graphic, 0.0, y, playback(PlayMode::SingleFrame, 2))
            }),
            row(56.0, &|y| playing(clip, 0.0, y, Playback::default())),
            row(70.0, &|y| playing(button, 0.0, y, Playback::default())),
            row(84.0, &|y| playing(outer, 0.0, y, Playback::default())),
        ]),
    );
    document(80, 110, vec![scene], symbols)
}

#[test]
fn every_kind_of_symbol_plays_in_the_swf_as_it_does_in_the_document() {
    let doc = kinds();
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    assert_eq!(published.frames, 16);
    let parsed = parse(&published.bytes);
    let background = doc.stage().background();
    for frame in 0..16 {
        let (max, mean) = difference(
            &render_original(&doc, 0, frame),
            &render_swf_frame(&parsed, frame as usize, background),
        );
        assert!(
            max <= 8 && mean < 0.05,
            "frame {frame}: max {max}, mean {mean}"
        );
    }
    // Both ends of the gap are something: the rows have started again.
    assert_ne!(
        render_swf_frame(&parsed, 7, background).rgba(),
        render_swf_frame(&parsed, 9, background).rgba()
    );
}

#[test]
fn symbols_are_sprites_defined_once_with_a_stop_only_where_a_symbol_plays_once() {
    let doc = kinds();
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let found = sprites(&parsed);
    // Loop from 0 (the rows and the instances inside Outer share it), Loop from 1, play once
    // from 1, hold frame 2, the Button's first frame, the Movie Clip, and Outer.
    let frames: Vec<u16> = found.iter().map(|s| s.1).collect();
    assert_eq!(found.len(), 7, "{found:?}");
    assert_eq!(frames.iter().filter(|f| **f == 3).count(), 3, "{frames:?}");
    assert_eq!(
        frames.iter().filter(|f| **f == 2).count(),
        1,
        "play once from 1: {frames:?}"
    );
    assert_eq!(
        frames.iter().filter(|f| **f == 1).count(),
        2,
        "the held frame and the Button: {frames:?}"
    );
    let stops: usize = found.iter().map(|s| s.2).sum();
    assert_eq!(stops, 1, "only the sprite that plays once stops: {found:?}");
    let mut ids: Vec<u16> = found.iter().map(|s| s.0).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), found.len(), "character ids are unique");
}

#[test]
fn buttons_are_reported_as_showing_their_up_state() {
    let published = publish_swf(&kinds(), &PublishOptions::default()).expect("publish");
    assert!(
        published
            .diagnostics
            .iter()
            .any(|d| d.message.contains("buttons") && d.message.contains("Up")),
        "{:?}",
        published.diagnostics
    );
    let without = publish_swf(&animated(), &PublishOptions::default()).expect("publish");
    assert!(
        without
            .diagnostics
            .iter()
            .all(|d| !d.message.contains("buttons"))
    );
}

#[test]
fn a_symbol_that_draws_nothing_is_not_placed() {
    let empty = SymbolId::new();
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![layer(
            "a",
            vec![span(0, 2, instance(empty, 0.0, 0.0))],
        )]),
    );
    let doc = document(
        10,
        10,
        vec![scene],
        vec![Symbol::new(empty, "Empty", Timeline::default())],
    );
    let parsed = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    assert!(sprites(&parsed).is_empty());
    assert!(
        !parsed.tags.iter().any(|t| matches!(t, Tag::PlaceObject(_))),
        "nothing was placed"
    );
}

#[test]
fn symbols_nested_too_deep_are_an_error_not_a_stack_overflow() {
    let ids: Vec<SymbolId> = (0..40).map(|_| SymbolId::new()).collect();
    let symbols: Vec<Symbol> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let content = match ids.get(i + 1) {
                Some(next) => instance(*next, 0.0, 0.0),
                None => Content::Shape(rect(0.0, 0.0, 4.0, 4.0, Rgba8::WHITE)),
            };
            Symbol::new(
                *id,
                "S",
                Timeline::new(vec![layer("L", vec![span(0, 1, content)])]),
            )
        })
        .collect();
    let scene = Scene::new(
        SceneId::new(),
        "S",
        Timeline::new(vec![layer(
            "L",
            vec![span(0, 1, instance(ids[0], 0.0, 0.0))],
        )]),
    );
    let doc = document(10, 10, vec![scene], symbols);
    let result = publish_swf(&doc, &PublishOptions::default());
    assert!(
        matches!(result, Err(PublishError::SymbolsTooDeep)),
        "{:?}",
        result.map(|p| p.frames)
    );
}

#[test]
fn more_layers_than_a_swf_has_depths_for_are_an_error() {
    let layers: Vec<Layer> = (0..16_001)
        .map(|i| layer(&format!("L{i}"), vec![]))
        .collect();
    let doc = document(
        10,
        10,
        vec![Scene::new(SceneId::new(), "S", Timeline::new(layers))],
        vec![],
    );
    let result = publish_swf(&doc, &PublishOptions::default());
    assert!(
        matches!(result, Err(PublishError::TooManyLayers(16_001))),
        "{:?}",
        result.map(|p| p.frames)
    );
}
