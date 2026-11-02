use super::layer_names::{click, settle};
use super::*;

#[test]
fn with_many_documents_the_new_button_stays_in_view_and_still_makes_a_document() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    // Far more than a window is wide: eight used to push the button out of the strip.
    for _ in 0..15 {
        ws.new_tab();
    }
    assert_eq!(ws.tabs().len(), 16);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    let plus = ws.tab_plus;
    assert!(
        plus.is_positive() && plus.max.x <= 1280.0 && plus.min.x >= 0.0,
        "the button is on screen: {plus:?}"
    );
    click(
        &mut ws,
        &ctx,
        &mut time,
        plus.center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(ws.tabs().len(), 17, "and it makes one more");
}

#[test]
fn the_button_sits_right_after_the_last_tab_while_they_all_fit() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    ws.new_tab();
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    let plus = ws.tab_plus;
    assert!(
        plus.min.x < 400.0,
        "two tabs, so the button is near the left: {plus:?}"
    );
}
