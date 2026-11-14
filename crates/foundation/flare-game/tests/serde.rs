//! Stored descriptions go through the same checks as edits: a file cannot name entity zero,
//! a component under the wrong kind, or an entity under another ID.

#![allow(clippy::expect_used)]

use flare_game::{EntityId, GameSpec};

#[test]
fn entity_zero_is_refused_when_read() {
    assert!(serde_json::from_str::<EntityId>("0").is_err());
    assert_eq!(serde_json::from_str::<EntityId>("7").expect("id").get(), 7);
}

#[test]
fn a_stored_game_is_checked_when_read() {
    let stored = r#"{"gravity":[0.0,9.8],"entities":{"1":{"id":1,"name":"a","symbol":null,
        "components":{"Transform":{"Transform":{"x":1.0,"y":2.0}}}}}}"#;
    let spec: GameSpec = serde_json::from_str(stored).expect("valid");
    assert_eq!(spec.len(), 1);
    assert_eq!(spec.gravity(), (0.0, 9.8));
}

#[test]
fn a_stored_entity_under_another_id_is_refused() {
    let stored = r#"{"gravity":[0.0,0.0],"entities":{"2":{"id":1,"name":"a","symbol":null,
        "components":{}}}}"#;
    assert!(serde_json::from_str::<GameSpec>(stored).is_err());
}

#[test]
fn a_stored_component_with_a_bad_value_is_refused() {
    let stored = r#"{"gravity":[0.0,0.0],"entities":{"1":{"id":1,"name":"a","symbol":null,
        "components":{"Transform":{"Transform":{"x":1.0,"y":2.0,"rotation":0.0,"scale_x":1.0,
        "scale_y":1.0,"pivot_x":9.0,"pivot_y":0.5}}}}}}"#;
    assert!(serde_json::from_str::<GameSpec>(stored).is_err());
}

#[test]
fn a_trigger_a_hook_and_actions_round_trip() {
    use flare_game::EntitySpec;
    use flare_game::{Actions, Component, Hook, HookWhen, Transform, Trigger};
    let mut spec = GameSpec::new();
    spec.add_entity(
        EntitySpec::new(EntityId::new(1).expect("id"), "zone")
            .expect("name")
            .with_component(Component::Transform(Transform::default()))
            .expect("transform")
            .with_component(Component::Trigger(Trigger {
                width: 64.0,
                height: 32.0,
                once: true,
            }))
            .expect("trigger")
            .with_component(Component::Hook(Hook {
                when: HookWhen::Trigger,
                ..Hook::default()
            }))
            .expect("hook")
            .with_component(Component::Actions(Actions {
                names: vec!["pounce".to_owned()],
            }))
            .expect("actions"),
    )
    .expect("add");
    let stored = serde_json::to_string(&spec).expect("stored");
    let back: GameSpec = serde_json::from_str(&stored).expect("read");
    assert_eq!(back, spec);
}

#[test]
fn a_stored_hook_without_its_zone_is_refused_when_read() {
    let stored = r#"{"gravity":[0.0,0.0],"entities":{"1":{"id":1,"name":"a","symbol":null,
        "components":{"Transform":{"Transform":{"x":0.0,"y":0.0,"rotation":0.0,"scale_x":1.0,
        "scale_y":1.0,"pivot_x":0.5,"pivot_y":0.5}},"Hook":{"Hook":{"when":"Trigger",
        "sound":null,"spawn":null,"follow":false,"destroy":"Never"}}}}}}"#;
    assert!(serde_json::from_str::<GameSpec>(stored).is_err());
}
