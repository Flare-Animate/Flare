use super::*;

/// Every panel in one tab group, so bringing one to the front is visible in the layout.
fn all_in_one_stack(ws: &mut Workspace) {
    let mut tiles = egui_tiles::Tiles::default();
    let panes: Vec<_> = Pane::ALL
        .into_iter()
        .map(|pane| tiles.insert_pane(pane))
        .collect();
    let root = tiles.insert_tab_tile(panes);
    let tree = egui_tiles::Tree::new(egui::Id::unique("stack"), root, tiles);
    ws.restore_layout(&layout::to_json(&tree).expect("serialize"))
        .expect("a tab group of every panel is valid");
}

#[test]
fn window_menu_entries_bring_their_panel_to_the_front() {
    let mut ws = workspace();
    all_in_one_stack(&mut ws);
    let json = |ws: &Workspace| ws.layout_json().expect("serialize");
    ws.perform(MenuAction::ShowHistory);
    let history = json(&ws);
    ws.perform(MenuAction::ShowLibrary);
    let library = json(&ws);
    assert_ne!(history, library, "a different panel came to the front");
    ws.perform(MenuAction::ShowHistory);
    assert_eq!(json(&ws), history, "and back again");
}

#[test]
fn every_window_entry_is_enabled_and_named_for_its_panel() {
    let ws = workspace();
    for (action, name) in [
        (MenuAction::ShowProperties, "Properties"),
        (MenuAction::ShowLibrary, "Library"),
        (MenuAction::ShowColor, "Color"),
        (MenuAction::ShowAlign, "Align"),
        (MenuAction::ShowTransform, "Transform"),
        (MenuAction::ShowHistory, "History"),
        (MenuAction::ShowTimeline, "Timeline"),
        (MenuAction::ShowOutput, "Output"),
        (MenuAction::ShowInfo, "Info"),
    ] {
        assert!(ws.is_enabled(action), "{name}");
        assert_eq!(ws.label(action), name);
    }
}

#[test]
fn help_windows_open_and_the_shortcut_list_names_real_commands() {
    let mut ws = workspace();
    ws.perform(MenuAction::About);
    ws.perform(MenuAction::KeyboardShortcuts);
    let lines = ws.shortcut_lines();
    let keys = |name: &str| {
        lines
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, k)| k.clone())
    };
    assert!(keys("Undo").is_some_and(|k| k.contains('Z')));
    assert_eq!(keys("Select / Move").as_deref(), Some("V"));
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0); // both windows draw without panicking
}

#[test]
fn clicking_the_outline_chip_cycles_the_layer_color_as_one_undo_step() {
    let mut ws = workspace();
    let id = ws.active_layer_id().expect("a layer");
    let color = |ws: &Workspace| ws.document().scenes()[0].timeline().layers()[0].outline();
    assert_eq!(color(&ws), None, "a new layer has no color of its own");
    ws.cycle_layer_outline(id);
    // Layer 0 shows the first palette color until it is given one, so the next is the second.
    assert_eq!(color(&ws), Some(flare_document::OUTLINE_PALETTE[1]));
    ws.cycle_layer_outline(id);
    assert_eq!(color(&ws), Some(flare_document::OUTLINE_PALETTE[2]));
    ws.undo();
    assert_eq!(color(&ws), Some(flare_document::OUTLINE_PALETTE[1]));
    ws.undo();
    assert_eq!(color(&ws), None);
}

#[test]
fn outline_mode_redraws_the_stage_as_lines_and_back() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_stage_size(40, 40);
    rect(&mut ws);
    run(&mut ws, &ctx, 0.0);
    let normal = ws.frames_rendered;
    ws.perform(MenuAction::ToggleOutlineMode);
    assert_eq!(ws.label(MenuAction::ToggleOutlineMode), "Show Artwork");
    run(&mut ws, &ctx, 0.1);
    assert!(
        ws.frames_rendered > normal,
        "the stage was drawn again in the new mode"
    );
    ws.perform(MenuAction::ToggleOutlineMode);
    assert_eq!(ws.label(MenuAction::ToggleOutlineMode), "Outline Mode");
}

#[test]
fn insert_new_symbol_adds_an_empty_one_and_is_off_while_editing_inside_one() {
    let mut ws = workspace();
    assert!(ws.is_enabled(MenuAction::NewSymbol));
    let before = ws.document().symbols().len();
    ws.perform(MenuAction::NewSymbol);
    assert_eq!(ws.document().symbols().len(), before + 1);
    ws.undo();
    assert_eq!(ws.document().symbols().len(), before);
    let (ws, _) = with_symbol();
    let mut ws = ws;
    let id = ws.document().symbols()[0].id();
    assert!(ws.enter_symbol(id));
    assert!(!ws.is_enabled(MenuAction::NewSymbol));
}

#[test]
fn guides_are_bounded_move_snap_and_go_back_to_the_ruler() {
    use super::super::guides::{Guide, MAX_GUIDES, snap_to_guides};
    let mut ws = workspace();
    assert!(!ws.is_enabled(MenuAction::ClearGuides));
    let first = ws.begin_guide(true, 40.0).expect("a guide");
    assert!(ws.is_enabled(MenuAction::ClearGuides));
    ws.move_guide(first, 55.0);
    ws.move_guide(first, f32::NAN);
    assert_eq!(
        ws.guides(),
        &[Guide {
            vertical: true,
            at: 55.0
        }]
    );
    assert_eq!(ws.begin_guide(false, f32::INFINITY), None, "not a number");
    ws.release_guide(first, true);
    assert!(ws.guides().is_empty(), "dropped on the ruler it goes away");
    for i in 0..MAX_GUIDES {
        assert!(ws.begin_guide(false, i as f32).is_some());
    }
    assert_eq!(ws.begin_guide(false, 1.0), None, "there is a limit");
    ws.perform(MenuAction::ClearGuides);
    assert!(ws.guides().is_empty());

    let guides = [
        Guide {
            vertical: true,
            at: 100.0,
        },
        Guide {
            vertical: false,
            at: 50.0,
        },
    ];
    let snapped = snap_to_guides(egui::pos2(103.0, 90.0), &guides, 5.0);
    assert_eq!(snapped, egui::pos2(100.0, 90.0), "x snaps, y is too far");
    assert_eq!(
        snap_to_guides(egui::pos2(103.0, 52.0), &guides, 1.0),
        egui::pos2(103.0, 52.0),
        "nothing within reach"
    );
}

#[test]
fn the_stage_draws_with_guides_showing_and_hidden() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.begin_guide(true, 10.0);
    ws.begin_guide(false, 20.0);
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.label(MenuAction::ToggleGuides), "Hide Guides");
    ws.perform(MenuAction::ToggleGuides);
    assert_eq!(ws.label(MenuAction::ToggleGuides), "Show Guides");
    run(&mut ws, &ctx, 0.1);
    assert_eq!(ws.guides().len(), 2, "hiding keeps them");
}

#[test]
fn the_info_panel_reports_the_color_under_the_pointer_and_draws() {
    use super::super::panels::info_color_at;
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws);
    let bounds = ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .flat_map(|l| l.spans().iter())
        .find_map(|s| match s.content() {
            Content::Shape(shape) => {
                Some((shape.fill(), ops::shape_bounds(shape).expect("bounds")))
            }
            _ => None,
        })
        .expect("a rectangle");
    let (fill, rect) = bounds;
    let inside = glamx::DVec2::new(rect.center().x.into(), rect.center().y.into());
    assert_eq!(info_color_at(ws.document(), 0, inside), Some(fill));
    assert_eq!(
        info_color_at(ws.document(), 0, glamx::DVec2::new(-500.0, -500.0)),
        None,
        "nothing there"
    );
    ws.perform(MenuAction::ShowInfo);
    run(&mut ws, &ctx, 0.0);
}

#[test]
fn the_document_tab_with_its_publish_settings_draws() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.perform(MenuAction::DocumentSettings);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
}
