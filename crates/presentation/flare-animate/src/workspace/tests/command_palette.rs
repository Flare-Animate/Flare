use super::*;

fn press(ws: &mut Workspace, ctx: &egui::Context, time: f64, k: Key, mods: Modifiers) {
    pass(ws, ctx, input(time, vec![key(k, mods)]));
}

fn type_text(ws: &mut Workspace, ctx: &egui::Context, time: f64, text: &str) {
    pass(ws, ctx, input(time, vec![Event::Text(text.to_owned())]));
}

#[test]
fn the_chord_opens_the_palette_and_typing_finds_commands_by_name_id_or_keys() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    let shift_cmd = Modifiers {
        command: true,
        shift: true,
        ..Modifiers::NONE
    };
    press(&mut ws, &ctx, 0.1, Key::P, shift_cmd);
    assert!(ws.palette.open, "Cmd/Ctrl+Shift+P opens it");
    run(&mut ws, &ctx, 0.2);
    type_text(&mut ws, &ctx, 0.3, "save as");
    run(&mut ws, &ctx, 0.4);
    assert_eq!(ws.palette.query, "save as");
    assert_eq!(ws.palette_commands().first(), Some(&MenuAction::SaveAs));

    // Commands with no keys are found too, and the palette never lists itself.
    ws.palette.query = "workspace".into();
    let found = ws.palette_commands();
    assert!(found.contains(&MenuAction::WorkspaceAnimator), "{found:?}");
    ws.palette.query = "palette".into();
    assert!(!ws.palette_commands().contains(&MenuAction::CommandPalette));
    ws.palette.query = "f5".into();
    assert!(ws.palette_commands().contains(&MenuAction::InsertFrame));
    ws.palette.query = String::new();
    assert_eq!(
        ws.palette_commands().len(),
        crate::workspace::command_palette::MAX_ROWS,
        "bounded"
    );
}

#[test]
fn arrows_choose_enter_runs_and_escape_closes() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.perform(MenuAction::CommandPalette);
    ws.palette.query = "workspace".into();
    run(&mut ws, &ctx, 0.0);
    let found = ws.palette_commands();
    let animator = found
        .iter()
        .position(|a| *a == MenuAction::WorkspaceAnimator)
        .expect("listed");
    assert!(animator >= 1, "{found:?}");
    for (step, k) in std::iter::repeat_n(Key::ArrowDown, animator + 1)
        .chain([Key::ArrowUp])
        .enumerate()
    {
        press(&mut ws, &ctx, 0.1 + step as f64 * 0.01, k, Modifiers::NONE);
    }
    assert_eq!(ws.palette.chosen, animator);
    press(&mut ws, &ctx, 0.4, Key::Enter, Modifiers::NONE);
    assert!(!ws.palette.open, "it closes to run the command");
    assert_eq!(
        ws.active_workspace(),
        &flare_dock::WorkspaceId::Preset(flare_dock::Preset::Animator)
    );

    ws.perform(MenuAction::CommandPalette);
    run(&mut ws, &ctx, 0.5);
    press(&mut ws, &ctx, 0.6, Key::Escape, Modifiers::NONE);
    assert!(!ws.palette.open);
}

#[test]
fn a_command_that_cannot_run_now_is_listed_but_not_run() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    assert!(!ws.is_enabled(MenuAction::Ungroup), "nothing is selected");
    ws.perform(MenuAction::CommandPalette);
    ws.palette.query = "ungroup".into();
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.palette_commands(), vec![MenuAction::Ungroup]);
    let document = ws.document().clone();
    press(&mut ws, &ctx, 0.1, Key::Enter, Modifiers::NONE);
    assert!(ws.palette.open, "nothing ran, so it stays open");
    assert_eq!(ws.document(), &document);
}

#[test]
fn a_click_outside_closes_it() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.perform(MenuAction::CommandPalette);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let at = Pos2::new(40.0, 700.0);
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(at)]));
    pass(&mut ws, &ctx, input(0.3, vec![click(true)]));
    assert!(!ws.palette.open);
}
