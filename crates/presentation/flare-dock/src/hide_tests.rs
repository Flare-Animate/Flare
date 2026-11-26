#![allow(clippy::expect_used)]

use egui_tiles::Tile;

use super::*;
use crate::arrange::find;

#[test]
fn only_the_timeline_and_the_output_can_be_hidden() {
    let mut layout = default_layout();
    for pane in Pane::ALL {
        let result = layout.set_hidden(pane, true);
        assert_eq!(result.is_ok(), pane.can_hide(), "{pane:?}: {result:?}");
    }
    assert!(layout.is_hidden(Pane::Timeline) && layout.is_hidden(Pane::Output));
    for pane in [Pane::Stage, Pane::Tools, Pane::Properties, Pane::Layers] {
        assert!(!layout.is_hidden(pane), "{pane:?} stays shown");
    }
    check(&layout).expect("a hidden panel is still in the layout once");
}

#[test]
fn a_shown_panel_comes_back_in_front_of_its_stack() {
    let mut layout = default_layout();
    layout.set_hidden(Pane::Output, true).expect("hide");
    layout.set_hidden(Pane::Output, false).expect("show");
    assert!(!layout.is_hidden(Pane::Output));
    let output = find(&layout.tree, Pane::Output).expect("docked");
    assert!(layout.tree.active_tiles().contains(&output), "in front");
}

#[test]
fn hidden_panels_are_kept_in_a_saved_layout() {
    let mut layout = default_layout();
    layout.set_hidden(Pane::Timeline, true).expect("hide");
    let back = from_json(&to_json(&layout).expect("serialize")).expect("read");
    assert!(back.is_hidden(Pane::Timeline));
    assert!(!back.is_hidden(Pane::Output));

    let (workspaces, live) = (Workspaces::default(), layout);
    let json = workspaces_to_json(&workspaces, &live).expect("serialize");
    let (_, back) = workspaces_from_json(&json).expect("read");
    assert!(back.is_hidden(Pane::Timeline), "and in the workspaces file");
}

#[test]
fn a_file_that_hides_the_stage_or_a_stack_opens_with_them_shown() {
    let mut layout = default_layout();
    let ids: Vec<_> = layout.tree.tiles.tile_ids().collect();
    for id in ids {
        layout.tree.tiles.set_visible(id, false);
    }
    let back = from_json(&to_json(&layout).expect("serialize")).expect("read");
    for (id, tile) in back.tree.tiles.iter() {
        let hideable = matches!(tile, Tile::Pane(p) if p.can_hide());
        assert_eq!(
            back.tree.tiles.is_visible(*id),
            !hideable,
            "{tile:?}: only the timeline and output stay hidden"
        );
    }
}

#[test]
fn the_window_menu_toggle_hides_the_panel_in_front_and_brings_one_behind_forward() {
    let mut layout = default_layout();
    // The timeline is in front of the output in the stack under the stage.
    assert!(!layout.toggle_hidden(Pane::Timeline).expect("toggle"));
    assert!(layout.is_hidden(Pane::Timeline));
    assert!(layout.toggle_hidden(Pane::Timeline).expect("toggle"));
    assert!(!layout.is_hidden(Pane::Timeline));

    // The output is behind the timeline: the first toggle brings it forward, the next hides it.
    assert!(layout.toggle_hidden(Pane::Output).expect("toggle"));
    assert!(!layout.is_hidden(Pane::Output));
    let output = find(&layout.tree, Pane::Output).expect("docked");
    assert!(layout.tree.active_tiles().contains(&output));
    assert!(!layout.toggle_hidden(Pane::Output).expect("toggle"));
    assert!(layout.is_hidden(Pane::Output));

    assert!(
        layout.toggle_hidden(Pane::Stage).is_err(),
        "the stage stays"
    );
}
