//! Long random edit sequences. The point is not any one outcome but the invariants: no
//! sequence of operations, valid or not, can leave an invalid document; a failed batch
//! changes nothing; and undoing everything restores the starting document exactly.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Stage, Timeline};
use flare_edit::{Command, EditorSession, LayerChange, frames};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n.max(1)
    }
}

fn start() -> (Document, SceneId) {
    let scene_id = SceneId::new();
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(scene_id, "S", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    (
        Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc"),
        scene_id,
    )
}

fn rect_span(rng: &mut Rng) -> FrameSpan {
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, Rgba8::WHITE).expect("rect");
    let duration = NonZeroU32::new(1 + rng.below(8) as u32).expect("nonzero");
    FrameSpan::new(rng.below(40) as u32, duration, Content::Shape(shape))
}

/// A random operation as a command batch (possibly one that will be rejected).
fn random_batch(rng: &mut Rng, session: &EditorSession, scene: SceneId) -> Vec<Command> {
    let layers = session.document().scenes()[0].timeline().layers();
    let pick = |rng: &mut Rng| layers[rng.below(layers.len())].clone();
    let frame = rng.below(45) as u32;
    match rng.below(13) {
        0 => vec![Command::AddLayer {
            scene,
            layer: Layer::new(LayerId::new(), "n", vec![]).expect("layer"),
        }],
        1 => {
            let layer = pick(rng).id();
            vec![Command::InsertSpan {
                scene,
                layer,
                span: rect_span(rng),
            }]
        }
        2 => {
            let layer = pick(rng).id();
            vec![Command::RemoveSpan {
                scene,
                layer,
                start: rng.below(40) as u32,
            }]
        }
        3 => frames::insert_frame(scene, &pick(rng), frame).unwrap_or_default(),
        4 => frames::remove_frame(scene, &pick(rng), frame).unwrap_or_default(),
        5 => frames::insert_keyframe(scene, &pick(rng), frame).unwrap_or_default(),
        6 => frames::insert_blank_keyframe(scene, &pick(rng), frame).unwrap_or_default(),
        7 => frames::clear_keyframe(scene, &pick(rng), frame).unwrap_or_default(),
        8 => {
            let layer = pick(rng).id();
            let change = match rng.below(3) {
                0 => LayerChange::Visible(rng.below(2) == 0),
                1 => LayerChange::Locked(rng.below(2) == 0),
                _ => LayerChange::Name(format!("layer {}", rng.below(100))),
            };
            vec![Command::UpdateLayer {
                scene,
                layer,
                change,
            }]
        }
        9 => {
            let layer = pick(rng).id();
            vec![Command::MoveLayer {
                scene,
                layer,
                index: rng.below(8),
            }]
        }
        10 if layers.len() > 1 => vec![Command::RemoveLayer {
            scene,
            layer: pick(rng).id(),
        }],
        11 => vec![Command::RemoveLayer {
            scene,
            layer: LayerId::new(),
        }], // always invalid
        _ => match Stage::new(
            rng.below(300) as u32,
            1 + rng.below(300) as u32,
            Rgba8::WHITE,
        ) {
            Ok(stage) => vec![Command::SetStage(stage)],
            Err(_) => Vec::new(),
        },
    }
}

fn assert_valid(document: &Document) {
    // Rebuilding through the validating constructor re-checks every invariant.
    let rebuilt = Document::new(
        document.id(),
        document.frame_rate(),
        document.scenes().to_vec(),
        document.symbols().to_vec(),
    );
    assert!(rebuilt.is_ok(), "invalid document: {:?}", rebuilt.err());
    for layer in document.scenes()[0].timeline().layers() {
        for pair in layer.spans().windows(2) {
            assert!(
                pair[0].end() <= u64::from(pair[1].start()),
                "overlapping spans survived"
            );
        }
    }
}

#[test]
fn random_edit_sequences_never_break_the_document_and_undo_restores_the_start() {
    for seed in 1..=6_u64 {
        let (initial, scene) = start();
        let mut session = EditorSession::with_history_depth(initial.clone(), 100_000);
        let mut rng = Rng(seed * 0x9E37_79B9);
        let (mut applied, mut rejected) = (0_u32, 0_u32);

        for _ in 0..2_500 {
            if rng.below(9) == 0 {
                if rng.below(2) == 0 {
                    session.undo();
                } else {
                    session.redo();
                }
                assert_valid(session.document());
                continue;
            }
            let batch = random_batch(&mut rng, &session, scene);
            let before = session.document().clone();
            let revision = session.revision();
            match session.apply(batch) {
                Ok(()) => applied += 1,
                Err(_) => {
                    rejected += 1;
                    assert_eq!(
                        session.document(),
                        &before,
                        "a rejected batch changed the document"
                    );
                    assert_eq!(
                        session.revision(),
                        revision,
                        "a rejected batch bumped the revision"
                    );
                }
            }
            assert_valid(session.document());
        }
        assert!(
            applied > 300 && rejected > 20,
            "seed {seed}: {applied} applied, {rejected} rejected"
        );

        while session.undo() {}
        assert_eq!(
            session.document(),
            &initial,
            "seed {seed}: undoing everything restores the start"
        );
        assert!(!session.is_dirty());
        // Redo everything and the document is valid again.
        while session.redo() {}
        assert_valid(session.document());
    }
}

#[test]
fn history_depth_bounds_memory_but_never_corrupts() {
    let (initial, scene) = start();
    let mut session = EditorSession::with_history_depth(initial, 5);
    let mut rng = Rng(77);
    for _ in 0..500 {
        let batch = random_batch(&mut rng, &session, scene);
        let _ = session.apply(batch);
        assert_valid(session.document());
    }
    let mut steps = 0;
    while session.undo() {
        steps += 1;
        assert_valid(session.document());
    }
    assert!(steps <= 5, "history is bounded: undid {steps} steps");
}
