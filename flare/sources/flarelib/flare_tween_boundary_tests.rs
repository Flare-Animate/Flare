// Ported from Flare (github.com/Flare-Animate/Flare, flare/sources/flare/rust/core/src,
// commit a27136a; MIT OR Apache-2.0). Each test switches on Flare's timeline
// semantics (retained tween curves) for its thread.
use crate::*;

fn fixture(tween: Tween) -> Layer {
    let mut layer = Layer::new("Curve", 0);
    layer.end = 13;
    let mut from = Keyframe::with(vec![Element::shape(
        "Shape",
        Shape::new(
            Path::rect(0.0, 0.0, 30.0, 20.0),
            Some(Color::rgb(255, 0, 0)),
            None,
        ),
    )]);
    let mut to = Keyframe::with(vec![Element::shape(
        "Shape",
        Shape::new(
            Path::ellipse(5.0, 15.0, 15.0, 25.0),
            Some(Color::rgb(0, 0, 255)),
            None,
        ),
    )]);
    to.elements[0].matrix = Matrix::translate(60.0, 20.0).concat(&Matrix::rotate(0.4));
    to.elements[0].alpha = 0.4;
    from.tween = tween;
    from.label = Some("cue".into());
    from.script = Some("stop();".into());
    from.shape_hints = vec![pt(0.0, 0.0), pt(30.0, 20.0)];
    to.shape_hints = vec![pt(5.0, -10.0), pt(20.0, 15.0)];
    from.ease_curve = Some(motion::Ease::preset(
        motion::EaseFamily::Cubic,
        motion::EaseDir::InOut,
    ));
    if tween.is_motion() {
        let mut m = motion::Motion::new(pt(0.0, 0.0));
        m.set_pos(0, pt(0.0, 0.0));
        m.set_pos(11, pt(60.0, 20.0));
        m.path[0].h_out = pt(10.0, 30.0);
        m.path[1].h_in = pt(-20.0, 25.0);
        m.rotate_turns = 2;
        from.motion = Some(Box::new(m));
    }
    layer.keyframes.insert(0, from);
    layer.keyframes.insert(12, to);
    layer
}

fn tweens() -> [Tween; 3] {
    [
        Tween::Classic {
            ease: 80.0,
            rotate_turns: 2,
        },
        Tween::Shape { ease: -70.0 },
        Tween::Motion { ease: 60.0 },
    ]
}

fn clipped(source: &Layer, len: u32) -> Layer {
    let clip = frames::copy_frames(source, 0, len - 1);
    assert_eq!(clip.keyframes.len(), 1, "a curve remains one authored span");
    assert_eq!(clip.keyframes[0].1.tween, source.keyframes[&0].tween);
    assert!(clip.keyframes[0].1.tween_boundary.is_some());
    let mut dest = Layer::new("Pasted", 1);
    dest.end = 1;
    frames::paste_frames(&mut dest, 0, &clip);
    dest
}

#[test]
fn right_trim_preserves_curves_poses_metadata_and_repeat_copy() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        let source = fixture(tween);
        for len in [1, 4, 8, 12] {
            // A complete motion span needs no retained endpoint.
            if tween.is_motion() && len == 12 {
                continue;
            }
            let dest = clipped(&source, len);
            let key = &dest.keyframes[&0];
            assert_eq!(key.motion, source.keyframes[&0].motion);
            assert_eq!(key.ease_curve, source.keyframes[&0].ease_curve);
            assert_eq!(key.shape_hints, source.keyframes[&0].shape_hints);
            assert_eq!(key.label.as_deref(), Some("cue"));
            assert_eq!(key.script.as_deref(), Some("stop();"));
            for f in 0..len {
                assert_eq!(
                    layer_elements_at(&source, f),
                    layer_elements_at(&dest, f),
                    "{tween:?} frame {f}"
                );
            }
            let again = clipped(&dest, len);
            assert_eq!(again.keyframes, dest.keyframes);
            let json = serde_json::to_string(&dest).unwrap();
            assert_eq!(serde_json::from_str::<Layer>(&json).unwrap(), dest);
        }
    }
}

#[test]
fn right_trim_retimes_without_mutating_authored_curve() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        let source = fixture(tween);
        let mut dest = clipped(&source, 6);
        let original_key = dest.keyframes[&0].clone();
        frames::set_last_span_end(&mut dest, 12);
        assert_eq!(dest.keyframes[&0], original_key);
        for f in 0..6 {
            assert_eq!(
                layer_elements_at(&dest, 2 * f).unwrap().0,
                layer_elements_at(&source, f).unwrap().0
            );
        }
        let smaller = clipped(&dest, 4);
        for f in 0..4 {
            assert_eq!(layer_elements_at(&smaller, f), layer_elements_at(&dest, f));
        }
    }
}

#[test]
fn retained_curves_remain_live_when_ease_path_or_start_artwork_is_edited() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        let mut source = fixture(tween);
        let mut dest = clipped(&source, 8);
        for l in [&mut source, &mut dest] {
            let key = l.keyframes.get_mut(&0).unwrap();
            key.ease_curve = None;
            key.elements[0].matrix.tx = 15.0;
            key.elements[0].alpha = 0.7;
            if let Some(m) = &mut key.motion {
                m.path[0].h_out = pt(25.0, -30.0);
            }
        }
        for f in 0..8 {
            assert_eq!(layer_elements_at(&dest, f), layer_elements_at(&source, f));
        }
    }
}

#[test]
fn retained_interval_edits_preserve_curves_and_unedited_poses() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        let original = clipped(&fixture(tween), 8);
        let mut split = original.clone();
        frames::convert_to_keyframes(&mut split, 3, 3, false);
        assert_eq!(split.keyframes.len(), 2);
        assert!(split.keyframes.values().all(|k| k.tween == tween));
        for f in 0..8 {
            assert_eq!(
                layer_elements_at(&split, f),
                layer_elements_at(&original, f)
            );
        }
        let mut reversed = original.clone();
        frames::reverse_frames(&mut reversed, 2, 5);
        assert_eq!(reversed.keyframes.len(), 3);
        assert!(reversed.keyframes.values().all(|k| k.tween == tween));
        for f in 0..8 {
            let source_frame = if (2..=5).contains(&f) { 7 - f } else { f };
            assert_eq!(
                layer_elements_at(&reversed, f),
                layer_elements_at(&original, source_frame)
            );
        }
        let mut overwritten = original.clone();
        frames::paste_frames(
            &mut overwritten,
            3,
            &frames::FrameClip {
                len: 2,
                keyframes: vec![(0, Keyframe::blank())],
            },
        );
        for f in [0, 1, 2, 5, 6, 7] {
            assert_eq!(
                layer_elements_at(&overwritten, f),
                layer_elements_at(&original, f)
            );
        }
    }
}

#[test]
fn interior_intervals_recopy_resize_and_keep_live_authored_properties() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        let mut source = fixture(tween);
        let clip = frames::copy_frames(&source, 3, 8);
        assert_eq!(clip.keyframes.len(), 1);
        let mut dest = Layer::new("Interval", 1);
        dest.end = 1;
        frames::paste_frames(&mut dest, 0, &clip);
        assert!(dest.keyframes[&0].label.is_none());
        assert!(dest.keyframes[&0].script.is_none());
        // The authored source/ease/path remains live even after a left trim.
        for layer in [&mut source, &mut dest] {
            let key = layer.keyframes.get_mut(&0).unwrap();
            key.ease_curve = Some(motion::Ease::preset(
                motion::EaseFamily::Bounce,
                motion::EaseDir::Out,
            ));
            key.elements[0].matrix.ty = 17.0;
            key.elements[0].alpha = 0.8;
            if let Some(m) = &mut key.motion {
                m.path[0].h_out = pt(-30.0, 40.0);
            }
        }
        for f in 0..6 {
            assert_eq!(
                layer_elements_at(&dest, f),
                layer_elements_at(&source, f + 3)
            );
        }
        let again = frames::copy_frames(&dest, 1, 3);
        assert_eq!(again.keyframes.len(), 1);
        let mut smaller = Layer::new("Again", 2);
        smaller.end = 1;
        frames::paste_frames(&mut smaller, 0, &again);
        for f in 0..3 {
            assert_eq!(
                layer_elements_at(&smaller, f),
                layer_elements_at(&source, f + 4)
            );
        }
        frames::set_last_span_end(&mut dest, 12);
        for f in 0..6 {
            assert_eq!(
                layer_elements_at(&dest, f * 2).unwrap().0,
                layer_elements_at(&source, f + 3).unwrap().0
            );
        }
        assert_eq!(
            serde_json::from_str::<Layer>(&serde_json::to_string(&dest).unwrap()).unwrap(),
            dest
        );
    }
}

#[test]
fn partial_bake_clear_remove_and_overwrite_preserve_nonlinear_exterior() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        let source = fixture(tween);
        for operation in 0..4 {
            let mut dest = source.clone();
            match operation {
                0 => frames::convert_to_frame_by_frame(&mut dest, 3, 7),
                1 => frames::clear_frames(&mut dest, 3, 7),
                2 => frames::remove_frames(&mut dest, 3, 7),
                _ => frames::paste_frames(
                    &mut dest,
                    3,
                    &frames::FrameClip {
                        len: 5,
                        keyframes: vec![(0, Keyframe::blank())],
                    },
                ),
            }
            assert_eq!(dest.keyframes[&0].tween, tween);
            let tail = if operation == 2 { 3 } else { 8 };
            assert_eq!(dest.keyframes[&tail].tween, tween);
            for f in 0..dest.end {
                if operation != 2 && (3..=7).contains(&f) && operation != 0 {
                    assert!(layer_elements_at(&dest, f).unwrap().0.is_empty());
                    continue;
                }
                let original = if operation == 2 && f >= 3 { f + 5 } else { f };
                assert_eq!(
                    layer_elements_at(&dest, f),
                    layer_elements_at(&source, original),
                    "{tween:?} op {operation} frame {f}"
                );
            }
            assert_eq!(dest.keyframes[&0].label, source.keyframes[&0].label);
            assert!(dest.keyframes[&tail].label.is_none());
        }
    }
}

#[test]
fn reverse_retains_exact_curves_metadata_and_supports_recopy_and_double_reverse() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        for (f0, f1) in [(0, 12), (2, 8), (0, 11), (10, 12)] {
            let source = fixture(tween);
            let mut dest = source.clone();
            frames::reverse_frames(&mut dest, f0, f1);
            assert!(
                dest.keyframes.len() <= 6,
                "reversal must not bake the curve"
            );
            for f in 0..dest.end {
                let original = if (f0..=f1).contains(&f) {
                    f0 + f1 - f
                } else {
                    f
                };
                assert_eq!(
                    layer_elements_at(&dest, f),
                    layer_elements_at(&source, original),
                    "{tween:?} [{f0}, {f1}] frame {f}"
                );
                if source
                    .keyframes
                    .get(&original)
                    .is_some_and(|k| k.label.is_some())
                {
                    assert_eq!(dest.keyframes[&f].label, source.keyframes[&original].label);
                    assert_eq!(
                        dest.keyframes[&f].script,
                        source.keyframes[&original].script
                    );
                }
            }
            let clip = frames::copy_frames(&dest, 3, 7);
            let mut pasted = Layer::new("Reverse copy", 1);
            pasted.end = 1;
            frames::paste_frames(&mut pasted, 0, &clip);
            for f in 0..5 {
                assert_eq!(
                    layer_elements_at(&pasted, f),
                    layer_elements_at(&dest, f + 3)
                );
            }
            frames::reverse_frames(&mut dest, f0, f1);
            for f in 0..dest.end {
                assert_eq!(layer_elements_at(&dest, f), layer_elements_at(&source, f));
            }
        }
    }
}

#[test]
fn clearing_split_boundaries_rejoins_matching_authored_intervals() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        for reverse in [false, true] {
            let mut source = clipped(&fixture(tween), 10);
            if reverse {
                source.keyframes.get_mut(&0).unwrap().label = None;
                source.keyframes.get_mut(&0).unwrap().script = None;
                frames::reverse_frames(&mut source, 0, 9);
            }
            let mut dest = source.clone();
            frames::convert_to_keyframes(&mut dest, 3, 6, false);
            assert_eq!(dest.keyframes.len(), 5);
            frames::clear_keyframes(&mut dest, 3, 6);
            assert_eq!(dest.keyframes.len(), 1);
            for f in 0..10 {
                assert_eq!(layer_elements_at(&source, f), layer_elements_at(&dest, f));
            }
        }
    }
}

#[test]
fn retained_pose_edits_are_sparse_live_and_preserve_prefix_and_endpoint() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        for reversed in [false, true] {
            let mut layer = clipped(&fixture(tween), 10);
            if reversed {
                frames::reverse_frames(&mut layer, 0, 9);
            }
            let mut doc = Document::default();
            doc.scene.layers[0] = layer.clone();
            let mut editor = Editor::new(doc);
            editor.set_frame(4);
            editor.selection = vec![Selection {
                layer: 0,
                element: 0,
            }];
            let before = editor.doc.clone();
            let old = editor.selected_element().unwrap();
            let sampled = editor.modify_selected("Pose", |el| {
                el.matrix.tx += 15.0;
                el.matrix.ty -= 6.0;
                el.alpha = 0.3;
                if let ElementKind::Shape(s) = &mut el.kind {
                    s.fill = Some(Color::rgb(20, 100, 70));
                }
            });
            assert!(!sampled);
            let edited = editor.selected_element().unwrap();
            assert!((edited.matrix.tx - old.matrix.tx - 15.0).abs() < 0.001);
            assert!((edited.alpha - 0.3).abs() < 0.001);
            let key = &editor.doc.scene.layers[0].keyframes[&4];
            assert_eq!(key.tween, tween);
            assert_eq!(key.tween_boundary.as_ref().unwrap().pose_edits.len(), 1);
            assert_eq!(
                key.elements,
                layer.keyframes[&layer.span_at(4).unwrap().0].elements
            );
            for f in 0..4 {
                assert_eq!(
                    layer_elements_at(&editor.doc.scene.layers[0], f),
                    layer_elements_at(&layer, f)
                );
            }
            // The source curve remains live beneath the correction.
            let original_tx = layer_elements_at(&editor.doc.scene.layers[0], 6).unwrap().0[0]
                .matrix
                .tx;
            editor.doc.scene.layers[0]
                .keyframes
                .get_mut(&4)
                .unwrap()
                .ease_curve = None;
            assert_ne!(
                layer_elements_at(&editor.doc.scene.layers[0], 6).unwrap().0[0]
                    .matrix
                    .tx,
                original_tx
            );
            editor.undo();
            assert_eq!(editor.doc, before);
        }
    }
}

#[test]
fn edited_intervals_survive_copy_reverse_resize_and_serialization() {
    crate::frames::set_retain_tween_curves(true);
    for tween in tweens() {
        let mut doc = Document::default();
        doc.scene.layers[0] = clipped(&fixture(tween), 10);
        let mut editor = Editor::new(doc);
        editor.set_frame(3);
        editor.selection = vec![Selection {
            layer: 0,
            element: 0,
        }];
        assert!(!editor.modify_selected("Move", |e| e.matrix.tx += 20.0));
        let source = editor.doc.scene.layers[0].clone();
        let clip = frames::copy_frames(&source, 4, 8);
        let mut dest = Layer::new("Copy", 1);
        dest.end = 1;
        frames::paste_frames(&mut dest, 0, &clip);
        assert_eq!(dest.keyframes.len(), 1);
        for f in 0..5 {
            assert_eq!(
                layer_elements_at(&dest, f),
                layer_elements_at(&source, f + 4)
            );
        }
        frames::reverse_frames(&mut dest, 0, 4);
        for f in 0..5 {
            assert_eq!(
                layer_elements_at(&dest, f),
                layer_elements_at(&source, 8 - f)
            );
        }
        frames::reverse_frames(&mut dest, 0, 4);
        frames::set_last_span_end(&mut dest, 10);
        for f in 0..5 {
            assert_eq!(
                layer_elements_at(&dest, 2 * f).unwrap().0,
                layer_elements_at(&source, f + 4).unwrap().0
            );
        }
        let json = serde_json::to_string(&dest).unwrap();
        assert_eq!(serde_json::from_str::<Layer>(&json).unwrap(), dest);
    }
}

#[test]
fn continuous_retained_pose_edits_coalesce_and_cancel_in_one_undo() {
    crate::frames::set_retain_tween_curves(true);
    let mut doc = Document::default();
    doc.scene.layers[0] = clipped(&fixture(tweens()[0]), 10);
    let original = doc.clone();
    let mut editor = Editor::new(doc);
    editor.set_frame(3);
    editor.selection = vec![Selection {
        layer: 0,
        element: 0,
    }];
    let x = editor.selected_element().unwrap().matrix.tx;
    editor.begin_drag();
    for _ in 0..20 {
        assert!(!editor.modify_selected_continuous(|e| e.matrix.tx += 0.5));
    }
    assert!((editor.selected_element().unwrap().matrix.tx - x - 10.0).abs() < 0.001);
    assert_eq!(editor.doc.scene.layers[0].keyframes.len(), 2);
    assert_eq!(
        editor.doc.scene.layers[0].keyframes[&3]
            .tween_boundary
            .as_ref()
            .unwrap()
            .pose_edits
            .len(),
        1
    );
    assert_eq!(editor.undo_depth(), 1);
    editor.undo();
    assert_eq!(editor.doc, original);
}

#[test]
fn unsupported_retained_topology_edit_is_reported_and_preserves_other_poses() {
    crate::frames::set_retain_tween_curves(true);
    let mut doc = Document::default();
    doc.scene.layers[0] = clipped(&fixture(tweens()[1]), 10);
    let source = doc.clone();
    let mut editor = Editor::new(doc);
    editor.set_frame(3);
    editor.selection = vec![Selection {
        layer: 0,
        element: 0,
    }];
    assert!(editor.modify_selected("Topology", |e| {
        if let ElementKind::Shape(s) = &mut e.kind {
            s.path = Path::rect(0.0, 0.0, 10.0, 10.0);
        }
    }));
    for f in [0, 1, 2, 4, 5, 6, 7, 8, 9] {
        assert_eq!(
            layer_elements_at(&editor.doc.scene.layers[0], f),
            layer_elements_at(&source.scene.layers[0], f)
        );
    }
}

#[test]
fn retained_pose_corrections_survive_parent_space_conversion_and_undo() {
    crate::frames::set_retain_tween_curves(true);
    let mut doc = Document::default();
    let mut parent = Layer::new("Parent", 0);
    let mut el = fixture(tweens()[0]).keyframes[&0].elements[0].clone();
    el.matrix = Matrix::translate(30.0, 20.0).concat(&Matrix::scale(2.0, 2.0));
    parent.keyframes.insert(0, Keyframe::with(vec![el]));
    parent.end = 13;
    doc.scene.layers = vec![parent, clipped(&fixture(tweens()[0]), 10)];
    let mut editor = Editor::new(doc);
    let selection = Selection {
        layer: 1,
        element: 0,
    };
    editor.set_frame(3);
    editor.selection = vec![selection];
    assert!(!editor.modify_selected("Move", |el| el.matrix.tx += 20.0));
    let original = editor.doc.clone();
    let before: Vec<_> = (0..10)
        .map(|f| {
            editor.set_frame(f);
            editor.element_at(selection).unwrap()
        })
        .collect();
    editor.set_anim_parent(1, Some(0));
    for f in 0..10 {
        editor.set_frame(f);
        let after = editor.element_at(selection).unwrap();
        assert!((after.matrix.tx - before[f as usize].matrix.tx).abs() < 0.001);
        assert!((after.matrix.ty - before[f as usize].matrix.ty).abs() < 0.001);
    }
    editor.undo();
    assert_eq!(editor.doc, original);
}

#[test]
fn retained_pose_object_and_library_deletion_keep_other_corrections_attached() {
    crate::frames::set_retain_tween_curves(true);
    for library in [false, true] {
        let mut doc = Document::default();
        let a = doc.add_symbol("A", SymbolKind::Graphic, Timeline::new("A"));
        let b = doc.add_symbol("B", SymbolKind::Graphic, Timeline::new("B"));
        let mut layer = fixture(tweens()[0]);
        for key in layer.keyframes.values_mut() {
            key.elements = vec![
                Element::instance("A", a, Matrix::IDENTITY),
                Element::instance("B", b, Matrix::translate(20.0, 0.0)),
            ];
        }
        doc.scene.layers[0] = clipped(&layer, 10);
        let mut editor = Editor::new(doc);
        editor.selection = vec![Selection {
            layer: 0,
            element: 1,
        }];
        assert!(!editor.modify_selected("Move B", |e| e.matrix.tx += 7.0));
        let before = editor.doc.clone();
        let visible = editor.selected_element().unwrap();
        if library {
            editor.delete_symbol(a);
        } else {
            editor.selection = vec![Selection {
                layer: 0,
                element: 0,
            }];
            editor.delete_selected();
        }
        let key = &editor.doc.scene.layers[0].keyframes[&0];
        assert_eq!(key.elements.len(), 1);
        assert_eq!(
            key.tween_boundary.as_ref().unwrap().pose_edits[0].element,
            0
        );
        assert_eq!(
            editor
                .element_at(Selection {
                    layer: 0,
                    element: 0
                })
                .unwrap(),
            visible
        );
        editor.undo();
        assert_eq!(editor.doc, before);
    }
}
