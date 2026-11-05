//! Helpers the game tests share: small descriptions built in one line.
#![allow(clippy::expect_used)]
#![allow(dead_code)]
use flare_game::{Component, EntityId, EntitySpec, GameSpec, Transform};

pub fn id(raw: u32) -> EntityId {
    EntityId::new(raw).expect("non-zero test id")
}

/// An entity with a transform at `(x, y)`.
pub fn at(raw: u32, x: f64, y: f64) -> EntitySpec {
    EntitySpec::new(id(raw), format!("e{raw}"))
        .expect("name")
        .with_component(Component::Transform(Transform {
            x,
            y,
            ..Transform::default()
        }))
        .expect("transform")
}

/// A game holding `entities`, with no gravity.
pub fn game(entities: Vec<EntitySpec>) -> GameSpec {
    let mut spec = GameSpec::new();
    for entity in entities {
        spec.add_entity(entity).expect("entity is valid");
    }
    spec
}

/// The fields of a component written as the surfaces write them.
pub fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}
