//! `flare symbol increment`, `bake` and `add-instance` (issue #11): the library edits of
//! `flare_tools::library`, parsed by `symbol.rs` and run and printed here.

use std::path::Path;

use anyhow::Result;
use flare_core::SymbolId;
use flare_tools::library;
use flare_tools::ops::SpanRef;
use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) fn increment(path: &Path, symbol: Uuid, from: u32, by: i64) -> Result<Value> {
    let mut session = crate::project::open_session(path)?;
    let commands =
        library::increment_instances(session.document(), SymbolId::from_uuid(symbol), from, by)?;
    let changes = commands.len();
    session.apply(commands)?;
    crate::project::commit_session(&session, path)?;
    Ok(json!({ "symbol": symbol, "from": from, "by": by, "changes": changes }))
}

pub(crate) fn bake(path: &Path, at: u32, layer: Option<Uuid>, element: usize) -> Result<Value> {
    let mut session = crate::project::open_session(path)?;
    let (scene, layer) = crate::project::first_scene_layer(&session, layer)?;
    let at = SpanRef {
        layer,
        start: at,
        element,
    };
    let commands = library::bake_transform(session.document(), scene, at)?;
    let changes = commands.len();
    session.apply(commands)?;
    crate::project::commit_session(&session, path)?;
    Ok(json!({ "changes": changes }))
}

pub(crate) fn add_instance(
    path: &Path,
    symbol: Uuid,
    at: u32,
    layer: Option<Uuid>,
    element: usize,
) -> Result<Value> {
    let mut session = crate::project::open_session(path)?;
    let (scene, layer) = crate::project::first_scene_layer(&session, layer)?;
    let at = SpanRef {
        layer,
        start: at,
        element,
    };
    let commands = library::add_instance_to_symbol(
        session.document(),
        SymbolId::from_uuid(symbol),
        scene,
        at,
    )?;
    session.apply(commands)?;
    crate::project::commit_session(&session, path)?;
    Ok(json!({ "symbol": symbol, "added": true }))
}
