//! Portable JSON interchange for Flare's bounded shader graph.

use flare_core::{MAX_SHADER_NODES, ShaderGraph, ShaderGraphError, ShaderNode};
use serde::{Deserialize, Serialize};

pub const MAX_SHADER_GRAPH_FILE_BYTES: usize = 1024 * 1024;
const SHADER_GRAPH_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum ShaderGraphImportError {
    #[error("shader graph JSON is larger than {MAX_SHADER_GRAPH_FILE_BYTES} bytes")]
    TooLarge,
    #[error("invalid shader graph JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported shader graph JSON version {0}")]
    UnsupportedVersion(u32),
    #[error(transparent)]
    Graph(#[from] ShaderGraphError),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShaderGraphFile {
    version: u32,
    nodes: Vec<ShaderNode>,
    output: usize,
}

/// Reads and validates the portable JSON form before returning a graph.
pub fn import_shader_graph(bytes: &[u8]) -> Result<ShaderGraph, ShaderGraphImportError> {
    if bytes.len() > MAX_SHADER_GRAPH_FILE_BYTES {
        return Err(ShaderGraphImportError::TooLarge);
    }
    let file: ShaderGraphFile = serde_json::from_slice(bytes)?;
    if file.version != SHADER_GRAPH_VERSION {
        return Err(ShaderGraphImportError::UnsupportedVersion(file.version));
    }
    if file.nodes.len() > MAX_SHADER_NODES {
        return Err(ShaderGraphError::TooManyNodes.into());
    }
    Ok(ShaderGraph::new(file.nodes, file.output)?)
}

/// Writes a graph in the versioned interchange form.
pub fn export_shader_graph(graph: &ShaderGraph) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec_pretty(&ShaderGraphFile {
        version: SHADER_GRAPH_VERSION,
        nodes: graph.nodes().to_vec(),
        output: graph.output(),
    })
}
