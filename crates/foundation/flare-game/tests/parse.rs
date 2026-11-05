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
