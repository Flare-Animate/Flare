//! The Shaders panel: graphs imported from a file become one undo step each, exported as the
//! file they came from, removed again, and a file that is no graph says why and changes nothing.

use super::*;

const GRAPH: &[u8] = br#"{"version":1,"nodes":[{"op":"constant","args":0.5},{"op":"constant","args":0.0},{"op":"constant","args":1.0},{"op":"color","args":[0,1,2,2]}],"output":3}"#;

fn names(ws: &Workspace) -> Vec<String> {
    ws.document()
        .shader_assets()
        .iter()
        .map(|a| a.name().to_owned())
        .collect()
}

#[test]
fn a_graph_is_imported_chosen_exported_and_removed_each_in_one_undo_step() {
    let mut ws = workspace();
    ws.import_shader_graph(" Glow ", GRAPH);
    assert_eq!(names(&ws), vec!["Glow"]);
    assert_eq!(ws.shaders.selected.as_deref(), Some("Glow"));
    let (name, bytes) = ws.shader_export().expect("export");
    assert_eq!(name, "Glow");
    assert_eq!(
        flare_import::import_shader_graph(&bytes)
            .expect("round trip")
            .nodes()
            .len(),
        4
    );
    ws.undo();
    assert!(names(&ws).is_empty(), "one undo takes the import back");
    ws.redo();
    ws.shaders.selected = Some("Glow".into());
    ws.remove_selected_shader();
    assert!(names(&ws).is_empty());
    assert!(ws.shaders.selected.is_none());
    ws.undo();
    assert_eq!(names(&ws), vec!["Glow"], "and one undo brings it back");
}

#[test]
fn a_file_that_is_no_graph_is_refused_with_its_name_and_changes_nothing() {
    let mut ws = workspace();
    let revision = ws.active().revision();
    ws.import_shader_graph("Broken", br#"{"version":2,"nodes":[],"output":0}"#);
    assert!(names(&ws).is_empty());
    assert_eq!(ws.active().revision(), revision);
    assert!(ws.status().is_some_and(|s| s.contains("Broken")));
    assert!(
        ws.shader_export().is_none(),
        "nothing chosen, nothing to write"
    );
}

#[test]
fn the_panel_draws_empty_and_with_graphs_and_the_menu_opens_it() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.perform(MenuAction::ShowShaders);
    run(&mut ws, &ctx, 0.0);
    ws.import_shader_graph("Glow", GRAPH);
    ws.perform(MenuAction::ShowShaders);
    run(&mut ws, &ctx, 1.0);
    run(&mut ws, &ctx, 2.0);
    assert_eq!(names(&ws), vec!["Glow"]);
}
