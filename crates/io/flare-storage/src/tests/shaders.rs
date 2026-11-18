use super::*;
use crate::records::SHADER_SCHEMA_VERSION;
use flare_core::{ShaderGraph, ShaderNode};
use flare_document::ShaderAsset;

fn shader_asset(name: &str) -> ShaderAsset {
    let graph = ShaderGraph::new(
        vec![
            ShaderNode::Constant(0.1),
            ShaderNode::Constant(0.2),
            ShaderNode::Constant(0.3),
            ShaderNode::Constant(1.0),
            ShaderNode::Color([0, 1, 2, 3]),
        ],
        4,
    )
    .expect("graph");
    ShaderAsset::new(name, graph).expect("asset")
}

#[test]
fn shader_graphs_survive_save_and_open_and_select_schema_five() {
    let document = sample()
        .with_shader_assets(vec![shader_asset("Wavy")])
        .expect("shader library");
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("shader.flare");
    save(&document, &path).expect("save");
    assert_eq!(load(&path).expect("load"), document);
    let parts = to_parts(&document, &mut NativeZstd::new().expect("zstd")).expect("parts");
    assert_eq!(parts.schema_version(), SHADER_SCHEMA_VERSION);
    let back = from_parts(parts, &mut NativeZstd::new().expect("zstd")).expect("load parts");
    assert_eq!(back, document);
}

#[test]
fn invalid_shader_graph_records_are_refused() {
    let wire = wire::ShaderGraphWire {
        name: "Broken".to_owned(),
        nodes: vec![ShaderNode::Sin(0)],
        output: 0,
    };
    assert!(matches!(wire.build(), Err(StorageError::Corrupt(_))));
}
