use super::*;
use ac_core::{FrameSel, Keyframe, Layer};
use egui_kittest::{kittest::Queryable, Harness};

pub(super) fn fixture() -> ac_core::Document {
    let mut doc = document::blank("Scene", "A");
    doc.scene.layers.push(Layer::new("B", 1));
    for (i, layer) in doc.scene.layers.iter_mut().enumerate() {
        layer.end = 12;
        for f in [0, 4, 8] {
            let mut e = Element::shape(
                &format!("{i}:{f}"),
                Shape::new(
                    Path::rect(10.0 + f as f32 * 5.0, 10.0 + i as f32 * 30.0, 15.0, 15.0),
                    Some(Color::rgb(255, i as u8 * 150, f as u8 * 20)),
                    None,
                ),
            );
            e.matrix = Matrix::translate(f as f32, 0.0);
            layer.keyframes.insert(f, Keyframe::with(vec![e]));
        }
    }
    doc
}

pub(super) fn app() -> (egui::Context, Studio) {
    let ctx = egui::Context::default();
    let mut app = Studio::with_preferences(
        &ctx,
        Preferences {
            locale: Locale::English,
            ..Default::default()
        },
    );
    app.replace(fixture(), None, false);
    (ctx, app)
}

pub(super) fn roundtrip(app: &Studio) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("phase-one.acx.json");
    document::save(&app.editor.doc, &path).unwrap();
    assert_eq!(document::open(&path).unwrap(), app.editor.doc);
}

fn cell(h: &Harness<Studio>, layer: &str, frame: u32) -> egui::Rect {
    h.get_by_label(&h.state().i18n.format(
        "timeline-cell",
        &[("layer", layer), ("frame", &frame.to_string())],
    ))
    .rect()
}

pub(super) fn click(h: &mut Harness<Studio>, pos: egui::Pos2, modifiers: egui::Modifiers) {
    h.hover_at(pos);
    h.event(egui::Event::ModifiersChanged(modifiers));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        });
    }
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run();
}

pub(super) fn drag(
    h: &mut Harness<Studio>,
    start: egui::Pos2,
    end: egui::Pos2,
    cancel: bool,
    modifiers: egui::Modifiers,
) {
    h.hover_at(start);
    h.event(egui::Event::ModifiersChanged(modifiers));
    h.event(egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers,
    });
    h.run();
    h.hover_at(end);
    h.run();
    if cancel {
        h.key_press(egui::Key::Escape);
        h.run();
    }
    h.event(egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers,
    });
    h.run();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run();
}

#[test]
fn localized_range_clipboard_keyboard_undo_and_save_workflow() {
    for locale in [Locale::English, Locale::French] {
        for theme in [theme::Theme::Dark, theme::Theme::Light] {
            let mut h = Harness::builder()
                .with_size(vec2(1600.0, 1000.0))
                .build_eframe(|cc| {
                    let mut app = Studio::with_preferences(
                        &cc.egui_ctx,
                        Preferences {
                            locale,
                            theme,
                            ..Default::default()
                        },
                    );
                    app.replace(fixture(), None, false);
                    app
                });
            h.run();
            let before = h.state().editor.doc.clone();
            let a = cell(&h, "A", 1).center();
            let b = cell(&h, "B", 4).center();
            click(&mut h, a, egui::Modifiers::NONE);
            click(&mut h, b, egui::Modifiers::SHIFT);
            assert_eq!(
                h.state().editor.frame_sel,
                Some(FrameSel {
                    layer0: 0,
                    layer1: 1,
                    f0: 0,
                    f1: 3
                })
            );
            h.key_press_modifiers(
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::C,
            );
            h.run();
            assert_eq!(h.state().editor.doc, before, "copy is not an edit");
            let dest = cell(&h, "A", 20).center();
            click(&mut h, dest, egui::Modifiers::NONE);
            h.key_press_modifiers(
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::V,
            );
            h.run();
            assert_eq!(
                h.state().editor.frame_sel,
                Some(FrameSel {
                    layer0: 0,
                    layer1: 1,
                    f0: 19,
                    f1: 22
                })
            );
            for i in 0..2 {
                assert_eq!(
                    h.state().editor.timeline().layers[i].keyframes[&19],
                    before.scene.layers[i].keyframes[&0]
                );
            }
            assert_eq!(h.state().editor.undo_depth(), 1);
            let pasted = h.state().editor.doc.clone();
            roundtrip(h.state());
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
            h.run();
            assert_eq!(h.state().editor.doc, before);
            h.key_press_modifiers(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            );
            h.run();
            assert_eq!(h.state().editor.doc, pasted);
            // Text-edit focus must retain its normal clipboard behavior.
            let revision = h.state().editor.revision;
            h.get_by_label(&h.state().i18n.text("field-frame-label"))
                .click();
            h.run();
            h.event(egui::Event::Text("intro".into()));
            h.run();
            h.key_press_modifiers(
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::X,
            );
            h.run();
            assert_eq!(h.state().editor.revision, revision);
            h.get_by_label(&h.state().i18n.text("action-apply-timing"))
                .click();
            h.run();
            assert_eq!(
                h.state().editor.timeline().layers[0].keyframes[&19]
                    .label
                    .as_deref(),
                Some("intro")
            );
        }
    }
}

#[test]
fn frame_range_mutations_are_atomic_for_locks_limits_and_destinations() {
    let (ctx, mut app) = app();
    app.select_frame_range((0, 0), (1, 3));
    app.dispatch(Command::CopyFrames, &ctx);
    app.editor.timeline_mut().layers[1].locked = true;
    let before = app.editor.doc.clone();
    let depth = app.editor.undo_depth();
    for command in [
        Command::CutFrames,
        Command::PasteFrames,
        Command::ClearFrames,
        Command::InsertFrame,
        Command::RemoveFrame,
        Command::Keyframe,
        Command::BlankKeyframe,
        Command::ReverseFrames,
        Command::Tween,
    ] {
        assert!(!app.enabled(command), "{command:?}");
        app.dispatch(command, &ctx);
        assert_eq!(app.editor.doc, before);
    }
    assert_eq!(app.editor.undo_depth(), depth);
    app.editor.timeline_mut().layers[1].locked = false;
    app.select_timeline_cell(1, 0);
    assert!(
        !app.enabled(Command::PasteFrames),
        "do not truncate two copied layers into one destination"
    );
    app.select_timeline_cell(0, document::MAX_FRAMES - 2);
    assert!(!app.enabled(Command::PasteFrames));
    app.select_frame_range((0, 0), (1, 3));
    app.editor.timeline_mut().layers[1].end = document::MAX_FRAMES - 2;
    assert!(
        !app.enabled(Command::InsertFrame),
        "check the whole range, not only the active layer"
    );
    app.editor.timeline_mut().layers[1].visible = false;
    assert!(!app.enabled(Command::BlankKeyframe));
    roundtrip(&app);
}

#[test]
fn clear_blank_reverse_retime_preserve_content_and_transaction_history() {
    let (ctx, mut app) = app();
    app.editor.timeline_mut().layers[0]
        .keyframes
        .get_mut(&0)
        .unwrap()
        .label = Some("intro".into());
    let original = app.editor.doc.clone();
    app.select_frame_range((0, 4), (1, 7));
    app.dispatch(Command::CutFrames, &ctx);
    for layer in &app.editor.timeline().layers {
        assert!(layer.keyframes[&4].elements.is_empty());
        assert_eq!(layer.end, 12);
        assert!(!layer.keyframes[&8].elements.is_empty());
    }
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, original);
    app.select_frame_range((0, 0), (1, 0));
    app.dispatch(Command::BlankKeyframe, &ctx);
    assert_eq!(
        app.editor.timeline().layers[0].keyframes[&0]
            .label
            .as_deref(),
        Some("intro")
    );
    assert!(app
        .editor
        .timeline()
        .layers
        .iter()
        .all(|l| l.keyframes[&0].elements.is_empty()));
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, original);
    app.select_frame_range((0, 0), (1, 11));
    app.dispatch(Command::ReverseFrames, &ctx);
    for i in 0..2 {
        for f in 0..12 {
            assert_eq!(
                ac_core::layer_elements_at(&app.editor.timeline().layers[i], f)
                    .unwrap()
                    .0,
                ac_core::layer_elements_at(&original.scene.layers[i], 11 - f)
                    .unwrap()
                    .0
            );
        }
    }
    roundtrip(&app);
    app.dispatch(Command::Undo, &ctx);
    let s = FrameSel {
        layer0: 0,
        layer1: 1,
        f0: 4,
        f1: 7,
    };
    assert!(app.retime_frames(s, 12, 0, true));
    for i in 0..2 {
        assert_eq!(
            app.editor.timeline().layers[i].keyframes[&16],
            original.scene.layers[i].keyframes[&4]
        );
    }
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, original);
    assert!(app.retime_frames(s, 2, 0, false));
    assert_eq!(
        app.editor.timeline().layers[0].keyframes[&6],
        original.scene.layers[0].keyframes[&4]
    );
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, original);
    assert!(app.retime_frames(FrameSel::cell(0, 4), 2, 1, false));
    assert_eq!(
        app.editor.timeline().layers[1].keyframes[&6],
        original.scene.layers[0].keyframes[&4]
    );
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, original);
}

#[test]
fn folder_lifecycle_preserves_hierarchy_animation_parents_and_undo() {
    let (ctx, mut app) = app();
    app.editor.timeline_mut().layers[1].anim_parent = Some(0);
    let original = app.editor.doc.clone();
    app.editor.layer = 0;
    app.dispatch(Command::AddFolder, &ctx);
    assert_eq!(app.editor.timeline().layers[1].parent, Some(0));
    assert_eq!(app.editor.timeline().layers[2].anim_parent, Some(1));
    app.editor.layer = 2;
    app.dispatch(Command::IndentLayer, &ctx);
    assert_eq!(app.editor.timeline().layers[2].parent, Some(0));
    app.editor.layer = 0;
    app.dispatch(Command::DuplicateLayer, &ctx);
    assert_eq!(app.editor.timeline().layers.len(), 6);
    assert_eq!(app.editor.timeline().layers[2].anim_parent, Some(1));
    assert_eq!(app.editor.timeline().layers[5].anim_parent, Some(4));
    let duplicated = app.editor.doc.clone();
    app.dispatch(Command::LayerDown, &ctx);
    assert_eq!(app.editor.layer, 3);
    assert_eq!(app.editor.timeline().layers[5].anim_parent, Some(4));
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, duplicated);
    app.editor.layer = 0;
    app.dispatch(Command::DeleteLayer, &ctx);
    assert_eq!(app.editor.timeline().layers.len(), 3);
    assert_eq!(app.editor.timeline().layers[2].anim_parent, Some(1));
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, duplicated);
    app.editor.layer = 1;
    app.dispatch(Command::OutdentLayer, &ctx);
    assert_eq!(app.editor.timeline().layers[app.editor.layer].parent, None);
    roundtrip(&app);
    while app.editor.undo().is_some() {}
    assert_eq!(app.editor.doc, original);
    app.editor.layer = 0;
    app.dispatch(Command::AddFolder, &ctx);
    app.editor.timeline_mut().layers[0].locked = true;
    app.select_timeline_cell(1, 0);
    assert!(
        !app.enabled(Command::BlankKeyframe),
        "folder locking protects descendants"
    );
    assert!(!app.enabled(Command::OutdentLayer));
}

#[test]
fn retime_gestures_commit_once_and_escape_cancels() {
    let mut h = Harness::builder()
        .with_size(vec2(1600.0, 1000.0))
        .build_eframe(|cc| {
            let mut app = Studio::with_preferences(
                &cc.egui_ctx,
                Preferences {
                    locale: Locale::English,
                    ..Default::default()
                },
            );
            app.replace(fixture(), None, false);
            app.select_frame_range((0, 4), (0, 7));
            app
        });
    h.run();
    let before = h.state().editor.doc.clone();
    let start = cell(&h, "A", 5).center();
    let end = cell(&h, "A", 17).center();
    drag(&mut h, start, end, true, egui::Modifiers::NONE);
    assert_eq!(h.state().editor.doc, before);
    assert_eq!(h.state().editor.undo_depth(), 0);
    drag(&mut h, start, end, false, egui::Modifiers::ALT);
    assert_eq!(h.state().editor.undo_depth(), 1);
    assert_eq!(
        h.state().editor.timeline().layers[0].keyframes[&16],
        before.scene.layers[0].keyframes[&4]
    );
    assert_eq!(
        h.state().editor.timeline().layers[0].keyframes[&4],
        before.scene.layers[0].keyframes[&4]
    );
    roundtrip(h.state());
}

#[test]
fn timing_and_clipboard_work_inside_symbols_without_editing_scene() {
    let (ctx, mut app) = app();
    let id = app.editor.new_symbol("Nested", SymbolKind::Graphic);
    app.editor.enter_symbol(id);
    app.editor.timeline_mut().layers = fixture().scene.layers;
    let scene = app.editor.doc.scene.clone();
    let before = app.editor.doc.clone();
    app.select_frame_range((0, 0), (1, 3));
    app.dispatch(Command::CopyFrames, &ctx);
    app.select_timeline_cell(0, 20);
    app.dispatch(Command::PasteFrames, &ctx);
    app.editor.set_frame_label("intro");
    app.set_timeline_duration(0, 30);
    assert_eq!(app.editor.doc.scene, scene);
    assert_eq!(
        app.editor.timeline().layers[0].keyframes[&20]
            .label
            .as_deref(),
        Some("intro")
    );
    assert_eq!(app.editor.timeline().layers[0].end, 30);
    roundtrip(&app);
    app.dispatch(Command::Undo, &ctx);
    app.dispatch(Command::Undo, &ctx);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, before);
    assert_eq!(app.editor.editing, Some(id));
}

#[test]
fn duration_gesture_is_one_edit_and_ruler_scrubs_without_editing() {
    let mut h = Harness::builder()
        .with_size(vec2(1600.0, 1000.0))
        .build_eframe(|cc| {
            let mut app = Studio::with_preferences(
                &cc.egui_ctx,
                Preferences {
                    locale: Locale::English,
                    ..Default::default()
                },
            );
            app.replace(fixture(), None, false);
            app
        });
    h.run();
    let last = cell(&h, "A", 12);
    let next = cell(&h, "A", 13);
    let start = pos2(next.left(), last.center().y);
    let end = cell(&h, "A", 24).center();
    drag(&mut h, start, end, true, egui::Modifiers::NONE);
    assert_eq!(h.state().editor.timeline().layers[0].end, 12);
    assert_eq!(h.state().editor.undo_depth(), 0);
    drag(&mut h, start, end, false, egui::Modifiers::NONE);
    assert_eq!(h.state().editor.timeline().layers[0].end, 24);
    assert_eq!(h.state().editor.undo_depth(), 1);
    assert_eq!(h.state().editor.timeline().layers[1].end, 12);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(h.state().editor.timeline().layers[0].end, 12);
    let revision = h.state().editor.revision;
    let ruler = cell(&h, "A", 30).center() - vec2(0.0, 24.0);
    click(&mut h, ruler, egui::Modifiers::NONE);
    assert_eq!(h.state().editor.frame, 29);
    assert_eq!(h.state().editor.revision, revision);
}

#[test]
fn frame_bake_matches_evaluation_and_preserves_surrounding_keys() {
    let (ctx, mut app) = app();
    app.editor.timeline_mut().layers[0]
        .keyframes
        .get_mut(&0)
        .unwrap()
        .tween = Tween::Classic {
        ease: 0.0,
        rotate_turns: 0,
    };
    let original = app.editor.doc.clone();
    let evaluated: Vec<_> = (0..4)
        .map(|f| {
            ac_core::layer_elements_at(&original.scene.layers[0], f)
                .unwrap()
                .0
        })
        .collect();
    app.select_frame_range((0, 0), (0, 3));
    app.dispatch(Command::FrameByFrame, &ctx);
    for f in 0..4 {
        assert_eq!(
            ac_core::layer_elements_at(&app.editor.timeline().layers[0], f)
                .unwrap()
                .0,
            evaluated[f as usize]
        );
        assert_eq!(
            app.editor.timeline().layers[0].keyframes[&f].tween,
            Tween::None
        );
    }
    assert_eq!(
        app.editor.timeline().layers[0].keyframes[&8],
        original.scene.layers[0].keyframes[&8]
    );
    assert_eq!(app.editor.timeline().layers[1], original.scene.layers[1]);
    roundtrip(&app);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, original);
}

#[test]
fn layer_context_menu_deletes_and_undo_restores_content() {
    let mut h = Harness::builder()
        .with_size(vec2(1600.0, 1000.0))
        .build_eframe(|cc| {
            let mut app = Studio::with_preferences(
                &cc.egui_ctx,
                Preferences {
                    locale: Locale::English,
                    ..Default::default()
                },
            );
            app.replace(fixture(), None, false);
            app
        });
    h.run();
    let before = h.state().editor.doc.clone();
    let pos = h.get_by_label("B").rect().center();
    h.hover_at(pos);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run();
    h.get_by_label("Delete layer and contents").click();
    h.run();
    assert_eq!(h.state().editor.timeline().layers.len(), 1);
    assert_eq!(h.state().editor.timeline().layers[0].name, "A");
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(h.state().editor.doc, before);
}

#[test]
fn folder_drop_and_bulk_flags_preserve_content_and_clear_protected_selection() {
    let (ctx, mut app) = app();
    app.dispatch(Command::AddFolder, &ctx);
    assert!(app.drop_layers_at(2, 0, 0.5));
    assert_eq!(app.editor.timeline().layers[2].parent, Some(0));
    let before = app.editor.doc.clone();
    app.editor.layer = 1;
    app.editor.selection = vec![ac_core::Selection {
        layer: 1,
        element: 0,
    }];
    app.set_layer_flags(&[0], true, true);
    assert!(app.editor.selection.is_empty());
    assert!(!app.timeline_editable(1));
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, before);
    app.dispatch(Command::CollapseFolders, &ctx);
    assert!(app.editor.layer_collapsed_away(1));
    app.dispatch(Command::ExpandFolders, &ctx);
    assert!(!app.editor.layer_collapsed_away(1));
    app.dispatch(Command::HideAllLayers, &ctx);
    assert!(app.editor.evaluate_current(0).is_empty());
    app.dispatch(Command::ShowAllLayers, &ctx);
    assert_eq!(app.editor.evaluate_current(0).len(), 2);
    app.dispatch(Command::LockAllLayers, &ctx);
    assert!(app.editor.timeline().layers.iter().all(|l| l.locked));
    app.dispatch(Command::UnlockAllLayers, &ctx);
    assert!(app.editor.timeline().layers.iter().all(|l| !l.locked));
    app.solo_layer_flag(1, false);
    assert!(
        app.editor.timeline().layers[0].visible,
        "keep the selected layer's parent visible"
    );
    assert!(!app.editor.timeline().layers[2].visible);
    assert_eq!(app.editor.evaluate_current(0).len(), 1);
    app.solo_layer_flag(1, false);
    assert!(app.editor.timeline().layers.iter().all(|l| l.visible));
    roundtrip(&app);
}
