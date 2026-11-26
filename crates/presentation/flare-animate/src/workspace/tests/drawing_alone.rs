//! Using Flare to draw, with no animation: the timeline and the output put away so the stage has
//! the room, and pictures exported with a transparent background.

use super::*;
use crate::workspace::rail::PanelChange;
use flare_menubar::MenuEntry;

fn stage_height(ws: &Workspace) -> f32 {
    let tree = &ws.panels.layout.tree;
    tree.tiles
        .iter()
        .find_map(|(id, tile)| match tile {
            egui_tiles::Tile::Pane(Pane::Stage) => tree.tiles.rect(*id),
            _ => None,
        })
        .expect("the stage was drawn")
        .height()
}

#[test]
fn window_timeline_and_output_hide_their_panel_and_give_the_stage_its_room() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let with_timeline = stage_height(&ws);

    ws.perform(MenuAction::ShowTimeline);
    assert!(ws.panels.layout.is_hidden(Pane::Timeline));
    // With the timeline gone the output is the tab in front, so one press hides it too.
    ws.perform(MenuAction::ShowOutput);
    assert!(ws.panels.layout.is_hidden(Pane::Output));
    run(&mut ws, &ctx, 0.2);
    run(&mut ws, &ctx, 0.3);
    assert!(
        stage_height(&ws) > with_timeline + 100.0,
        "the stage took the timeline's room: {} then {}",
        with_timeline,
        stage_height(&ws)
    );

    // Kept between sessions with the arrangement.
    let json = ws.layout_json().expect("serialize");
    let mut again = workspace();
    again.restore_layout(&json).expect("restore");
    assert!(again.panels.layout.is_hidden(Pane::Timeline));
    assert!(again.panels.layout.is_hidden(Pane::Output));

    ws.perform(MenuAction::ShowTimeline);
    assert!(!ws.panels.layout.is_hidden(Pane::Timeline));
    run(&mut ws, &ctx, 0.4);
    run(&mut ws, &ctx, 0.5);
    assert!((stage_height(&ws) - with_timeline).abs() < 2.0, "and back");
    assert!(ws.document().scenes()[0].timeline().layers().len() == 1);
}

#[test]
fn the_hide_button_on_the_timelines_tabs_hides_it_and_reset_layout_brings_it_back() {
    let mut ws = workspace();
    ws.panels.change = Some(PanelChange::Hide(Pane::Timeline));
    ws.apply_panel_change();
    assert!(ws.panels.layout.is_hidden(Pane::Timeline));
    ws.reset_layout();
    assert!(!ws.panels.layout.is_hidden(Pane::Timeline));
}

#[test]
fn the_timeline_and_output_keys_are_animates() {
    assert_eq!(MenuAction::ShowTimeline.shortcut(), Some("CmdOrCtrl+Alt+T"));
    assert_eq!(MenuAction::ShowOutput.shortcut(), Some("F2"));
}

#[test]
fn file_export_switches_the_transparent_background_that_export_image_follows() {
    let mut ws = workspace();
    let in_menu = flare_menubar::all_entries()
        .iter()
        .any(|e| matches!(e, MenuEntry::Action(MenuAction::ToggleTransparentExport)));
    assert!(in_menu, "File > Export has it");
    assert!(!ws.publish_settings().transparent);
    assert_eq!(
        ws.label(MenuAction::ToggleTransparentExport),
        "Export with a Transparent Background"
    );
    ws.perform(MenuAction::ToggleTransparentExport);
    assert!(ws.publish_settings().transparent);
    assert!(ws.publish_settings().backdrop().is_transparent());
    assert_eq!(
        ws.label(MenuAction::ToggleTransparentExport),
        "Keep the Stage Color in Exports"
    );
    ws.perform(MenuAction::ToggleTransparentExport);
    assert!(!ws.publish_settings().transparent);
}
