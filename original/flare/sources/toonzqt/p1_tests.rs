use super::timeline_tests::{app, click, fixture, roundtrip};
use super::*;
use ac_core::{ElementKind, FrameSel, Keyframe, LayerKind};
use egui_kittest::{kittest::Queryable, Harness};

#[test]
fn exact_frame_sets_preserve_holes_across_clipboard_mutation_and_undo() {
    let (ctx, mut studio) = app();
    studio.select_timeline_cell(0, 0);
    studio.toggle_frame_cell(0, 4);
    studio.toggle_frame_cell(1, 8);
    let before = studio.editor.doc.clone();
    assert!(!studio.frame_selected(0, 2));
    studio.dispatch(Command::CopyFrames, &ctx);
    studio.dispatch(Command::BlankKeyframe, &ctx);
    assert_eq!(studio.editor.undo_depth(), 1);
    assert!(studio.editor.timeline().layers[0].keyframes[&0]
        .elements
        .is_empty());
    assert!(studio.editor.timeline().layers[0].keyframes[&4]
        .elements
        .is_empty());
    assert_eq!(
        studio.editor.timeline().layers[0].keyframes[&8],
        before.scene.layers[0].keyframes[&8]
    );
    assert_eq!(
        studio.editor.timeline().layers[1].keyframes[&4],
        before.scene.layers[1].keyframes[&4]
    );
    studio.dispatch(Command::Undo, &ctx);
    assert_eq!(studio.editor.doc, before);
    studio.select_timeline_cell(0, 1);
    studio.dispatch(Command::PasteOverwriteFrames, &ctx);
    assert!(studio.frame_selected(0, 1));
    assert!(studio.frame_selected(0, 5));
    assert!(studio.frame_selected(1, 9));
    assert!(!studio.frame_selected(0, 3));
    assert_eq!(
        studio.editor.timeline().layers[0].keyframes[&4],
        before.scene.layers[0].keyframes[&4]
    );
    roundtrip(&studio);
    studio.dispatch(Command::Undo, &ctx);
    assert_eq!(studio.editor.doc, before);
    studio.select_timeline_cell(0, 0);
    studio.toggle_frame_cell(1, 8);
    studio.editor.timeline_mut().layers[1].locked = true;
    let locked = studio.editor.doc.clone();
    studio.dispatch(Command::RemoveFrame, &ctx);
    assert_eq!(studio.editor.doc, locked);
}

#[test]
fn paste_replaces_selection_while_overwrite_preserves_tail_positions() {
    let (ctx, mut studio) = app();
    studio.select_frame_range((0, 0), (0, 3));
    studio.dispatch(Command::CopyFrames, &ctx);
    let before = studio.editor.doc.clone();
    studio.select_frame_range((0, 4), (0, 5));
    studio.dispatch(Command::PasteFrames, &ctx);
    assert_eq!(studio.editor.timeline().layers[0].end, 14);
    assert_eq!(
        studio.editor.timeline().layers[0].keyframes[&10],
        before.scene.layers[0].keyframes[&8]
    );
    assert_eq!(studio.editor.timeline().layers[1], before.scene.layers[1]);
    roundtrip(&studio);
    studio.dispatch(Command::Undo, &ctx);
    studio.select_frame_range((0, 4), (0, 5));
    studio.dispatch(Command::PasteOverwriteFrames, &ctx);
    assert_eq!(studio.editor.timeline().layers[0].end, 12);
    assert_eq!(
        studio.editor.timeline().layers[0].keyframes[&8],
        before.scene.layers[0].keyframes[&8]
    );
    studio.dispatch(Command::Undo, &ctx);
    studio.editor.timeline_mut().layers[0].end = document::MAX_FRAMES;
    studio.select_timeline_cell(0, 4);
    assert!(!studio.enabled(Command::PasteFrames));
    assert!(studio.enabled(Command::PasteOverwriteFrames));
    studio.toggle_frame_cell(0, 8);
    assert!(
        !studio.enabled(Command::InsertFrame),
        "aggregate insertion must respect the limit"
    );
}

#[test]
fn cross_document_clipboards_remap_dependencies_and_undo_assets_with_content() {
    let (ctx, mut studio) = app();
    let bitmap =
        ac_core::bitmap::BitmapAsset::from_rgba(7, "pixel", 1, 1, &[255, 0, 0, 255]).unwrap();
    studio.editor.doc.bitmaps.push(bitmap);
    let mut shape = Shape::new(
        Path::rect(0.0, 0.0, 1.0, 1.0),
        Some(Color::rgb(255, 0, 0)),
        None,
    );
    shape.bitmap_fill = Some(ac_core::BitmapFill {
        bitmap: 7,
        matrix: Matrix::IDENTITY,
    });
    let mut tl = ac_core::Timeline::new("Asset");
    tl.layers[0]
        .keyframes
        .insert(0, Keyframe::with(vec![Element::shape("Image", shape)]));
    let id = studio
        .editor
        .doc
        .add_symbol("Asset", SymbolKind::Graphic, tl);
    studio.editor.timeline_mut().layers[0].keyframes.insert(
        0,
        Keyframe::with(vec![Element::instance("Use", id, Matrix::IDENTITY)]),
    );
    studio.select_layer_rows(0, false, false);
    studio.dispatch(Command::CopyLayers, &ctx);
    studio.select_timeline_cell(0, 0);
    studio.dispatch(Command::CopyFrames, &ctx);
    let mut destination = fixture();
    destination.add_symbol(
        "Collision",
        SymbolKind::Graphic,
        ac_core::Timeline::new("Different"),
    );
    destination.bitmaps.push(
        ac_core::bitmap::BitmapAsset::from_rgba(7, "pixel", 1, 1, &[0, 0, 255, 255]).unwrap(),
    );
    studio.replace(destination.clone(), None, false);
    studio.dispatch(Command::PasteLayers, &ctx);
    assert_eq!(studio.editor.undo_depth(), 1);
    assert_eq!(studio.editor.doc.library.len(), 2);
    assert_eq!(studio.editor.doc.bitmaps.len(), 2);
    let ElementKind::Instance(instance) =
        &studio.editor.timeline().layers[0].keyframes[&0].elements[0].kind
    else {
        panic!()
    };
    assert_ne!(instance.symbol, id);
    let symbol = studio.editor.doc.symbol(instance.symbol).unwrap();
    let ElementKind::Shape(shape) = &symbol.timeline.layers[0].keyframes[&0].elements[0].kind
    else {
        panic!()
    };
    assert_ne!(shape.bitmap_fill.unwrap().bitmap, 7);
    roundtrip(&studio);
    studio.dispatch(Command::PasteLayers, &ctx);
    assert_eq!(
        studio.editor.doc.library.len(),
        2,
        "repeat paste reuses identical imported symbols"
    );
    assert_eq!(studio.editor.doc.bitmaps.len(), 2);
    studio.dispatch(Command::Undo, &ctx);
    studio.dispatch(Command::Undo, &ctx);
    assert_eq!(studio.editor.doc, destination);
    studio.select_timeline_cell(0, 2);
    studio.dispatch(Command::PasteOverwriteFrames, &ctx);
    assert_eq!(studio.editor.doc.library.len(), 2);
    assert_eq!(studio.editor.doc.bitmaps.len(), 2);
    assert_eq!(studio.editor.undo_depth(), 1);
    roundtrip(&studio);
    studio.dispatch(Command::Undo, &ctx);
    assert_eq!(studio.editor.doc, destination);
}

#[test]
fn span_preference_and_ctrl_selection_work_through_localized_ui() {
    for locale in [Locale::English, Locale::French] {
        let mut h = Harness::builder()
            .with_size(vec2(1600.0, 1100.0))
            .build_eframe(|cc| {
                let mut studio = Studio::with_preferences(
                    &cc.egui_ctx,
                    Preferences {
                        locale,
                        span_selection: true,
                        ..Default::default()
                    },
                );
                studio.replace(fixture(), None, false);
                studio
            });
        h.run();
        let cell = |h: &Harness<Studio>, frame: u32| {
            h.get_by_label(&h.state().i18n.format(
                "timeline-cell",
                &[("layer", "A"), ("frame", &frame.to_string())],
            ))
            .rect()
            .center()
        };
        let p = cell(&h, 3);
        click(&mut h, p, egui::Modifiers::NONE);
        assert_eq!(
            h.state().editor.frame_sel,
            Some(FrameSel {
                layer0: 0,
                layer1: 0,
                f0: 0,
                f1: 3
            })
        );
        let p = cell(&h, 10);
        click(&mut h, p, egui::Modifiers::SHIFT);
        assert!(h.state().frame_selected(0, 0));
        assert!(h.state().frame_selected(0, 11));
        assert!(!h.state().frame_selected(0, 5));
        let p = cell(&h, 3);
        click(&mut h, p, egui::Modifiers::NONE);
        let p = cell(&h, 6);
        click(&mut h, p, egui::Modifiers::COMMAND);
        assert!(!h.state().frame_selected(0, 0));
        assert!(h.state().frame_selected(0, 5));
        let p = cell(&h, 10);
        click(&mut h, p, egui::Modifiers::COMMAND);
        assert!(h.state().frame_selected(0, 5));
        assert!(h.state().frame_selected(0, 9));
        assert!(!h.state().frame_selected(0, 7));
        h.key_press(egui::Key::F7);
        h.run();
        assert!(h.state().editor.timeline().layers[0].keyframes[&5]
            .elements
            .is_empty());
        assert!(h.state().editor.timeline().layers[0].keyframes[&9]
            .elements
            .is_empty());
        roundtrip(h.state());
    }
}

#[test]
fn layer_types_outline_and_inspector_protection_preserve_history() {
    let (ctx, mut studio) = app();
    let before = studio.editor.doc.clone();
    studio.set_layer_kind_checked(&[0, 1], LayerKind::Guide);
    assert_eq!(studio.editor.undo_depth(), 1);
    assert!(studio
        .editor
        .timeline()
        .layers
        .iter()
        .all(|l| l.kind == LayerKind::Guide));
    studio.dispatch(Command::Undo, &ctx);
    assert_eq!(studio.editor.doc, before);
    studio.editor.timeline_mut().layers[0].locked = true;
    assert!(!studio.can_set_layer_kind(&[0, 1], LayerKind::Mask));
    let protected = studio.editor.doc.clone();
    studio.set_layer_kind_checked(&[0, 1], LayerKind::Mask);
    assert_eq!(studio.editor.doc, protected);
}

#[test]
fn layer_heights_survive_undo_clipboard_and_disk_with_legacy_default() {
    let (ctx, mut studio) = app();
    let original = studio.editor.doc.clone();
    studio.set_layer_height(&[0, 1], 300);
    assert_eq!(studio.editor.undo_depth(), 1);
    roundtrip(&studio);
    studio.select_layer_rows(0, false, false);
    studio.dispatch(Command::CopyLayers, &ctx);
    studio.dispatch(Command::Undo, &ctx);
    assert_eq!(studio.editor.doc, original);
    studio.replace(fixture(), None, false);
    studio.dispatch(Command::PasteLayers, &ctx);
    assert_eq!(studio.editor.timeline().layers[0].row_height, 300);
    roundtrip(&studio);
    let mut value = serde_json::to_value(&original).unwrap();
    for layer in value["scene"]["layers"].as_array_mut().unwrap() {
        layer.as_object_mut().unwrap().remove("row_height");
    }
    let loaded: ac_core::Document = serde_json::from_value(value).unwrap();
    assert_eq!(loaded, original);
}

#[test]
fn layer_drag_autoscroll_reaches_offscreen_rows_and_undo_restores_order() {
    let mut h = Harness::builder()
        .with_size(vec2(1600.0, 1000.0))
        .build_eframe(|cc| {
            let mut studio = Studio::with_preferences(
                &cc.egui_ctx,
                Preferences {
                    locale: Locale::English,
                    ..Default::default()
                },
            );
            let mut doc = fixture();
            doc.scene.layers = (0..40)
                .map(|i| ac_core::Layer::new(&format!("Row{i}"), i))
                .collect();
            studio.replace(doc, None, false);
            studio
        });
    h.run();
    let original = h.state().editor.doc.clone();
    let start = h.get_by_label("Row0").rect().center();
    click(&mut h, start, egui::Modifiers::NONE);
    h.hover_at(start);
    h.event(egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    let viewport = h.state().timeline_state.viewport.unwrap();
    let end = pos2(start.x, viewport.bottom() - 2.0);
    h.hover_at(end);
    for _ in 0..55 {
        h.step();
    }
    assert_eq!(
        h.state().editor.doc,
        original,
        "autoscroll is a view-only preview"
    );
    assert!(
        h.query_by_label("Row0").is_none(),
        "source row should have scrolled offscreen"
    );
    h.event(egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert!(
        h.state()
            .editor
            .timeline()
            .layers
            .iter()
            .position(|l| l.name == "Row0")
            .unwrap()
            > 6
    );
    assert_eq!(h.state().editor.undo_depth(), 1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(h.state().editor.doc, original);
}

#[test]
fn folder_drop_edges_and_center_preserve_hierarchy_and_reject_self_nesting() {
    for (position, inside) in [(0.1, false), (0.5, true), (0.9, false)] {
        let (ctx, mut studio) = app();
        studio.editor.timeline_mut().layers[1].kind = LayerKind::Folder;
        let mut child = ac_core::Layer::new("Child", 2);
        child.parent = Some(1);
        studio.editor.timeline_mut().layers.push(child);
        let before = studio.editor.doc.clone();
        studio.select_layer_rows(0, false, false);
        let changed = studio.drop_layers_at(0, 1, position);
        let at = studio
            .editor
            .timeline()
            .layers
            .iter()
            .position(|l| l.name == "A")
            .unwrap();
        let folder = studio
            .editor
            .timeline()
            .layers
            .iter()
            .position(|l| l.name == "B")
            .unwrap();
        assert_eq!(
            studio.editor.timeline().layers[at].parent,
            inside.then_some(folder)
        );
        if position > 0.75 {
            assert_eq!(at, 2);
        }
        if position < 0.25 {
            assert!(!changed, "same order is not an edit");
        }
        roundtrip(&studio);
        if changed {
            studio.dispatch(Command::Undo, &ctx);
        }
        assert_eq!(studio.editor.doc, before);
        studio.select_layer_rows(1, false, false);
        assert!(!studio.drop_layers_at(1, 2, 0.5));
        assert!(!studio.can_set_layer_kind(&[1, 2], LayerKind::Mask));
        assert_eq!(studio.editor.doc, before);
    }
}

#[test]
fn static_span_resize_preserves_tail_metadata_and_has_atomic_history() {
    let (ctx, mut studio) = app();
    let key = studio.editor.timeline_mut().layers[0]
        .keyframes
        .get_mut(&8)
        .unwrap();
    key.label = Some("tail".into());
    key.script = Some("stop();".into());
    let original = studio.editor.doc.clone();
    let span = FrameSel {
        layer0: 0,
        layer1: 0,
        f0: 4,
        f1: 7,
    };
    assert!(studio.resize_static_span(span, false, 3));
    assert_eq!(studio.editor.timeline().layers[0].end, 15);
    assert_eq!(
        studio.editor.timeline().layers[0].keyframes[&11],
        original.scene.layers[0].keyframes[&8]
    );
    assert_eq!(studio.editor.undo_depth(), 1);
    roundtrip(&studio);
    studio.dispatch(Command::Undo, &ctx);
    assert_eq!(studio.editor.doc, original);
    assert!(studio.resize_static_span(span, true, -2));
    assert_eq!(studio.editor.timeline().layers[0].end, 12);
    assert_eq!(
        studio.editor.timeline().layers[0].keyframes[&2],
        original.scene.layers[0].keyframes[&4]
    );
    studio.dispatch(Command::Undo, &ctx);
    assert!(!studio.resize_static_span(span, false, -4));
    assert!(!studio.resize_static_span(span, true, -4));
    assert!(!studio.resize_static_span(span, false, document::MAX_FRAMES as i64));
    assert_eq!(studio.editor.doc, original);
}

#[test]
fn protected_parent_hit_testing_falls_through_to_editable_artwork() {
    let (_, mut studio) = app();
    let mut folder = ac_core::Layer::new("Locked folder", 2);
    folder.kind = LayerKind::Folder;
    folder.locked = true;
    let mut upper = studio.editor.timeline().layers[0].clone();
    upper.parent = Some(0);
    let lower = studio.editor.timeline().layers[0].clone();
    studio.editor.timeline_mut().layers = vec![folder, upper, lower];
    let selected = studio.stage_hit_test(pt(15.0, 15.0)).unwrap();
    assert_eq!(selected.layer, 2);
    studio.editor.timeline_mut().layers[2].locked = true;
    assert!(studio.stage_hit_test(pt(15.0, 15.0)).is_none());
}

#[test]
fn ctrl_drag_span_edge_previews_cancels_and_commits_once() {
    let mut h = Harness::builder()
        .with_size(vec2(1600.0, 1100.0))
        .build_eframe(|cc| {
            let mut studio = Studio::with_preferences(
                &cc.egui_ctx,
                Preferences {
                    locale: Locale::English,
                    ..Default::default()
                },
            );
            studio.replace(fixture(), None, false);
            studio
        });
    h.run();
    let cell = |h: &Harness<Studio>, f| {
        h.get_by_label(&h.state().i18n.format(
            "timeline-cell",
            &[("layer", "A"), ("frame", &format!("{f}"))],
        ))
        .rect()
        .center()
    };
    let before = h.state().editor.doc.clone();
    let start = cell(&h, 8);
    let end = cell(&h, 11);
    super::timeline_tests::drag(&mut h, start, end, true, egui::Modifiers::COMMAND);
    assert_eq!(h.state().editor.doc, before);
    assert_eq!(h.state().editor.undo_depth(), 0);
    super::timeline_tests::drag(&mut h, start, end, false, egui::Modifiers::COMMAND);
    assert_eq!(h.state().editor.timeline().layers[0].end, 15);
    assert_eq!(h.state().editor.undo_depth(), 1);
    roundtrip(h.state());
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(h.state().editor.doc, before);
}
