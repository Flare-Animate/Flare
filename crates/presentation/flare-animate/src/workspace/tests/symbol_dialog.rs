//! Insert > New Symbol and Modify > Convert to Symbol ask for the name and type first, in
//! Animate's dialog, and the Save Changes question is laid out as a desktop's.

use super::layer_names::{click, find, settle};
use super::*;
use crate::workspace::symbol_dialog::SymbolPurpose;
use egui::PointerButton;
use flare_document::SymbolKind;
use flare_tools::library::Registration;

fn texts(ws: &mut Workspace, ctx: &egui::Context, time: f64) -> Vec<String> {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect()
}

#[test]
fn new_symbol_asks_for_a_name_and_type_then_makes_it_and_opens_it_for_drawing() {
    let mut ws = workspace();
    ws.library.new_kind = SymbolKind::Button;
    ws.perform(MenuAction::NewSymbol);
    assert!(
        ws.document().symbols().is_empty(),
        "nothing is made before OK"
    );
    let form = ws.symbol_dialog().expect("the dialog is open");
    assert_eq!(form.purpose, SymbolPurpose::New);
    assert_eq!(form.name, "Symbol 1", "the first free name");
    assert_eq!(form.kind, SymbolKind::Button, "the type chosen last time");
    form.name = " Walk cycle ".to_owned();
    form.kind = SymbolKind::MovieClip;
    ws.confirm_symbol_dialog();
    assert!(ws.symbol_dialog().is_none());
    let symbol = &ws.active().document().symbols()[0];
    assert_eq!(
        (symbol.name(), symbol.kind()),
        ("Walk cycle", SymbolKind::MovieClip)
    );
    assert!(ws.in_symbol(), "Animate opens a new symbol for drawing");
    assert_eq!(
        ws.breadcrumb().last().map(String::as_str),
        Some("Walk cycle")
    );
    assert_eq!(
        ws.library.new_kind,
        SymbolKind::MovieClip,
        "and remembers the type"
    );
    assert!(ws.exit_symbol());
    ws.undo();
    assert!(ws.document().symbols().is_empty(), "one undo step");
}

#[test]
fn a_name_that_cannot_be_used_keeps_the_dialog_open_and_changes_nothing() {
    let mut ws = workspace();
    ws.new_symbol();
    let revision = ws.active().revision();
    for name in ["Symbol 1", "  ", &"x".repeat(300)] {
        ws.perform(MenuAction::NewSymbol);
        ws.symbol_dialog().expect("open").name = name.to_owned();
        ws.confirm_symbol_dialog();
        assert!(ws.symbol_dialog().is_some(), "{name:?} is refused");
        assert_eq!(ws.active().revision(), revision);
        assert!(!ws.in_symbol());
        ws.windows.symbol = None;
    }
}

#[test]
fn convert_asks_and_makes_the_chosen_type_with_its_origin_on_the_registration_point() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    let before = shape_bounds(&ws);
    ws.perform(MenuAction::ConvertToSymbol);
    assert!(
        ws.document().symbols().is_empty(),
        "nothing is made before OK"
    );
    let form = ws.symbol_dialog().expect("the dialog is open");
    assert!(matches!(form.purpose, SymbolPurpose::Convert { .. }));
    assert_eq!(
        form.registration,
        Registration::TopLeft,
        "Animate's default"
    );
    form.name = "Box".to_owned();
    form.kind = SymbolKind::Button;
    form.registration = Registration::Center;
    ws.confirm_symbol_dialog();
    let id = ws.library.selected.expect("the new symbol is selected");
    let symbol = ws.document().symbol(id).expect("in the library");
    assert_eq!((symbol.name(), symbol.kind()), ("Box", SymbolKind::Button));
    assert!(!ws.in_symbol(), "a conversion stays on the stage");
    let scene = ws.scene_id().expect("scene");
    let Some(Content::Instance(instance)) =
        ops::find_element(ws.document(), scene, ws.selected.expect("still selected"))
    else {
        panic!("the shape became an instance");
    };
    let origin = instance.transform().as_affine().translation;
    let center = before.center();
    assert!(
        (origin - DVec2::new(f64::from(center.x), f64::from(center.y))).length() < 1e-3,
        "the instance stands on the middle of the artwork: {origin:?} {center:?}"
    );
    let after = shape_bounds(&ws);
    assert!((after.min - before.min).length() < 1e-3, "nothing moved");
    // The next dialog starts on the choices made in this one.
    draw_and_select(&mut ws);
    ws.perform(MenuAction::ConvertToSymbol);
    let form = ws.symbol_dialog().expect("open");
    assert_eq!(
        (form.kind, form.registration),
        (SymbolKind::Button, Registration::Center)
    );
    assert_eq!(form.name, "Symbol 1", "Box is taken by name only");
    ws.windows.symbol = None;
}

#[test]
fn convert_with_nothing_to_convert_says_so_instead_of_asking() {
    let mut ws = workspace();
    assert!(
        !ws.is_enabled(MenuAction::ConvertToSymbol),
        "the menu item is dimmed"
    );
    // F8 or the stage menu reach the same place; it explains rather than asking.
    ws.open_convert_symbol_dialog();
    assert!(ws.symbol_dialog().is_none());
    assert_eq!(ws.status(), Some("Select a shape to convert to a symbol"));
}

#[test]
fn the_dialog_is_drawn_in_the_language_and_ok_and_escape_answer_it() {
    let mut ws = workspace();
    ws.set_system_locale("de-DE");
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    ws.perform(MenuAction::NewSymbol);
    settle(&mut ws, &ctx, &mut time);
    time += 0.1;
    let shown = texts(&mut ws, &ctx, time);
    for wanted in [
        "Neues Symbol erstellen",
        "Name",
        "Typ",
        "Ordner",
        "OK",
        "Abbrechen",
        "Grafik",
    ] {
        assert!(shown.iter().any(|t| t == wanted), "{wanted}: {shown:?}");
    }
    assert!(
        !shown
            .iter()
            .any(|t| t == "Ausrichtung" || t == "Registrierung"),
        "no grid for a new symbol"
    );
    // A click on a type card picks it.
    time += 0.1;
    let card = find(&mut ws, &ctx, time, vec![], "Schaltfläche").expect("the Button card");
    click(&mut ws, &ctx, &mut time, card, PointerButton::Primary);
    assert_eq!(ws.symbol_dialog().expect("open").kind, SymbolKind::Button);
    // Escape gives the dialog up and makes nothing.
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert!(ws.symbol_dialog().is_none());
    assert!(ws.document().symbols().is_empty());
    // OK makes the symbol.
    ws.perform(MenuAction::NewSymbol);
    settle(&mut ws, &ctx, &mut time);
    time += 0.1;
    let ok = find(&mut ws, &ctx, time, vec![], "OK").expect("OK");
    click(&mut ws, &ctx, &mut time, ok, PointerButton::Primary);
    assert!(ws.symbol_dialog().is_none());
    assert_eq!(ws.active().document().symbols().len(), 1);
    assert_eq!(
        ws.active().document().symbols()[0].kind(),
        SymbolKind::Graphic,
        "a type picked in a dialog that was given up is not kept"
    );
}

#[test]
fn a_taken_name_is_explained_under_the_field_and_ok_is_dimmed() {
    let mut ws = workspace();
    ws.new_symbol();
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    ws.perform(MenuAction::NewSymbol);
    ws.symbol_dialog().expect("open").name = "Symbol 1".to_owned();
    settle(&mut ws, &ctx, &mut time);
    time += 0.1;
    let shown = texts(&mut ws, &ctx, time);
    let reason = "The library already has a symbol called Symbol 1";
    assert!(shown.iter().any(|t| t == reason), "{shown:?}");
    time += 0.1;
    let ok = find(&mut ws, &ctx, time, vec![], "OK").expect("OK");
    click(&mut ws, &ctx, &mut time, ok, PointerButton::Primary);
    assert!(ws.symbol_dialog().is_some(), "a dimmed OK does nothing");
    assert_eq!(ws.document().symbols().len(), 1);
}

#[test]
fn the_save_changes_question_says_what_is_lost_and_its_buttons_answer_it() {
    let mut ws = workspace();
    rect(&mut ws);
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    ws.perform(MenuAction::Close);
    settle(&mut ws, &ctx, &mut time);
    time += 0.1;
    let shown = texts(&mut ws, &ctx, time);
    for wanted in [
        "Your changes will be lost if you don't save them.",
        "Save",
        "Don't Save",
        "Cancel",
    ] {
        assert!(shown.iter().any(|t| t == wanted), "{wanted}: {shown:?}");
    }
    // Don't Save sits apart at the left, Save at the right edge.
    time += 0.1;
    let at =
        |ws: &mut Workspace, time: f64, label| find(ws, &ctx, time, vec![], label).expect(label);
    let (discard, cancel, save) = (
        at(&mut ws, time, "Don't Save"),
        at(&mut ws, time + 0.01, "Cancel"),
        at(&mut ws, time + 0.02, "Save"),
    );
    assert!(
        discard.x < cancel.x && cancel.x < save.x,
        "{discard:?} {cancel:?} {save:?}"
    );
    time += 0.1;
    click(&mut ws, &ctx, &mut time, cancel, PointerButton::Primary);
    assert_eq!(ws.close_prompt, None, "Cancel answers");
    assert!(ws.is_dirty(), "and keeps the work");
}
