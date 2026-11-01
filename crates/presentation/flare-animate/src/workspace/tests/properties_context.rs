//! The Properties panel follows the selection, as Animate's does.

use super::*;
use flare_document::{Content, SymbolKind};

fn texts(ws: &mut Workspace) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut all = Vec::new();
    for step in 0..4 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
        all = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
    }
    all
}

fn shape_fill(ws: &Workspace) -> flare_core::Rgba8 {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .flat_map(|l| l.spans())
        .find_map(|s| match s.content() {
            Content::Shape(shape) => Some(shape.fill()),
            _ => None,
        })
        .expect("a shape")
}

#[test]
fn the_panel_is_about_what_is_selected_then_the_frames_then_the_tool_then_the_document() {
    let mut ws = workspace();
    ws.set_tool(Tool::Select);
    assert_eq!(
        ws.props_context(),
        PropsContext::Document,
        "nothing selected"
    );
    ws.set_tool(Tool::Rectangle);
    assert_eq!(
        ws.props_context(),
        PropsContext::Tool,
        "a drawing tool in hand"
    );
    ws.set_tool(Tool::Hand);
    assert_eq!(
        ws.props_context(),
        PropsContext::Document,
        "a tool with no settings"
    );

    draw_and_select(&mut ws);
    assert_eq!(
        ws.props_context(),
        PropsContext::Object,
        "something selected"
    );
    ws.clear_selection();
    ws.set_tool(Tool::Select);
    let layer = ws.active_layer_id().expect("layer");
    ws.select_frames(layer, 0, 3);
    assert_eq!(ws.props_context(), PropsContext::Frame, "frames selected");
}

#[test]
fn a_context_a_command_asked_for_lasts_until_the_selection_or_the_tool_changes() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    assert_eq!(ws.props_context(), PropsContext::Object);
    ws.perform(MenuAction::DocumentSettings);
    assert_eq!(ws.props_context(), PropsContext::Document);
    assert_eq!(ws.props_context(), PropsContext::Document, "and it stays");
    ws.set_tool(Tool::Lasso);
    assert_eq!(
        ws.props_context(),
        PropsContext::Object,
        "a new tool hands the panel back to the selection"
    );
}

#[test]
fn a_selected_shape_shows_its_paint_and_a_new_fill_is_one_undo_step() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    let words = texts(&mut ws);
    for wanted in [
        "Shape",
        "Position and size",
        "Fill and stroke",
        "Outline",
        "Width",
    ] {
        assert!(words.iter().any(|w| w == wanted), "{wanted} in {words:?}");
    }
    let before = shape_fill(&ws);
    let at = ws.selected.expect("selected");
    // What the panel holds while a color is being picked, and let go of.
    ws.shape_paint_edit = Some(ShapePaintEdit {
        at,
        fill: Color32::from_rgb(1, 2, 3),
        fill_changed: true,
        outline: false,
        color: Color32::BLACK,
        width: 1.0,
        cap: flare_document::Cap::Round,
        join: flare_document::Join::Round,
        stroke_changed: false,
    });
    texts(&mut ws);
    assert_eq!(shape_fill(&ws), flare_core::Rgba8::opaque(1, 2, 3));
    assert!(ws.shape_paint_edit.is_none(), "applied and let go");
    ws.undo();
    assert_eq!(shape_fill(&ws), before, "one undo step");
}

#[test]
fn an_outline_the_panel_turns_on_gives_the_shape_a_stroke_and_off_takes_it_away() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    let at = ws.selected.expect("selected");
    let stroke_of = |ws: &Workspace| {
        ws.document().scenes()[0]
            .timeline()
            .layers()
            .iter()
            .flat_map(|l| l.spans())
            .find_map(|s| match s.content() {
                Content::Shape(shape) => Some(shape.stroke()),
                _ => None,
            })
            .expect("a shape")
    };
    let edit = |outline| ShapePaintEdit {
        at,
        fill: Color32::RED,
        fill_changed: false,
        outline,
        color: Color32::from_rgb(9, 9, 200),
        width: 6.0,
        cap: flare_document::Cap::Square,
        join: flare_document::Join::Miter,
        stroke_changed: true,
    };
    ws.shape_paint_edit = Some(edit(true));
    texts(&mut ws);
    let stroke = stroke_of(&ws).expect("a stroke now");
    assert_eq!(
        (stroke.width(), stroke.cap(), stroke.join()),
        (
            6.0,
            flare_document::Cap::Square,
            flare_document::Join::Miter
        )
    );
    ws.shape_paint_edit = Some(edit(false));
    texts(&mut ws);
    assert!(stroke_of(&ws).is_none(), "and none again");
}

#[test]
fn a_selected_instance_shows_its_symbol_and_type_with_the_way_to_edit_it() {
    let mut ws = workspace();
    ws.library.new_kind = SymbolKind::MovieClip;
    draw_and_select(&mut ws);
    ws.convert_selection_to_symbol();
    let words = texts(&mut ws);
    for wanted in [
        "Symbol instance",
        "Instance",
        "Movie Clip",
        "Edit Symbol",
        "Position and size",
        "Color Effect",
    ] {
        assert!(words.iter().any(|w| w == wanted), "{wanted} in {words:?}");
    }
    assert!(
        !words.iter().any(|w| w == "Fill and stroke"),
        "an instance has no paint of its own"
    );
}

#[test]
fn several_things_selected_show_only_their_shared_position_and_size() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    ws.select_all();
    ws.commit_rectangle(Pos2::new(200.0, 200.0), Pos2::new(260.0, 240.0));
    ws.set_tool(Tool::Select);
    ws.select_all();
    assert_eq!(ws.selection_all().len(), 2, "two shapes selected");
    let words = texts(&mut ws);
    assert!(words.iter().any(|w| w == "Selection (2)"), "{words:?}");
    assert!(words.iter().any(|w| w == "Position and size"));
    assert!(!words.iter().any(|w| w == "Fill and stroke"));
}
