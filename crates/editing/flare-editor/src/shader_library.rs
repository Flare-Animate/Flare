//! The document's named shader graphs, edited: the one place that decides how a graph is added,
//! replaced or taken out, so the Shaders panel, `flare shader` and the MCP server do the same
//! thing and every change is one undo step.

use flare_commands::Command;
use flare_core::ShaderGraph;
use flare_document::{Document, ShaderAsset, ShaderAssetError};

/// Why a graph could not be added or removed.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ShaderLibraryError {
    #[error(transparent)]
    Asset(#[from] ShaderAssetError),
    #[error("shader graph {0:?} is not in this project")]
    Missing(String),
}

/// The graph called `name`, as the document has it.
pub fn find<'a>(document: &'a Document, name: &str) -> Option<&'a ShaderAsset> {
    document
        .shader_assets()
        .iter()
        .find(|asset| asset.name() == name.trim())
}

/// The command that adds `graph` as `name`, or replaces the graph of that name (a name is
/// matched after trimming). The library's size is checked when the command is applied.
pub fn set_graph(
    document: &Document,
    name: &str,
    graph: ShaderGraph,
) -> Result<Command, ShaderLibraryError> {
    let asset = ShaderAsset::new(name, graph)?;
    let mut assets = document.shader_assets().to_vec();
    assets.retain(|existing| existing.name() != asset.name());
    assets.push(asset);
    Ok(Command::SetShaderAssets(assets))
}

/// The command that takes the graph called `name` out of the library.
pub fn remove_graph(document: &Document, name: &str) -> Result<Command, ShaderLibraryError> {
    find(document, name).ok_or_else(|| ShaderLibraryError::Missing(name.to_owned()))?;
    let mut assets = document.shader_assets().to_vec();
    assets.retain(|existing| existing.name() != name.trim());
    Ok(Command::SetShaderAssets(assets))
}

#[cfg(test)]
#[path = "shader_library_tests.rs"]
mod tests;
