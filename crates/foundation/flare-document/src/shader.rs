use flare_core::ShaderGraph;

pub const MAX_SHADER_ASSETS: usize = 64;
pub const MAX_SHADER_NAME_CHARS: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderAsset {
    name: String,
    graph: ShaderGraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShaderAssetError {
    #[error("shader name must contain 1 to {MAX_SHADER_NAME_CHARS} characters")]
    BadName,
}

impl ShaderAsset {
    pub fn new(name: impl Into<String>, graph: ShaderGraph) -> Result<Self, ShaderAssetError> {
        let name = name.into();
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_SHADER_NAME_CHARS {
            return Err(ShaderAssetError::BadName);
        }

        Ok(Self {
            name: name.to_owned(),
            graph,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn graph(&self) -> &ShaderGraph {
        &self.graph
    }
}
