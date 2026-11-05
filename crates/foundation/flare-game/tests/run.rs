//! Determinism and refusals: the same description and script give the same run, and every bad
//! input is refused without changing the world.
#![allow(clippy::expect_used)]

mod support;

use flare_game::{
    Body, Component, Control, GameError, Input, Key, MAX_RUN_FRAMES, World, parse_script, play,
};
use support::{at, game};

fn mover() -> flare_game::GameSpec {
    game(vec![
        at(1, 0.0, 0.0)
            .with_component(Component::Body(Body {
                kind: flare_game::BodyKind::Kinematic,
                ..Body::default()
            }))
            .expect("body")
            .with_component(Component::Control(Control::default()))
            .expect("control"),
    ])
}

#[test]
fn the_same_script_gives_the_same_digest_and_events() {
    let script = parse_script("right;right,down;;up;click:5:5").expect("script");
    let a = play(&mover(), 30.0, 5, &script).expect("run");
    let b = play(&mover(), 30.0, 5, &script).expect("run");
    assert_eq!(a, b);
}

#[test]
fn a_different_script_gives_a_different_digest() {
    let right = parse_script("right;right;right").expect("script");
    let left = parse_script("left;left;left").expect("script");
    let a = play(&mover(), 30.0, 3, &right).expect("run");
    let b = play(&mover(), 30.0, 3, &left).expect("run");
    assert_ne!(a.digest, b.digest);
}

#[test]
fn stepping_one_at_a_time_is_the_same_as_a_run() {
    let script = parse_script("right;right,down;;up").expect("script");
    let run = play(&mover(), 30.0, 4, &script).expect("run");
    let mut world = World::from_spec(&mover(), 30.0).expect("world");
    for input in &script {
        world.step(input).expect("step");
    }
    assert_eq!(world.digest(), run.digest);
    assert_eq!(world.steps(), 4);
}

#[test]
fn a_nan_pointer_is_refused_by_the_constructor_and_by_the_parser() {
    assert_eq!(Input::click(f64::NAN, 0.0), Err(GameError::NonFinite("x")));
    assert_eq!(Input::parse("click:NaN:0"), Err(GameError::NonFinite("x")));
    assert_eq!(Input::parse("click:0:inf"), Err(GameError::NonFinite("y")));
}

#[test]
fn an_unknown_key_or_a_malformed_click_is_refused() {
    assert_eq!(
        Input::parse("shift"),
        Err(GameError::UnknownKey("shift".to_owned()))
    );
    assert!(matches!(
        Input::parse("click:1"),
        Err(GameError::BadInput(_))
    ));
    assert!(matches!(
        Input::parse("click:1:2:3"),
        Err(GameError::BadInput(_))
    ));
}

#[test]
fn a_refused_step_changes_nothing() {
    // 10,000 pixels a second at one step a second: the body leaves the coordinate range on the
    // step that takes it past 1,000,000.
    let spec = game(vec![
        at(1, 0.0, 0.0)
            .with_component(Component::Body(Body {
                kind: flare_game::BodyKind::Kinematic,
                ..Body::default()
            }))
            .expect("body")
            .with_component(Component::Control(Control {
                speed: 10_000.0,
                ..Control::default()
            }))
            .expect("control"),
    ]);
    let mut world = World::from_spec(&spec, 1.0).expect("world");
    let right = Input::with_keys([Key::Right]);
    for _ in 0..100 {
        world.step(&right).expect("inside the range");
    }
    let before = world.clone();
    assert_eq!(world.step(&right), Err(GameError::Diverged));
    assert_eq!(world, before, "the refused step left the world as it was");
    assert_eq!(world.steps(), 100);
}

#[test]
fn a_run_longer_than_its_limit_or_with_a_longer_script_is_refused() {
    assert_eq!(
        play(&mover(), 30.0, MAX_RUN_FRAMES + 1, &[]).map(|_| ()),
        Err(GameError::TooManyFrames)
    );
    let script = parse_script("right;right;right").expect("script");
    assert!(matches!(
        play(&mover(), 30.0, 2, &script),
        Err(GameError::BadInput(_))
    ));
}

#[test]
fn a_frame_rate_must_be_positive_finite_and_within_the_limit() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 241.0] {
        assert!(World::from_spec(&mover(), bad).is_err(), "rate {bad}");
    }
    assert!(World::from_spec(&mover(), 240.0).is_ok());
}

#[test]
fn a_step_that_would_produce_too_many_events_is_refused_whole() {
    // Enough overlapping bodies that one step's collisions pass the per-step event limit.
    let mut entities = Vec::new();
    for raw in 1..=flare_game::MAX_BODIES as u32 {
        entities.push(
            at(raw, 0.0, 0.0)
                .with_component(Component::Body(Body {
                    kind: flare_game::BodyKind::Kinematic,
                    width: 10.0,
                    height: 10.0,
                    ..Body::default()
                }))
                .expect("body"),
        );
    }
    let spec = game(entities);
    let mut world = World::from_spec(&spec, 10.0).expect("world");
    let before = world.clone();
    assert_eq!(world.step(&Input::none()), Err(GameError::TooManyEvents));
    assert_eq!(world, before, "the refused step left the world as it was");
}
