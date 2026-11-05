//! Physics and control: gravity, velocities, the controls, collisions and their refusals.
#![allow(clippy::expect_used)]

mod support;

use flare_game::{Body, BodyKind, Component, Control, Event, GameError, Input, Key, World};
use support::{at, game, id};

fn body(kind: BodyKind, width: f64, height: f64) -> Body {
    Body {
        kind,
        width,
        height,
        ..Body::default()
    }
}

fn with_body(raw: u32, x: f64, y: f64, body: Body) -> flare_game::EntitySpec {
    at(raw, x, y)
        .with_component(Component::Body(body))
        .expect("body")
}

#[test]
fn a_dynamic_body_falls_under_gravity_by_the_fixed_step() {
    let spec = game(vec![with_body(1, 0.0, 0.0, Body::default())])
        .with_gravity(0.0, 10.0)
        .expect("gravity");
    let mut world = World::from_spec(&spec, 10.0).expect("world");
    world.step(&Input::none()).expect("step");
    // One tenth of a second at 10 pixels per second squared: v = 1, y = 0.1.
    assert_eq!(world.velocity(id(1)).expect("v"), (0.0, 1.0));
    let (_, y) = world.position(id(1)).expect("p");
    assert!((y - 0.1).abs() < 1e-12);
}

#[test]
fn a_static_body_never_moves_and_a_kinematic_body_ignores_gravity() {
    let spec = game(vec![
        with_body(1, 0.0, 0.0, body(BodyKind::Static, 5.0, 5.0)),
        with_body(2, 0.0, 0.0, body(BodyKind::Kinematic, 5.0, 5.0)),
    ])
    .with_gravity(0.0, 10.0)
    .expect("gravity");
    let mut world = World::from_spec(&spec, 60.0).expect("world");
    for _ in 0..30 {
        world.step(&Input::none()).expect("step");
    }
    assert_eq!(world.position(id(1)).expect("p"), (0.0, 0.0));
    assert_eq!(world.position(id(2)).expect("p"), (0.0, 0.0));
}

#[test]
fn the_arrow_keys_set_the_velocity_and_releasing_them_stops_the_body() {
    let mover = with_body(1, 0.0, 0.0, body(BodyKind::Kinematic, 5.0, 5.0))
        .with_component(Component::Control(Control::default()))
        .expect("control");
    let spec = game(vec![mover]);
    let mut world = World::from_spec(&spec, 10.0).expect("world");
    world
        .step(&Input::with_keys([Key::Right, Key::Down]))
        .expect("step");
    assert_eq!(world.velocity(id(1)).expect("v"), (100.0, 100.0));
    assert_eq!(world.position(id(1)).expect("p"), (10.0, 10.0));
    world.step(&Input::none()).expect("step");
    assert_eq!(world.velocity(id(1)).expect("v"), (0.0, 0.0));
}

#[test]
fn opposite_keys_cancel_and_bound_keys_can_be_changed() {
    let control = Control {
        left: Key::Space,
        right: Key::Space,
        ..Control::default()
    };
    let spec = game(vec![
        with_body(1, 0.0, 0.0, body(BodyKind::Kinematic, 5.0, 5.0))
            .with_component(Component::Control(control))
            .expect("control"),
    ]);
    let mut world = World::from_spec(&spec, 10.0).expect("world");
    world
        .step(&Input::with_keys([Key::Space, Key::Left]))
        .expect("step");
    assert_eq!(world.velocity(id(1)).expect("v").0, 0.0);
}

#[test]
fn overlapping_bodies_report_a_collision_when_they_meet_and_its_end_when_they_part() {
    // The kinematic box moves 10 pixels a step (100 px/s at 10 steps a second) past a static
    // box at x = 20: it touches at x = 10, overlaps from x = 20 and parts at x = 30.
    let spec = game(vec![
        with_body(1, 20.0, 0.0, body(BodyKind::Static, 10.0, 10.0)),
        with_body(2, 0.0, 0.0, body(BodyKind::Kinematic, 10.0, 10.0))
            .with_component(Component::Control(Control::default()))
            .expect("control"),
    ]);
    let mut world = World::from_spec(&spec, 10.0).expect("world");
    let right = Input::with_keys([Key::Right]);
    let mut seen = Vec::new();
    for step in 0..4 {
        for event in world.step(&right).expect("step") {
            match event {
                Event::Collision { a, b, begin } => seen.push((step, a, b, begin)),
                other => panic!("unexpected event {other:?}"),
            }
        }
    }
    assert_eq!(
        seen,
        vec![(1, id(1), id(2), true), (2, id(1), id(2), false)],
        "begins on the second step, ends on the third"
    );
}

#[test]
fn static_pairs_never_collide_and_masks_filter_the_rest() {
    let mut walls = body(BodyKind::Static, 10.0, 10.0);
    walls.category = 0b10;
    let mut ghost = body(BodyKind::Kinematic, 10.0, 10.0);
    ghost.category = 0b01;
    ghost.mask = 0b01;
    let mut other_static = body(BodyKind::Static, 10.0, 10.0);
    other_static.category = 0b100;
    let spec = game(vec![
        with_body(1, 0.0, 0.0, walls),
        with_body(2, 0.0, 0.0, other_static),
        with_body(3, 0.0, 0.0, ghost),
    ]);
    let mut world = World::from_spec(&spec, 10.0).expect("world");
    let events = world.step(&Input::none()).expect("step");
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn a_body_that_leaves_the_coordinate_range_stops_the_run_with_an_error() {
    let spec = game(vec![
        with_body(1, 999_990.0, 0.0, body(BodyKind::Kinematic, 5.0, 5.0))
            .with_component(Component::Control(Control {
                speed: 10_000.0,
                ..Control::default()
            }))
            .expect("control"),
    ]);
    let mut world = World::from_spec(&spec, 10.0).expect("world");
    let right = Input::with_keys([Key::Right]);
    let mut result = Ok(Vec::new());
    for _ in 0..10 {
        result = world.step(&right);
        if result.is_err() {
            break;
        }
    }
    assert_eq!(result, Err(GameError::Diverged));
}
