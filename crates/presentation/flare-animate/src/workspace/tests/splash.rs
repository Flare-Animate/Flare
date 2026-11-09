use super::*;
use crate::workspace::splash::SPLASH_SECONDS;

/// On the very first frame the window shows, an opaque sheet covers all of it and nothing but the
/// splash's own words is drawn after that sheet, so no part of the app can be seen.
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
    assert_eq!(
        words.first().map(String::as_str),
        Some("Flare"),
        "{words:?}"
    );
    assert_eq!(
        words.len(),
        3,
        "the name, the tagline and the version: {words:?}"
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
