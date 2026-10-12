use super::timeline_tests::{app as test_app, click, drag, fixture, roundtrip};
use super::*;
use ac_core::{Keyframe, Layer, LayerKind};
use egui_kittest::{kittest::Queryable, Harness};

#[test]
fn noncontiguous_layer_clipboard_ui_preserves_unselected_rows_in_both_languages() {
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
                    let mut doc = fixture();
                    doc.scene.layers.push(Layer::new("C", 2));
                    app.replace(doc, None, false);
                    app
                });
            h.run();
            let original = h.state().editor.doc.clone();
            let a = h.get_by_label("A").rect().center();
            let c = h.get_by_label("C").rect().center();
            click(&mut h, a, egui::Modifiers::NONE);
            click(&mut h, c, egui::Modifiers::COMMAND);
            assert!(h.state().layer_row_selected(0));
            assert!(!h.state().layer_row_selected(1));
            assert!(h.state().layer_row_selected(2));
            assert!(
                !h.state().enabled(Command::ClearFrames),
                "a layer set must not become a bounding frame rectangle"
            );
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::C);
            h.run();
            assert_eq!(h.state().editor.doc, original);
            assert_eq!(h.state().editor.undo_depth(), 0);
            // Keep the layer set when opening the menu on any selected row.
            let pos = h.get_by_label("A").rect().center();
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
            h.get_by_label(&h.state().i18n.text("cmd-cut-layers"))
                .click();
            h.run();
            assert_eq!(h.state().editor.timeline().layers.len(), 1);
            assert_eq!(
                h.state().editor.timeline().layers[0],
                original.scene.layers[1]
            );
            assert_eq!(h.state().editor.undo_depth(), 1);
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::V);
            h.run();
            assert_eq!(
                h.state()
                    .editor
                    .timeline()
                    .layers
                    .iter()
                    .map(|l| l.name.as_str())
                    .collect::<Vec<_>>(),
                ["A", "C", "B"]
            );
            assert_eq!(
                h.state().editor.timeline().layers[0],
                original.scene.layers[0]
            );
            assert_eq!(
                h.state().editor.timeline().layers[2],
                original.scene.layers[1]
            );
            assert_eq!(h.state().editor.undo_depth(), 2);
            roundtrip(h.state());
            for _ in 0..2 {
                h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
                h.run();
            }
            assert_eq!(h.state().editor.doc, original);
        }
    }
}

#[test]
fn folder_clipboard_pastes_into_symbol_and_scene_with_local_parent_indices() {
    let (ctx, mut app) = test_app();
    app.editor.timeline_mut().layers[1].anim_parent = Some(0);
    app.dispatch(Command::AddFolder, &ctx);
    app.select_layer_rows(2, false, false);
    app.dispatch(Command::IndentLayer, &ctx);
    app.select_layer_rows(0, false, false);
    let source = app.editor.timeline().clone();
    app.dispatch(Command::CopyLayers, &ctx);
    let id = app.editor.new_symbol("Destination", SymbolKind::Graphic);
    app.editor.enter_symbol(id);
    let before = app.editor.doc.clone();
    let depth = app.editor.undo_depth();
    app.dispatch(Command::PasteLayers, &ctx);
    assert_eq!(app.editor.undo_depth(), depth + 1);
    assert_eq!(app.editor.doc.scene, source);
    assert_eq!(app.editor.timeline().layers[..3], source.layers);
    assert_eq!(app.editor.timeline().layers[2].anim_parent, Some(1));
    assert_eq!(app.editor.timeline().layers[2].parent, Some(0));
    roundtrip(&app);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, before);
    app.dispatch(Command::Redo, &ctx);
    let symbol = app.editor.timeline().clone();
    app.editor.add_scene();
    app.dispatch(Command::PasteLayers, &ctx);
    assert_eq!(app.editor.timeline().layers[..3], source.layers);
    assert_eq!(app.editor.doc.symbol(id).unwrap().timeline, symbol);
    roundtrip(&app);
}

#[test]
fn paste_into_folder_preserves_existing_links_and_rejects_locked_capacity_or_recursive_targets() {
    let (ctx, mut app) = test_app();
    app.select_layer_rows(1, false, false);
    app.dispatch(Command::CopyLayers, &ctx);
    app.select_layer_rows(0, false, false);
    app.dispatch(Command::AddFolder, &ctx);
    let original = app.editor.doc.clone();
    app.editor.timeline_mut().layers[0].locked = true;
    let locked = app.editor.doc.clone();
    assert!(!app.enabled(Command::PasteLayers));
    app.dispatch(Command::PasteLayers, &ctx);
    assert_eq!(app.editor.doc, locked);
    app.editor.timeline_mut().layers[0].locked = false;
    app.dispatch(Command::PasteLayers, &ctx);
    assert_eq!(app.editor.timeline().layers[1].name, "B");
    assert_eq!(app.editor.timeline().layers[1].parent, Some(0));
    assert_eq!(app.editor.timeline().layers[2].name, "A");
    assert_eq!(app.editor.timeline().layers[2].parent, Some(0));
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, original);
    while app.editor.timeline().layers.len() < 256 {
        let n = app.editor.timeline().layers.len();
        app.editor
            .timeline_mut()
            .layers
            .push(Layer::new("Limit", n));
    }
    assert!(!app.enabled(Command::PasteLayers));

    let (ctx, mut app) = test_app();
    let a = app.editor.new_symbol("A", SymbolKind::Graphic);
    let b = app.editor.new_symbol("B", SymbolKind::Graphic);
    app.editor.doc.symbol_mut(b).unwrap().timeline.layers[0]
        .keyframes
        .insert(
            0,
            Keyframe::with(vec![Element::instance("link", a, Matrix::IDENTITY)]),
        );
    app.editor.timeline_mut().layers[0].keyframes.insert(
        0,
        Keyframe::with(vec![Element::instance("link", b, Matrix::IDENTITY)]),
    );
    app.select_layer_rows(0, false, false);
    app.dispatch(Command::CopyLayers, &ctx);
    app.editor.enter_symbol(a);
    let before = app.editor.doc.clone();
    let depth = app.editor.undo_depth();
    assert!(
        !app.enabled(Command::PasteLayers),
        "indirect self-reference is rejected"
    );
    app.dispatch(Command::PasteLayers, &ctx);
    assert_eq!(app.editor.doc, before);
    assert_eq!(app.editor.undo_depth(), depth);
    app.editor.exit_to_scene();
    app.editor.doc.library.retain(|s| s.id != b);
    assert!(
        !app.enabled(Command::PasteLayers),
        "stale clipboard symbol references are rejected"
    );
}

#[test]
fn linked_parent_dependencies_are_copied_without_baking_or_losing_animation() {
    let (ctx, mut app) = test_app();
    app.editor.timeline_mut().layers[1].anim_parent = Some(0);
    app.editor.timeline_mut().layers[0]
        .keyframes
        .get_mut(&0)
        .unwrap()
        .tween = Tween::Classic {
        ease: 0.2,
        rotate_turns: 0,
    };
    app.select_layer_rows(1, false, false);
    let source = app.editor.timeline().clone();
    app.dispatch(Command::CopyLayers, &ctx);
    assert_eq!(
        app.status,
        app.i18n.format(
            "status-layer-clipboard",
            &[("count", "2"), ("dependencies", "1")]
        )
    );
    let id = app.editor.new_symbol("Destination", SymbolKind::Graphic);
    app.editor.enter_symbol(id);
    app.dispatch(Command::PasteLayers, &ctx);
    assert_eq!(app.editor.timeline().layers[0..2], source.layers);
    for frame in 0..12 {
        assert_eq!(
            ac_core::layer_parent_matrix(app.editor.timeline(), 1, frame),
            ac_core::layer_parent_matrix(&source, 1, frame)
        );
    }
    assert_eq!(
        app.editor.timeline().layers[0].keyframes.len(),
        3,
        "do not bake into per-frame keys"
    );
    roundtrip(&app);
}

#[test]
fn layer_group_drag_moves_noncontiguous_rows_without_moving_unselected_artwork() {
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
            let mut doc = fixture();
            doc.scene.layers.push(Layer::new("C", 2));
            doc.scene.layers.push(Layer::new("D", 3));
            doc.scene.layers[2].anim_parent = Some(0);
            app.replace(doc, None, false);
            app
        });
    h.run();
    let before = h.state().editor.doc.clone();
    let a = h.get_by_label("A").rect().center();
    let c = h.get_by_label("C").rect().center();
    let d = h.get_by_label("D").rect().center();
    click(&mut h, a, egui::Modifiers::NONE);
    click(&mut h, c, egui::Modifiers::COMMAND);
    drag(&mut h, c, d, true, egui::Modifiers::NONE);
    assert_eq!(h.state().editor.doc, before);
    click(&mut h, a, egui::Modifiers::NONE);
    click(&mut h, c, egui::Modifiers::COMMAND);
    drag(&mut h, c, d, false, egui::Modifiers::NONE);
    assert_eq!(
        h.state()
            .editor
            .timeline()
            .layers
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        ["B", "D", "A", "C"]
    );
    assert_eq!(h.state().editor.timeline().layers[3].anim_parent, Some(2));
    assert_eq!(
        h.state().editor.timeline().layers[0],
        before.scene.layers[1]
    );
    assert_eq!(h.state().editor.undo_depth(), 1);
    roundtrip(h.state());
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(h.state().editor.doc, before);
}

#[test]
fn layer_selection_context_does_not_leak_into_frame_edits_or_another_document() {
    let (ctx, mut app) = test_app();
    app.select_layer_rows(0, false, false);
    app.select_layer_rows(1, false, true);
    assert!(app.layer_selection_active());
    app.dispatch(Command::CopyLayers, &ctx);
    app.select_timeline_cell(0, 3);
    assert!(!app.layer_selection_active());
    app.dispatch(Command::BlankKeyframe, &ctx);
    assert!(app.editor.timeline().layers[0].keyframes[&3]
        .elements
        .is_empty());
    assert!(!app.editor.timeline().layers[1].keyframes.contains_key(&3));
    app.replace(fixture(), None, false);
    assert!(
        app.enabled(Command::PasteLayers),
        "clipboard survives replacement for cross-document paste"
    );
    app.select_layer_rows(0, false, false);
    app.select_layer_rows(1, true, false);
    app.editor.timeline_mut().layers[1].locked = true;
    assert!(!app.enabled(Command::CutLayers));
    assert!(!app.enabled(Command::DeleteLayer));
    assert!(
        app.enabled(Command::CopyLayers),
        "copy may inspect locked content"
    );
    let before = app.editor.doc.clone();
    app.dispatch(Command::DeleteLayer, &ctx);
    assert_eq!(app.editor.doc, before);
    assert_eq!(app.editor.timeline().layers[0].kind, LayerKind::Normal);
}

#[test]
fn deep_folder_clipboard_and_group_moves_preserve_hierarchy_and_protected_descendants() {
    let (ctx, mut app) = test_app();
    let mut doc = fixture();
    let a = doc.scene.layers.remove(0);
    let b = doc.scene.layers.remove(0);
    let mut outer = Layer::new("Outer", 0);
    outer.kind = LayerKind::Folder;
    let mut inner = Layer::new("Inner", 1);
    inner.kind = LayerKind::Folder;
    inner.parent = Some(0);
    let mut a = a;
    a.parent = Some(1);
    let mut b = b;
    b.parent = Some(0);
    doc.scene.layers = vec![outer, inner, a, b, Layer::new("Other", 4)];
    app.replace(doc, None, false);
    let original = app.editor.doc.clone();
    app.select_layer_rows(0, false, false);
    app.dispatch(Command::CopyLayers, &ctx);
    app.select_layer_rows(4, false, false);
    app.dispatch(Command::PasteLayers, &ctx);
    for i in 0..4 {
        let mut expected = original.scene.layers[i].clone();
        expected.parent = expected.parent.map(|p| p + 4);
        assert_eq!(app.editor.timeline().layers[i + 4], expected);
    }
    roundtrip(&app);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, original);

    // A loaded tree may interleave roots and descendants. A locked late child
    // still protects the entire selected subtree, independent of row adjacency.
    app.editor.timeline_mut().layers.swap(3, 4);
    app.editor.timeline_mut().layers[4].locked = true;
    app.select_layer_rows(0, false, false);
    let protected = app.editor.doc.clone();
    assert!(!app.enabled(Command::CutLayers));
    assert!(!app.enabled(Command::DeleteLayer));
    assert!(!app.drop_layers_at(0, 3, 0.5));
    assert_eq!(app.editor.doc, protected);
    app.editor.timeline_mut().layers[4].locked = false;
    assert!(app.drop_layers_at(0, 3, 0.5));
    assert_eq!(app.editor.timeline().layers[0].name, "Other");
    assert_eq!(app.editor.timeline().layers[3].parent, Some(2));
    assert_eq!(app.editor.timeline().layers[4].parent, Some(1));
    roundtrip(&app);
}

#[test]
fn mask_dependencies_and_duplicate_destination_use_valid_container_rules() {
    let (ctx, mut app) = test_app();
    app.editor.timeline_mut().layers[0].kind = LayerKind::Mask;
    app.editor.timeline_mut().layers[1].parent = Some(0);
    app.select_layer_rows(1, false, false);
    app.dispatch(Command::CopyLayers, &ctx);
    // Copying masked artwork retains its mask, so pasting that container under
    // another mask must be rejected instead of silently dropping the mask.
    assert!(!app.enabled(Command::PasteLayers));
    assert!(app.enabled(Command::DuplicateLayer));
    let before = app.editor.doc.clone();
    app.dispatch(Command::DuplicateLayer, &ctx);
    assert_eq!(app.editor.timeline().layers.len(), 4);
    assert_eq!(app.editor.timeline().layers[1].parent, Some(0));
    assert_eq!(app.editor.timeline().layers[3].parent, Some(2));
    roundtrip(&app);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.editor.doc, before);

    let mut folder = Layer::new("Folder", 2);
    folder.kind = LayerKind::Folder;
    app.editor.timeline_mut().layers.push(folder);
    app.select_layer_rows(2, false, false);
    let before = app.editor.doc.clone();
    assert!(!app.drop_layers_at(2, 1, 0.5));
    assert_eq!(app.editor.doc, before);

    // Duplicating a locked folder is a read-only source operation. The copies
    // are inserted alongside it, not into its protected contents.
    app.editor.timeline_mut().layers[2].locked = true;
    assert!(app.enabled(Command::DuplicateLayer));
    assert!(!app.enabled(Command::PasteLayers));
    app.dispatch(Command::DuplicateLayer, &ctx);
    assert!(app.editor.timeline().layers[2].locked);
    assert_eq!(app.editor.timeline().layers[2].parent, None);
    roundtrip(&app);
}

#[test]
fn direct_rename_and_selected_layer_properties_are_undoable_in_both_languages() {
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
                    let mut doc = fixture();
                    doc.scene.layers.push(Layer::new("C", 2));
                    app.replace(doc, None, false);
                    app
                });
            h.run();
            let original = h.state().editor.doc.clone();
            let a = h.get_by_label("A").rect().center();
            click(&mut h, a, egui::Modifiers::NONE);
            h.key_press(egui::Key::F2);
            h.run();
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
            h.event(egui::Event::Text("Foreground".into()));
            h.run();
            assert_eq!(h.state().editor.doc, original, "typing is a preview");
            h.key_press(egui::Key::Escape);
            h.run();
            assert_eq!(h.state().editor.doc, original);
            h.key_press(egui::Key::F2);
            h.run();
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
            h.event(egui::Event::Text("Foreground".into()));
            h.run();
            h.key_press(egui::Key::Enter);
            h.run();
            assert_eq!(h.state().editor.timeline().layers[0].name, "Foreground");
            assert_eq!(h.state().editor.undo_depth(), 1);
            roundtrip(h.state());
            let a = h.get_by_label("Foreground").rect().center();
            let c = h.get_by_label("C").rect().center();
            click(&mut h, a, egui::Modifiers::NONE);
            click(&mut h, c, egui::Modifiers::COMMAND);
            h.get_by_label(&h.state().i18n.text("field-layer-locked"))
                .click();
            h.run();
            assert!(h.state().editor.timeline().layers[0].locked);
            assert!(!h.state().editor.timeline().layers[1].locked);
            assert!(h.state().editor.timeline().layers[2].locked);
            assert_eq!(h.state().editor.undo_depth(), 2);
            h.key_press(egui::Key::F2);
            h.run();
            h.event(egui::Event::Text("Protected".into()));
            h.key_press(egui::Key::Enter);
            h.run();
            assert_eq!(h.state().editor.timeline().layers[2].name, "C");
            for _ in 0..2 {
                h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
                h.run();
            }
            assert_eq!(h.state().editor.doc, original);
        }
    }
}

#[test]
fn dragging_flag_columns_previews_cancels_and_commits_one_edit() {
    for lock in [false, true] {
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
                let mut doc = fixture();
                doc.scene.layers.push(Layer::new("C", 2));
                app.replace(doc, None, false);
                app
            });
        h.run();
        let key = if lock {
            "action-lock-layer"
        } else {
            "action-show-layer"
        };
        let flags: Vec<_> = h
            .query_all_by_label(&h.state().i18n.text(key))
            .map(|n| n.rect().center())
            .collect();
        assert_eq!(flags.len(), 3);
        let original = h.state().editor.doc.clone();
        drag(&mut h, flags[0], flags[2], true, egui::Modifiers::NONE);
        assert_eq!(h.state().editor.doc, original);
        assert_eq!(h.state().editor.undo_depth(), 0);
        drag(&mut h, flags[0], flags[2], false, egui::Modifiers::NONE);
        assert!(h.state().editor.timeline().layers.iter().all(|l| if lock {
            l.locked
        } else {
            !l.visible
        }));
        assert_eq!(h.state().editor.undo_depth(), 1);
        roundtrip(h.state());
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        h.run();
        assert_eq!(h.state().editor.doc, original);
        // Releasing outside the layer strip is cancellation, not a partial edit.
        drag(
            &mut h,
            flags[0],
            pos2(600.0, 500.0),
            false,
            egui::Modifiers::NONE,
        );
        assert_eq!(h.state().editor.doc, original);
    }
}

#[test]
fn localized_layer_inspector_does_not_change_demo_pixels_or_document() {
    let expected = document::example();
    let mut baseline = None;
    for locale in [Locale::English, Locale::French, Locale::Expanded] {
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
                    app.dispatch(Command::Demo, &cc.egui_ctx);
                    app
                });
            h.run();
            assert_eq!(h.state().editor.doc, expected);
            let pixels = h.state().render_pixels(1.0).unwrap();
            if let Some(data) = &baseline {
                assert_eq!(pixels.data(), data);
            } else {
                baseline = Some(pixels.data().to_vec());
            }
            assert!(pixels
                .data()
                .chunks_exact(4)
                .any(|p| p[0] == 235 && p[1] == 112 && p[2] == 74));
        }
    }
}
