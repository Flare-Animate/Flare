//! `flare game`: the game description from the command line. The behaviour is in
//! `flare_tools::game` and `flare_runtime::export_game`; this parses arguments and prints.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use flare_core::SymbolId;
use flare_game::{EntityId, parse_fields};
use flare_tools::game as model;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::project::{commit_session, open_session};

/// Largest script file read, in bytes; a run is also bounded by its frame count.
const MAX_SCRIPT_BYTES: u64 = 1024 * 1024;

#[derive(Subcommand)]
pub enum GameCommand {
    /// Print the game: its gravity and its entities with their components (`null` when the
    /// project has none).
    Show { path: PathBuf },
    /// Add an entity named `name`, drawing the library symbol `--symbol` when given. Prints the
    /// entity's ID.
    AddEntity {
        path: PathBuf,
        name: String,
        #[arg(long)]
        symbol: Option<Uuid>,
    },
    /// Set a component of an entity: its kind (transform, sprite, body, control, camera, ui,
    /// audio, particle or script) and `name=value` fields. Every field is named; a misspelt one
    /// is an error.
    SetComponent {
        path: PathBuf,
        #[arg(long)]
        entity: u32,
        kind: String,
        #[arg(num_args = 0..)]
        fields: Vec<String>,
    },
    /// Remove an entity's component of `kind`.
    RemoveComponent {
        path: PathBuf,
        #[arg(long)]
        entity: u32,
        kind: String,
    },
    /// Remove an entity and its components.
    RemoveEntity {
        path: PathBuf,
        #[arg(long)]
        entity: u32,
    },
    /// Set the gravity, in pixels per second squared.
    Gravity {
        path: PathBuf,
        #[arg(long, allow_hyphen_values = true)]
        x: f64,
        #[arg(long, allow_hyphen_values = true)]
        y: f64,
    },
    /// Step the game `--frames` frames at `--fps` under `--script`, and print its events and
    /// digest. The script is `;`-separated frames of `,`-separated keys (`right;right,space;`).
    /// The project is not changed.
    Run {
        path: PathBuf,
        #[arg(long, default_value_t = 60.0, allow_hyphen_values = true)]
        fps: f64,
        #[arg(long)]
        frames: u32,
        #[arg(long)]
        script: Option<PathBuf>,
    },
    /// Export the game into `--dir` as `<name>.json` (the description), `<name>.png` (the scene's
    /// frames `--start` to `--end` as a sprite sheet) and `<name>.sheet.json` (its atlas).
    /// Nothing is overwritten.
    Export {
        path: PathBuf,
        #[arg(long)]
        dir: PathBuf,
        #[arg(long, default_value = "game")]
        name: String,
        #[arg(long, default_value_t = 0)]
        start: u32,
        #[arg(long)]
        end: Option<u32>,
        #[arg(long)]
        columns: Option<u32>,
    },
}

pub fn run(op: GameCommand) -> Result<Value> {
    match op {
        GameCommand::Show { path } => Ok(model::describe(open_session(&path)?.document())),
        GameCommand::AddEntity { path, name, symbol } => {
            let mut session = open_session(&path)?;
            let symbol = symbol.map(SymbolId::from_uuid);
            let (id, commands) = model::add_entity(session.document(), &name, symbol)?;
            session.apply(commands)?;
            commit_session(&session, &path)?;
            Ok(json!({ "entity": id }))
        }
        GameCommand::SetComponent {
            path,
            entity,
            kind,
            fields,
        } => {
            let entity = entity_id(entity)?;
            let fields = parse_fields(&fields)?;
            let mut session = open_session(&path)?;
            session.apply(model::set_component(entity, &kind, &fields)?)?;
            commit_session(&session, &path)?;
            Ok(json!({ "entity": entity, "kind": kind }))
        }
        GameCommand::RemoveComponent { path, entity, kind } => {
            let entity = entity_id(entity)?;
            let mut session = open_session(&path)?;
            session.apply(model::remove_component(entity, &kind)?)?;
            commit_session(&session, &path)?;
            Ok(json!({ "entity": entity, "removed": kind }))
        }
        GameCommand::RemoveEntity { path, entity } => {
            let entity = entity_id(entity)?;
            let mut session = open_session(&path)?;
            session.apply(model::remove_entity(entity))?;
            commit_session(&session, &path)?;
            Ok(json!({ "removed": entity }))
        }
        GameCommand::Gravity { path, x, y } => {
            let mut session = open_session(&path)?;
            session.apply(model::set_gravity(x, y))?;
            commit_session(&session, &path)?;
            Ok(json!({ "gravity": [x, y] }))
        }
        GameCommand::Run {
            path,
            fps,
            frames,
            script,
        } => {
            let script = script.map(|file| read_script(&file)).transpose()?;
            let session = open_session(&path)?;
            let playback = model::run(session.document(), fps, frames, script.as_deref())?;
            Ok(model::playback_json(&playback))
        }
        GameCommand::Export {
            path,
            dir,
            name,
            start,
            end,
            columns,
        } => export(&path, &dir, &name, start, end, columns),
    }
}

fn entity_id(raw: u32) -> Result<EntityId> {
    EntityId::new(raw).with_context(|| format!("{raw} is not an entity ID"))
}

fn read_script(file: &Path) -> Result<String> {
    let size = std::fs::metadata(file)
        .with_context(|| format!("cannot read {}", file.display()))?
        .len();
    if size > MAX_SCRIPT_BYTES {
        bail!("{} is larger than {MAX_SCRIPT_BYTES} bytes", file.display());
    }
    std::fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))
}

fn export(
    path: &Path,
    dir: &Path,
    name: &str,
    start: u32,
    end: Option<u32>,
    columns: Option<u32>,
) -> Result<Value> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        bail!("{name:?} is not a file name");
    }
    let session = open_session(path)?;
    let export = flare_runtime::export_game(session.document(), start, end, columns, name)
        .map_err(anyhow::Error::msg)?;
    let json = dir.join(format!("{name}.json"));
    let png = dir.join(format!("{name}.png"));
    let atlas = dir.join(format!("{name}.sheet.json"));
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    let files: [(&PathBuf, &[u8]); 3] = [
        (&json, export.json.as_bytes()),
        (&png, export.sheet.png.as_slice()),
        (&atlas, export.sheet.json.as_bytes()),
    ];
    for (file, _) in &files {
        if file.exists() {
            bail!("{} already exists", file.display());
        }
    }
    for (file, bytes) in files {
        let mut staged = tempfile::NamedTempFile::new_in(dir)?;
        staged.write_all(bytes)?;
        staged.persist(file).map_err(|error| error.error)?;
    }
    Ok(json!({
        "json": json,
        "image": png,
        "atlas": atlas,
        "frames": export.sheet.frames,
        "width": export.sheet.width,
        "height": export.sheet.height,
    }))
}
