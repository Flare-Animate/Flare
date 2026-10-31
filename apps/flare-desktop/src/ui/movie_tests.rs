use super::*;

#[test]
fn a_stall_adds_at_most_one_step_of_movie_time() {
    assert_eq!(step(Duration::from_millis(16)), Duration::from_millis(16));
    assert_eq!(step(Duration::from_secs(30)), MAX_STEP);
    assert_eq!(step(Duration::ZERO), Duration::ZERO);
}

#[test]
fn a_picture_that_fits_is_shown_as_it_is_and_one_that_does_not_is_shrunk_to_fit() {
    let room = egui::vec2(1000.0, 800.0);
    assert_eq!(
        fit(egui::vec2(400.0, 300.0), room),
        egui::vec2(400.0, 300.0)
    );
    // Too wide: shrunk until it fits inside nine tenths of the room, keeping its shape.
    let shrunk = fit(egui::vec2(1800.0, 900.0), room);
    assert!(
        (shrunk.x - 900.0).abs() < 0.01 && (shrunk.y - 450.0).abs() < 0.01,
        "{shrunk:?}"
    );
    // Too tall.
    let shrunk = fit(egui::vec2(500.0, 1440.0), room);
    assert!(
        (shrunk.y - 720.0).abs() < 0.01 && (shrunk.x - 250.0).abs() < 0.01,
        "{shrunk:?}"
    );
}

#[test]
fn a_picture_with_no_size_or_a_room_with_none_shows_nothing_and_does_not_divide_by_zero() {
    assert_eq!(
        fit(egui::Vec2::ZERO, egui::vec2(100.0, 100.0)),
        egui::Vec2::ZERO
    );
    assert_eq!(
        fit(egui::vec2(10.0, 10.0), egui::Vec2::ZERO),
        egui::Vec2::ZERO
    );
}
