#![allow(clippy::expect_used)]

use egui::{Pos2, vec2};

use super::*;
use crate::arrange::find;
use crate::default_layout;

const GAP: f32 = 3.0;

/// Draws nothing in each panel: only where the panels are matters here.
struct Blank;

impl egui_tiles::Behavior<Pane> for Blank {
    fn pane_ui(
        &mut self,
        _ui: &mut egui::Ui,
        _tile: TileId,
        _pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.title().into()
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        GAP
    }
}

/// Draws `tree` in a window `width` wide, keeping the panels' sizes first when `keep` is set.
fn draw(tree: &mut Tree<Pane>, ctx: &egui::Context, width: f32, keep: bool) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 800.0))),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        if keep {
            keep_panel_sizes(tree, ui.available_rect_before_wrap(), GAP);
        }
        tree.ui(&mut Blank, ui);
    });
    // No renderer takes the font texture here.
    output.textures_delta.clear();
}

fn width_of(tree: &Tree<Pane>, pane: Pane) -> f32 {
    let tile = find(tree, pane).expect("docked");
    tree.tiles.rect(tile).expect("drawn").width()
}

#[test]
fn a_narrower_window_keeps_the_tool_strip_and_side_panels_and_narrows_the_stage() {
    let (ctx, mut tree) = (egui::Context::default(), default_layout().tree);
    draw(&mut tree, &ctx, 1600.0, true);
    let (tools, side, stage) = (
        width_of(&tree, Pane::Tools),
        width_of(&tree, Pane::Properties),
        width_of(&tree, Pane::Stage),
    );
    draw(&mut tree, &ctx, 1100.0, true);
    assert!((width_of(&tree, Pane::Tools) - tools).abs() < 1.0);
    assert!((width_of(&tree, Pane::Properties) - side).abs() < 1.0);
    assert!(
        (stage - width_of(&tree, Pane::Stage) - 500.0).abs() < 1.0,
        "the stage gave up all 500 points"
    );
}

#[test]
fn a_wider_window_gives_the_stage_the_new_room() {
    let (ctx, mut tree) = (egui::Context::default(), default_layout().tree);
    // Twice: the first draw's tool strip is a share of the window, narrower than a tool here,
    // and the second widens it.
    draw(&mut tree, &ctx, 1100.0, true);
    draw(&mut tree, &ctx, 1100.0, true);
    let (side, stage) = (
        width_of(&tree, Pane::Properties),
        width_of(&tree, Pane::Stage),
    );
    draw(&mut tree, &ctx, 1500.0, true);
    assert!((width_of(&tree, Pane::Properties) - side).abs() < 1.0);
    assert!((width_of(&tree, Pane::Stage) - stage - 400.0).abs() < 1.0);
}

#[test]
fn without_keeping_sizes_the_side_panels_shrink_with_the_window() {
    // What the dock did before, and what the tests above would see without `keep_panel_sizes`.
    let (ctx, mut tree) = (egui::Context::default(), default_layout().tree);
    draw(&mut tree, &ctx, 1600.0, false);
    let side = width_of(&tree, Pane::Properties);
    draw(&mut tree, &ctx, 1100.0, false);
    assert!(width_of(&tree, Pane::Properties) < side - 50.0);
}

#[test]
fn a_window_too_narrow_for_the_panels_leaves_the_stage_its_least_width() {
    let (ctx, mut tree) = (egui::Context::default(), default_layout().tree);
    draw(&mut tree, &ctx, 1600.0, true);
    draw(&mut tree, &ctx, 500.0, true);
    let stage = width_of(&tree, Pane::Stage);
    assert!(
        (stage - MIN_STAGE_SIDE).abs() < 1.0,
        "the stage is {stage} wide"
    );
    assert!(width_of(&tree, Pane::Properties) > 0.0);
}

#[test]
fn a_tool_strip_squeezed_narrower_than_a_tool_is_widened_to_one() {
    let (ctx, mut tree) = (egui::Context::default(), default_layout().tree);
    // The default share of a narrow window is narrower than a tool.
    draw(&mut tree, &ctx, 800.0, true);
    assert!(width_of(&tree, Pane::Tools) < MIN_TOOL_STRIP);
    draw(&mut tree, &ctx, 800.0, true);
    assert!((width_of(&tree, Pane::Tools) - MIN_TOOL_STRIP).abs() < 1.0);
}

#[test]
fn drawing_at_the_same_size_leaves_the_shares_alone() {
    // So a split a person has dragged is never pulled back.
    let (ctx, mut tree) = (egui::Context::default(), default_layout().tree);
    draw(&mut tree, &ctx, 1400.0, true);
    let before = serde_json::to_string(&tree).expect("json");
    draw(&mut tree, &ctx, 1400.0, true);
    assert_eq!(serde_json::to_string(&tree).expect("json"), before);
}

#[test]
fn a_tree_not_drawn_yet_is_left_alone() {
    let mut tree = default_layout().tree;
    let before = serde_json::to_string(&tree).expect("json");
    keep_panel_sizes(
        &mut tree,
        Rect::from_min_size(Pos2::ZERO, vec2(900.0, 700.0)),
        GAP,
    );
    assert_eq!(serde_json::to_string(&tree).expect("json"), before);
}

#[test]
fn a_split_without_the_stage_shares_its_room_in_proportion() {
    let lengths = lengths_for(&[100.0, 300.0], &[0.0, 0.0], None, 200.0);
    assert_eq!(lengths, vec![50.0, 150.0]);
}

#[test]
fn the_side_with_the_stage_takes_what_the_others_leave() {
    let lengths = lengths_for(&[40.0, 900.0, 300.0], &[36.0, 0.0, 0.0], Some(1), 1000.0);
    assert_eq!(lengths, vec![40.0, 660.0, 300.0]);
    // A tool strip narrower than its least is widened, from the stage's side.
    let lengths = lengths_for(&[20.0, 900.0, 300.0], &[36.0, 0.0, 0.0], Some(1), 1000.0);
    assert_eq!(lengths, vec![36.0, 664.0, 300.0]);
}
