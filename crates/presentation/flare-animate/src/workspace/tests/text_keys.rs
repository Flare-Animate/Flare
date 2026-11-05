//! The text flow from the keyboard: click to place the box, type, then commit.

use super::*;

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// A text box opened with the tool and filled by typing, with the editor drawn.
fn typed(text: &str) -> (egui::Context, Workspace) {
    let (ctx, mut ws) = (egui::Context::default(), text_workspace());
    flare_ui::icons::install(&ctx);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    ws.text_click(Pos2::new(40.0, 40.0));
    run(&mut ws, &ctx, 0.2);
    pass(&mut ws, &ctx, input(0.3, vec![Event::Text(text.into())]));
    assert_eq!(ws.text.edit.as_ref().map(|e| e.buffer.as_str()), Some(text));
    (ctx, ws)
}

#[test]
fn escape_commits_the_typed_text_as_a_shape() {
    let (ctx, mut ws) = typed("Hi");
    let before = drawing_count(&ws);
    pass(
        &mut ws,
        &ctx,
        input(0.4, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert!(ws.text.edit.is_none(), "{:?}", ws.status());
    assert_eq!(drawing_count(&ws), before + 1);
}

#[test]
fn ctrl_enter_commits_but_plain_enter_adds_a_line() {
    let (ctx, mut ws) = typed("Hi");
    pass(
        &mut ws,
        &ctx,
        input(0.4, vec![key(Key::Enter, Modifiers::NONE)]),
    );
    assert!(ws.text.edit.is_some(), "Enter goes on typing");
    let before = drawing_count(&ws);
    pass(
        &mut ws,
        &ctx,
        input(0.5, vec![key(Key::Enter, Modifiers::COMMAND)]),
    );
    assert!(ws.text.edit.is_none());
    assert_eq!(drawing_count(&ws), before + 1);
}

#[test]
fn the_text_hint_names_the_keys_that_finish_it_not_a_missing_button() {
    let ws = text_workspace();
    let hint = tr!(ws, "tools-status-text");
    assert!(!hint.contains("Place"), "{hint}");
    assert!(hint.contains("Esc"), "{hint}");
}
