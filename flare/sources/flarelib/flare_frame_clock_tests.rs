// Ported from Flare (github.com/Flare-Animate/Flare, flare/sources/flare/rust/core/src,
// commit a27136a; MIT OR Apache-2.0). Each test switches on Flare's timeline
// semantics (retained tween curves) for its thread.
//! Graphic playback must survive timeline edits independently of tween timing.
use crate::*;

fn animated(kind: SymbolKind, mode: LoopMode, tween: Tween) -> Document {
    let mut doc = Document::default();
    let mut leaf = Timeline::new("Leaf");
    leaf.layers[0].end = 7;
    for f in 0..7 {
        let mut el = Element::shape(
            "Frame",
            Shape::new(
                Path::rect(f as f32 * 3.0, 0.0, 5.0 + f as f32, 8.0),
                Some(Color::rgb(f as u8 * 30, 100, 200)),
                None,
            ),
        );
        el.matrix = Matrix::translate(0.0, f as f32);
        leaf.layers[0].keyframes.insert(f, Keyframe::with(vec![el]));
    }
    let leaf_id = doc.add_symbol("Leaf", kind, leaf);
    let mut wrapper = Timeline::new("Nested");
    wrapper.layers[0].end = 11;
    let mut nested = Element::instance("Nested", leaf_id, Matrix::IDENTITY);
    if let ElementKind::Instance(i) = &mut nested.kind {
        i.first_frame = 2;
    }
    wrapper.layers[0]
        .keyframes
        .insert(0, Keyframe::with(vec![Element::group(vec![nested])]));
    let id = doc.add_symbol("Wrapper", kind, wrapper);
    let mut el = Element::instance("Outer", id, Matrix::IDENTITY);
    if let ElementKind::Instance(i) = &mut el.kind {
        i.first_frame = 3;
        i.loop_mode = mode;
    }
    let mut last = el.clone();
    last.matrix = Matrix::translate(60.0, 20.0);
    let mut key = Keyframe::with(vec![Element::group(vec![el])]);
    key.tween = tween;
    if tween.is_motion() {
        let mut m = motion::Motion::new(pt(0.0, 0.0));
        m.set_pos(0, pt(0.0, 0.0));
        m.set_pos(11, pt(60.0, 20.0));
        m.path[0].h_out = pt(10.0, 35.0);
        m.path[1].h_in = pt(-25.0, 15.0);
        key.motion = Some(Box::new(m));
    }
    key.label = Some("start".into());
    key.script = Some("this.stop();".into());
    doc.scene.layers[0].end = 15;
    doc.scene.layers[0].keyframes.insert(0, key);
    if tween != Tween::None {
        doc.scene.layers[0]
            .keyframes
            .insert(12, Keyframe::with(vec![Element::group(vec![last])]));
    }
    doc
}

fn render(doc: &Document, frame: u32, global: u32) -> String {
    let mut out = vec![];
    evaluate(
        &EvalCtx::new(doc, global),
        &doc.scene,
        frame,
        &Matrix::IDENTITY,
        1.0,
        0,
        &mut out,
    );
    assert!(
        !out.is_empty(),
        "fixture must render visible nested artwork"
    );
    format!("{out:?}")
}

fn modes() -> [LoopMode; 3] {
    [LoopMode::Loop, LoopMode::PlayOnce, LoopMode::SingleFrame]
}

#[test]
fn nested_graphics_keep_phase_across_partial_copy_and_recopy() {
    crate::frames::set_retain_tween_curves(true);
    for kind in [
        SymbolKind::Graphic,
        SymbolKind::MovieClip,
        SymbolKind::Button,
    ] {
        for mode in modes() {
            for tween in [
                Tween::None,
                Tween::Classic {
                    ease: 80.0,
                    rotate_turns: 2,
                },
                Tween::Motion { ease: -70.0 },
            ] {
                let source = animated(kind, mode, tween);
                let mut dest = source.clone();
                let clip = frames::copy_frames(&source.scene.layers[0], 3, 8);
                frames::paste_frames(&mut dest.scene.layers[0], 20, &clip);
                for i in 0..6 {
                    assert_eq!(
                        render(&source, 3 + i, 41),
                        render(&dest, 20 + i, 41),
                        "{kind:?} {mode:?} {tween:?} frame {i}"
                    );
                }
                let clip = frames::copy_frames(&dest.scene.layers[0], 22, 24);
                frames::paste_frames(&mut dest.scene.layers[0], 30, &clip);
                for i in 0..3 {
                    assert_eq!(render(&source, 5 + i, 41), render(&dest, 30 + i, 41));
                }
                let bytes = serde_json::to_vec(&dest).unwrap();
                let restored: Document = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(dest, restored);
            }
        }
    }
}

#[test]
fn static_symbol_boundaries_survive_split_clear_remove_and_overwrite() {
    crate::frames::set_retain_tween_curves(true);
    for mode in modes() {
        let source = animated(SymbolKind::Graphic, mode, Tween::None);
        let mut split = source.clone();
        frames::convert_to_keyframes(&mut split.scene.layers[0], 3, 8, false);
        for f in 0..15 {
            assert_eq!(render(&source, f, 41), render(&split, f, 41));
        }
        for remove in [false, true] {
            let mut dest = source.clone();
            if remove {
                frames::remove_frames(&mut dest.scene.layers[0], 3, 8);
            } else {
                frames::clear_frames(&mut dest.scene.layers[0], 3, 8);
            }
            for f in 0..3 {
                assert_eq!(render(&source, f, 41), render(&dest, f, 41));
            }
            for f in 9..15 {
                assert_eq!(
                    render(&source, f, 41),
                    render(&dest, f - if remove { 6 } else { 0 }, 41)
                );
            }
        }
        let mut dest = source.clone();
        let clip = frames::FrameClip {
            len: 4,
            keyframes: vec![(0, Keyframe::blank())],
        };
        frames::paste_frames(&mut dest.scene.layers[0], 3, &clip);
        for f in 7..15 {
            assert_eq!(render(&source, f, 41), render(&dest, f, 41));
        }
    }
}

#[test]
fn baking_nested_tweens_samples_original_curve_and_playback() {
    crate::frames::set_retain_tween_curves(true);
    for mode in modes() {
        for tween in [
            Tween::Classic {
                ease: 85.0,
                rotate_turns: 2,
            },
            Tween::Motion { ease: -70.0 },
        ] {
            let source = animated(SymbolKind::Graphic, mode, tween);
            for (f0, f1) in [(0, 11), (3, 8)] {
                let mut dest = source.clone();
                frames::convert_to_frame_by_frame(&mut dest.scene.layers[0], f0, f1);
                for f in f0..=f1 {
                    assert_eq!(
                        render(&source, f, 41),
                        render(&dest, f, 41),
                        "{mode:?} {tween:?} frame {f}"
                    );
                    assert_eq!(dest.scene.layers[0].keyframes[&f].tween, Tween::None);
                }
                assert_eq!(
                    dest.scene.layers[0].keyframes[&0].label,
                    source.scene.layers[0].keyframes[&0].label
                );
                assert_eq!(
                    dest.scene.layers[0].keyframes[&0].script,
                    source.scene.layers[0].keyframes[&0].script
                );
                assert!(dest.scene.layers[0]
                    .keyframes
                    .range(1..=f1)
                    .all(|(_, k)| k.label.is_none() && k.script.is_none()));
            }
        }
    }
}

#[test]
fn reverse_static_graphics_runs_clock_backwards_and_twice_restores_playback() {
    crate::frames::set_retain_tween_curves(true);
    for mode in modes() {
        let source = animated(SymbolKind::Graphic, mode, Tween::None);
        for (f0, f1) in [(0, 14), (3, 8)] {
            let mut dest = source.clone();
            frames::reverse_frames(&mut dest.scene.layers[0], f0, f1);
            for f in 0..15 {
                let expected = if (f0..=f1).contains(&f) {
                    f0 + f1 - f
                } else {
                    f
                };
                assert_eq!(render(&source, expected, 41), render(&dest, f, 41));
            }
            frames::reverse_frames(&mut dest.scene.layers[0], f0, f1);
            for f in 0..15 {
                assert_eq!(render(&source, f, 41), render(&dest, f, 41));
            }
        }
    }
}

#[test]
fn legacy_keyframes_default_to_forward_clock_and_large_instance_offsets_do_not_overflow() {
    crate::frames::set_retain_tween_curves(true);
    let key = Keyframe::blank();
    let encoded = serde_json::to_string(&key).unwrap();
    assert!(!encoded.contains("playback_"));
    let restored: Keyframe = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored.playback_frame(7), 7);
    let mut doc = animated(SymbolKind::Graphic, LoopMode::Loop, Tween::None);
    let id = doc.library[0].id;
    let mut el = Element::instance("Large offset", id, Matrix::IDENTITY);
    if let ElementKind::Instance(i) = &mut el.kind {
        i.first_frame = u32::MAX;
    }
    doc.scene.layers[0].keyframes.get_mut(&0).unwrap().elements = vec![el];
    let large = render(&doc, 4, 41);
    if let ElementKind::Instance(i) =
        &mut doc.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].kind
    {
        i.first_frame = (u64::from(u32::MAX) % 7) as u32;
    }
    assert_eq!(large, render(&doc, 4, 41));
}

#[test]
fn reverse_tweens_preserve_nested_graphic_phase_and_endpoint_transforms() {
    crate::frames::set_retain_tween_curves(true);
    for mode in modes() {
        for tween in [
            Tween::Classic {
                ease: 80.0,
                rotate_turns: 2,
            },
            Tween::Shape { ease: -70.0 },
            Tween::Motion { ease: -70.0 },
        ] {
            let source = animated(SymbolKind::Graphic, mode, tween);
            let mut dest = source.clone();
            frames::reverse_frames(&mut dest.scene.layers[0], 0, 14);
            for f in 0..15 {
                assert_eq!(
                    render(&source, 14 - f, 41),
                    render(&dest, f, 41),
                    "{tween:?} {mode:?} frame {f}"
                );
            }
        }
    }
}

#[test]
fn animated_mask_uses_preserved_clock_after_split_and_copy() {
    crate::frames::set_retain_tween_curves(true);
    let mut source = animated(SymbolKind::Graphic, LoopMode::Loop, Tween::None);
    source.scene.layers[0].kind = LayerKind::Mask;
    let mut child = Layer::new("Masked artwork", 1);
    child.parent = Some(0);
    child.end = 30;
    child.keyframes.insert(
        0,
        Keyframe::with(vec![Element::shape(
            "Art",
            Shape::new(
                Path::rect(0.0, 0.0, 100.0, 100.0),
                Some(Color::rgb(255, 0, 0)),
                None,
            ),
        )]),
    );
    source.scene.layers.push(child);
    let mut dest = source.clone();
    frames::convert_to_keyframes(&mut dest.scene.layers[0], 3, 8, false);
    for f in 0..15 {
        assert_eq!(render(&source, f, 41), render(&dest, f, 41));
    }
    let clip = frames::copy_frames(&source.scene.layers[0], 3, 8);
    frames::paste_frames(&mut dest.scene.layers[0], 20, &clip);
    for f in 0..6 {
        assert_eq!(render(&source, 3 + f, 41), render(&dest, 20 + f, 41));
    }
}
