#![allow(clippy::expect_used)]

use super::*;
use crate::{Diagnostic, Document, Scene, Timeline};
use flare_core::{ShaderGraph, ShaderNode};

fn asset(name: &str) -> ShaderAsset {
    let graph = ShaderGraph::new(
        vec![
            ShaderNode::Constant(0.0),
            ShaderNode::Constant(0.0),
            ShaderNode::Constant(0.0),
            ShaderNode::Constant(1.0),
            ShaderNode::Color([0, 1, 2, 3]),
        ],
        4,
    )
    .expect("graph");
    ShaderAsset::new(name, graph).expect("asset")
}

#[test]
fn shader_asset_names_are_trimmed_and_bounded() {
    assert_eq!(asset("  Snow  ").name(), "Snow");
    assert_eq!(
        ShaderAsset::new("   ", asset("valid").graph().clone()),
        Err(ShaderAssetError::BadName)
    );
    assert_eq!(
        ShaderAsset::new(
            "x".repeat(MAX_SHADER_NAME_CHARS + 1),
            asset("valid").graph().clone()
        ),
        Err(ShaderAssetError::BadName)
    );
}

#[test]
fn document_rejects_duplicate_shader_asset_names() {
    let document = Document::new(
        flare_core::DocumentId::new(),
        flare_core::FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(
            flare_core::SceneId::new(),
            "Scene",
            Timeline::default(),
        )],
        vec![],
    )
    .expect("document");
    let result = document.with_shader_assets(vec![asset("Snow"), asset("Snow")]);
    assert_eq!(
        result,
        Err(vec![Diagnostic::DuplicateShaderAsset("Snow".to_owned())])
    );
}
