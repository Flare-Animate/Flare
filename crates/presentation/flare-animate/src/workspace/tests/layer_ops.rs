//! Show All, Lock Others, Hide Others and Duplicate Layer: in the layer menu on a right click,
//! each one undo step.

use super::layer_names::{click, find, settle};
use super::*;
use egui::PointerButton;

fn layers(ws: &Workspace) -> Vec<(String, bool, bool)> {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| (l.name().to_owned(), l.is_visible(), l.is_locked()))
        .collect()
}

/// Three layers, Layer 1 at the top, with the window settled.
fn three() -> (egui::Context, Workspace, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws);
    ws.add_layer();
    ws.add_layer();
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    (ctx, ws, time)
}

#[test]
fn hide_others_then_show_all_from_the_layer_menu() {
    let (ctx, mut ws, mut time) = three();
    let mut at = None;
    for _ in 0..3 {
        time += 0.1;
        at = find(&mut ws, &ctx, time, vec![], "Layer 1").or(at);
    }
    let at = at.expect("Layer 1 is drawn");
    click(&mut ws, &ctx, &mut time, at, PointerButton::Secondary);
    let mut item = None;
    for _ in 0..4 {
        time += 0.1;
        item = find(&mut ws, &ctx, time, vec![], "Hide Others").or(item);
    }
    for wanted in ["Show All", "Lock Others", "Duplicate Layer"] {
        time += 0.1;
        assert!(
            find(&mut ws, &ctx, time, vec![], wanted).is_some(),
            "{wanted} is in the menu"
        );
    }
    click(
        &mut ws,
        &ctx,
        &mut time,
        item.expect("the menu offers Hide Others"),
        PointerButton::Primary,
    );
    settle(&mut ws, &ctx, &mut time);
    let shown: Vec<(String, bool)> = layers(&ws).into_iter().map(|(n, v, _)| (n, v)).collect();
    assert_eq!(
        shown,
        vec![
            ("Layer 1".to_owned(), true),
            ("Layer 2".to_owned(), false),
            ("Layer 3".to_owned(), false),
        ]
    );
    ws.show_all_layers();
    assert!(layers(&ws).iter().all(|(_, v, _)| *v));
    ws.undo();
    ws.undo();
    assert!(layers(&ws).iter().all(|(_, v, _)| *v), "each one undo step");
}

#[test]
fn lock_others_and_duplicate_layer_act_on_the_layer_picked() {
    let (_, mut ws, _) = three();
    let ids: Vec<LayerId> = ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| l.id())
        .collect();
    ws.lock_other_layers(ids[1]);
    let locked: Vec<bool> = layers(&ws).into_iter().map(|(_, _, l)| l).collect();
    assert_eq!(locked, vec![true, false, true]);
    let undo_steps = ws.active().undo_labels().len();
    ws.lock_other_layers(ids[1]);
    assert_eq!(
        ws.active().undo_labels().len(),
        undo_steps,
        "nothing to change is not an undo step"
    );
    ws.duplicate_layer(ids[2]);
    let names: Vec<String> = layers(&ws).into_iter().map(|(n, _, _)| n).collect();
    assert_eq!(names, vec!["Layer 1", "Layer 2", "Layer 3 copy", "Layer 3"]);
    assert_eq!(
        ws.active_layer_id(),
        Some(ws.document().scenes()[0].timeline().layers()[2].id()),
        "the copy is the active layer"
    );
}

#[test]
fn convert_layers_to_symbol_puts_one_instance_layer_where_the_layer_was() {
    let (ctx, mut ws, mut time) = three();
    let mut at = None;
    for _ in 0..3 {
        time += 0.1;
        at = find(&mut ws, &ctx, time, vec![], "Layer 1").or(at);
    }
    click(
        &mut ws,
        &ctx,
        &mut time,
        at.expect("Layer 1 is drawn"),
        PointerButton::Secondary,
    );
    let mut item = None;
    for _ in 0..4 {
        time += 0.1;
        item = find(&mut ws, &ctx, time, vec![], "Convert Layers to Symbol").or(item);
    }
    click(
        &mut ws,
        &ctx,
        &mut time,
        item.expect("the menu offers Convert Layers to Symbol"),
        PointerButton::Primary,
    );
    settle(&mut ws, &ctx, &mut time);
    let names: Vec<String> = layers(&ws).into_iter().map(|(n, _, _)| n).collect();
    assert_eq!(
        names,
        vec!["Layer 1", "Layer 2", "Layer 3"],
        "named after it"
    );
    let symbol = &ws.document().symbols()[0];
    assert_eq!(symbol.name(), "Layer 1");
    assert_eq!(symbol.kind(), flare_document::SymbolKind::Graphic);
    let new = &ws.document().scenes()[0].timeline().layers()[0];
    assert_eq!(ws.active_layer_id(), Some(new.id()));
    assert!(matches!(
        new.spans()[0].content(),
        flare_document::Content::Instance(_)
    ));
    ws.undo();
    assert!(ws.document().symbols().is_empty(), "one undo step");
    // A layer with nothing on it says why it is not converted.
    let empty = ws.document().scenes()[0].timeline().layers()[2].id();
    ws.convert_layer_to_symbol(empty);
    assert!(
        ws.status().is_some_and(|s| s.contains("no frames")),
        "{:?}",
        ws.status()
    );
}
