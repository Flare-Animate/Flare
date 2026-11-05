//! The game description's edits and runs, shared by the command line and MCP. An edit is the
//! command batch for one undo step; a run steps the description under a script and reports its
//! events and digest. Nothing here draws, plays sound or touches a file.

use flare_commands::Command;
use flare_core::SymbolId;
use flare_document::Document;
use flare_game::{
    Component, ComponentKind, EntityId, EntitySpec, Event, GameError, GameSpec, Playback,
    parse_script, play,
};
use serde_json::{Value, json};

/// Adds an entity named `name`, optionally drawing `symbol`. Returns its new ID with the batch.
pub fn add_entity(
    document: &Document,
    name: &str,
    symbol: Option<SymbolId>,
) -> Result<(EntityId, Vec<Command>), GameError> {
    let id = document
        .game()
        .cloned()
        .unwrap_or_else(GameSpec::new)
        .next_id()?;
    let entity = EntitySpec::new(id, name)?.with_symbol(symbol);
    Ok((id, vec![Command::AddGameEntity(entity)]))
}

/// Sets one component of `entity` from its kind name and `name=value` fields.
pub fn set_component(
    entity: EntityId,
    kind: &str,
    fields: &[(String, String)],
) -> Result<Vec<Command>, GameError> {
    let component = Component::parse(kind, fields)?;
    Ok(vec![Command::SetGameComponent { entity, component }])
}

/// Removes the component of `kind` (by kind name) from `entity`.
pub fn remove_component(entity: EntityId, kind: &str) -> Result<Vec<Command>, GameError> {
    let kind =
        ComponentKind::parse(kind).ok_or_else(|| GameError::UnknownComponent(kind.to_owned()))?;
    Ok(vec![Command::RemoveGameComponent { entity, kind }])
}

/// Removes `entity` and its components.
#[must_use]
pub fn remove_entity(entity: EntityId) -> Vec<Command> {
    vec![Command::RemoveGameEntity(entity)]
}

/// Sets the world's gravity, in pixels per second squared.
#[must_use]
pub fn set_gravity(x: f64, y: f64) -> Vec<Command> {
    vec![Command::SetGameGravity { x, y }]
}

/// Steps the document's game for `frames` frames at `frames_per_second`, under `script` (one
/// `input` line per frame, as [`parse_script`] reads it) when given.
pub fn run(
    document: &Document,
    frames_per_second: f64,
    frames: u32,
    script: Option<&str>,
) -> Result<Playback, GameError> {
    let game = document.game().ok_or_else(|| {
        GameError::BadInput("the project has no game; add an entity first".to_owned())
    })?;
    let script = match script {
        Some(text) => parse_script(text)?,
        None => Vec::new(),
    };
    play(game, frames_per_second, frames, &script)
}

/// The game as JSON, in its stored form; `null` when the project has none.
pub fn describe(document: &Document) -> Value {
    document
        .game()
        .and_then(|game| serde_json::to_value(game).ok())
        .unwrap_or(Value::Null)
}

/// A run as JSON: its frame count, the digest of the final state as hex (a 64-bit number is
/// not exact in every JSON reader), and each event with the frame it happened on.
#[must_use]
pub fn playback_json(playback: &Playback) -> Value {
    let events: Vec<Value> = playback
        .events
        .iter()
        .map(|(frame, event)| {
            let mut value = match event {
                Event::Collision { a, b, begin } => {
                    json!({ "kind": "collision", "a": a, "b": b, "begin": begin })
                }
                Event::Click { entity, x, y } => {
                    json!({ "kind": "click", "entity": entity, "x": x, "y": y })
                }
                Event::Sound {
                    entity,
                    sound,
                    volume,
                    looping,
                } => json!({
                    "kind": "sound",
                    "entity": entity,
                    "sound": sound.as_uuid().to_string(),
                    "volume": volume,
                    "looping": looping,
                }),
                Event::Emit { entity, count } => {
                    json!({ "kind": "emit", "entity": entity, "count": count })
                }
            };
            value["frame"] = json!(frame);
            value
        })
        .collect();
    json!({
        "frames": playback.frames,
        "digest": format!("{:016x}", playback.digest),
        "events": events,
    })
}

#[cfg(test)]
mod tests;
