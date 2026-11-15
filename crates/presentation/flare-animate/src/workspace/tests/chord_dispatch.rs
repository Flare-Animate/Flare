//! Ctrl, Alt and F-key commands follow the shortcut map: a rebinding moves the key that runs the
//! command, and the old key stops.

use super::*;

fn drawn_once() -> (egui::Context, Workspace) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    rect(&mut ws);
    (ctx, ws)
}

#[test]
fn a_rebound_undo_runs_on_its_new_chord_and_not_the_old_one() {
    let (ctx, mut ws) = drawn_once();
    ws.set_shortcut(MenuAction::Undo, "CmdOrCtrl+U")
        .expect("free chord");
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Z, Modifiers::COMMAND)]),
    );
    assert_eq!(drawing_count(&ws), 1, "the old chord must do nothing");
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::U, Modifiers::COMMAND)]),
    );
    assert_eq!(drawing_count(&ws), 0);
}

#[test]
fn a_set_with_another_redo_chord_redoes_on_it() {
    let (ctx, mut ws) = drawn_once();
    ws.choose_shortcut_set("opentoonz").expect("set");
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Z, Modifiers::COMMAND)]),
    );
    assert_eq!(drawing_count(&ws), 0);
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::Y, Modifiers::COMMAND)]),
    );
    assert_eq!(drawing_count(&ws), 1);
}

#[test]
fn an_unbound_command_has_no_chord() {
    let (ctx, mut ws) = drawn_once();
    ws.clear_shortcut(MenuAction::Undo);
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Z, Modifiers::COMMAND)]),
    );
    assert_eq!(drawing_count(&ws), 1);
}

#[test]
fn a_rebound_frame_key_inserts_a_frame_and_the_old_key_does_not() {
    let (ctx, mut ws) = drawn_once();
    let frames = |ws: &Workspace| {
        ws.document().scenes()[0].timeline().layers()[0]
            .spans()
            .len()
    };
    ws.set_shortcut(MenuAction::InsertKeyframe, "F9")
        .expect("free chord");
    let before = frames(&ws);
    ws.set_frame(3);
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::F6, Modifiers::NONE)]),
    );
    assert_eq!(frames(&ws), before, "F6 no longer inserts a keyframe");
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::F9, Modifiers::NONE)]),
    );
    assert!(frames(&ws) > before, "F9 does");
}

#[test]
fn shift_chords_win_over_the_plain_chord_they_contain() {
    let (ctx, mut ws) = drawn_once();
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Z, Modifiers::COMMAND)]),
    );
    assert_eq!(drawing_count(&ws), 0);
    pass(
        &mut ws,
        &ctx,
        input(
            0.2,
            vec![key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)],
        ),
    );
    assert_eq!(drawing_count(&ws), 1, "redo, not another undo");
}

#[test]
fn a_default_chord_outside_the_old_list_runs_open_and_quit() {
    let (ctx, mut ws) = drawn_once();
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::O, Modifiers::COMMAND)]),
    );
    assert_eq!(ws.take_dialog(), Some(DialogRequest::Open));
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::Q, Modifiers::COMMAND)]),
    );
    assert!(ws.take_quit_request(), "Cmd+Q asks to quit");
}

#[test]
fn cut_frames_runs_from_its_chord_on_a_selected_range() {
    let (ctx, mut ws) = drawn_once();
    ws.set_frame(2);
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::X, Modifiers::COMMAND | Modifiers::ALT)]),
    );
    assert!(
        ws.is_enabled(MenuAction::PasteFrames),
        "the cut frames landed on the clipboard"
    );
}

#[test]
fn a_tool_letter_bound_as_a_command_stays_out_of_the_chords() {
    // The opentoonz set moves the Select tool onto `S` and leaves `V` bound to nothing: a bare
    // letter is not a chord, so the command chords pass never takes it and the tool keys pass
    // keeps it.
    let (ctx, mut ws) = drawn_once();
    ws.choose_shortcut_set("opentoonz").expect("set");
    let before = ws.tool();
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::V, Modifiers::NONE)]),
    );
    assert_eq!(
        ws.tool(),
        before,
        "V is neither a chord nor a tool of this set"
    );
}

#[test]
fn a_f_key_is_a_chord_even_without_a_modifier() {
    let (ctx, mut ws) = drawn_once();
    // ConvertToSymbol is bound to F8 by default and was in the fixed list; a command bound to
    // another F-key the table does not know must run too.
    ws.set_shortcut(MenuAction::ToggleOnionSkin, "F11")
        .expect("free chord");
    let before = ws.onion_skin().enabled;
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::F11, Modifiers::NONE)]),
    );
    assert_ne!(
        ws.onion_skin().enabled,
        before,
        "F11 toggles the onion skin"
    );
}
