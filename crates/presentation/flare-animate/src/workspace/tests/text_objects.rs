//! Text that stays text: edited on the stage after it is placed, restyled, and taken apart.

use super::*;

/// A workspace with "Hi" placed, selected.
fn with_text() -> Workspace {
    let mut ws = text_workspace();
    type_text(&mut ws, Pos2::new(100.0, 50.0), "Hi");
    let layer = ws.document().scenes()[0].timeline().layers()[1].id();
    ws.selected = Some(SpanRef { layer, start: 0 });
    ws
}

fn source(ws: &Workspace) -> flare_document::TextSource {
    selected_shape(ws).text().cloned().expect("text")
}

#[test]
fn placed_text_remembers_its_words_font_and_corner() {
    let ws = with_text();
    let text = source(&ws);
    assert_eq!(text.text(), "Hi");
    assert_eq!(text.family(), "Hack");
    assert_eq!(text.size(), 40.0);
    let corner = text.origin().as_vec();
    assert_eq!((corner.x, corner.y), (100.0, 50.0));
}

#[test]
fn clicking_text_with_the_text_tool_edits_it_and_finishing_replaces_it_in_one_undo_step() {
    let mut ws = with_text();
    let before = selected_shape(&ws);
    let layers = layer_count(&ws);
    ws.selected = None;
    // A click on the letters (inside the first one's body) opens the text, not a new box.
    let b = shape_bounds_of(&before);
    ws.text_click(Pos2::new(b.min.x + 4.0, b.center().y));
    let edit = ws.text.edit.as_ref().expect("the editor opened");
    assert_eq!(edit.buffer, "Hi");
    assert!(edit.editing.is_some());
    assert_eq!((ws.text.family.as_str(), ws.text.size), ("Hack", 40.0));

    ws.text.edit.as_mut().expect("edit").buffer = "Hello".into();
    ws.place_text();
    assert_eq!(layer_count(&ws), layers, "the same layer, not a new one");
    let after = selected_shape(&ws);
    assert_eq!(after.id(), before.id(), "the same object");
    assert_eq!(after.text().map(|t| t.text()), Some("Hello"));
    assert!(shape_bounds(&ws).width() > shape_bounds_of(&before).width());

    ws.undo();
    assert_eq!(selected_shape(&ws).text().map(|t| t.text()), Some("Hi"));
}

fn shape_bounds_of(shape: &Shape) -> Rect {
    let scene_bounds = flare_tools::ops::shape_bounds(shape);
    scene_bounds.expect("bounds")
}

#[test]
fn emptied_text_is_removed_and_a_click_elsewhere_starts_new_text() {
    let mut ws = with_text();
    ws.selected = None;
    ws.text_click(Pos2::new(110.0, 70.0));
    assert!(ws.text.edit.as_ref().is_some_and(|e| e.editing.is_some()));
    ws.text.edit.as_mut().expect("edit").buffer = "  ".into();
    ws.place_text();
    let spans: usize = ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| l.spans().len())
        .sum();
    assert_eq!(spans, 0, "the text went");
    ws.undo();
    assert!(
        ws.document().scenes()[0].timeline().layers()[1]
            .spans()
            .len()
            == 1
    );

    ws.text_click(Pos2::new(600.0, 400.0));
    let edit = ws.text.edit.as_ref().expect("new text");
    assert!(edit.editing.is_none() && edit.buffer.is_empty());
}

#[test]
fn double_clicking_text_with_the_selection_tool_edits_it() {
    let mut ws = with_text();
    ws.set_tool(Tool::Select);
    assert!(ws.edit_selected_text());
    assert_eq!(ws.tool(), Tool::Text);
    assert!(ws.text.edit.is_some());
    // Something that is not text is not edited.
    let mut plain = workspace();
    rect(&mut plain);
    assert!(!plain.edit_selected_text());
}

#[test]
fn changing_the_font_or_size_of_selected_text_sets_it_again_as_one_undo_step() {
    let mut ws = with_text();
    let before = selected_shape(&ws);
    let mut next = source(&ws);
    next = flare_document::TextSource::new(
        next.text(),
        next.family(),
        80.0,
        true,
        false,
        next.origin(),
    )
    .expect("source");
    ws.retext_selected(next);
    let after = selected_shape(&ws);
    let text = after.text().expect("still text");
    assert_eq!((text.size(), text.bold()), (80.0, true));
    assert!(shape_bounds(&ws).height() > shape_bounds_of(&before).height() * 1.5);
    ws.undo();
    assert_eq!(source(&ws).size(), 40.0);
}

#[test]
fn break_apart_makes_text_plain_outlines_in_place_and_undo_brings_the_text_back() {
    let mut ws = with_text();
    assert!(ws.is_enabled(MenuAction::BreakApart));
    let before = selected_shape(&ws);
    ws.perform(MenuAction::BreakApart);
    let apart = selected_shape(&ws);
    assert!(apart.text().is_none());
    assert_eq!(apart.path(), before.path());
    assert!(
        !ws.is_enabled(MenuAction::BreakApart),
        "nothing left to break"
    );
    ws.undo();
    assert!(selected_shape(&ws).text().is_some());
}

#[test]
fn moving_text_keeps_it_text_with_its_corner_moved() {
    let mut ws = with_text();
    let scene = ws.scene_id().expect("scene");
    let at = ws.selected.expect("selected");
    let commands =
        flare_tools::ops::translate(ws.document(), scene, at, glamx::DVec2::new(30.0, 10.0))
            .expect("move");
    ws.run_batch(commands, "move");
    let corner = source(&ws).origin().as_vec();
    assert_eq!((corner.x, corner.y), (130.0, 60.0));
}

#[test]
fn the_families_are_asked_for_once_and_listed() {
    let mut ws = workspace();
    assert!(ws.text.families.is_empty());
    ws.ask_font_families();
    ws.ask_font_families();
    assert!(
        ws.text.families.iter().any(|f| f == "Hack"),
        "{:?}",
        ws.text.families
    );
}

#[test]
fn the_editor_and_the_character_controls_draw() {
    let (ctx, mut ws) = (egui::Context::default(), with_text());
    flare_ui::icons::install(&ctx);
    // The text object's Properties.
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    // The editor open on the stage.
    ws.text_click(Pos2::new(5.0, 5.0));
    run(&mut ws, &ctx, 0.2);
    run(&mut ws, &ctx, 0.3);
    assert!(ws.text.edit.as_ref().is_some_and(|e| e.focused));
}

#[test]
fn a_press_on_the_stage_outside_the_editor_finishes_the_text_but_a_press_elsewhere_does_not() {
    let (ctx, mut ws) = (egui::Context::default(), text_workspace());
    flare_ui::icons::install(&ctx);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    ws.text_click(Pos2::new(5.0, 5.0));
    ws.text.edit.as_mut().expect("edit").buffer = "Yo".into();
    run(&mut ws, &ctx, 0.2);
    run(&mut ws, &ctx, 0.3);
    let press = |at: Pos2| {
        vec![
            Event::PointerMoved(at),
            Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]
    };
    // Off the stage (the timeline, the panels): the text goes on being typed.
    pass(&mut ws, &ctx, input(0.4, press(Pos2::new(640.0, 790.0))));
    assert!(ws.text.edit.is_some());
    let layers = layer_count(&ws);
    // On the stage, away from the editor.
    let away = ws.stage_panel.right_bottom() - egui::vec2(20.0, 20.0);
    pass(&mut ws, &ctx, input(0.5, press(away)));
    assert!(ws.text.edit.is_none(), "the text was finished");
    assert_eq!(layer_count(&ws), layers + 1, "and placed");
}

#[test]
fn text_is_picked_anywhere_in_its_box_not_only_on_the_ink_of_its_letters() {
    let mut ws = text_workspace();
    type_text(&mut ws, Pos2::new(100.0, 50.0), "I I");
    let layer = ws.document().scenes()[0].timeline().layers()[1].id();
    let at = SpanRef { layer, start: 0 };
    ws.selected = Some(at);
    let bounds = shape_bounds(&ws);
    ws.selected = None;

    // The middle of "I I" is the space between the letters: no ink there at all.
    let gap = bounds.center();
    assert!(ws.glyph_hit(gap).is_none(), "the point is off the ink");
    assert_eq!(ws.hit(gap), Some(at), "but it is on the text");
    ws.select_at(gap, false);
    assert_eq!(
        ws.selected,
        Some(at),
        "the Selection tool picks the text there"
    );

    // The Text tool edits it instead of starting new text on top of it.
    ws.selected = None;
    ws.text_click(gap);
    let edit = ws.text.edit.as_ref().expect("an editor");
    assert_eq!(edit.editing, Some(at));
    assert_eq!(edit.buffer, "I I");

    // Well away from the box is empty stage: new text.
    ws.text.edit = None;
    ws.text_click(Pos2::new(bounds.max.x + 40.0, bounds.center().y));
    assert!(ws.text.edit.as_ref().is_some_and(|e| e.editing.is_none()));
}

#[test]
fn a_double_click_on_text_with_the_selection_tool_opens_the_text_tool_on_it() {
    let (ctx, mut ws) = (egui::Context::default(), text_workspace());
    flare_ui::icons::install(&ctx);
    // "I I" over the middle of the stage (1920 by 1080), with the Selection tool in hand.
    type_text(&mut ws, Pos2::new(930.0, 520.0), "I I");
    ws.set_tool(Tool::Select);
    ws.selected = None;
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let at = ws.stage_panel.center();
    let button = |pressed: bool| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    // Two clicks close together in time, as a double click is.
    pass(
        &mut ws,
        &ctx,
        input(0.20, vec![Event::PointerMoved(at), button(true)]),
    );
    pass(&mut ws, &ctx, input(0.25, vec![button(false)]));
    assert_eq!(ws.tool(), Tool::Select, "one click only selects");
    assert!(ws.selected.is_some(), "and it picked the text");
    pass(&mut ws, &ctx, input(0.30, vec![button(true)]));
    pass(&mut ws, &ctx, input(0.35, vec![button(false)]));
    assert_eq!(ws.tool(), Tool::Text, "the Text tool took over");
    let edit = ws.text.edit.as_ref().expect("the text is open to edit");
    assert_eq!(edit.buffer, "I I");
    assert!(edit.editing.is_some());
}
