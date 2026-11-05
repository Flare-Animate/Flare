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
