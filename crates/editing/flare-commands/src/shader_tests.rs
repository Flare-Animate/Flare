#![allow(clippy::expect_used)]

use super::*;
use flare_core::{DocumentId, FrameRate, SceneId, ShaderGraph, ShaderNode};
use flare_document::{Document, Scene, ShaderAsset, Timeline};

fn document() -> Document {
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "Scene", Timeline::default())],
        vec![],
    )
    .expect("document")
}

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
fn replaces_the_shader_library_as_one_validated_edit() {
    let original = document();
    let updated = apply_all(&original, [Command::SetShaderAssets(vec![asset("Wavy")])])
        .expect("valid edit")
        .expect("changed");
    assert_eq!(updated.shader_assets()[0].name(), "Wavy");
    assert!(original.shader_assets().is_empty());

    let rejected = apply_all(
        &original,
        [Command::SetShaderAssets(vec![
            asset("Duplicate"),
            asset("Duplicate"),
        ])],
    );
    assert!(matches!(rejected, Err(EditError::Invalid(_))));
    assert!(original.shader_assets().is_empty());
}
