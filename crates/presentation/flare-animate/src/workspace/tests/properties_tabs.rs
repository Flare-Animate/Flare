//! Animate's tabs over Properties, and its slider rows for a tool's amounts.

use super::layer_names::{click, find, settle};
use super::*;
use egui::PointerButton;

fn shown(ws: &mut Workspace) -> (egui::Context, f64) {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(ws, &ctx, &mut time);
    (ctx, time)
}

#[test]
fn the_tabs_follow_the_tool_and_a_click_shows_another_but_not_a_dimmed_one() {
    let mut ws = workspace();
    ws.tool = Tool::Brush;
    let (ctx, mut time) = shown(&mut ws);
    assert_eq!(ws.props_context(), PropsContext::Tool, "the brush in hand");
    time += 0.1;
    let doc = find(&mut ws, &ctx, time, vec![], "Doc").expect("the Doc tab is drawn");
    click(&mut ws, &ctx, &mut time, doc, PointerButton::Primary);
    assert_eq!(ws.props_context(), PropsContext::Document);
    time += 0.1;
    let object = find(&mut ws, &ctx, time, vec![], "Object").expect("the Object tab");
    click(&mut ws, &ctx, &mut time, object, PointerButton::Primary);
    assert_eq!(
        ws.props_context(),
        PropsContext::Document,
        "Object is dimmed with nothing selected"
    );
    ws.tool = Tool::Rectangle;
    assert_eq!(
        ws.props_context(),
        PropsContext::Tool,
        "another tool takes the panel back to it"
    );
}

#[test]
fn the_brush_size_is_a_slider_row_with_its_value_beside_its_name() {
    let mut ws = workspace();
    ws.tool = Tool::Brush;
    ws.brush_size = 12.0;
    let (ctx, mut time) = shown(&mut ws);
    time += 0.1;
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    let texts: Vec<(String, Pos2)> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some((
                t.galley.text().to_owned(),
                t.visual_bounding_rect().center(),
            )),
            _ => None,
        })
        .collect();
    let at = |wanted: &str| {
        texts
            .iter()
            .find(|(text, _)| text == wanted)
            .map(|(_, p)| *p)
            .unwrap_or_else(|| {
                panic!(
                    "{wanted:?} is not drawn among {:?}",
                    texts.iter().map(|t| &t.0).collect::<Vec<_>>()
                )
            })
    };
    // The number and its unit are drawn as two pieces of text.
    let (name, value) = (at("Size"), at("12"));
    assert!((name.y - value.y).abs() < 4.0, "on one line");
    assert!(value.x > name.x + 100.0, "the value at the right");
}
