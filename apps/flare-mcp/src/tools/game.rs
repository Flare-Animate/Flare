//! The game description, through the same edits, runs and export as `flare game`. The behaviour
//! is in `flare_tools::game` and `flare_runtime::export_game`; this maps arguments and results.

use std::collections::BTreeMap;
use std::path::PathBuf;

use flare_core::SymbolId;
use flare_game::{EntityId, parse_fields};
use flare_tools::game as model;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use super::export::{export_dir, fresh};
use crate::color::safe_name;
use crate::ops::{apply, parse_id};
use crate::params_game::{
    AddGameEntity, ExportGame, GameComponentRef, GameEntityRef, RunGame, SetGameComponent,
    SetGameGravity,
};
use crate::server::{FlareServer, Outcome};

fn entity(raw: u32) -> Result<EntityId, String> {
    EntityId::new(raw).ok_or_else(|| format!("{raw} is not an entity ID"))
}

/// The fields as `name=value` tokens. A name with an `=` in it could not be told apart from
/// its value, so it is refused.
fn field_tokens(fields: &BTreeMap<String, String>) -> Result<Vec<String>, String> {
    fields
        .iter()
        .map(|(name, value)| {
            if name.contains('=') {
                Err(format!("field name {name:?} must not contain '='"))
            } else {
                Ok(format!("{name}={value}"))
            }
        })
        .collect()
}

#[tool_router(router = game_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "The game description: gravity and entities with their components, or null when the project has no game."
    )]
    fn get_game(&self) -> Outcome {
        self.read(|state| Ok(model::describe(state.session.document())))
    }

    #[tool(
        description = "Add an entity to the game, optionally drawing a library symbol (its ID). Returns the entity's ID. One undo step."
    )]
    fn add_game_entity(&self, Parameters(a): Parameters<AddGameEntity>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let symbol = a
                .symbol
                .as_deref()
                .map(|text| parse_id(text, "symbol"))
                .transpose()?
                .map(SymbolId::from_uuid);
            let (id, commands) = model::add_entity(state.session.document(), &a.name, symbol)
                .map_err(|e| e.to_string())?;
            let mut value = apply(state, commands)?;
            value["entity"] = json!(id);
            Ok(value)
        })
    }

    #[tool(
        description = "Set one component of an entity: its kind and its fields as text. Replaces a component of that kind. One undo step."
    )]
    fn set_game_component(&self, Parameters(a): Parameters<SetGameComponent>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let id = entity(a.entity)?;
            let fields = parse_fields(&field_tokens(&a.fields)?).map_err(|e| e.to_string())?;
            let commands = model::set_component(id, &a.kind, &fields).map_err(|e| e.to_string())?;
            let mut value = apply(state, commands)?;
            value["entity"] = json!(id);
            Ok(value)
        })
    }

    #[tool(
        description = "Remove an entity's component of the given kind. Refused while another component needs it. One undo step."
    )]
    fn remove_game_component(&self, Parameters(a): Parameters<GameComponentRef>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let id = entity(a.entity)?;
            let commands = model::remove_component(id, &a.kind).map_err(|e| e.to_string())?;
            apply(state, commands)
        })
    }

    #[tool(
        description = "Remove an entity and its components. Refused while a camera follows it. One undo step."
    )]
    fn remove_game_entity(&self, Parameters(a): Parameters<GameEntityRef>) -> Outcome {
        self.write(a.expected_revision, |state| {
            apply(state, model::remove_entity(entity(a.entity)?))
        })
    }

    #[tool(description = "Set the game's gravity in pixels per second squared. One undo step.")]
    fn set_game_gravity(&self, Parameters(a): Parameters<SetGameGravity>) -> Outcome {
        self.write(a.expected_revision, |state| {
            apply(state, model::set_gravity(a.x, a.y))
        })
    }

    #[tool(
        description = "Step the game for `frames` frames at `fps` (default 60) under an optional script (`;`-separated frames of `,`-separated keys). Returns the frame count, the digest of the final state as hex, and every event with its frame. The project is not changed."
    )]
    fn run_game(&self, Parameters(a): Parameters<RunGame>) -> Outcome {
        self.read(|state| {
            let playback = model::run(
                state.session.document(),
                a.fps.unwrap_or(60.0),
                a.frames,
                a.script.as_deref(),
            )
            .map_err(|e| e.to_string())?;
            Ok(model::playback_json(&playback))
        })
    }

    #[tool(
        description = "Export the game into the export directory (disabled when none is configured): `<name>.json` with the description, `<name>.png` with the scene's frames as a sprite sheet, and `<name>.sheet.json` as its atlas. Nothing is overwritten."
    )]
    fn export_game(&self, Parameters(a): Parameters<ExportGame>) -> Outcome {
        self.read(|state| {
            let dir = export_dir(state)?;
            let base = safe_name(a.name.as_deref().unwrap_or("game"))?;
            let json: PathBuf = fresh(&dir, &format!("{base}.json"))?;
            let png: PathBuf = fresh(&dir, &format!("{base}.png"))?;
            let atlas: PathBuf = fresh(&dir, &format!("{base}.sheet.json"))?;
            let export = flare_runtime::export_game(
                state.session.document(),
                a.start.unwrap_or(0),
                a.end,
                a.columns,
                base,
            )?;
            std::fs::write(&json, &export.json).map_err(|e| e.to_string())?;
            std::fs::write(&png, &export.sheet.png).map_err(|e| e.to_string())?;
            std::fs::write(&atlas, &export.sheet.json).map_err(|e| e.to_string())?;
            Ok(json!({
                "json": json.to_string_lossy(),
                "image": png.to_string_lossy(),
                "atlas": atlas.to_string_lossy(),
                "frames": export.sheet.frames,
                "width": export.sheet.width,
                "height": export.sheet.height,
            }))
        })
    }
}
