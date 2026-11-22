//! `flare import-into`: File > Import > Import to Stage and Import to Library on the command line.
//! A Flash movie (or another file Flare imports) is brought into an existing project by
//! `flare_editor::import_into`, the same edit the desktop app and the browser make.

use anyhow::{Context, Result};
use clap::Args;
use flare_editor::import_into::{Destination, import_into as bring_in};
use serde_json::{Value, json};
use std::path::PathBuf;

use super::{commit, open};

/// What `flare import-into` is asked.
#[derive(Args)]
pub struct ImportIntoArgs {
    /// The project to bring the file into; it is written back in place.
    path: PathBuf,
    /// The movie or document to bring in (`.swf`, `.fla`, `.xfl` and the rest `flare import` reads).
    source: PathBuf,
    /// Only into the library, as Import to Library; otherwise also onto a new top layer of the
    /// first scene, as Import to Stage.
    #[arg(long)]
    library: bool,
    /// The frame the movie starts on, with Import to Stage (zero based).
    #[arg(long, default_value_t = 0)]
    frame: u32,
}

pub fn import_into(args: ImportIntoArgs) -> Result<Value> {
    let imported = flare_import::import_path(&args.source)
        .with_context(|| format!("cannot import {}", args.source.display()))?;
    let mut session = open(&args.path)?;
    let destination = if args.library {
        Destination::Library
    } else {
        let scene = session
            .document()
            .scenes()
            .first()
            .map(|scene| scene.id())
            .context("the project has no scene to place the movie in")?;
        Destination::Stage {
            scene,
            frame: args.frame,
        }
    };
    let name = args.source.file_stem().map_or_else(
        || "Imported".to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let done = bring_in(session.document(), &imported.document, &name, destination)
        .with_context(|| format!("cannot import {}", args.source.display()))?;
    session.apply(done.commands)?;
    commit(&session, &args.path)?;
    let symbol = session
        .document()
        .symbols()
        .iter()
        .find(|s| s.id() == done.symbol)
        .map(|s| s.name().to_owned());
    let notes: Vec<String> = imported
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .chain(done.notes.iter().map(ToString::to_string))
        .collect();
    Ok(json!({
        "symbol": done.symbol.as_uuid(),
        "name": symbol,
        "layer": done.layer.map(|l| l.as_uuid().to_string()),
        "symbols": session.document().symbols().len(),
        "notes": notes,
    }))
}
