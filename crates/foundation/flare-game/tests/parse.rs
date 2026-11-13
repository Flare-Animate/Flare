//! Components from the `name=value` fields the surfaces send: defaults, every refusal named, and
//! keys and bounds read as written.
#![allow(clippy::expect_used)]

mod support;

use flare_game::{Component, ComponentKind, GameError, Key, Sprite};
use support::fields;

#[test]
fn missing_fields_take_their_defaults() {
    let component = Component::parse("sprite", &[]).expect("defaults are valid");
    assert_eq!(component, Component::Sprite(Sprite::default()));
}

#[test]
fn every_kind_has_a_name_that_parses_back() {
    for kind in ComponentKind::ALL {
        assert_eq!(ComponentKind::parse(kind.name()), Some(kind));
    }
    assert_eq!(ComponentKind::parse("Transform"), None);
}

#[test]
fn an_unknown_component_is_named() {
    assert_eq!(
        Component::parse("gravity", &[]),
        Err(GameError::UnknownComponent("gravity".to_owned()))
    );
}

#[test]
fn a_misspelt_field_is_refused_not_ignored() {
    let err = Component::parse("body", &fields(&[("widht", "10")])).expect_err("typo");
    assert_eq!(
        err,
        GameError::UnknownField {
            component: "body",
            field: "widht".to_owned(),
        }
    );
}

#[test]
fn a_field_given_twice_is_refused() {
    let err =
        Component::parse("body", &fields(&[("width", "1"), ("width", "2")])).expect_err("twice");
    assert!(matches!(err, GameError::BadInput(_)));
}

#[test]
fn numbers_must_parse_and_be_finite_and_in_range() {
    assert!(matches!(
        Component::parse("body", &fields(&[("width", "wide")])),
        Err(GameError::BadField { field: "width", .. })
    ));
    assert!(matches!(
        Component::parse("body", &fields(&[("width", "NaN")])),
        Err(GameError::BadField { field: "width", .. })
    ));
    assert!(matches!(
        Component::parse("body", &fields(&[("width", "-1")])),
        Err(GameError::BadField { field: "width", .. })
    ));
    assert!(matches!(
        Component::parse("audio", &fields(&[("volume", "1.5")])),
        Err(GameError::BadField {
            field: "volume",
            ..
        })
    ));
}

#[test]
fn a_frame_must_be_inside_the_animation() {
    assert!(matches!(
        Component::parse("sprite", &fields(&[("frames", "4"), ("frame", "4")])),
        Err(GameError::BadField { field: "frame", .. })
    ));
}

#[test]
fn keys_are_named_and_unknown_keys_are_refused() {
    let component = Component::parse("control", &fields(&[("left", "space")])).expect("key");
    match component {
        Component::Control(control) => assert_eq!(control.left, Key::Space),
        other => panic!("expected a control, got {other:?}"),
    }
    assert_eq!(
        Component::parse("control", &fields(&[("left", "shift")])),
        Err(GameError::UnknownKey("shift".to_owned()))
    );
}

#[test]
fn camera_bounds_are_four_numbers() {
    let ok = Component::parse("camera", &fields(&[("bounds", "0,0,800,600")]));
    assert!(matches!(ok, Ok(Component::Camera(c)) if c.bounds.is_some()));
    assert!(matches!(
        Component::parse("camera", &fields(&[("bounds", "0,0,800")])),
        Err(GameError::BadField {
            field: "bounds",
            ..
        })
    ));
}

#[test]
fn a_body_kind_is_one_of_three_names() {
    assert!(Component::parse("body", &fields(&[("kind", "static")])).is_ok());
    assert!(matches!(
        Component::parse("body", &fields(&[("kind", "ghost")])),
        Err(GameError::BadField { field: "kind", .. })
    ));
}

#[test]
fn a_script_needs_a_name() {
    assert!(matches!(
        Component::parse("script", &[]),
        Err(GameError::BadField { field: "name", .. })
    ));
}

#[test]
fn a_trigger_has_a_size_and_an_once_flag() {
    let component = Component::parse("trigger", &fields(&[("width", "64"), ("once", "true")]))
        .expect("trigger");
    match component {
        Component::Trigger(trigger) => {
            assert_eq!(trigger.width, 64.0);
            assert_eq!(trigger.height, 32.0, "the height keeps its default");
            assert!(trigger.once);
        }
        other => panic!("expected a trigger, got {other:?}"),
    }
    assert!(matches!(
        Component::parse("trigger", &fields(&[("width", "-1")])),
        Err(GameError::BadField { field: "width", .. })
    ));
}

#[test]
fn a_hook_reads_its_when_its_sound_and_what_it_does() {
    let component = Component::parse(
        "hook",
        &fields(&[
            ("when", "trigger"),
            ("sound", "3f2a1b6c-0000-4000-8000-000000000001"),
            ("spawn", "4"),
            ("follow", "true"),
            ("destroy", "visitor"),
        ]),
    )
    .expect("hook");
    match component {
        Component::Hook(hook) => {
            assert_eq!(hook.when, flare_game::HookWhen::Trigger);
            assert!(hook.sound.is_some());
            assert_eq!(hook.spawn, Some(flare_game::EntityId::new(4).expect("id")));
            assert!(hook.follow);
            assert_eq!(hook.destroy, flare_game::DestroyWho::Visitor);
        }
        other => panic!("expected a hook, got {other:?}"),
    }
    let action = Component::parse("hook", &fields(&[("when", "action:pounce")]))
        .expect("an action when parses");
    match action {
        Component::Hook(hook) => {
            assert_eq!(hook.when, flare_game::HookWhen::Action("pounce".to_owned()))
        }
        other => panic!("expected a hook, got {other:?}"),
    }
}

#[test]
fn a_hook_when_must_be_trigger_or_an_action() {
    assert!(matches!(
        Component::parse("hook", &fields(&[("when", "pressure")])),
        Err(GameError::BadField { field: "when", .. })
    ));
    assert!(matches!(
        Component::parse("hook", &fields(&[("destroy", "both")])),
        Err(GameError::BadField {
            field: "destroy",
            ..
        })
    ));
    assert!(matches!(
        Component::parse("hook", &fields(&[("spawn", "zero")])),
        Err(GameError::BadField { field: "spawn", .. })
    ));
    assert!(matches!(
        Component::parse("hook", &fields(&[("sound", "not-a-uuid")])),
        Err(GameError::BadField { field: "sound", .. })
    ));
}

#[test]
fn an_actions_list_is_comma_separated_names() {
    let component =
        Component::parse("actions", &fields(&[("names", "pounce, hide ,")])).expect("actions");
    match component {
        Component::Actions(actions) => {
            assert_eq!(actions.names, vec!["pounce".to_owned(), "hide".to_owned()]);
        }
        other => panic!("expected an actions list, got {other:?}"),
    }
    // An empty field is an empty list, not a name of one space.
    let empty = Component::parse("actions", &fields(&[("names", "")])).expect("empty");
    match empty {
        Component::Actions(actions) => assert!(actions.names.is_empty()),
        other => panic!("expected an actions list, got {other:?}"),
    }
}

#[test]
fn input_actions_are_named_tokens() {
    let input = flare_game::Input::parse("action:pounce,action:hide").expect("actions");
    assert!(input.action("pounce"));
    assert!(input.action("hide"));
    assert!(!input.action("roar"));
    // A typo is refused by name, as the keys are.
    assert!(matches!(
        flare_game::Input::parse("action:"),
        Err(GameError::BadInput(_))
    ));
    assert!(matches!(
        flare_game::Input::parse("action:has\ncontrol"),
        Err(GameError::BadInput(_))
    ));
    // Unknown bare tokens stay unknown keys, not actions.
    assert_eq!(
        flare_game::Input::parse("pounce"),
        Err(GameError::UnknownKey("pounce".to_owned()))
    );
}
