#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, ShaderNode};
use flare_history::EditorSession;

use super::*;

fn graph(red: f32) -> ShaderGraph {
    ShaderGraph::new(
        vec![
            ShaderNode::Constant(red),
            ShaderNode::Constant(0.0),
            ShaderNode::Constant(1.0),
            ShaderNode::Color([0, 1, 1, 2]),
        ],
        3,
    )
    .expect("graph")
}

fn session() -> EditorSession {
    let rate = FrameRate::new(24, 1).expect("rate");
    EditorSession::new(Document::new(DocumentId::new(), rate, vec![], vec![]).expect("doc"))
}

#[test]
fn a_graph_is_added_then_replaced_by_name_and_one_undo_takes_each_back() {
    let mut session = session();
    let add = set_graph(session.document(), " glow ", graph(0.1)).expect("add");
    session.apply([add]).expect("apply");
    assert_eq!(session.document().shader_assets().len(), 1);
    assert!(
        find(session.document(), "glow").is_some(),
        "found by trimmed name"
    );
    let replace = set_graph(session.document(), "glow", graph(0.9)).expect("replace");
    session.apply([replace]).expect("apply");
    assert_eq!(
        session.document().shader_assets().len(),
        1,
        "same name, one graph"
    );
    assert_eq!(
        find(session.document(), "glow").expect("glow").graph(),
        &graph(0.9)
    );
    session.undo();
    assert_eq!(
        find(session.document(), "glow").expect("glow").graph(),
        &graph(0.1)
    );
}

#[test]
fn removing_takes_the_graph_out_and_refuses_one_that_is_not_there_or_a_blank_name() {
    let mut session = session();
    let add = set_graph(session.document(), "glow", graph(0.1)).expect("add");
    session.apply([add]).expect("apply");
    let remove = remove_graph(session.document(), "glow").expect("remove");
    session.apply([remove]).expect("apply");
    assert!(session.document().shader_assets().is_empty());
    assert_eq!(
        remove_graph(session.document(), "glow"),
        Err(ShaderLibraryError::Missing("glow".into()))
    );
    assert!(matches!(
        set_graph(session.document(), "  ", graph(0.1)),
        Err(ShaderLibraryError::Asset(_))
    ));
}
