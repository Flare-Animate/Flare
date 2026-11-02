use super::*;

/// A drawing on layer 1 copied, with an empty second layer made and active.
fn copied_with_a_second_layer() -> Workspace {
    let mut ws = workspace();
    ws.commit_rectangle(Pos2::new(10.0, 10.0), Pos2::new(60.0, 60.0));
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.perform(MenuAction::Copy);
    ws.add_layer();
    ws
}

fn elements_on(ws: &Workspace, layer: flare_core::LayerId) -> usize {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.id() == layer)
        .map_or(0, |l| {
            l.spans()
                .iter()
                .map(flare_document::FrameSpan::element_count)
                .sum()
        })
}

#[test]
fn a_paste_goes_into_the_layer_that_is_active_not_the_one_it_came_from() {
    let mut ws = copied_with_a_second_layer();
    let active = ws.current_layer.expect("the new layer is active");
    let first = ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(flare_document::Layer::id)
        .find(|id| *id != active)
        .expect("the layer the drawing is on");
    assert_ne!(active, first);
    let before = layer_count(&ws);
    ws.perform(MenuAction::Paste);
    assert_eq!(layer_count(&ws), before, "no layer is made");
    assert_eq!(elements_on(&ws, active), 1, "the active layer has it");
    assert_eq!(
        elements_on(&ws, first),
        1,
        "the layer it came from keeps its one"
    );
    ws.undo();
    assert_eq!(elements_on(&ws, active), 0, "one undo step takes it away");
    assert_eq!(elements_on(&ws, first), 1);
}

#[test]
fn pasting_into_a_locked_layer_says_so_and_changes_nothing() {
    let mut ws = copied_with_a_second_layer();
    let layer = ws.current_layer.expect("the active layer");
    ws.toggle_layer_locked(layer);
    ws.perform(MenuAction::Paste);
    assert_eq!(elements_on(&ws, layer), 0);
    assert!(
        ws.status()
            .is_some_and(|m| m.contains("Cannot paste") && m.contains("locked")),
        "{:?}",
        ws.status()
    );
}
