//! Looping and the mouse, on a real GPU: what the player's controls do.

use super::*;

/// A two-frame movie, red then blue, at 12 frames a second.
#[allow(clippy::expect_used)]
fn two_frames() -> Vec<u8> {
    use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
    use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Stage, Timeline};
    let frame = |start, color| {
        let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 120.0, 90.0, color).expect("rect");
        FrameSpan::new(start, std::num::NonZeroU32::MIN, Content::Shape(shape))
    };
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![
            frame(0, Rgba8::opaque(220, 30, 30)),
            frame(1, Rgba8::opaque(30, 30, 220)),
        ],
    )
    .expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(120, 90, Rgba8::WHITE).expect("stage"));
    flare_publish::publish_swf(&document, &flare_publish::PublishOptions::default())
        .expect("publish")
        .bytes
}

#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
#[allow(clippy::expect_used)]
fn without_looping_the_movie_holds_its_last_frame_and_with_looping_it_goes_on() {
    let state = render_state();
    let run = |looping: bool| {
        let mut window = MovieWindow::open(two_frames(), &state, 1.0).expect("starts");
        window.looping = looping;
        window.movie.set_muted(true).expect("mute");
        for _ in 0..40 {
            window.advance(Duration::from_millis(50));
        }
        window
    };
    let mut held = run(false);
    assert_eq!(held.movie.frame_count(), 2);
    assert_eq!(held.movie.current_frame(), Some(2), "on its last frame");
    assert!(!held.movie.is_playing(), "paused there");
    let stopped_at = held.elapsed;
    held.advance(Duration::from_millis(500));
    assert_eq!(held.elapsed, stopped_at, "the clock does not run on");
    // Play at the end starts it over.
    held.toggle_play();
    assert!(held.movie.is_playing());
    assert_eq!(held.elapsed, Duration::ZERO, "from the start");

    let going = run(true);
    assert!(
        going.movie.is_playing(),
        "a looping movie goes on past its end"
    );
    assert!(going.elapsed >= Duration::from_secs(1));
}

/// One egui pass of the player, in the viewport of its own window, in a 640 x 480 screen with
/// `events`, at `time`; the texts it drew.
#[allow(clippy::expect_used)]
fn pass(
    ctx: &egui::Context,
    window: &mut MovieWindow,
    time: f64,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Pos2)> {
    let id = crate::ui::movie_window::viewport_id();
    let raw = egui::RawInput {
        viewport_id: id,
        viewports: std::iter::once((id, egui::ViewportInfo::default())).collect(),
        time: Some(time),
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(640.0, 480.0),
        )),
        events,
        ..egui::RawInput::default()
    };
    let mut output = ctx.run_ui(raw, |ui| window.show(ui));
    output.textures_delta.clear();
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some((
                t.galley.text().to_owned(),
                t.visual_bounding_rect().center(),
            )),
            _ => None,
        })
        .collect()
}

/// A pass of the editor's own window, which comes between the Test Movie's frames in the app.
/// `declare` is whether it says the Test Movie's viewport is still in use, as the app's does.
fn editor_pass(ctx: &egui::Context, time: f64, declare: bool) {
    let raw = egui::RawInput {
        time: Some(time),
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(640.0, 480.0),
        )),
        ..egui::RawInput::default()
    };
    let mut output = ctx.run_ui(raw, |ui| {
        if declare {
            crate::ui::movie_window::keep_viewport(ui.ctx());
        }
    });
    output.textures_delta.clear();
}

/// Plays the clicks of a person on the player's controls, with the editor drawing between every
/// frame of the player, and says whether a click paused the movie and whether Close closed it.
#[allow(clippy::expect_used)]
fn clicks_reach_the_controls(declare: bool) -> (bool, bool) {
    let state = render_state();
    let mut window = MovieWindow::open(two_frames(), &state, 1.0).expect("starts");
    window.movie.set_muted(true).expect("mute");
    let ctx = crate::ui::window::new_context();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    let frame = |window: &mut MovieWindow, time: &mut f64, events| {
        *time += 0.05;
        let texts = pass(&ctx, window, *time, events);
        editor_pass(&ctx, *time, declare);
        texts
    };
    let mut texts = Vec::new();
    for _ in 0..8 {
        texts = frame(&mut window, &mut time, vec![]);
    }
    let click = |window: &mut MovieWindow, time: &mut f64, at: egui::Pos2| {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            vec![egui::Event::PointerMoved(at)],
            vec![button(true)],
            vec![button(false)],
            vec![],
        ] {
            frame(window, time, events);
        }
    };
    let find = |texts: &[(String, egui::Pos2)], name: &str| {
        texts
            .iter()
            .find(|(text, _)| text == name)
            .unwrap_or_else(|| panic!("{name} is drawn: {texts:?}"))
            .1
    };
    // The Pause button has an icon and no text: sweep along the row, left of the speed.
    let speed = find(&texts, "1×");
    let mut paused = false;
    for step in 0..30 {
        click(
            &mut window,
            &mut time,
            egui::pos2(speed.x - 150.0 + step as f32 * 5.0, speed.y),
        );
        if !window.movie.is_playing() {
            paused = true;
            break;
        }
    }
    click(&mut window, &mut time, find(&texts, "Close"));
    (paused, window.is_closed())
}

#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
fn the_controls_answer_the_mouse_while_the_editor_draws_between_their_frames() {
    let (paused, closed) = clicks_reach_the_controls(true);
    assert!(paused, "a click on the Pause button pauses the movie");
    assert!(closed, "a click on Close closes the player");
}

/// Why `keep_viewport` is there: without it egui forgets the window's widgets every time the
/// editor draws, and no click lands.
#[test]
#[ignore = "needs a GPU adapter; scripts/gpu_tests.sh"]
fn without_declaring_the_viewport_no_control_can_be_clicked() {
    let (paused, closed) = clicks_reach_the_controls(false);
    assert!(
        !paused && !closed,
        "egui drops an undeclared viewport's widgets"
    );
}
