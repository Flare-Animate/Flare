//! The Publish popup: formats, the Publish button and the places to share.

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
fn the_popup_offers_every_format_and_the_three_places_to_share() {
    let (ctx, mut ws, mut time) = opened();
    for wanted in [
        "Flash (.swf)",
        "HTML5 Canvas (.html)",
        "Animated GIF (.gif)",
        "MP4 video (.mp4)",
        "Twitter",
        "YouTube",
        "Newgrounds",
        "Facebook",
    ] {
        time += 0.1;
        assert!(find(&mut ws, &ctx, time, wanted).is_some(), "{wanted}");
    }
    time += 0.1;
    assert!(
        find(&mut ws, &ctx, time, "X").is_none(),
        "it is called Twitter in the interface"
    );
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
fn each_share_button_asks_the_shell_to_open_its_page_and_the_popup_stays() {
    for target in ShareTarget::ALL {
        let (ctx, mut ws, mut time) = opened();
        click_text(&mut ws, &ctx, &mut time, target.label());
        assert_eq!(ws.take_dialog(), Some(DialogRequest::Share(target)));
        assert!(ws.windows.publish);
    }
}

#[test]
fn close_shuts_the_popup_without_asking_for_anything() {
    let (ctx, mut ws, mut time) = opened();
    click_text(&mut ws, &ctx, &mut time, "Close");
    assert!(!ws.windows.publish);
    assert_eq!(ws.take_dialog(), None);
}

#[test]
fn the_share_pages_are_fixed_https_addresses_and_twitter_goes_to_x() {
    for target in ShareTarget::ALL {
        assert!(target.url().starts_with("https://"), "{}", target.label());
        assert!(!target.url().contains(' '));
    }
    assert!(ShareTarget::Twitter.url().starts_with("https://x.com/"));
    assert!(ShareTarget::YouTube.url().contains("youtube.com"));
    assert!(ShareTarget::Newgrounds.url().contains("newgrounds.com"));
    let labels: Vec<_> = ShareTarget::ALL.iter().map(|t| t.label()).collect();
    assert_eq!(labels, ["Twitter", "YouTube", "Newgrounds", "Facebook"]);
    assert!(ShareTarget::Facebook.url().contains("facebook.com"));
}
