//! A small window: nothing is cut off at its edge, no control is lost past the end of its bar,
//! the menus that do not fit are kept in a "»" menu, and the panels keep their width while the
//! stage gives way.

use super::*;
use egui::epaint::Shape;
use egui::{PointerButton, pos2};

fn screen(size: [f32; 2]) -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::from(size))
}

fn sized(size: [f32; 2], time: f64, events: Vec<Event>) -> RawInput {
    RawInput {
        time: Some(time),
        screen_rect: Some(screen(size)),
        events,
        ..RawInput::default()
    }
}

/// Draws one pass in a window of `size` and gives back the text drawn, with where each is.
fn pass_at(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: &mut f64,
    size: [f32; 2],
    events: Vec<Event>,
) -> Vec<(String, Rect)> {
    *time += 0.1;
    let mut output = ctx.run_ui(sized(size, *time, events), |ui| ws.show(ui));
    output.textures_delta.clear();
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Text(t) => Some((t.galley.text().to_owned(), t.visual_bounding_rect())),
            _ => None,
        })
        .collect()
}

/// A workspace drawn a few times in a window of `size`.
fn drawn(size: [f32; 2]) -> (egui::Context, Workspace, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    for _ in 0..4 {
        pass_at(&mut ws, &ctx, &mut time, size, vec![]);
    }
    (ctx, ws, time)
}

/// Every control that can be clicked, with its whole rectangle and the part of it on show.
fn controls(ctx: &egui::Context) -> Vec<(Rect, Rect)> {
    ctx.viewport(|v| {
        v.this_pass
            .widgets
            .layers()
            .flat_map(|(_, rects)| rects.iter())
            .filter(|r| r.sense.interactive())
            .map(|r| (r.rect, r.interact_rect))
            .collect()
    })
}

fn tile_rect(ws: &Workspace, pane: Pane) -> Rect {
    let tree = &ws.panels.layout.tree;
    let tile = tree.tiles.find_pane(&pane).expect("docked");
    tree.tiles.rect(tile).expect("drawn")
}

#[test]
fn nothing_is_cut_off_at_the_window_edge_at_the_smallest_size_or_a_usual_one() {
    for size in [MIN_WINDOW_SIZE, [1280.0, 800.0], [1600.0, 1000.0]] {
        let (ctx, _ws, _) = drawn(size);
        let edge = screen(size);
        for (rect, shown) in controls(&ctx) {
            // The window, not something inside it, is what cut the control short.
            let past_right =
                rect.right() > edge.right() + 0.5 && shown.right() >= edge.right() - 0.5;
            let past_bottom =
                rect.bottom() > edge.bottom() + 0.5 && shown.bottom() >= edge.bottom() - 0.5;
            assert!(
                !past_right && !past_bottom,
                "{rect:?} runs off a {size:?} window"
            );
        }
    }
}

#[test]
fn the_bars_over_the_stage_and_the_timeline_lose_no_control_in_the_smallest_window() {
    let (ctx, ws, _) = drawn(MIN_WINDOW_SIZE);
    // The rows of controls at the top of each: the tabs and the stage bar, and the timeline's
    // toolbar however many rows it takes.
    let bands = [Pane::Stage, Pane::Timeline].map(|pane| {
        let tile = tile_rect(&ws, pane);
        Rect::from_min_max(tile.min, pos2(tile.right(), tile.top() + 70.0))
    });
    let mut seen = 0;
    for (rect, shown) in controls(&ctx) {
        if bands.iter().any(|band| band.contains(rect.left_top())) && rect.width() < 120.0 {
            seen += 1;
            assert!(
                shown.width() > 0.0 && shown.height() > 0.0,
                "{rect:?} is drawn past the end of its bar"
            );
        }
    }
    assert!(seen > 20, "the bars' controls were found ({seen})");
}

#[test]
fn the_timeline_toolbar_takes_a_second_row_rather_than_lose_its_end() {
    let (ctx, ws, _) = drawn(MIN_WINDOW_SIZE);
    let timeline = tile_rect(&ws, Pane::Timeline);
    let rows: std::collections::BTreeSet<i32> = controls(&ctx)
        .into_iter()
        .filter(|(rect, _)| {
            timeline.contains(rect.left_top())
                && rect.top() < timeline.top() + 70.0
                && rect.height() <= 26.0
        })
        .map(|(rect, _)| (rect.center().y / 12.0).round() as i32)
        .collect();
    assert!(rows.len() >= 2, "the toolbar wrapped: rows at {rows:?}");
    // A wide window keeps it on one row.
    let (ctx, ws, _) = drawn([1600.0, 1000.0]);
    let timeline = tile_rect(&ws, Pane::Timeline);
    let tops: Vec<f32> = controls(&ctx)
        .into_iter()
        .filter(|(rect, _)| {
            timeline.contains(rect.left_top())
                && rect.top() < timeline.top() + 40.0
                && rect.height() <= 26.0
        })
        .map(|(rect, _)| rect.center().y)
        .collect();
    let spread = tops.iter().copied().fold(f32::MIN, f32::max)
        - tops.iter().copied().fold(f32::MAX, f32::min);
    assert!(spread < 4.0, "one row: {tops:?}");
}

#[test]
fn the_corner_button_sits_over_the_rail_inside_the_window() {
    for size in [MIN_WINDOW_SIZE, [1600.0, 1000.0]] {
        let (ctx, ws, _) = drawn(size);
        let corner = controls(&ctx)
            .into_iter()
            .map(|(rect, _)| rect)
            .filter(|rect| rect.top() < 30.0 && rect.width() < 40.0)
            .max_by(|a, b| a.right().total_cmp(&b.right()))
            .expect("the corner's buttons");
        let rail_middle = size[0] - crate::workspace::rail::center_from_edge(ws.touch);
        assert!(
            corner.right() <= size[0],
            "{corner:?} is inside a {size:?} window"
        );
        assert!(
            (corner.center().x - rail_middle).abs() < 1.0,
            "{corner:?} is over the rail at {rail_middle}"
        );
    }
}

#[test]
fn a_window_too_narrow_for_every_menu_keeps_the_rest_in_a_more_menu() {
    let wide = drawn([1280.0, 800.0]);
    let (ctx, mut ws, mut time) = wide;
    let texts = pass_at(&mut ws, &ctx, &mut time, [1280.0, 800.0], vec![]);
    assert!(texts.iter().any(|(t, _)| t == "Help"), "every menu fits");
    assert!(
        !texts.iter().any(|(t, _)| t == "»"),
        "so there is no more menu"
    );

    // Narrower than the workspace is laid out for, so that menus have to give way.
    let narrow = [560.0, 600.0];
    let (ctx, mut ws, mut time) = drawn(narrow);
    let texts = pass_at(&mut ws, &ctx, &mut time, narrow, vec![]);
    let more = texts
        .iter()
        .find(|(t, _)| t == "»")
        .map(|(_, r)| r.center())
        .expect("the more menu");
    assert!(
        texts.iter().any(|(t, _)| t == "File"),
        "the first menus stay"
    );
    assert!(
        !texts.iter().any(|(t, _)| t == "Help"),
        "the last ones move"
    );
    // Nothing in the bar is drawn over anything else.
    let bar: Vec<Rect> = controls(&ctx)
        .into_iter()
        .map(|(rect, _)| rect)
        .filter(|rect| rect.top() < 30.0 && rect.width() < 200.0)
        .collect();
    for (i, a) in bar.iter().enumerate() {
        for b in &bar[i + 1..] {
            assert!(!a.intersects(b.shrink(0.5)), "{a:?} overlaps {b:?}");
        }
    }
    // The more menu holds the menus there was no room for.
    let button = |pressed| Event::PointerButton {
        pos: more,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    for events in [
        vec![Event::PointerMoved(more)],
        vec![button(true)],
        vec![button(false)],
    ] {
        pass_at(&mut ws, &ctx, &mut time, narrow, events);
    }
    let texts = pass_at(&mut ws, &ctx, &mut time, narrow, vec![]);
    assert!(
        texts.iter().any(|(t, _)| t == "Help"),
        "Help is in the more menu"
    );
}

#[test]
fn a_narrower_window_narrows_the_stage_and_keeps_the_panels_their_width() {
    let (ctx, mut ws, mut time) = drawn([1600.0, 1000.0]);
    let (tools, side, stage) = (
        tile_rect(&ws, Pane::Tools).width(),
        tile_rect(&ws, Pane::Properties).width(),
        tile_rect(&ws, Pane::Stage).width(),
    );
    for _ in 0..2 {
        pass_at(&mut ws, &ctx, &mut time, MIN_WINDOW_SIZE, vec![]);
    }
    assert!((tile_rect(&ws, Pane::Tools).width() - tools).abs() < 1.0);
    assert!((tile_rect(&ws, Pane::Properties).width() - side).abs() < 1.0);
    assert!(
        (stage - tile_rect(&ws, Pane::Stage).width() - (1600.0 - MIN_WINDOW_SIZE[0])).abs() < 1.0,
        "the stage gave up what the window lost"
    );
}
