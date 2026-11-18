use super::*;
use crate::workspace::splash::SPLASH_SECONDS;

/// On the very first frame the window shows, an opaque sheet covers all of it and nothing but the
/// splash card's own words is drawn after that sheet, so no part of the app can be seen.
#[test]
fn the_splash_hides_the_app_from_the_very_first_frame() {
    use egui::epaint::Shape;
    let (mut ws, ctx) = (workspace(), egui::Context::default());
    flare_ui::icons::install(&ctx);
    ws.set_home(true);
    ws.show_splash();
    let mut out = ctx.run_ui(input(0.0, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    let screen = ctx.content_rect();
    let cover = out
        .shapes
        .iter()
        .rposition(|clipped| {
            matches!(&clipped.shape, Shape::Rect(r) if r.rect == screen && r.fill.a() == 255)
        })
        .expect("an opaque sheet over the whole window");
    let words: Vec<String> = out.shapes[cover..]
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            Shape::Text(text) => Some(text.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    // Animate's card: the name by the mark, the version and what Flare is, who makes it, and a
    // word on what is loading at the bottom.
    assert_eq!(
        words,
        [
            "Flare".to_owned(),
            format!("Version {}", env!("CARGO_PKG_VERSION")),
            "An animation tool in the spirit of Adobe Animate.".to_owned(),
            "Free and open source, made by the Flare contributors.".to_owned(),
            "Loading…".to_owned(),
        ]
    );
}

#[test]
fn the_splash_is_off_until_asked_and_goes_by_itself() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    assert!(!ws.splash_shown());
    ws.show_splash();
    run(&mut ws, &ctx, 1.0);
    assert!(ws.splash_shown(), "shown on its first frame");
    run(&mut ws, &ctx, 1.0 + SPLASH_SECONDS - 0.5);
    assert!(ws.splash_shown(), "still there before its time is up");
    run(&mut ws, &ctx, 1.0 + SPLASH_SECONDS + 0.1);
    assert!(!ws.splash_shown(), "gone after its time");
}

#[test]
fn a_key_dismisses_the_splash_at_once() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.show_splash();
    run(&mut ws, &ctx, 0.0);
    assert!(ws.splash_shown());
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Space, Modifiers::NONE)]),
    );
    assert!(!ws.splash_shown());
}

#[test]
fn a_small_window_still_draws_the_splash_without_panicking() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.show_splash();
    let mut tiny = input(0.0, vec![]);
    tiny.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 30.0)));
    pass(&mut ws, &ctx, tiny);
    assert!(ws.splash_shown());
    ws.dismiss_splash();
    assert!(!ws.splash_shown());
}

#[test]
fn the_card_is_light_on_the_left_and_fits_inside_the_window() {
    use egui::epaint::Shape;
    let (mut ws, ctx) = (workspace(), egui::Context::default());
    flare_ui::icons::install(&ctx);
    ws.show_splash();
    let mut out = ctx.run_ui(input(0.0, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    let screen = ctx.content_rect();
    // The card is drawn last, over the sheet; the stage's own white is under it.
    let card = out
        .shapes
        .iter()
        .rev()
        .find_map(|clipped| match &clipped.shape {
            Shape::Rect(r) if r.fill == egui::Color32::WHITE && r.rect != screen => Some(r.rect),
            _ => None,
        })
        .expect("a white card");
    assert!(screen.contains_rect(card), "{card:?} in {screen:?}");
    assert!(
        card.width() < screen.width() * 0.75,
        "a card, not a cover: {card:?}"
    );
    let name = out
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            Shape::Text(t) if t.galley.text() == "Flare" => Some(t.visual_bounding_rect()),
            _ => None,
        })
        .expect("the name");
    assert!(
        name.right() < card.center().x,
        "the name is on the light half"
    );
}
