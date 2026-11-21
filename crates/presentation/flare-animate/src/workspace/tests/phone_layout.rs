//! A phone in portrait, drawn with fingers: the side panels sit in the icon strip, nothing runs
//! off the right edge, the stage keeps the width, and a wider window gives the panels back.

use super::narrow_window::{controls, pass_at, screen, tile_rect};
use super::*;

const PHONE: [f32; 2] = [390.0, 844.0];

#[test]
fn a_phone_draws_with_no_control_or_text_past_its_right_edge() {
    let (ctx, mut ws, mut time) = {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        flare_ui::icons::install(&ctx);
        ws.set_touch(true);
        let mut time = 0.0;
        let mut texts = Vec::new();
        for _ in 0..4 {
            texts = pass_at(&mut ws, &ctx, &mut time, PHONE, vec![]);
        }
        let edge = screen(PHONE).right();
        for (text, bounds) in &texts {
            assert!(
                bounds.right() <= edge + 0.5,
                "“{text}” runs past the right edge of a phone: {bounds:?}"
            );
        }
        (ctx, ws, time)
    };
    let edge = screen(PHONE);
    for (rect, shown) in controls(&ctx) {
        let past_right = rect.right() > edge.right() + 0.5 && shown.right() >= edge.right() - 0.5;
        assert!(!past_right, "{rect:?} runs off a phone");
    }
    // The stage has the width the rail leaves, not the dock's share.
    let stage = tile_rect(&ws, Pane::Stage);
    assert!(
        stage.width() > PHONE[0] - 120.0,
        "the stage keeps the phone's width: {stage:?}"
    );
    // Nothing is left docked on the side but the stage and the timeline.
    for pane in Pane::ALL {
        if pane.can_collapse() {
            assert!(ws.panels.layout.in_strip(pane), "{pane:?} is in the strip");
        }
    }
    // The same workspace drawn again at a phone size changes nothing more.
    pass_at(&mut ws, &ctx, &mut time, PHONE, vec![]);
    assert!(ws.phone_stash.is_some());
}

#[test]
fn a_window_widened_from_a_phone_gets_its_panels_back() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    ws.set_touch(true);
    let before = ws.panels.layout.strip.clone();
    let mut time = 0.0;
    for _ in 0..3 {
        pass_at(&mut ws, &ctx, &mut time, PHONE, vec![]);
    }
    assert_ne!(ws.panels.layout.strip, before, "the phone put panels away");
    for _ in 0..3 {
        pass_at(&mut ws, &ctx, &mut time, [1280.0, 800.0], vec![]);
    }
    assert_eq!(
        ws.panels.layout.strip, before,
        "a wide window has the arrangement back"
    );
    assert!(ws.phone_stash.is_none());
}
