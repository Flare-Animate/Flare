//! The game description: IDs, names, requirements, limits and the refusals that must change
//! nothing.
#![allow(clippy::expect_used)]

mod support;

use flare_core::SymbolId;
use flare_game::{
    Body, Camera, Component, ComponentKind, EntitySpec, GameError, GameSpec, MAX_ENTITIES,
    Transform,
};
use support::{at, game, id};

#[test]
fn entity_zero_is_never_an_entity() {
    assert!(flare_game::EntityId::new(0).is_none());
    assert_eq!(id(7).get(), 7);
}

#[test]
fn ids_count_up_from_the_largest_in_use() {
    let mut spec = GameSpec::new();
    assert_eq!(spec.next_id().expect("free").get(), 1);
    spec.add_entity(at(4, 0.0, 0.0)).expect("add");
    assert_eq!(spec.next_id().expect("free").get(), 5);
}

#[test]
fn a_duplicate_id_is_refused_and_the_game_is_unchanged() {
    let mut spec = game(vec![at(1, 0.0, 0.0)]);
    let before = spec.clone();
    assert_eq!(
        spec.add_entity(at(1, 9.0, 9.0)),
        Err(GameError::DuplicateEntity(id(1)))
    );
    assert_eq!(spec, before);
}

#[test]
fn names_are_one_to_128_characters_without_control_characters() {
    assert!(EntitySpec::new(id(1), "").is_err());
    assert!(EntitySpec::new(id(1), "a\nb").is_err());
    assert!(EntitySpec::new(id(1), "x".repeat(129)).is_err());
    assert!(EntitySpec::new(id(1), "x".repeat(128)).is_ok());
}

#[test]
fn a_body_without_a_transform_is_refused() {
    let bare = EntitySpec::new(id(1), "bare").expect("name");
    let err = bare
        .with_component(Component::Body(Body::default()))
        .expect_err("body needs a transform");
    assert_eq!(
        err,
        GameError::MissingComponent {
            component: ComponentKind::Body,
            needs: ComponentKind::Transform,
        }
    );
}

#[test]
fn a_control_needs_a_body_and_removing_a_needed_body_is_refused() {
    let mut spec = game(vec![at(1, 0.0, 0.0)]);
    let no_body = spec.set_component(id(1), Component::Control(flare_game::Control::default()));
    assert!(matches!(no_body, Err(GameError::MissingComponent { .. })));

    spec.set_component(id(1), Component::Body(Body::default()))
        .expect("body");
    spec.set_component(id(1), Component::Control(flare_game::Control::default()))
        .expect("control");
    assert_eq!(
        spec.remove_component(id(1), ComponentKind::Body),
        Err(GameError::StillNeeded {
            component: ComponentKind::Body,
            by: ComponentKind::Control,
        })
    );
}

#[test]
fn a_camera_cannot_follow_a_missing_entity_and_the_edit_changes_nothing() {
    let mut spec = game(vec![at(1, 0.0, 0.0)]);
    let before = spec.clone();
    let camera = Camera {
        follow: Some(id(99)),
        ..Camera::default()
    };
    assert_eq!(
        spec.set_component(id(1), Component::Camera(camera)),
        Err(GameError::NoSuchEntity(id(99)))
    );
    assert_eq!(spec, before);
}

#[test]
fn an_followed_entity_cannot_be_removed_or_lose_its_transform() {
    let mut spec = game(vec![at(1, 0.0, 0.0), at(2, 5.0, 5.0)]);
    spec.set_component(
        id(2),
        Component::Camera(Camera {
            follow: Some(id(1)),
            ..Camera::default()
        }),
    )
    .expect("camera follows entity 1");
    assert_eq!(spec.remove_entity(id(1)), Err(GameError::Followed(id(1))));
    assert_eq!(
        spec.remove_component(id(1), ComponentKind::Transform),
        Err(GameError::Followed(id(1)))
    );
    spec.remove_entity(id(2)).expect("the camera itself can go");
    spec.remove_entity(id(1)).expect("now nothing follows it");
}

#[test]
fn an_unknown_entity_is_refused_by_name() {
    let mut spec = GameSpec::new();
    assert_eq!(
        spec.set_component(id(3), Component::Transform(Transform::default())),
        Err(GameError::NoSuchEntity(id(3)))
    );
    assert_eq!(
        spec.remove_entity(id(3)).map(|_| ()),
        Err(GameError::NoSuchEntity(id(3)))
    );
}

#[test]
fn an_entity_may_draw_a_symbol() {
    let symbol = SymbolId::new();
    let entity = at(1, 0.0, 0.0).with_symbol(Some(symbol));
    let mut spec = GameSpec::new();
    spec.add_entity(entity).expect("add");
    assert_eq!(
        spec.entity(id(1)).and_then(EntitySpec::symbol),
        Some(symbol)
    );
}

#[test]
fn the_entity_count_has_a_limit() {
    let mut spec = GameSpec::new();
    for raw in 1..=MAX_ENTITIES as u32 {
        spec.add_entity(at(raw, 0.0, 0.0))
            .expect("within the limit");
    }
    assert_eq!(
        spec.add_entity(at(MAX_ENTITIES as u32 + 1, 0.0, 0.0)),
        Err(GameError::TooManyEntities {
            limit: MAX_ENTITIES
        })
    );
}

#[test]
fn the_body_count_has_a_limit() {
    let mut spec = GameSpec::new();
    for raw in 1..=flare_game::MAX_BODIES as u32 {
        let entity = at(raw, 0.0, 0.0)
            .with_component(Component::Body(Body::default()))
            .expect("body");
        spec.add_entity(entity).expect("within the limit");
    }
    let one_more = at(9999, 0.0, 0.0)
        .with_component(Component::Body(Body::default()))
        .expect("body");
    assert_eq!(
        spec.add_entity(one_more),
        Err(GameError::TooManyBodies {
            limit: flare_game::MAX_BODIES
        })
    );
}

#[test]
fn gravity_must_be_finite() {
    assert_eq!(
        GameSpec::new().with_gravity(f64::NAN, 0.0).map(|_| ()),
        Err(GameError::NonFinite("x"))
    );
    assert!(GameSpec::new().with_gravity(0.0, 9.8).is_ok());
}

#[test]
fn a_valid_game_has_no_problems() {
    let spec = game(vec![at(1, 0.0, 0.0), at(2, 3.0, 4.0)]);
    assert!(spec.problems().is_empty());
    assert!(spec.validate().is_ok());
}

#[test]
fn a_trigger_walking_hook_needs_the_zone_on_the_same_entity() {
    let entity = EntitySpec::new(id(1), "hooked")
        .expect("name")
        .with_component(Component::Transform(Transform::default()))
        .expect("transform");
    let err = entity
        .clone()
        .with_component(Component::Hook(flare_game::Hook::default()))
        .expect_err("a trigger when needs a trigger");
    assert_eq!(
        err,
        GameError::MissingComponent {
            component: ComponentKind::Hook,
            needs: ComponentKind::Trigger,
        }
    );
    // An action `when` does not.
    let action = flare_game::Hook {
        when: flare_game::HookWhen::Action("pounce".to_owned()),
        ..flare_game::Hook::default()
    };
    entity
        .with_component(Component::Hook(action))
        .expect("an action hook needs no zone");
}

#[test]
fn a_listening_hook_keeps_its_zone_and_a_spawned_entity_cannot_go() {
    let mut spec = game(vec![at(1, 0.0, 0.0), at(2, 5.0, 5.0)]);
    spec.set_component(id(1), Component::Trigger(flare_game::Trigger::default()))
        .expect("zone");
    spec.set_component(id(1), Component::Hook(flare_game::Hook::default()))
        .expect("hook on the zone");
    assert_eq!(
        spec.remove_component(id(1), ComponentKind::Trigger),
        Err(GameError::StillNeeded {
            component: ComponentKind::Trigger,
            by: ComponentKind::Hook,
        })
    );

    spec.set_component(
        id(1),
        Component::Hook(flare_game::Hook {
            spawn: Some(id(2)),
            ..flare_game::Hook::default()
        }),
    )
    .expect("the hook brings entity 2");
    assert_eq!(spec.remove_entity(id(2)), Err(GameError::Spawned(id(2))));
}

#[test]
fn a_hook_cannot_spawn_an_entity_that_does_not_exist() {
    let mut spec = game(vec![at(1, 0.0, 0.0)]);
    let before = spec.clone();
    assert_eq!(
        spec.set_component(
            id(1),
            Component::Hook(flare_game::Hook {
                spawn: Some(id(9)),
                ..flare_game::Hook::default()
            }),
        ),
        Err(GameError::NoSuchEntity(id(9)))
    );
    assert_eq!(spec, before);
}

#[test]
fn a_hook_cannot_follow_or_destroy_without_a_zone_to_have_a_visitor() {
    let follow = flare_game::Hook {
        when: flare_game::HookWhen::Action("pounce".to_owned()),
        follow: true,
        ..flare_game::Hook::default()
    };
    let entity = EntitySpec::new(id(1), "hooked")
        .expect("name")
        .with_component(Component::Transform(Transform::default()))
        .expect("transform");
    assert!(
        entity
            .clone()
            .with_component(Component::Hook(follow))
            .is_err()
    );

    let destroy = flare_game::Hook {
        when: flare_game::HookWhen::Action("pounce".to_owned()),
        destroy: flare_game::DestroyWho::Visitor,
        ..flare_game::Hook::default()
    };
    assert!(entity.with_component(Component::Hook(destroy)).is_err());
}

#[test]
fn the_trigger_count_has_a_limit() {
    let mut spec = GameSpec::new();
    for raw in 1..=flare_game::MAX_TRIGGERS as u32 {
        let zone = at(raw, 0.0, 0.0)
            .with_component(Component::Trigger(flare_game::Trigger::default()))
            .expect("zone");
        spec.add_entity(zone).expect("within the limit");
    }
    let one_more = at(9999, 0.0, 0.0)
        .with_component(Component::Trigger(flare_game::Trigger::default()))
        .expect("zone");
    assert_eq!(
        spec.add_entity(one_more),
        Err(GameError::TooManyTriggers {
            limit: flare_game::MAX_TRIGGERS
        })
    );
}

#[test]
fn an_actions_list_has_at_most_eight_unique_short_names() {
    let too_many = flare_game::Actions {
        names: (0..9).map(|i| format!("action{i}")).collect(),
    };
    let entity = EntitySpec::new(id(1), "actions")
        .expect("name")
        .with_component(Component::Transform(Transform::default()))
        .expect("transform");
    assert!(
        entity
            .clone()
            .with_component(Component::Actions(too_many))
            .is_err()
    );

    let long = flare_game::Actions {
        names: vec!["x".repeat(flare_game::MAX_ACTION_CHARS + 1)],
    };
    assert!(
        entity
            .clone()
            .with_component(Component::Actions(long))
            .is_err()
    );

    let twice = flare_game::Actions {
        names: vec!["pounce".to_owned(), "pounce".to_owned()],
    };
    assert!(entity.with_component(Component::Actions(twice)).is_err());
}
