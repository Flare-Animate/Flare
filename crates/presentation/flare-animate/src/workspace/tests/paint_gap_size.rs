//! The Paint Bucket's Gap Size, and the Tool tab showing only the paint a tool uses.

use super::layer_names::{click, find, settle};
use super::paint_tools::{at_full_size, hand_drawn_loop};
use super::*;
use egui::PointerButton;
use flare_tools::paint::GapSize;

const BLUE: Color32 = Color32::from_rgb(20, 40, 200);

/// Whether a click inside a loop left `gap` open fills it with the Gap Size `size`.
fn fills(gap: f64, size: GapSize) -> bool {
    let mut ws = at_full_size();
    hand_drawn_loop(&mut ws, gap);
    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    ws.paint.gap = size;
    ws.click_with_tool(Pos2::new(150.0, 150.0), false);
    let [r, g, b, a] = BLUE.to_array();
    drawn_shape(&ws, 1).fill() == Rgba8::new(r, g, b, a)
}

#[test]
fn a_larger_gap_size_fills_across_a_wider_gap_and_none_needs_the_lines_to_meet() {
    assert!(fills(6.0, GapSize::Small), "the default closes a near miss");
    assert!(!fills(6.0, GapSize::DontClose));
    assert!(!fills(20.0, GapSize::Small));
    assert!(fills(20.0, GapSize::Medium));
    assert!(!fills(40.0, GapSize::Medium));
    assert!(fills(40.0, GapSize::Large));
    assert!(!fills(80.0, GapSize::Large), "lines far apart stay open");
}

#[test]
fn the_gap_size_is_chosen_from_the_buckets_options() {
    let mut ws = workspace();
    ws.tool = Tool::Bucket;
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    time += 0.1;
    let menu = find(&mut ws, &ctx, time, vec![], "Close small gaps").expect("the choice");
    click(&mut ws, &ctx, &mut time, menu, PointerButton::Primary);
    time += 0.1;
    let large = find(&mut ws, &ctx, time, vec![], "Close large gaps").expect("the list opens");
    click(&mut ws, &ctx, &mut time, large, PointerButton::Primary);
    assert_eq!(ws.paint.gap, GapSize::Large);
}

#[test]
fn the_bucket_shows_its_fill_and_the_ink_bottle_its_stroke_and_neither_the_other() {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;

    let mut ws = workspace();
    ws.tool = Tool::Bucket;
    settle(&mut ws, &ctx, &mut time);
    time += 0.1;
    let mut drawn = |ws: &mut Workspace, text: &str| {
        time += 0.1;
        find(ws, &ctx, time, vec![], text).is_some()
    };
    assert!(drawn(&mut ws, "Gap size"));
    assert!(drawn(&mut ws, "Fill"));
    for stroke_only in ["Cap", "Join", "Width", "Outline"] {
        assert!(!drawn(&mut ws, stroke_only), "{stroke_only} is a stroke's");
    }

    ws.tool = Tool::InkBottle;
    assert!(drawn(&mut ws, "Cap") && drawn(&mut ws, "Width"));
    assert!(!drawn(&mut ws, "Gap size"));
    assert!(!drawn(&mut ws, "Gradient"), "the ink bottle pours no fill");

    ws.tool = Tool::Rectangle;
    assert!(
        drawn(&mut ws, "Fill") && drawn(&mut ws, "Cap"),
        "a shape has both"
    );
}
