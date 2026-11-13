//! Tests of the shared dialog pieces in `dialog.rs`.

use super::*;

/// Runs `add` for a press and release at `at`, then returns what it gave on the release.
fn click_at<R>(at: egui::Pos2, mut add: impl FnMut(&mut Ui) -> Option<R>) -> Option<R> {
    let ctx = egui::Context::default();
    crate::icons::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(600.0, 300.0));
    let pointer = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let steps = [
        vec![egui::Event::PointerMoved(at)],
        vec![pointer(true)],
        vec![pointer(false)],
    ];
    let mut last = None;
    for (step, events) in steps.into_iter().enumerate() {
        let input = egui::RawInput {
            time: Some(step as f64 * 0.05),
            screen_rect: Some(screen),
            events,
            ..egui::RawInput::default()
        };
        let mut out = ctx.run_ui(input, |ui| last = add(ui));
        out.textures_delta.clear();
    }
    last
}

#[test]
fn a_dialog_has_more_room_inside_than_a_menu() {
    let style = egui::Style::default();
    let margin = frame(&style).inner_margin;
    assert!(margin.left >= 20 && margin.top >= 16, "{margin:?}");
    assert!(margin.left > style.spacing.menu_margin.left);
}

#[test]
fn a_click_on_a_square_of_the_grid_picks_it() {
    // The grid starts at the panel's top left: squares of 12 with gaps of 6.
    let origin = |ui: &mut Ui| ui.min_rect().min;
    let mut corner = egui::Pos2::ZERO;
    let picked = click_at(egui::pos2(0.0, 0.0), |ui| {
        corner = origin(ui);
        None::<usize>
    });
    assert_eq!(picked, None);
    for (index, (column, row)) in [
        (0, (0.0, 0.0)),
        (4, (1.0, 1.0)),
        (8, (2.0, 2.0)),
        (5, (2.0, 1.0)),
    ] {
        let at = corner + vec2(6.0 + column * 18.0, 6.0 + row * 18.0);
        assert_eq!(
            click_at(at, |ui| point_grid(ui, 0)),
            Some(index),
            "the square at column {column}, row {row}"
        );
    }
}

#[test]
fn a_click_on_a_card_picks_it_and_a_click_beside_them_picks_nothing() {
    let choices = [("symbol-graphic", "One"), ("symbol-button", "Two")];
    let mut corner = egui::Pos2::ZERO;
    click_at(egui::pos2(0.0, 0.0), |ui| {
        corner = ui.min_rect().min;
        None::<usize>
    });
    // Cards are 92 wide with 8 between.
    let second = corner + vec2(92.0 + 8.0 + 40.0, 30.0);
    assert_eq!(
        click_at(second, |ui| choice_cards(ui, &choices, 0)),
        Some(1)
    );
    let beside = corner + vec2(2.0 * 100.0 + 60.0, 30.0);
    assert_eq!(click_at(beside, |ui| choice_cards(ui, &choices, 0)), None);
}

#[test]
fn a_dimmed_primary_button_does_nothing() {
    let mut corner = egui::Pos2::ZERO;
    click_at(egui::pos2(0.0, 0.0), |ui| {
        corner = ui.min_rect().min;
        None::<bool>
    });
    let on = corner + vec2(20.0, 10.0);
    assert_eq!(
        click_at(on, |ui| Some(primary_button(ui, "OK", true).clicked())),
        Some(true)
    );
    assert_eq!(
        click_at(on, |ui| Some(primary_button(ui, "OK", false).clicked())),
        Some(false)
    );
}
