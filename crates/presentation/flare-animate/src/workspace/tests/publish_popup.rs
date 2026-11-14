//! The Publish popup: the formats and the Publish button.

use super::*;
use egui::PointerButton;

fn find(ws: &mut Workspace, ctx: &egui::Context, time: f64, wanted: &str) -> Option<Pos2> {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes.iter().rev().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == wanted => {
            Some(t.visual_bounding_rect().center())
        }
        _ => None,
    })
}

/// A workspace with the popup open and drawn a few times.
fn opened() -> (egui::Context, Workspace, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    ws.perform(MenuAction::Publish);
    let mut time = 0.0;
    for _ in 0..4 {
        time += 0.1;
        find(&mut ws, &ctx, time, "Publish");
    }
    (ctx, ws, time)
}

fn click_text(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, wanted: &str) {
    *time += 0.1;
    let at = find(ws, ctx, *time, wanted).unwrap_or_else(|| panic!("{wanted} is on screen"));
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    for events in [
        vec![Event::PointerMoved(at)],
        vec![button(true)],
        vec![button(false)],
        vec![],
    ] {
        *time += 0.05;
        pass(ws, ctx, input(*time, events));
    }
}

#[test]
fn the_popup_offers_every_format_and_no_places_to_share_until_sharing_uploads() {
    let (ctx, mut ws, mut time) = opened();
    for wanted in [
        "Flash (.swf)",
        "HTML5 Canvas (.html)",
        "Animated GIF (.gif)",
        "MP4 video (.mp4)",
        "QuickTime movie (.mov)",
    ] {
        time += 0.1;
        assert!(find(&mut ws, &ctx, time, wanted).is_some(), "{wanted}");
    }
    // A row of buttons that only opened a site, with the file still to be added by hand, was
    // taken out; sharing comes back when it can upload (see the roadmap).
    for gone in ["Share", "Twitter", "YouTube", "Newgrounds", "Facebook"] {
        time += 0.1;
        assert!(find(&mut ws, &ctx, time, gone).is_none(), "{gone}");
    }
}

#[test]
fn the_publish_button_asks_the_shell_for_a_place_and_leaves_the_popup() {
    let (ctx, mut ws, mut time) = opened();
    click_text(&mut ws, &ctx, &mut time, "Publish");
    // The heading is also "Publish"; the button is the lower one, and the topmost-last search
    // finds the last drawn, which is the button.
    assert_eq!(ws.take_dialog(), Some(DialogRequest::Publish));
    assert!(!ws.windows.publish);
}

#[test]
fn with_no_format_chosen_publish_does_nothing() {
    let (ctx, mut ws, mut time) = opened();
    ws.publish.swf = false;
    ws.publish.html5 = false;
    ws.publish.gif = false;
    for _ in 0..3 {
        time += 0.1;
        find(&mut ws, &ctx, time, "Publish");
    }
    click_text(&mut ws, &ctx, &mut time, "Publish");
    assert_eq!(ws.take_dialog(), None);
    assert!(ws.windows.publish, "the popup stays up");
}

#[test]
fn close_shuts_the_popup_without_asking_for_anything() {
    let (ctx, mut ws, mut time) = opened();
    click_text(&mut ws, &ctx, &mut time, "Close");
    assert!(!ws.windows.publish);
    assert_eq!(ws.take_dialog(), None);
}

#[test]
fn mp4_is_an_ordinary_format_that_ticks_counts_and_is_kept() {
    let (ctx, mut ws, mut time) = opened();
    ws.publish.swf = false;
    ws.publish.html5 = false;
    ws.publish.gif = false;
    assert!(!ws.publish_settings().any(), "nothing ticked");
    // The warning that appears when nothing is ticked moves the boxes; let the popup settle.
    for _ in 0..3 {
        time += 0.1;
        find(&mut ws, &ctx, time, "Publish");
    }
    click_text(&mut ws, &ctx, &mut time, "MP4 video (.mp4)");
    assert!(ws.publish.mp4, "ticked");
    assert!(ws.publish_settings().any(), "one format is enough");
    let kept = ws.preferences();
    assert!(kept.publish_mp4);
    let mut fresh = workspace();
    fresh.apply_preferences(&kept);
    assert!(
        fresh.publish_settings().mp4,
        "and kept with the preferences"
    );
    // Publish is possible with only the video.
    for _ in 0..3 {
        time += 0.1;
        find(&mut ws, &ctx, time, "Publish");
    }
    click_text(&mut ws, &ctx, &mut time, "Publish");
    assert_eq!(ws.take_dialog(), Some(DialogRequest::Publish));
}

#[test]
fn publish_is_the_button_in_the_corner_below_the_formats_with_close_beside_it() {
    let (ctx, mut ws, mut time) = opened();
    let mut at = |wanted: &str| {
        time += 0.1;
        find(&mut ws, &ctx, time, wanted).unwrap_or_else(|| panic!("{wanted} is on screen"))
    };
    let (publish, close, last) = (at("Publish"), at("Close"), at("QuickTime movie (.mov)"));
    assert!(publish.y > last.y, "below the formats");
    assert!(
        (publish.y - close.y).abs() < 2.0,
        "on the same row as Close"
    );
    assert!(publish.x > close.x, "in the corner, Close beside it");
}

#[test]
fn mov_is_an_ordinary_format_that_ticks_counts_and_is_kept() {
    let (ctx, mut ws, mut time) = opened();
    ws.publish.swf = false;
    ws.publish.html5 = false;
    ws.publish.gif = false;
    assert!(!ws.publish_settings().any(), "nothing ticked");
    for _ in 0..3 {
        time += 0.1;
        find(&mut ws, &ctx, time, "Publish");
    }
    click_text(&mut ws, &ctx, &mut time, "QuickTime movie (.mov)");
    assert!(
        ws.publish.mov && !ws.publish.mp4,
        "only the movie is ticked"
    );
    assert!(ws.publish_settings().any(), "one format is enough");
    let kept = ws.preferences();
    assert!(kept.publish_mov);
    let mut fresh = workspace();
    fresh.apply_preferences(&kept);
    assert!(
        fresh.publish_settings().mov,
        "and kept with the preferences"
    );
}
