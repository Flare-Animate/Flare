//! The camera, sprite, audio, particle and UI systems, and the view a camera gives.
#![allow(clippy::expect_used)]

mod support;

use flare_core::SoundId;
use flare_game::{
    Audio, Bounds, Camera, Component, Event, GameError, Input, Particle, Sprite, Ui, World,
};
use support::{at, fields, game, id};

fn world_of(spec: &flare_game::GameSpec, fps: f64) -> World {
    World::from_spec(spec, fps).expect("world")
}

#[test]
fn a_camera_closes_on_its_target_by_the_lerp_each_second() {
    let camera = at(2, 0.0, 0.0)
        .with_component(Component::Camera(Camera {
            follow: Some(id(1)),
            lerp: 1.0,
            ..Camera::default()
        }))
        .expect("camera");
    let spec = game(vec![at(1, 100.0, 50.0), camera]);
    let mut world = world_of(&spec, 10.0);
    world.step(&Input::none()).expect("step");
    // k = lerp * dt = 0.1 of the way to the target.
    let (x, y) = world.position(id(2)).expect("camera position");
    assert!(
        (x - 10.0).abs() < 1e-9 && (y - 5.0).abs() < 1e-9,
        "({x}, {y})"
    );
}

#[test]
fn a_camera_stays_inside_its_bounds() {
    let camera = at(2, 0.0, 0.0)
        .with_component(Component::Camera(Camera {
            follow: Some(id(1)),
            lerp: 1000.0,
            bounds: Some(Bounds {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            }),
            ..Camera::default()
        }))
        .expect("camera");
    let spec = game(vec![at(1, 500.0, -500.0), camera]);
    let mut world = world_of(&spec, 10.0);
    world.step(&Input::none()).expect("step");
    assert_eq!(world.position(id(2)).expect("camera"), (50.0, 0.0));
}

#[test]
fn the_view_maps_the_camera_centre_to_the_middle_of_the_viewport() {
    let camera = at(1, 30.0, 40.0)
        .with_component(Component::Camera(Camera {
            width: 200.0,
            height: 100.0,
            zoom: 2.0,
            ..Camera::default()
        }))
        .expect("camera");
    let world = world_of(&game(vec![camera]), 10.0);
    let view = world.view(id(1)).expect("view");
    assert_eq!(view.to_screen(30.0, 40.0), (100.0, 50.0));
    // Two pixels right of the centre at zoom 2 is four pixels right on screen.
    assert_eq!(view.to_screen(32.0, 40.0), (104.0, 50.0));
}

#[test]
fn a_sprite_advances_at_its_frame_rate_and_loops() {
    let sprite = at(1, 0.0, 0.0)
        .with_component(Component::Sprite(Sprite {
            frames: 3,
            fps: 10.0,
            ..Sprite::default()
        }))
        .expect("sprite");
    let spec = game(vec![sprite]);
    let mut world = world_of(&spec, 10.0);
    let shown: Vec<u32> = (0..4)
        .map(|_| {
            world.step(&Input::none()).expect("step");
            world.sprite_frame(id(1)).expect("frame")
        })
        .collect();
    // Ten frames a second at ten steps a second: one frame a step, wrapping after three.
    assert_eq!(shown, vec![1, 2, 0, 1]);
}

#[test]
fn a_sprite_that_does_not_loop_holds_its_last_frame() {
    let sprite = at(1, 0.0, 0.0)
        .with_component(Component::Sprite(Sprite {
            frames: 2,
            fps: 10.0,
            looping: false,
            ..Sprite::default()
        }))
        .expect("sprite");
    let mut world = world_of(&game(vec![sprite]), 10.0);
    for _ in 0..5 {
        world.step(&Input::none()).expect("step");
    }
    assert_eq!(world.sprite_frame(id(1)).expect("frame"), 1);
}

#[test]
fn only_visible_sprites_are_drawn_lowest_order_first() {
    let make = |raw: u32, order: i32, visible: bool| {
        at(raw, 0.0, 0.0)
            .with_component(Component::Sprite(Sprite {
                order,
                visible,
                ..Sprite::default()
            }))
            .expect("sprite")
    };
    let world = world_of(
        &game(vec![
            make(1, 5, true),
            make(2, -1, true),
            make(3, 0, false),
            make(4, 5, true),
        ]),
        10.0,
    );
    let order: Vec<u32> = world
        .sprites_in_order()
        .iter()
        .map(|sprite| sprite.entity.get())
        .collect();
    assert_eq!(order, vec![2, 1, 4]);
}

#[test]
fn a_sound_starts_once_on_the_first_step_when_it_plays_on_start() {
    let sound = SoundId::new();
    let audio = at(1, 0.0, 0.0)
        .with_component(Component::Audio(Audio {
            sound: Some(sound),
            play_on_start: true,
            volume: 0.5,
            ..Audio::default()
        }))
        .expect("audio");
    let mut world = world_of(&game(vec![audio]), 10.0);
    let first = world.step(&Input::none()).expect("step");
    assert_eq!(
        first,
        vec![Event::Sound {
            entity: id(1),
            sound,
            volume: 0.5,
            looping: false,
        }]
    );
    assert!(world.step(&Input::none()).expect("step").is_empty());
}

#[test]
fn an_emitter_counts_particles_at_its_rate() {
    let emitter = at(1, 0.0, 0.0)
        .with_component(Component::Particle(Particle {
            rate: 5.0,
            ..Particle::default()
        }))
        .expect("particle");
    let mut world = world_of(&game(vec![emitter]), 10.0);
    let mut total = 0;
    for _ in 0..10 {
        for event in world.step(&Input::none()).expect("step") {
            if let Event::Emit { count, .. } = event {
                total += count;
            }
        }
    }
    // Five a second for one second of steps: exactly five, with no rounding loss.
    assert_eq!(total, 5);
}

#[test]
fn a_press_inside_an_interactive_element_is_a_click_and_outside_is_not() {
    let button = |raw: u32, x: f64, interactive: bool| {
        at(raw, x, 0.0)
            .with_component(Component::Ui(Ui {
                width: 10.0,
                height: 10.0,
                interactive,
                ..Ui::default()
            }))
            .expect("ui")
    };
    let spec = game(vec![
        button(1, 0.0, true),
        button(2, 50.0, true),
        button(3, 0.0, false),
    ]);
    let mut world = world_of(&spec, 10.0);
    let hit = world
        .step(&Input::click(5.0, 5.0).expect("click"))
        .expect("step");
    assert_eq!(
        hit,
        vec![Event::Click {
            entity: id(1),
            x: 5.0,
            y: 5.0,
        }]
    );
    let miss = world
        .step(&Input::click(30.0, 30.0).expect("click"))
        .expect("step");
    assert!(miss.is_empty());
    assert!(world.step(&Input::none()).expect("step").is_empty());
}

#[test]
fn an_audio_component_without_a_sound_or_without_play_on_start_is_silent() {
    let quiet = at(1, 0.0, 0.0)
        .with_component(Component::Audio(Audio {
            play_on_start: true,
            ..Audio::default()
        }))
        .expect("audio");
    let mut world = world_of(&game(vec![quiet]), 10.0);
    assert!(world.step(&Input::none()).expect("step").is_empty());
}

#[test]
fn a_ui_element_parses_its_fields_from_the_surface() {
    let parsed =
        Component::parse("ui", &fields(&[("kind", "button"), ("text", "Play")])).expect("ui");
    assert!(matches!(parsed, Component::Ui(ref u) if u.text == "Play"));
    assert!(matches!(
        Component::parse("ui", &fields(&[("kind", "hologram")])),
        Err(GameError::BadField { field: "kind", .. })
    ));
}
