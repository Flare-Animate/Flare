//! Helpers for the tests that type text and read back the shapes drawn.

use super::*;

/// A workspace whose text jobs use the Hack font by name, whatever the machine has.
pub(super) fn text_workspace() -> Workspace {
    let mut ws = workspace();
    ws.text.family = "Hack".into();
    ws.text.size = 40.0;
    ws.set_tool(Tool::Text);
    ws
}

pub(super) fn type_text(ws: &mut Workspace, at: Pos2, text: &str) {
    ws.text.edit = Some(PendingText {
        at,
        buffer: text.into(),
        focused: true,
        editing: None,
    });
    ws.place_text();
}

pub(super) fn drawn_shape(ws: &Workspace, layer: usize) -> Shape {
    match drawings(ws).get(layer - 1) {
        Some(Content::Shape(shape)) => shape.clone(),
        _ => panic!("shape expected"),
    }
}
