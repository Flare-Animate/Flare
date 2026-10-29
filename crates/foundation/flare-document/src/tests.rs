#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{InstanceId, LayerId, Transform2D};

use super::*;

fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
}

fn place(symbol: SymbolId) -> Content {
    Content::Instance(Instance::new(
        InstanceId::new(),
        symbol,
        Transform2D::IDENTITY,
    ))
}

fn timeline(spans: Vec<FrameSpan>) -> Timeline {
    Timeline::new(vec![
        Layer::new(LayerId::new(), "Layer 1", spans).expect("valid layer"),
    ])
}

fn build(scenes: Vec<Scene>, symbols: Vec<Symbol>) -> Result<Document, Vec<Diagnostic>> {
    let rate = FrameRate::new(24, 1).expect("rate");
    Document::new(DocumentId::new(), rate, scenes, symbols)
}

#[test]
fn two_instances_share_one_symbol_definition() {
    let art = SymbolId::new();
    let symbol = Symbol::new(art, "Art", Timeline::default());
    let scene = Scene::new(
        SceneId::new(),
        "Scene 1",
        timeline(vec![span(0, 10, place(art)), span(10, 5, place(art))]),
    );
    let document = build(vec![scene], vec![symbol]).expect("valid");
    assert_eq!(document.symbols().len(), 1);
    assert!(document.symbol(art).is_some());
}

#[test]
fn empty_span_is_distinct_from_absent_span() {
    let layer = Layer::new(LayerId::new(), "L", vec![span(0, 1, Content::Empty)]).expect("ok");
    assert_eq!(layer.spans().len(), 1);
    assert_eq!(layer.spans()[0].content(), &Content::Empty);
}

#[test]
fn overlapping_spans_are_rejected() {
    let spans = vec![span(0, 5, Content::Empty), span(4, 2, Content::Empty)];
    assert_eq!(
        Layer::new(LayerId::new(), "L", spans),
        Err(TimelineError::OverlappingSpans(4))
    );
}

#[test]
fn adjacent_spans_are_allowed_and_sorted() {
    let spans = vec![span(5, 1, Content::Empty), span(0, 5, Content::Empty)];
    let layer = Layer::new(LayerId::new(), "L", spans).expect("adjacent");
    assert_eq!(layer.spans()[0].start(), 0);
}

#[test]
fn layers_default_to_visible_and_unlocked_and_with_spans_keeps_flags() {
    let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
    assert!(layer.is_visible() && !layer.is_locked());
    let flagged = layer
        .with_visible(false)
        .with_locked(true)
        .with_name("Renamed");
    let rebuilt = flagged
        .with_spans(vec![span(0, 2, Content::Empty)])
        .expect("spans");
    assert_eq!(
        (rebuilt.name(), rebuilt.is_visible(), rebuilt.is_locked()),
        ("Renamed", false, true)
    );
    assert_eq!(rebuilt.spans().len(), 1);
    assert!(
        flagged
            .with_spans(vec![span(0, 5, Content::Empty), span(2, 2, Content::Empty)])
            .is_err()
    );
}

#[test]
fn span_lookup_and_frame_count() {
    let spans = vec![span(2, 3, Content::Empty), span(10, 1, Content::Empty)];
    let layer = Layer::new(LayerId::new(), "L", spans).expect("layer");
    assert!(layer.span_at(1).is_none());
    assert_eq!(layer.span_at(2).map(FrameSpan::start), Some(2));
    assert_eq!(layer.span_at(4).map(FrameSpan::start), Some(2));
    assert!(layer.span_at(5).is_none());
    assert_eq!(layer.span_at(10).map(FrameSpan::start), Some(10));
    assert!(layer.span_at(11).is_none());
    assert_eq!(Timeline::new(vec![layer]).frame_count(), 11);
    assert_eq!(Timeline::default().frame_count(), 0);
}

#[test]
fn span_end_cannot_overflow() {
    let tail = span(u32::MAX, u32::MAX, Content::Empty);
    assert_eq!(tail.end(), u64::from(u32::MAX) * 2);
}

#[test]
fn dangling_symbol_reference_is_diagnosed() {
    let missing = SymbolId::new();
    let scene = Scene::new(
        SceneId::new(),
        "S",
        timeline(vec![span(0, 1, place(missing))]),
    );
    let errors = build(vec![scene], vec![]).expect_err("dangling");
    assert!(matches!(errors[..], [Diagnostic::DanglingSymbol { symbol, .. }] if symbol == missing));
}

#[test]
fn duplicate_shape_ids_are_diagnosed() {
    use flare_core::{Rgba8, ShapeId};
    let id = ShapeId::new();
    let shape = || Shape::rect(id, 0.0, 0.0, 1.0, 1.0, Rgba8::WHITE).expect("rect");
    let spans = vec![
        span(0, 1, Content::Shape(shape())),
        span(1, 1, Content::Shape(shape())),
    ];
    let scene = Scene::new(SceneId::new(), "S", timeline(spans));
    assert_eq!(
        build(vec![scene], vec![]).expect_err("dup"),
        vec![Diagnostic::DuplicateShape(id)]
    );
}

#[test]
fn duplicate_symbol_ids_are_diagnosed() {
    let id = SymbolId::new();
    let symbols = vec![
        Symbol::new(id, "A", Timeline::default()),
        Symbol::new(id, "B", Timeline::default()),
    ];
    assert_eq!(
        build(vec![], symbols).expect_err("dup"),
        vec![Diagnostic::DuplicateSymbol(id)]
    );
}

#[test]
fn direct_and_indirect_symbol_cycles_are_rejected() {
    let (a, b) = (SymbolId::new(), SymbolId::new());
    let direct = Symbol::new(a, "A", timeline(vec![span(0, 1, place(a))]));
    assert_eq!(
        build(vec![], vec![direct]).expect_err("cycle"),
        vec![Diagnostic::SymbolCycle(a)]
    );

    let sym_a = Symbol::new(a, "A", timeline(vec![span(0, 1, place(b))]));
    let sym_b = Symbol::new(b, "B", timeline(vec![span(0, 1, place(a))]));
    let errors = build(vec![], vec![sym_a, sym_b]).expect_err("cycle");
    assert!(
        errors
            .iter()
            .all(|d| matches!(d, Diagnostic::SymbolCycle(_)))
    );
    assert!(!errors.is_empty());
}

#[test]
fn deep_symbol_chains_do_not_overflow_the_stack() {
    let ids: Vec<SymbolId> = (0..50_000).map(|_| SymbolId::new()).collect();
    let symbols = ids
        .windows(2)
        .map(|pair| Symbol::new(pair[0], "S", timeline(vec![span(0, 1, place(pair[1]))])))
        .chain([Symbol::new(ids[ids.len() - 1], "leaf", Timeline::default())])
        .collect();
    assert!(build(vec![], symbols).is_ok());
}

fn note(anchor: Anchor, placement: Placement) -> Attachment {
    Attachment::new("xfl", anchor, placement, "<DOMStaticText/>").expect("attachment")
}

#[test]
fn attachments_must_hang_off_something_that_exists() {
    let layer_id = LayerId::new();
    let layer = Layer::new(layer_id, "L", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let scene_id = scene.id();
    let doc = build(vec![scene], vec![]).expect("doc");
    assert!(doc.attachments().is_empty(), "a new document carries none");

    let good = vec![
        note(Anchor::Document, Placement::Child),
        note(Anchor::Scene(scene_id), Placement::LayerAt { index: 1 }),
        note(
            Anchor::Layer(layer_id),
            Placement::FrameElement { frame: 3 },
        ),
    ];
    let doc = doc.with_attachments(good).expect("valid anchors");
    assert_eq!(doc.attachments().len(), 3);

    let ghost = LayerId::new();
    let bad = vec![note(
        Anchor::Layer(ghost),
        Placement::FrameExtra { frame: 0 },
    )];
    let errors = doc.with_attachments(bad).expect_err("dangling");
    assert_eq!(
        errors,
        vec![Diagnostic::DanglingAttachment(Anchor::Layer(ghost))]
    );
}

#[test]
fn retained_attachments_drop_exactly_those_whose_anchor_is_gone() {
    let (keep, gone) = (LayerId::new(), LayerId::new());
    let layer = Layer::new(keep, "L", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let doc = build(vec![scene], vec![]).expect("doc");
    let all = vec![
        note(Anchor::Document, Placement::Child),
        note(Anchor::Layer(keep), Placement::FrameElement { frame: 0 }),
        note(Anchor::Layer(gone), Placement::FrameElement { frame: 0 }),
    ];
    let doc = doc.with_attachments_retained(&all);
    let anchors: Vec<Anchor> = doc.attachments().iter().map(Attachment::anchor).collect();
    assert_eq!(anchors, vec![Anchor::Document, Anchor::Layer(keep)]);
}

#[test]
fn attachments_inside_symbols_count_as_anchors_and_the_count_is_capped() {
    let (symbol_id, inner) = (SymbolId::new(), LayerId::new());
    let inner_layer = Layer::new(inner, "in", vec![]).expect("layer");
    let symbol = Symbol::new(symbol_id, "S", Timeline::new(vec![inner_layer]));
    let doc = build(vec![], vec![symbol]).expect("doc");
    let ok = vec![
        note(Anchor::Symbol(symbol_id), Placement::Child),
        note(Anchor::Layer(inner), Placement::FrameElement { frame: 0 }),
    ];
    assert!(doc.clone().with_attachments(ok).is_ok());
    let flood: Vec<Attachment> = (0..=MAX_ATTACHMENTS)
        .map(|_| note(Anchor::Document, Placement::Child))
        .collect();
    assert_eq!(
        doc.with_attachments(flood).expect_err("too many"),
        vec![Diagnostic::TooManyAttachments]
    );
}

fn clip(id: flare_core::SoundId) -> Sound {
    Sound::new(id, "Clip", vec![1, 2, 3, 4], 44_100, 2, 2).expect("sound")
}

fn scene_with_cues(cues: Vec<SoundCue>) -> Scene {
    let layer = Layer::new(LayerId::new(), "Sound", vec![])
        .expect("layer")
        .with_cues(cues)
        .expect("cues");
    Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))
}

#[test]
fn cues_must_name_a_sound_in_the_library() {
    let id = flare_core::SoundId::new();
    let cues = vec![SoundCue {
        start: 3,
        sound: id,
    }];
    let document = build(vec![scene_with_cues(cues.clone())], vec![]).expect("valid");
    let diagnostics = document.clone().with_sounds(vec![]).expect_err("dangling");
    assert!(matches!(diagnostics[0], Diagnostic::DanglingSound { sound, .. } if sound == id));
    let ok = document
        .with_sounds(vec![clip(id)])
        .expect("with its sound");
    assert_eq!(ok.sounds().len(), 1);
    assert!(ok.sound(id).is_some());
    assert_eq!(ok.scenes()[0].timeline().layers()[0].cues(), cues);
}

#[test]
fn sound_limits_and_duplicate_ids_are_refused() {
    let id = flare_core::SoundId::new();
    let document = build(vec![scene_with_cues(vec![])], vec![]).expect("valid");
    let twice = document.clone().with_sounds(vec![clip(id), clip(id)]);
    assert!(
        twice
            .expect_err("duplicate")
            .contains(&Diagnostic::DuplicateSound(id))
    );
    let many: Vec<Sound> = (0..=MAX_SOUNDS)
        .map(|_| clip(flare_core::SoundId::new()))
        .collect();
    assert!(
        document
            .clone()
            .with_sounds(many)
            .expect_err("many")
            .contains(&Diagnostic::TooManySounds)
    );
    // Single sounds are bounded at construction.
    assert_eq!(
        Sound::new(id, "big", vec![0; MAX_SOUND_BYTES + 1], 44_100, 1, 1),
        Err(SoundError::TooLarge)
    );
    for (rate, channels, frames, data) in [
        (0, 1, 1, vec![1]),
        (1, 0, 1, vec![1]),
        (1, 1, 0, vec![1]),
        (1, 1, 1, vec![]),
    ] {
        assert_eq!(
            Sound::new(id, "x", data, rate, channels, frames),
            Err(SoundError::Empty)
        );
    }
}

#[test]
fn cues_are_kept_sorted_one_per_frame_and_bounded_and_survive_new_spans() {
    let (a, b) = (flare_core::SoundId::new(), flare_core::SoundId::new());
    let layer = Layer::new(LayerId::new(), "L", vec![])
        .expect("layer")
        .with_cues(vec![
            SoundCue { start: 9, sound: a },
            SoundCue { start: 2, sound: b },
            SoundCue { start: 9, sound: b },
        ])
        .expect("cues");
    let starts: Vec<u32> = layer.cues().iter().map(|c| c.start).collect();
    assert_eq!(starts, vec![2, 9], "sorted, one per frame");
    let respanned = layer
        .with_spans(vec![span(0, 3, Content::Empty)])
        .expect("spans");
    assert_eq!(
        respanned.cues(),
        layer.cues(),
        "replacing spans keeps the cues"
    );
    let too_many = vec![SoundCue { start: 0, sound: a }; 1];
    assert!(
        Layer::new(LayerId::new(), "L", vec![])
            .expect("layer")
            .with_cues(too_many)
            .is_ok()
    );
    let flood: Vec<SoundCue> = (0..=MAX_CUES_PER_LAYER as u32)
        .map(|start| SoundCue { start, sound: a })
        .collect();
    assert_eq!(
        Layer::new(LayerId::new(), "L", vec![])
            .expect("layer")
            .with_cues(flood)
            .err(),
        Some(SoundError::TooManyCues)
    );
}

#[test]
fn folders_must_sit_above_what_they_hold_and_hold_no_frames_themselves() {
    let layer = |name: &str| Layer::new(LayerId::new(), name, vec![]).expect("layer");
    let folder = layer("folder").with_folder(true);
    let child = layer("child").with_parent(Some(folder.id()));
    let scene = |layers: Vec<Layer>| Scene::new(SceneId::new(), "S", Timeline::new(layers));
    assert!(build(vec![scene(vec![folder.clone(), child.clone()])], vec![]).is_ok());
    // A parent below its child is refused: that is also what keeps folders from looping.
    let below = build(vec![scene(vec![child.clone(), folder.clone()])], vec![]);
    assert!(
        matches!(below, Err(d) if d.iter().any(|d| matches!(d, Diagnostic::BadFolderParent { .. })))
    );
    // A parent that is not a folder, and one that does not exist, are refused.
    let plain = layer("plain");
    let in_plain = layer("x").with_parent(Some(plain.id()));
    assert!(build(vec![scene(vec![plain, in_plain])], vec![]).is_err());
    let orphan = layer("orphan").with_parent(Some(LayerId::new()));
    assert!(build(vec![scene(vec![orphan])], vec![]).is_err());
    // A folder with frames of its own is refused.
    let with_frames = Layer::new(LayerId::new(), "f", vec![span(0, 2, Content::Empty)])
        .expect("layer")
        .with_folder(true);
    let refused = build(vec![scene(vec![with_frames])], vec![]);
    assert!(
        matches!(refused, Err(d) if d.iter().any(|d| matches!(d, Diagnostic::FolderWithContent(_))))
    );
    // Folders nest.
    let inner = layer("inner")
        .with_folder(true)
        .with_parent(Some(folder.id()));
    let leaf = layer("leaf").with_parent(Some(inner.id()));
    assert!(build(vec![scene(vec![folder, inner, leaf])], vec![]).is_ok());
}

#[test]
fn a_motion_guide_cannot_also_be_a_mask_or_a_folder() {
    let layer = |name: &str| Layer::new(LayerId::new(), name, vec![]).expect("layer");
    let scene = |l: Layer| Scene::new(SceneId::new(), "S", Timeline::new(vec![l]));
    assert!(build(vec![scene(layer("g").with_guide(true))], vec![]).is_ok());
    for bad in [
        layer("m").with_guide(true).with_mask(true),
        layer("f").with_guide(true).with_folder(true),
    ] {
        let refused = build(vec![scene(bad)], vec![]);
        assert!(
            matches!(refused, Err(d) if d.iter().any(|d| matches!(d, Diagnostic::GuideConflict(_))))
        );
    }
}
