use super::layer_types::LayerType;
use super::timeline_tests::{app, click, drag, fixture, roundtrip};
use super::*;
use ac_core::{ElementKind, Layer, LayerKind, Selection};
use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};

fn folder_fixture() -> ac_core::Document {
    let mut doc = fixture();
    let mut folder = Layer::new("Folder", 0);
    folder.kind = LayerKind::Folder;
    folder.collapsed = true;
    doc.scene.layers.insert(0, folder);
    doc.scene.layers[1].parent = Some(0);
    doc.scene.layers[2].parent = Some(0);
    doc.scene.layers[2].anim_parent = Some(1);
    doc.scene.layers.push(Layer::new("Other", 3));
    doc
}

#[test]
fn collapsed_folder_frame_commands_copy_contents_across_documents_and_cut_atomically() {
    let (ctx, mut a) = app();
    a.replace(folder_fixture(), None, false);
    let source = a.editor.doc.clone();
    a.select_layer_rows(0, false, false);
    assert!(a.enabled(Command::CopyFrames));
    a.dispatch(Command::CopyFrames, &ctx);
    assert_eq!(a.editor.undo_depth(), 0);
    let mut target = document::blank("Target", "Destination");
    target.scene.layers[0].kind = LayerKind::Folder;
    a.replace(target.clone(), None, false);
    a.select_layer_rows(0, false, false);
    a.dispatch(Command::PasteFrames, &ctx);
    assert_eq!(a.editor.timeline().layers.len(), 3);
    assert_eq!(
        a.editor.timeline().layers[1].keyframes,
        source.scene.layers[1].keyframes
    );
    assert_eq!(a.editor.timeline().layers[1].parent, Some(0));
    assert_eq!(a.editor.timeline().layers[2].anim_parent, Some(1));
    assert_eq!(a.editor.undo_depth(), 1);
    roundtrip(&a);
    a.dispatch(Command::Undo, &ctx);
    assert_eq!(a.editor.doc, target);
    a.replace(source.clone(), None, false);
    a.select_layer_rows(0, false, false);
    a.dispatch(Command::CutFrames, &ctx);
    assert_eq!(a.editor.timeline().layers.len(), 4);
    assert!(a.editor.timeline().layers[1].keyframes[&0]
        .elements
        .is_empty());
    assert_eq!(a.editor.timeline().layers[3], source.scene.layers[3]);
    a.dispatch(Command::Undo, &ctx);
    assert_eq!(a.editor.doc, source);
    a.editor.timeline_mut().layers[1].locked = true;
    a.select_layer_rows(0, false, false);
    assert!(!a.enabled(Command::CutFrames));
    assert!(a.enabled(Command::CopyFrames));
}

#[test]
fn layer_role_batch_link_unlink_reorders_graphs_and_undo_restores_them() {
    for kind in [LayerKind::Mask, LayerKind::Guide] {
        let (ctx, mut a) = app();
        let role = if kind == LayerKind::Mask {
            LayerType::Masked
        } else {
            LayerType::Guided
        };
        let mut doc = folder_fixture();
        doc.scene.layers[0].kind = kind;
        doc.scene.layers[1].parent = None;
        doc.scene.layers[2].parent = None;
        // An unrelated intervening root must stay outside the linked subtree.
        doc.scene.layers.swap(1, 3);
        doc.scene.layers[2].anim_parent = Some(3);
        a.replace(doc.clone(), None, false);
        a.set_layer_type(&[2, 3], role);
        assert_eq!(a.editor.undo_depth(), 1);
        assert_eq!(a.layer_type(1), role);
        assert_eq!(a.layer_type(2), role);
        assert_eq!(a.editor.timeline().layers[1].anim_parent, Some(2));
        assert_eq!(a.editor.timeline().layers[3].name, "Other");
        roundtrip(&a);
        a.set_layer_type(&[1, 2], LayerType::Base(LayerKind::Normal));
        assert!(a.editor.timeline().layers[1].parent.is_none());
        assert!(a.editor.timeline().layers[2].parent.is_none());
        roundtrip(&a);
        a.dispatch(Command::Undo, &ctx);
        a.dispatch(Command::Undo, &ctx);
        assert_eq!(a.editor.doc, doc);
        a.editor.timeline_mut().layers[0].locked = true;
        assert!(a.planned_layer_type(&[2, 3], role).is_none());
        let protected = a.editor.doc.clone();
        a.set_layer_type(&[2, 3], role);
        assert_eq!(a.editor.doc, protected);
    }
}

#[test]
fn dragging_folder_to_trash_cancels_or_deletes_whole_subtree_in_both_languages() {
    for locale in [Locale::English, Locale::French] {
        let mut h = Harness::builder()
            .with_size(vec2(1700.0, 1100.0))
            .build_eframe(|cc| {
                let mut a = Studio::with_preferences(
                    &cc.egui_ctx,
                    Preferences {
                        locale,
                        ..Default::default()
                    },
                );
                a.replace(folder_fixture(), None, false);
                a
            });
        h.run();
        let before = h.state().editor.doc.clone();
        let start = h.get_by_label("Folder").rect().center();
        click(&mut h, start, egui::Modifiers::NONE);
        let end = h
            .get_by_label(&h.state().i18n.text("action-trash-layers"))
            .rect()
            .center();
        drag(&mut h, start, end, true, egui::Modifiers::NONE);
        assert_eq!(h.state().editor.doc, before);
        drag(&mut h, start, end, false, egui::Modifiers::NONE);
        assert_eq!(h.state().editor.timeline().layers.len(), 1);
        assert_eq!(h.state().editor.timeline().layers[0].name, "Other");
        assert_eq!(h.state().editor.undo_depth(), 1);
        roundtrip(h.state());
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        h.run();
        assert_eq!(h.state().editor.doc, before);
    }
}

#[test]
fn thumbnail_modes_preserve_motion_context_and_leave_document_untouched() {
    let (_, mut a) = app();
    let mut first = a.editor.timeline().layers[0].keyframes[&0].clone();
    first.elements[0].matrix.tx = 250.0;
    a.editor.timeline_mut().layers[0].keyframes.insert(4, first);
    let before = a.editor.doc.clone();
    let cropped0 = a.thumbnail_pixels(0, 0, false).unwrap();
    let cropped4 = a.thumbnail_pixels(0, 4, false).unwrap();
    assert_eq!(
        cropped0.data(),
        cropped4.data(),
        "content fit keeps identical artwork at the same thumbnail scale"
    );
    assert_ne!(
        a.thumbnail_pixels(0, 0, true).unwrap().data(),
        a.thumbnail_pixels(0, 4, true).unwrap().data(),
        "context retains stage placement"
    );
    assert_eq!(a.editor.doc, before);
    assert_eq!(a.editor.undo_depth(), 0);
    for mode in [
        FrameDisplay::Cells,
        FrameDisplay::Preview,
        FrameDisplay::Context,
    ] {
        a.prefs.frame_display = mode;
        let json = serde_json::to_string(&a.prefs).unwrap();
        let prefs: Preferences = serde_json::from_str(&json).unwrap();
        assert!(prefs.frame_display == mode);
    }
}

#[test]
fn protection_matrix_rejects_stale_inspector_edits_and_ends_pending_gestures() {
    for kind in [
        LayerKind::Normal,
        LayerKind::Folder,
        LayerKind::Mask,
        LayerKind::Guide,
        LayerKind::Camera,
    ] {
        for protection in 0..4 {
            let (_, mut a) = app();
            let mut doc = folder_fixture();
            doc.scene.layers[1].kind = kind;
            match protection {
                0 => doc.scene.layers[1].locked = true,
                1 => doc.scene.layers[1].visible = false,
                2 => doc.scene.layers[0].locked = true,
                _ => doc.scene.layers[0].visible = false,
            }
            a.replace(doc.clone(), None, false);
            a.editor.selection = vec![Selection {
                layer: 1,
                element: 0,
            }];
            a.transform_fields = [200.0, 300.0, 0.2];
            a.fill = Color32::RED;
            a.apply_selection_transform();
            a.apply_selection_fill();
            assert_eq!(a.editor.doc, doc);
            assert_eq!(a.editor.undo_depth(), 0);
        }
    }
    let (ctx, mut a) = app();
    let before = a.editor.doc.clone();
    a.editor.selection = vec![Selection {
        layer: 0,
        element: 0,
    }];
    a.editor.begin_drag();
    a.editor.modify_selected_continuous(|e| e.matrix.tx += 10.0);
    a.drag = Some(Drag {
        start: pos2(0.0, 0.0),
        last: vec2(10.0, 0.0),
        pan: egui::Vec2::ZERO,
        tool: Tool::Select,
        moved: true,
    });
    a.set_layer_flags(&[0], true, true);
    assert!(a.drag.is_none());
    assert!(a.editor.selection.is_empty());
    assert_eq!(a.editor.undo_depth(), 2);
    a.dispatch(Command::Undo, &ctx);
    a.dispatch(Command::Undo, &ctx);
    assert_eq!(a.editor.doc, before);
}

#[test]
fn first_all_and_future_frame_edges_preserve_metadata_and_history() {
    for command in [
        Command::InsertFrame,
        Command::RemoveFrame,
        Command::Keyframe,
        Command::BlankKeyframe,
        Command::ClearFrames,
        Command::ClearKeyframe,
    ] {
        for (start, end) in [(0, 0), (0, 11), (4, 4), (15, 15)] {
            let (ctx, mut a) = app();
            let key = a.editor.timeline_mut().layers[0]
                .keyframes
                .get_mut(&4)
                .unwrap();
            key.label = Some("cue".into());
            key.script = Some("stop();".into());
            let before = a.editor.doc.clone();
            a.select_frame_range((0, start), (0, end));
            let enabled = a.enabled(command);
            a.dispatch(command, &ctx);
            if !enabled {
                assert_eq!(a.editor.doc, before);
                continue;
            }
            assert_eq!(a.editor.undo_depth(), 1);
            assert!(a.editor.timeline().layers[0].keyframes.contains_key(&0));
            assert!(a.editor.timeline().layers[0].end >= 1);
            if command == Command::BlankKeyframe && start == 4 {
                let key = &a.editor.timeline().layers[0].keyframes[&4];
                assert_eq!(key.label.as_deref(), Some("cue"));
                assert_eq!(key.script.as_deref(), Some("stop();"));
                assert!(key.elements.is_empty());
            }
            assert_eq!(a.editor.timeline().layers[1], before.scene.layers[1]);
            roundtrip(&a);
            let edited = a.editor.doc.clone();
            a.dispatch(Command::Undo, &ctx);
            assert_eq!(a.editor.doc, before);
            a.dispatch(Command::Redo, &ctx);
            assert_eq!(a.editor.doc, edited);
        }
    }
}

#[test]
fn tween_key_boundaries_bake_and_motion_property_keys_are_multi_layer_transactions() {
    for tween in [
        Tween::Classic {
            ease: 0.0,
            rotate_turns: 0,
        },
        Tween::Shape { ease: 0.0 },
        Tween::Motion { ease: 0.0 },
    ] {
        let (ctx, mut a) = app();
        for l in &mut a.editor.timeline_mut().layers {
            l.keyframes.remove(&4);
            let k = l.keyframes.get_mut(&0).unwrap();
            k.tween = tween;
            if matches!(tween, Tween::Motion { .. }) {
                k.motion = Some(Box::new(ac_core::motion::Motion::new(pt(0.0, 0.0))));
            }
        }
        let original = a.editor.doc.clone();
        let poses: Vec<_> = (0..12)
            .map(|f| {
                ac_core::layer_elements_at(&a.editor.timeline().layers[0], f)
                    .unwrap()
                    .0
            })
            .collect();
        a.select_frame_range((0, 4), (1, 4));
        a.dispatch(Command::Keyframe, &ctx);
        assert_eq!(a.editor.undo_depth(), 1);
        for l in &a.editor.timeline().layers {
            if matches!(tween, Tween::Motion { .. }) {
                assert!(l.keyframes[&0]
                    .motion
                    .as_ref()
                    .unwrap()
                    .key_frames()
                    .contains(&4));
            } else {
                assert!(l.keyframes.contains_key(&4));
            }
        }
        assert!(a.enabled(Command::ClearKeyframe));
        a.dispatch(Command::ClearKeyframe, &ctx);
        for l in &a.editor.timeline().layers {
            if matches!(tween, Tween::Motion { .. }) {
                assert!(!l.keyframes[&0]
                    .motion
                    .as_ref()
                    .unwrap()
                    .key_frames()
                    .contains(&4));
            } else {
                assert!(!l.keyframes.contains_key(&4));
            }
        }
        a.dispatch(Command::Undo, &ctx);
        a.dispatch(Command::Undo, &ctx);
        assert_eq!(a.editor.doc, original);
        a.select_frame_range((0, 0), (0, 7));
        a.dispatch(Command::FrameByFrame, &ctx);
        for f in 0..12 {
            let after = ac_core::layer_elements_at(&a.editor.timeline().layers[0], f)
                .unwrap()
                .0;
            // Compare rendered poses, allowing insignificant float interpolation differences.
            for (x, y) in poses[f as usize].iter().zip(&after) {
                assert!((x.matrix.tx - y.matrix.tx).abs() < 0.001);
                assert!((x.matrix.ty - y.matrix.ty).abs() < 0.001);
            }
        }
        assert_eq!(
            a.editor.timeline().layers[0].keyframes[&8],
            original.scene.layers[0].keyframes[&8]
        );
        roundtrip(&a);
        a.dispatch(Command::Undo, &ctx);
        assert_eq!(a.editor.doc, original);
    }
}

#[test]
fn linked_layer_properties_and_frame_display_are_reachable_localized_and_stable() {
    for locale in [Locale::English, Locale::French] {
        for theme in [theme::Theme::Light, theme::Theme::Dark] {
            let mut h = Harness::builder()
                .with_size(vec2(1800.0, 1550.0))
                .build_eframe(|cc| {
                    let mut a = Studio::with_preferences(
                        &cc.egui_ctx,
                        Preferences {
                            locale,
                            theme,
                            ..Default::default()
                        },
                    );
                    let mut doc = folder_fixture();
                    doc.scene.layers[0].kind = LayerKind::Mask;
                    doc.scene.layers[1].parent = None;
                    doc.scene.layers[2].parent = None;
                    a.replace(doc, None, false);
                    a.select_layer_rows(1, false, false);
                    a
                });
            h.run();
            let original = h.state().editor.doc.clone();
            h.get_by_label(&h.state().i18n.text("field-layer-type"))
                .click();
            h.run();
            h.get_by_label(&h.state().i18n.text("layer-type-masked"))
                .click();
            h.run();
            assert_eq!(h.state().layer_type(1), LayerType::Masked);
            roundtrip(h.state());
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
            h.run();
            assert_eq!(h.state().editor.doc, original);
            for (mode, key) in [
                (FrameDisplay::Preview, "frames-preview"),
                (FrameDisplay::Context, "frames-context"),
                (FrameDisplay::Cells, "frames-cells"),
            ] {
                let label = h.state().i18n.text("field-frame-display");
                let rect = h.get_by_label(&label).rect();
                h.get_by_label(&label).hover();
                h.run();
                assert_eq!(h.get_by_label(&label).rect(), rect);
                h.get_by_label(&label).click();
                h.run();
                h.get_by_label(&h.state().i18n.text(key)).click();
                h.run();
                assert!(h.state().prefs.frame_display == mode);
                assert_eq!(h.state().editor.doc, original);
                let label = h
                    .state()
                    .i18n
                    .format("timeline-cell", &[("layer", "A"), ("frame", "1")]);
                let rect = h.get_by_label(&label).rect();
                h.get_by_label(&label).hover();
                h.run();
                assert_eq!(h.get_by_label(&label).rect(), rect);
                assert!(
                    rect.width()
                        > if mode == FrameDisplay::Cells {
                            5.0
                        } else {
                            60.0
                        }
                );
            }
        }
    }
}

#[test]
fn outline_columns_toggle_single_all_and_alt_others_without_changing_export() {
    let mut h = Harness::builder()
        .with_size(vec2(1700.0, 1100.0))
        .build_eframe(|cc| {
            let mut a = Studio::with_preferences(
                &cc.egui_ctx,
                Preferences {
                    locale: Locale::English,
                    ..Default::default()
                },
            );
            a.replace(fixture(), None, false);
            a
        });
    h.run();
    let before = h.state().editor.doc.clone();
    let export = h.state().render_export_pixels(0.5).unwrap();
    let label = h
        .state()
        .i18n
        .format("action-layer-outline", &[("name", "A")]);
    let rect = h
        .query_all_by_label(&label)
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .unwrap()
        .rect();
    h.query_all_by_label(&label)
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .unwrap()
        .hover();
    h.run();
    assert_eq!(
        h.query_all_by_label(&label)
            .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
            .unwrap()
            .rect(),
        rect
    );
    h.query_all_by_label(&label)
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .unwrap()
        .click();
    h.run();
    assert!(h.state().editor.timeline().layers[0].outline);
    assert!(!h.state().editor.timeline().layers[1].outline);
    assert_ne!(h.state().render_pixels(0.5).unwrap().data(), export.data());
    assert_eq!(
        h.state().render_export_pixels(0.5).unwrap().data(),
        export.data()
    );
    click(&mut h, rect.center(), egui::Modifiers::ALT);
    assert!(h.state().editor.timeline().layers[1].outline);
    h.get_by_label(&h.state().i18n.text("action-all-outlines"))
        .click();
    h.run();
    assert!(h
        .state()
        .editor
        .timeline()
        .layers
        .iter()
        .all(|l| !l.outline));
    for _ in 0..3 {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        h.run();
    }
    assert_eq!(h.state().editor.doc, before);
}

#[test]
fn partial_tween_clipboard_preserves_copied_frame_poses() {
    for tween in [
        Tween::Classic {
            ease: 0.0,
            rotate_turns: 0,
        },
        Tween::Shape { ease: 0.0 },
        Tween::Classic {
            ease: 80.0,
            rotate_turns: 2,
        },
        Tween::Shape { ease: -70.0 },
        Tween::Motion { ease: 65.0 },
    ] {
        let (ctx, mut a) = app();
        a.editor.timeline_mut().layers[0].keyframes.remove(&4);
        a.editor.timeline_mut().layers[0]
            .keyframes
            .get_mut(&0)
            .unwrap()
            .tween = tween;
        if matches!(tween, Tween::Motion { .. }) {
            let mut motion = ac_core::motion::Motion::new(pt(0.0, 0.0));
            motion.set_pos(0, pt(0.0, 0.0));
            motion.set_pos(7, pt(70.0, 20.0));
            a.editor.timeline_mut().layers[0]
                .keyframes
                .get_mut(&0)
                .unwrap()
                .motion = Some(Box::new(motion));
        }
        let source = a.editor.timeline().layers[0].clone();
        a.select_frame_range((0, 2), (0, 5));
        a.dispatch(Command::CopyFrames, &ctx);
        a.select_timeline_cell(1, 15);
        a.dispatch(Command::PasteOverwriteFrames, &ctx);
        for i in 0..4 {
            let from = ac_core::layer_elements_at(&source, 2 + i).unwrap().0;
            let to = ac_core::layer_elements_at(&a.editor.timeline().layers[1], 15 + i)
                .unwrap()
                .0;
            assert_eq!(from, to, "partial tween frame {i}");
        }
        assert_eq!(a.status, a.i18n.text("status-frames-copied"));
        roundtrip(&a);
        let whole = ac_core::frames::copy_frames(&source, 0, 11);
        assert_eq!(
            whole.keyframes[0].1.tween, tween,
            "whole spans retain their authored interpolation"
        );
    }
}

fn animated_graphic_fixture(tween: Tween) -> ac_core::Document {
    let mut doc = document::blank("Scene", "Animation");
    let mut leaf = ac_core::Timeline::new("Animated leaf");
    leaf.layers[0].end = 7;
    for f in 0..7 {
        leaf.layers[0].keyframes.insert(
            f,
            ac_core::Keyframe::with(vec![Element::shape(
                "Pose",
                Shape::new(
                    Path::rect(15.0 + f as f32 * 5.0, 20.0, 12.0, 12.0),
                    Some(Color::rgb(30 * f as u8, 120, 180)),
                    None,
                ),
            )]),
        );
    }
    let leaf_id = doc.add_symbol("Leaf", SymbolKind::Graphic, leaf);
    let mut nested = ac_core::Timeline::new("Nested animation");
    nested.layers[0].end = 11;
    nested.layers[0].keyframes.insert(
        0,
        ac_core::Keyframe::with(vec![Element::group(vec![Element::instance(
            "Leaf instance",
            leaf_id,
            Matrix::IDENTITY,
        )])]),
    );
    let nested_id = doc.add_symbol("Nested", SymbolKind::Graphic, nested);
    let mut start = ac_core::Keyframe::with(vec![Element::instance(
        "Animated instance",
        nested_id,
        Matrix::IDENTITY,
    )]);
    let mut end = start.clone();
    end.elements[0].matrix = Matrix::translate(80.0, 40.0);
    start.tween = tween;
    if tween.is_motion() {
        let mut motion = ac_core::motion::Motion::new(pt(0.0, 0.0));
        motion.set_pos(0, pt(0.0, 0.0));
        motion.set_pos(11, pt(80.0, 40.0));
        motion.path[0].h_out = pt(20.0, 40.0);
        motion.path[1].h_in = pt(-15.0, 25.0);
        start.motion = Some(Box::new(motion));
    }
    doc.scene.layers[0].end = 15;
    doc.scene.layers[0].keyframes.insert(0, start);
    if tween != Tween::None {
        doc.scene.layers[0].keyframes.insert(12, end);
    }
    doc
}

fn drawing_at(doc: &ac_core::Document, frame: u32) -> String {
    let drawing = ac_core::evaluate_scene(doc, frame);
    assert!(!drawing.is_empty());
    format!("{drawing:?}")
}

#[test]
fn nested_animation_clipboards_transfer_phase_assets_and_atomic_history() {
    for locale in [Locale::English, Locale::French] {
        for tween in [
            Tween::None,
            Tween::Classic {
                ease: 75.0,
                rotate_turns: 1,
            },
            Tween::Motion { ease: -80.0 },
        ] {
            for cut in [false, true] {
                let (ctx, mut a) = app();
                a.i18n = crate::i18n::Localizer::new(locale);
                let source = animated_graphic_fixture(tween);
                a.replace(source.clone(), None, false);
                a.select_frame_range((0, 3), (0, 8));
                a.dispatch(
                    if cut {
                        Command::CutFrames
                    } else {
                        Command::CopyFrames
                    },
                    &ctx,
                );
                assert_eq!(a.editor.undo_depth(), usize::from(cut));
                if cut {
                    a.dispatch(Command::Undo, &ctx);
                    assert_eq!(a.editor.doc, source);
                }
                let mut destination = document::blank("Destination", "Layer");
                destination.add_symbol(
                    "ID collision",
                    SymbolKind::Graphic,
                    ac_core::Timeline::new("Other"),
                );
                a.replace(destination.clone(), None, false);
                a.select_timeline_cell(0, 20);
                a.dispatch(Command::PasteOverwriteFrames, &ctx);
                assert_eq!(a.editor.undo_depth(), 1);
                assert_eq!(a.editor.doc.library.len(), 3);
                for f in 0..6 {
                    assert_eq!(
                        drawing_at(&source, 3 + f),
                        drawing_at(&a.editor.doc, 20 + f)
                    );
                }
                roundtrip(&a);
                let pasted = a.editor.doc.clone();
                a.dispatch(Command::Undo, &ctx);
                assert_eq!(a.editor.doc, destination);
                a.dispatch(Command::Redo, &ctx);
                assert_eq!(a.editor.doc, pasted);
                // Editing the receiving symbol's timeline must use the same clock
                // and dependency rules as a scene, including save and one-step undo.
                let receiver = a.editor.doc.add_symbol(
                    "Receiver",
                    SymbolKind::Graphic,
                    ac_core::Timeline::new("Receiver"),
                );
                a.editor.enter_symbol(receiver);
                a.select_timeline_cell(0, 10);
                let before = a.editor.doc.clone();
                a.dispatch(Command::PasteOverwriteFrames, &ctx);
                assert_eq!(
                    a.editor.timeline().layers[0].keyframes[&10].playback_offset,
                    3
                );
                roundtrip(&a);
                a.dispatch(Command::Undo, &ctx);
                assert_eq!(a.editor.doc, before);
            }
        }
    }
}

#[test]
fn nested_animation_bake_move_and_disjoint_copy_survive_history_and_disk() {
    use ac_core::FrameSel;
    for tween in [
        Tween::Classic {
            ease: 85.0,
            rotate_turns: 2,
        },
        Tween::Motion { ease: -65.0 },
    ] {
        let (ctx, mut a) = app();
        let source = animated_graphic_fixture(tween);
        a.replace(source.clone(), None, false);
        a.select_frame_range((0, 0), (0, 11));
        a.dispatch(Command::FrameByFrame, &ctx);
        assert_eq!(a.editor.undo_depth(), 1);
        for f in 0..15 {
            assert_eq!(drawing_at(&source, f), drawing_at(&a.editor.doc, f));
        }
        roundtrip(&a);
        a.dispatch(Command::Undo, &ctx);
        assert_eq!(a.editor.doc, source);
        let ranges = vec![
            FrameSel {
                layer0: 0,
                layer1: 0,
                f0: 2,
                f1: 3,
            },
            FrameSel {
                layer0: 0,
                layer1: 0,
                f0: 6,
                f1: 7,
            },
        ];
        a.install_frame_ranges(ranges.clone());
        a.dispatch(Command::CopyFrames, &ctx);
        a.select_timeline_cell(0, 20);
        a.dispatch(Command::PasteOverwriteFrames, &ctx);
        for (from, to) in [(2, 20), (3, 21), (6, 24), (7, 25)] {
            assert_eq!(drawing_at(&source, from), drawing_at(&a.editor.doc, to));
        }
        roundtrip(&a);
        a.dispatch(Command::Undo, &ctx);
        assert_eq!(a.editor.doc, source);
        a.install_frame_ranges(ranges);
        assert!(a.retime_frame_set(20, 0, false));
        for (from, to) in [(2, 22), (3, 23), (6, 26), (7, 27)] {
            assert_eq!(drawing_at(&source, from), drawing_at(&a.editor.doc, to));
        }
        assert_eq!(a.editor.undo_depth(), 1);
        roundtrip(&a);
        a.dispatch(Command::Undo, &ctx);
        assert_eq!(a.editor.doc, source);
    }
}

#[test]
fn nested_static_frame_commands_preserve_surrounding_playback_and_one_step_undo() {
    for command in [
        Command::Keyframe,
        Command::ClearFrames,
        Command::RemoveFrame,
        Command::ReverseFrames,
    ] {
        let (ctx, mut a) = app();
        let source = animated_graphic_fixture(Tween::None);
        a.replace(source.clone(), None, false);
        a.select_frame_range((0, 3), (0, 8));
        assert!(a.enabled(command));
        a.dispatch(command, &ctx);
        assert_eq!(a.editor.undo_depth(), 1);
        let duration = if command == Command::RemoveFrame {
            9
        } else {
            15
        };
        for f in 0..duration {
            let expected = match command {
                Command::ClearFrames if (3..=8).contains(&f) => {
                    assert!(a.editor.evaluate_current(f).is_empty());
                    continue;
                }
                Command::RemoveFrame if f >= 3 => f + 6,
                Command::ReverseFrames if (3..=8).contains(&f) => 11 - f,
                _ => f,
            };
            assert_eq!(
                drawing_at(&source, expected),
                drawing_at(&a.editor.doc, f),
                "{command:?} frame {f}"
            );
        }
        roundtrip(&a);
        let edited = a.editor.doc.clone();
        a.dispatch(Command::Undo, &ctx);
        assert_eq!(a.editor.doc, source);
        a.dispatch(Command::Redo, &ctx);
        assert_eq!(a.editor.doc, edited);
    }
}

#[test]
fn right_trimmed_tweens_stay_authored_across_transfer_resize_and_history() {
    for locale in [Locale::English, Locale::French] {
        for tween in [
            Tween::Classic {
                ease: 75.0,
                rotate_turns: 2,
            },
            Tween::Motion { ease: -80.0 },
        ] {
            let (ctx, mut a) = app();
            a.i18n = crate::i18n::Localizer::new(locale);
            let source = animated_graphic_fixture(tween);
            a.replace(source.clone(), None, false);
            a.status.clear();
            a.select_frame_range((0, 0), (0, 7));
            a.dispatch(Command::CopyFrames, &ctx);
            assert_eq!(
                a.status,
                a.i18n.text("status-frames-copied"),
                "an authored clip must not announce pose conversion"
            );
            let mut destination = document::blank("Other", "Layer");
            destination.add_symbol(
                "Collision",
                SymbolKind::Graphic,
                ac_core::Timeline::new("Different"),
            );
            a.replace(destination.clone(), None, false);
            a.select_timeline_cell(0, 0);
            a.dispatch(Command::PasteFrames, &ctx);
            let pasted = a.editor.doc.clone();
            let layer = &a.editor.timeline().layers[0];
            assert_eq!(layer.keyframes.len(), 1);
            assert_eq!(layer.keyframes[&0].tween, tween);
            assert!(layer.keyframes[&0].tween_boundary.is_some());
            for f in 0..8 {
                assert_eq!(drawing_at(&source, f), drawing_at(&a.editor.doc, f));
            }
            roundtrip(&a);
            a.set_timeline_duration(0, 16);
            assert_eq!(
                a.editor.timeline().layers[0].keyframes,
                pasted.scene.layers[0].keyframes
            );
            roundtrip(&a);
            a.dispatch(Command::Undo, &ctx);
            assert_eq!(a.editor.doc, pasted);
            // Splitting keeps two authored intervals, without pose conversion.
            a.select_timeline_cell(0, 3);
            a.dispatch(Command::Keyframe, &ctx);
            assert_ne!(a.status, a.i18n.text("status-tween-boundary-poses"));
            assert_eq!(a.editor.timeline().layers[0].keyframes.len(), 2);
            assert!(a.editor.timeline().layers[0]
                .keyframes
                .values()
                .all(|k| k.tween == tween));
            for f in 0..8 {
                assert_eq!(drawing_at(&source, f), drawing_at(&a.editor.doc, f));
            }
            roundtrip(&a);
            a.dispatch(Command::Undo, &ctx);
            assert_eq!(a.editor.doc, pasted);
            a.dispatch(Command::Undo, &ctx);
            assert_eq!(a.editor.doc, destination);
        }
    }
}

#[test]
fn retained_endpoints_transfer_bitmap_dependencies_and_library_deletion_undo() {
    let (ctx, mut a) = app();
    let mut source = fixture();
    source.scene.layers.truncate(1);
    source.scene.layers[0].keyframes.remove(&4);
    source.scene.layers[0].keyframes.get_mut(&0).unwrap().tween = Tween::Shape { ease: 0.0 };
    source.bitmaps.push(
        ac_core::bitmap::BitmapAsset::from_rgba(7, "Endpoint image", 1, 1, &[255, 0, 0, 255])
            .unwrap(),
    );
    let ElementKind::Shape(shape) = &mut source.scene.layers[0]
        .keyframes
        .get_mut(&8)
        .unwrap()
        .elements[0]
        .kind
    else {
        panic!()
    };
    shape.bitmap_fill = Some(ac_core::BitmapFill {
        bitmap: 7,
        matrix: Matrix::IDENTITY,
    });
    a.replace(source.clone(), None, false);
    a.select_frame_range((0, 0), (0, 6));
    a.dispatch(Command::CopyFrames, &ctx);
    let mut destination = document::blank("Other", "Layer");
    destination.bitmaps.push(
        ac_core::bitmap::BitmapAsset::from_rgba(7, "Collision", 1, 1, &[0, 0, 255, 255]).unwrap(),
    );
    a.replace(destination, None, false);
    a.dispatch(Command::PasteFrames, &ctx);
    let key = &a.editor.timeline().layers[0].keyframes[&0];
    let endpoint = key.tween_boundary.as_ref().unwrap();
    let ElementKind::Shape(shape) = &endpoint.elements[0].kind else {
        panic!()
    };
    let id = shape.bitmap_fill.unwrap().bitmap;
    assert_ne!(id, 7);
    assert_eq!(
        a.editor.bitmap_usage(id),
        1,
        "endpoint-only assets are real dependencies"
    );
    assert_eq!(
        a.editor.doc.bitmap(id).unwrap().png,
        source.bitmap(7).unwrap().png
    );
    roundtrip(&a);
    let before = a.editor.doc.clone();
    a.editor.delete_bitmap(id);
    assert_eq!(a.editor.bitmap_usage(id), 0);
    a.editor.undo();
    assert_eq!(a.editor.doc, before);
}

#[test]
fn retained_motion_inspector_edit_keeps_requested_pose_and_undo() {
    let (ctx, mut a) = app();
    a.replace(
        animated_graphic_fixture(Tween::Motion { ease: -80.0 }),
        None,
        false,
    );
    a.select_frame_range((0, 0), (0, 7));
    a.dispatch(Command::CopyFrames, &ctx);
    a.replace(document::blank("Other", "Layer"), None, false);
    a.dispatch(Command::PasteFrames, &ctx);
    a.set_timeline_duration(0, 16);
    a.editor.set_frame(5);
    a.editor.selection = vec![Selection {
        layer: 0,
        element: 0,
    }];
    let before = a.editor.doc.clone();
    let depth = a.editor.undo_depth();
    let original = a.editor.selected_element().unwrap();
    a.transform_fields = [original.matrix.tx + 10.0, original.matrix.ty + 8.0, 0.6];
    a.apply_selection_transform();
    let edited = a.editor.selected_element().unwrap();
    assert!((edited.matrix.tx - a.transform_fields[0]).abs() < 0.001);
    assert!((edited.alpha - 0.6).abs() < 0.001);
    assert_ne!(a.status, a.i18n.text("status-tween-boundary-poses"));
    assert_eq!(a.editor.timeline().layers[0].keyframes.len(), 2);
    assert!(a.editor.timeline().layers[0]
        .keyframes
        .values()
        .all(|k| k.tween.is_motion()));
    assert_eq!(a.editor.undo_depth(), depth + 1);
    roundtrip(&a);
    a.dispatch(Command::Undo, &ctx);
    assert_eq!(a.editor.doc, before);
}

#[test]
fn partial_curve_commands_preserve_exterior_nested_frames_history_and_persistence() {
    for locale in [Locale::English, Locale::French] {
        for tween in [
            Tween::Classic {
                ease: 80.0,
                rotate_turns: 2,
            },
            Tween::Shape { ease: -70.0 },
            Tween::Motion { ease: 60.0 },
        ] {
            for command in [
                Command::Keyframe,
                Command::BlankKeyframe,
                Command::ClearFrames,
                Command::RemoveFrame,
                Command::FrameByFrame,
                Command::ReverseFrames,
            ] {
                let (ctx, mut a) = app();
                a.i18n = crate::i18n::Localizer::new(locale);
                let mut source = animated_graphic_fixture(tween);
                let mut second = source.scene.layers[0].clone();
                second.name = "Second".into();
                source.scene.layers.push(second);
                a.replace(source.clone(), None, false);
                a.select_frame_range((0, 3), (1, 7));
                a.editor.timeline_mut().layers[1].locked = true;
                let protected = a.editor.doc.clone();
                assert!(!a.enabled(command));
                a.dispatch(command, &ctx);
                assert_eq!(a.editor.doc, protected);
                assert_eq!(a.editor.undo_depth(), 0);
                a.editor.timeline_mut().layers[1].locked = false;
                a.dispatch(command, &ctx);
                assert_eq!(a.editor.undo_depth(), 1);
                for layer in &a.editor.timeline().layers {
                    assert_eq!(layer.keyframes[&0].tween, tween);
                    let tail = if command == Command::RemoveFrame {
                        3
                    } else {
                        8
                    };
                    assert_eq!(layer.span_at(tail).unwrap().1.tween, tween);
                }
                for f in 0..a.editor.timeline().layers[0].end {
                    if (3..=7).contains(&f)
                        && matches!(command, Command::BlankKeyframe | Command::ClearFrames)
                    {
                        assert!(a.editor.evaluate_current(f).is_empty());
                        continue;
                    }
                    let original = match command {
                        Command::RemoveFrame if f >= 3 => f + 5,
                        Command::ReverseFrames if (3..=7).contains(&f) => 10 - f,
                        _ => f,
                    };
                    assert_eq!(
                        drawing_at(&source, original),
                        drawing_at(&a.editor.doc, f),
                        "{tween:?} {command:?} frame {f}"
                    );
                }
                roundtrip(&a);
                let edited = a.editor.doc.clone();
                a.dispatch(Command::Undo, &ctx);
                assert_eq!(a.editor.doc, source);
                a.dispatch(Command::Redo, &ctx);
                assert_eq!(a.editor.doc, edited);
            }
        }
    }
}

#[test]
fn interior_interval_inspector_edits_visible_pose_without_baking() {
    for tween in [
        Tween::Classic {
            ease: 80.0,
            rotate_turns: 2,
        },
        Tween::Shape { ease: -70.0 },
        Tween::Motion { ease: 60.0 },
    ] {
        for reverse in [false, true] {
            let (ctx, mut a) = app();
            a.replace(animated_graphic_fixture(tween), None, false);
            a.select_frame_range((0, 3), (0, 8));
            a.dispatch(Command::CopyFrames, &ctx);
            a.replace(document::blank("Target", "Layer"), None, false);
            a.dispatch(Command::PasteFrames, &ctx);
            if reverse {
                a.select_frame_range((0, 0), (0, 5));
                a.dispatch(Command::ReverseFrames, &ctx);
            }
            a.editor.set_frame(0);
            a.editor.selection = vec![Selection {
                layer: 0,
                element: 0,
            }];
            let before = a.editor.doc.clone();
            let depth = a.editor.undo_depth();
            let visible = a.editor.selected_element().unwrap();
            a.transform_fields = [visible.matrix.tx + 12.0, visible.matrix.ty - 7.0, 0.45];
            let requested = a.transform_fields;
            a.apply_selection_transform();
            let edited = a.editor.selected_element().unwrap();
            assert!((edited.matrix.tx - requested[0]).abs() < 0.001);
            assert!((edited.matrix.ty - requested[1]).abs() < 0.001);
            assert!((edited.alpha - requested[2]).abs() < 0.001);
            assert_ne!(a.status, a.i18n.text("status-tween-boundary-poses"));
            assert_eq!(a.editor.timeline().layers[0].keyframes.len(), 1);
            assert_eq!(a.editor.timeline().layers[0].keyframes[&0].tween, tween);
            assert_eq!(a.editor.undo_depth(), depth + 1);
            for f in 1..6 {
                let old = ac_core::layer_elements_at(&before.scene.layers[0], f).unwrap();
                let new = ac_core::layer_elements_at(&a.editor.doc.scene.layers[0], f).unwrap();
                let weight = 1.0 - f as f32 / 6.0;
                assert!((new.0[0].matrix.tx - old.0[0].matrix.tx - 12.0 * weight).abs() < 0.001);
                assert!((new.0[0].matrix.ty - old.0[0].matrix.ty + 7.0 * weight).abs() < 0.001);
                assert_eq!(new.1, old.1);
            }
            roundtrip(&a);
            a.dispatch(Command::Undo, &ctx);
            assert_eq!(a.editor.doc, before);
        }
    }
}

#[test]
fn replacement_paste_preserves_authored_prefix_and_shifted_tail() {
    for tween in [
        Tween::Classic {
            ease: 80.0,
            rotate_turns: 2,
        },
        Tween::Shape { ease: -70.0 },
        Tween::Motion { ease: 60.0 },
    ] {
        let (ctx, mut a) = app();
        let source = animated_graphic_fixture(tween);
        a.replace(source.clone(), None, false);
        a.select_frame_range((0, 4), (0, 5));
        a.dispatch(Command::CopyFrames, &ctx);
        a.select_frame_range((0, 3), (0, 7));
        a.dispatch(Command::PasteFrames, &ctx);
        assert_eq!(a.editor.timeline().layers[0].end, 12);
        for f in 0..12 {
            let original = if f < 3 {
                f
            } else if f < 5 {
                f + 1
            } else {
                f + 3
            };
            assert_eq!(drawing_at(&source, original), drawing_at(&a.editor.doc, f));
        }
        assert!(a.editor.timeline().layers[0].keyframes.len() <= 4);
        roundtrip(&a);
        assert_eq!(a.editor.undo_depth(), 1);
        a.dispatch(Command::Undo, &ctx);
        assert_eq!(a.editor.doc, source);
    }
}
