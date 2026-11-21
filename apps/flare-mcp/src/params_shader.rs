//! Parameters for shader graph library tools.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetShaderGraph {
    /// The asset's name; an asset with that name is replaced.
    pub name: String,
    /// Version 1 Flare shader graph JSON.
    pub graph_json: String,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetShaderGraph {
    /// Name of the saved graph.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveShaderGraph {
    /// Name of the saved graph to take out.
    pub name: String,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}
