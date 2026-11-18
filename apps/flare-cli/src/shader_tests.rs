#![allow(clippy::expect_used)]

use super::*;
use std::io::Write;

#[test]
fn imports_validates_and_lists_a_shader_graph() {
    let dir = tempfile::tempdir().expect("directory");
    let project_path = dir.path().join("shaders.flare");
    project::new(&project_path, 24, 1, flare_document::Stage::default()).expect("project");
    let graph_path = dir.path().join("flat.json");
    let mut file = File::create(&graph_path).expect("graph file");
    file.write_all(
        br#"{"version":1,"nodes":[{"op":"constant","args":0.0},{"op":"constant","args":0.0},{"op":"constant","args":0.0},{"op":"constant","args":1.0},{"op":"color","args":[0,1,2,3]}],"output":4}"#,
    )
    .expect("write graph");

    import(&project_path, " Flat ", &graph_path).expect("import");
    let result = run(ShaderCommand::List { path: project_path }).expect("list");
    assert_eq!(result["shader_graphs"][0]["name"], "Flat");
    assert_eq!(result["shader_graphs"][0]["nodes"], 5);
    let exported_path = dir.path().join("exported.json");
    run(ShaderCommand::Export {
        path: dir.path().join("shaders.flare"),
        name: "Flat".to_owned(),
        out: exported_path.clone(),
    })
    .expect("export graph");
    let exported = flare_import::import_shader_graph(
        &std::fs::read(exported_path).expect("read exported graph"),
    )
    .expect("round-trip graph");
    assert_eq!(exported.nodes().len(), 5);
}

#[test]
fn refuses_unknown_json_versions_and_invalid_graph_links() {
    let dir = tempfile::tempdir().expect("directory");
    let project_path = dir.path().join("shaders.flare");
    project::new(&project_path, 24, 1, flare_document::Stage::default()).expect("project");
    let graph_path = dir.path().join("bad.json");
    std::fs::write(&graph_path, br#"{"version":2,"nodes":[],"output":0}"#).expect("write graph");
    assert!(import(&project_path, "Bad", &graph_path).is_err());
    std::fs::write(
        &graph_path,
        br#"{"version":1,"nodes":[{"op":"sin","args":0}],"output":0}"#,
    )
    .expect("write graph");
    assert!(import(&project_path, "Bad", &graph_path).is_err());
}

#[test]
fn renders_a_project_frame_through_the_named_shader_stack() {
    let dir = tempfile::tempdir().expect("directory");
    let project_path = dir.path().join("shaders.flare");
    let stage =
        flare_document::Stage::new(1, 1, flare_core::Rgba8::WHITE).expect("one-pixel stage");
    project::new(&project_path, 24, 1, stage).expect("project");
    let graph_path = dir.path().join("flat.json");
    std::fs::write(
        &graph_path,
        br#"{"version":1,"nodes":[{"op":"constant","args":1.0},{"op":"constant","args":0.0},{"op":"constant","args":0.0},{"op":"constant","args":1.0},{"op":"color","args":[0,1,2,3]}],"output":4}"#,
    )
    .expect("write graph");
    import(&project_path, "Red", &graph_path).expect("import");
    let output_path = dir.path().join("render.png");

    let result = run(ShaderCommand::Render {
        path: project_path,
        out: output_path.clone(),
        shaders: vec!["Red".to_owned()],
        frame: 0,
        width: None,
        height: None,
        transparent: false,
    })
    .expect("render");

    assert_eq!(result["width"], 1);
    let bytes = std::fs::read(output_path).expect("read output");
    let still = flare_import::read_still("render.png", &bytes, 1).expect("decode rendered image");
    assert_eq!(still.rgba, vec![255, 0, 0, 255]);
}
