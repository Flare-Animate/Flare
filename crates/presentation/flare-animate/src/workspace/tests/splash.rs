use super::*;
use crate::workspace::splash::SPLASH_SECONDS;

#[test]
fn the_splash_is_painted_on_the_very_first_frame() {
    let (mut ws, ctx) = (workspace(), egui::Context::default());
    ws.show_splash();
    let mut out = ctx.run_ui(input(0.0, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    let dim = egui::Color32::from_black_alpha(150);
    assert!(
        out.shapes.iter().any(|clipped| matches!(
            &clipped.shape,
            egui::epaint::Shape::Rect(rect) if rect.fill == dim
        )),
        "the first frame the window shows must already have the splash on it"
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
