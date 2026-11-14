//! Triggers, hooks and named actions: a zone reports who walks in and out, a hook reacts
//! declaratively and only when its `when` first happens, and none of it leaves the world
//! behind a step it refused.
#![allow(clippy::expect_used)]

mod support;

use flare_game::{
    Body, BodyKind, Component, Event, Hook, Input, Sprite, Trigger, World, parse_script, play,
};
use support::{at, fields, game, id};

const FPS: f64 = 30.0;
/// 600 pixels a second at 30 frames a second: 20 pixels a step.
const SPEED: f64 = 600.0;

/// A zone 100 by 100 with its top-left corner at `(x, y)`.
fn zone(raw: u32, x: f64, y: f64, once: bool) -> flare_game::EntitySpec {
    at(raw, x, y)
        .with_component(Component::Trigger(Trigger {
            width: 100.0,
            height: 100.0,
            once,
        }))
        .expect("zone")
}

/// A visitor whose kinematic body drifts at `(vx, vy)`.
fn visitor(raw: u32, x: f64, y: f64, vx: f64, vy: f64) -> flare_game::EntitySpec {
    at(raw, x, y)
        .with_component(Component::Body(Body {
            kind: BodyKind::Kinematic,
            vx,
            vy,
            ..Body::default()
        }))
        .expect("body")
}

fn hold(count: usize) -> Vec<Input> {
    (0..count).map(|_| Input::none()).collect()
}

#[test]
fn a_zone_reports_the_entry_and_the_exit_of_a_body_once_each() {
    let spec = game(vec![
        zone(1, 0.0, 0.0, false),
        visitor(2, 50.0, 150.0, 0.0, -SPEED),
    ]);
    let run = play(&spec, FPS, 12, &hold(12)).expect("run");
    let reported: Vec<bool> = run
        .events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::Trigger { trigger, by, begin } if *trigger == id(1) && *by == id(2) => {
                Some(*begin)
            }
            _ => None,
        })
        .collect();
    assert_eq!(reported, vec![true, false], "one entry, then one exit");
}

#[test]
fn a_once_zone_is_silent_after_its_first_entry() {
    let once = game(vec![
        zone(1, 0.0, 0.0, true),
        visitor(2, 50.0, 50.0, 0.0, -SPEED),
    ]);
    let always = game(vec![
        zone(1, 0.0, 0.0, false),
        visitor(2, 50.0, 50.0, 0.0, -SPEED),
    ]);
    let count = |spec: &flare_game::GameSpec| {
        let run = play(spec, FPS, 5, &hold(5)).expect("run");
        run.events
            .iter()
            .filter(|(_, event)| matches!(event, Event::Trigger { begin: false, .. }))
            .count()
    };
    assert_eq!(count(&once), 0, "the exit of a once zone is not reported");
    assert_eq!(count(&always), 1, "an ordinary zone reports its exit");
}

fn hook_with(pairs: &[(&str, &str)]) -> Hook {
    match Component::parse("hook", &fields(pairs)).expect("hook") {
        Component::Hook(hook) => hook,
        other => panic!("expected a hook, got {other:?}"),
    }
}

#[test]
fn a_hook_on_a_zone_starts_its_sound_when_a_body_walks_in() {
    let hook = hook_with(&[("sound", "3f2a1b6c-0000-4000-8000-000000000001")]);
    let sound = hook.sound.expect("a sound was read");
    let spec = game(vec![
        at(1, 0.0, 0.0)
            .with_component(Component::Trigger(Trigger::default()))
            .expect("zone")
            .with_component(Component::Hook(hook))
            .expect("hook"),
        visitor(2, 10.0, 10.0, 0.0, 0.0),
    ]);
    let run = play(&spec, FPS, 3, &hold(3)).expect("run");
    let sounds: Vec<(u32, flare_core::SoundId, f64, bool)> = run
        .events
        .iter()
        .filter_map(|(frame, event)| match event {
            Event::Sound {
                entity: _,
                sound,
                volume,
                looping,
            } => Some((*frame, *sound, *volume, *looping)),
            _ => None,
        })
        .collect();
    assert_eq!(sounds, vec![(0, sound, 1.0, false)], "once, on the entry");
}

#[test]
fn a_hook_brings_its_spawn_to_the_zone_and_into_sight() {
    let hook = hook_with(&[("spawn", "3")]);
    let spec = game(vec![
        at(3, 999.0, 999.0)
            .with_component(Component::Sprite(Sprite {
                visible: false,
                ..Sprite::default()
            }))
            .expect("hidden"),
        at(1, 0.0, 0.0)
            .with_component(Component::Trigger(Trigger::default()))
            .expect("zone")
            .with_component(Component::Hook(hook))
            .expect("hook"),
        visitor(2, 10.0, 10.0, 0.0, 0.0),
    ]);
    let mut world = World::from_spec(&spec, FPS).expect("world");
    world.step(&Input::none()).expect("step");
    let place = world.transform(id(3)).expect("spawn moved");
    assert_eq!((place.x, place.y), (0.0, 0.0), "at the zone's place");
    assert!(
        world
            .sprites_in_order()
            .iter()
            .any(|sprite| sprite.entity == id(3)),
        "the spawn is in sight"
    );
}

#[test]
fn a_hook_hands_the_camera_to_its_visitor_and_survives_destroying_them() {
    let hook = hook_with(&[("follow", "true"), ("destroy", "visitor")]);
    let spec = game(vec![
        at(1, 0.0, 0.0)
            .with_component(Component::Trigger(Trigger::default()))
            .expect("zone")
            .with_component(Component::Hook(hook))
            .expect("hook"),
        visitor(2, 10.0, 10.0, 0.0, 0.0),
        at(3, 1000.0, 1000.0)
            .with_component(Component::Camera(flare_game::Camera::default()))
            .expect("camera"),
    ]);
    let mut world = World::from_spec(&spec, FPS).expect("world");
    world.step(&Input::none()).expect("the hook fires");
    assert_eq!(world.len(), 2, "the visitor is gone");
    let before = world.view(id(3)).expect("view");
    // The camera now follows a removed entity; the step holds still instead of ending.
    world.step(&Input::none()).expect("the next step");
    assert_eq!(world.view(id(3)).expect("view"), before);
}

#[test]
fn a_hook_may_remove_its_own_zone() {
    let hook = hook_with(&[("destroy", "this")]);
    let spec = game(vec![
        at(1, 0.0, 0.0)
            .with_component(Component::Trigger(Trigger::default()))
            .expect("zone")
            .with_component(Component::Hook(hook))
            .expect("hook"),
        visitor(2, 10.0, 10.0, 0.0, 0.0),
    ]);
    let mut world = World::from_spec(&spec, FPS).expect("world");
    world.step(&Input::none()).expect("step");
    assert_eq!(world.len(), 1, "the zone is gone, the visitor stays");
}

#[test]
fn an_action_hook_fires_when_its_action_starts_not_while_it_is_held() {
    let hook = hook_with(&[
        ("when", "action:pounce"),
        ("sound", "3f2a1b6c-0000-4000-8000-000000000002"),
    ]);
    let spec = game(vec![
        at(1, 0.0, 0.0)
            .with_component(Component::Hook(hook))
            .expect("hook"),
    ]);
    let script = parse_script("action:pounce;action:pounce;;").expect("script");
    let run = play(&spec, FPS, script.len() as u32, &script).expect("run");
    let sounds: Vec<u32> = run
        .events
        .iter()
        .filter_map(|(frame, event)| match event {
            Event::Sound { entity, .. } if *entity == id(1) => Some(*frame),
            _ => None,
        })
        .collect();
    assert_eq!(sounds, vec![0], "once on the rising edge of the action");
    assert!(
        run.events
            .iter()
            .any(|(_, event)| matches!(event, Event::Hook { entity } if *entity == id(1)))
    );
}

#[test]
fn a_run_with_triggers_is_the_same_twice() {
    let hook = hook_with(&[("spawn", "3"), ("follow", "true")]);
    let spec = game(vec![
        at(3, 999.0, 999.0)
            .with_component(Component::Sprite(Sprite::default()))
            .expect("sprite"),
        zone(1, 0.0, 0.0, false)
            .with_component(Component::Hook(hook))
            .expect("hook"),
        visitor(2, 150.0, 50.0, -SPEED, 0.0),
        at(4, 500.0, 500.0)
            .with_component(Component::Camera(flare_game::Camera::default()))
            .expect("camera"),
    ]);
    let a = play(&spec, FPS, 8, &hold(8)).expect("run");
    let b = play(&spec, FPS, 8, &hold(8)).expect("run");
    assert_eq!(a, b);
}
