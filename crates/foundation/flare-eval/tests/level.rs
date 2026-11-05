//! `evaluate_level` keeps a timeline's structure, and playing that structure out again (each
//! symbol on the clock `symbol_frame` gives it) draws what `evaluate_frame` draws.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::Transform2D;
use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PlayMode, Playback, Scene, Shape, Symbol,
    SymbolKind, Timeline, Tween,
};
use flare_eval::{LevelContent, evaluate_frame, evaluate_level, symbol_frame};
use glamx::{DAffine2, DVec2};

fn at(x: f64, y: f64) -> Transform2D {
    Transform2D::new(DAffine2::from_translation(DVec2::new(x, y))).expect("finite")
}

fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
}

fn shape(x: f64) -> Content {
    Content::Shape(Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, Rgba8::WHITE).expect("rect"))
}

fn instance(symbol: SymbolId, x: f64, playback: Playback) -> Content {
    Content::Instance(Instance::new(InstanceId::new(), symbol, at(x, 5.0)).with_playback(playback))
}

fn layer(spans: Vec<FrameSpan>) -> Layer {
    Layer::new(LayerId::new(), "L", spans).expect("layer")
}

/// Every drawn shape as (shape, transform, phase), found by walking levels.
fn flatten(
    document: &Document,
    timeline: &Timeline,
    frame: u32,
    parent: DAffine2,
    out: &mut Vec<(ShapeId, [f64; 6], u64)>,
) {
    for item in evaluate_level(document, timeline, frame)
        .expect("level")
        .items
    {
        match item.content {
            LevelContent::Shape { shape, phase } => {
                out.push((shape.id(), parent.to_cols_array(), phase));
            }
            LevelContent::Bitmap { .. } => {}
            LevelContent::Instance {
                symbol,
                transform,
                span_start,
                playback,
                ..
            } => {
                let symbol = document.symbol(symbol).expect("symbol");
                let length = symbol.timeline().frame_count();
                if length > 0 {
                    let local = symbol_frame(
                        symbol.kind(),
                        playback,
                        u64::from(frame - span_start),
                        length,
                    );
                    flatten(document, symbol.timeline(), local, parent * transform, out);
                }
            }
        }
    }
}

/// A scene whose instances show every kind of symbol and every way of playing one, with a motion
/// tween and a shape tween among them.
fn document() -> (Document, SceneId) {
    let inner = SymbolId::new();
    let outer = SymbolId::new();
    let clip = SymbolId::new();
    let button = SymbolId::new();
    let playbacks = [
        Playback::default(),
        Playback {
            mode: PlayMode::PlayOnce,
            first_frame: 1,
        },
        Playback {
            mode: PlayMode::SingleFrame,
            first_frame: 2,
        },
        Playback {
            mode: PlayMode::Loop,
            first_frame: 2,
        },
    ];
    let steps = || {
        Timeline::new(vec![layer(vec![
            span(0, 1, shape(0.0)),
            span(1, 1, shape(20.0)),
            span(2, 1, shape(40.0)),
        ])])
    };
    let nested = || {
        Timeline::new(vec![
            layer(vec![
                span(0, 4, instance(inner, 0.0, playbacks[0])).with_tween(Tween::Linear),
                span(4, 2, instance(inner, 60.0, playbacks[0])),
            ]),
            layer(vec![span(0, 6, instance(inner, 5.0, playbacks[1]))]),
            layer(vec![
                span(0, 3, shape(1.0)).with_tween(Tween::Shape),
                span(3, 3, shape(2.0)),
            ]),
        ])
    };
    let symbols = vec![
        Symbol::new(inner, "Inner", steps()),
        Symbol::new(outer, "Outer", nested()),
        Symbol::new(clip, "Clip", nested()).with_kind(SymbolKind::MovieClip),
        Symbol::new(button, "Button", steps()).with_kind(SymbolKind::Button),
    ];
    let scene = Timeline::new(
        playbacks
            .iter()
            .enumerate()
            .flat_map(|(i, p)| {
                let x = 10.0 * i as f64;
                [
                    layer(vec![
                        span(2, 9, instance(outer, x, *p)),
                        span(11, 3, instance(clip, x, *p)),
                    ]),
                    layer(vec![span(0, 14, instance(button, x, *p))]),
                ]
            })
            .collect(),
    );
    let id = SceneId::new();
    let rate = FrameRate::new(24, 1).expect("rate");
    (
        Document::new(
            DocumentId::new(),
            rate,
            vec![Scene::new(id, "S", scene)],
            symbols,
        )
        .expect("document"),
        id,
    )
}

#[test]
fn playing_a_level_on_the_symbols_own_clocks_draws_what_the_flattened_frame_draws() {
    let (document, id) = document();
    let timeline = document.scenes()[0].timeline();
    // The scene is 14 frames long; the two after it are empty in both.
    for frame in 0..16 {
        let mut found = Vec::new();
        flatten(&document, timeline, frame, DAffine2::IDENTITY, &mut found);
        let flat: Vec<_> = evaluate_frame(&document, id, frame)
            .expect("frame")
            .shapes
            .iter()
            .map(|d| (d.shape.id(), d.transform.to_cols_array(), d.phase))
            .collect();
        assert_eq!(flat.is_empty(), frame >= 14, "frame {frame}");
        assert_eq!(found, flat, "frame {frame}");
    }
}

#[test]
fn a_level_has_one_item_per_visible_layer_bottom_first_with_the_instances_own_transform() {
    let (document, _) = document();
    let timeline = document.scenes()[0].timeline();
    let level = evaluate_level(&document, timeline, 3).expect("level");
    assert_eq!(level.items.len(), timeline.layers().len());
    let layers: Vec<usize> = level.items.iter().map(|i| i.layer).collect();
    let mut bottom_first = layers.clone();
    bottom_first.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(layers, bottom_first);
    for item in &level.items {
        let LevelContent::Instance {
            transform,
            span_start,
            ..
        } = &item.content
        else {
            panic!("the scene holds only instances");
        };
        assert_eq!(
            transform.translation.y, 5.0,
            "local, not composed with anything"
        );
        assert!(*span_start <= 3);
    }
    // Past the end of every span there is nothing.
    assert!(
        evaluate_level(&document, timeline, 100)
            .expect("level")
            .items
            .is_empty()
    );
}

#[test]
fn a_hidden_layer_a_guide_and_a_folder_contribute_nothing_and_a_mask_becomes_a_clip() {
    let art = layer(vec![span(0, 4, shape(0.0))]);
    let mask = Layer::new(LayerId::new(), "M", vec![span(0, 4, shape(3.0))])
        .expect("layer")
        .with_mask(true);
    let hidden = layer(vec![span(0, 4, shape(9.0))]).with_visible(false);
    let timeline = Timeline::new(vec![mask, art, hidden]);
    let rate = FrameRate::new(24, 1).expect("rate");
    let id = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        rate,
        vec![Scene::new(id, "S", timeline)],
        vec![],
    )
    .expect("document");
    let level = evaluate_level(&document, document.scenes()[0].timeline(), 1).expect("level");
    assert_eq!(
        level.items.len(),
        1,
        "the mask and the hidden layer draw nothing themselves"
    );
    assert_eq!(level.items[0].layer, 1);
    assert_eq!(level.items[0].clip, Some(0));
    assert_eq!(level.clips.len(), 1);
    assert_eq!(level.clips[0].len(), 1);
}
