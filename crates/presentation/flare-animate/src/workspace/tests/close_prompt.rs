use super::*;
use crate::workspace::close_prompt::Choice;

fn drawn() -> Workspace {
    let mut ws = workspace();
    rect(&mut ws);
    ws
}

#[test]
fn closing_a_clean_tab_asks_nothing() {
    let mut ws = workspace();
    ws.perform(MenuAction::New);
    ws.close_tab(1);
    assert_eq!(ws.close_prompt, None);
    assert_eq!(ws.tabs().len(), 1);
}

#[test]
fn closing_unsaved_work_asks_and_cancel_keeps_everything() {
    let mut ws = drawn();
    ws.perform(MenuAction::Close);
    assert_eq!(ws.close_prompt, Some(0));
    assert!(ws.is_dirty());
    ws.apply_close_choice(0, Choice::Keep);
    assert_eq!(ws.close_prompt, None);
    assert_eq!(drawing_count(&ws), 1, "the drawing is still there");
}

#[test]
fn dont_save_closes_the_tab_and_gives_the_work_up() {
    let mut ws = drawn();
    ws.perform(MenuAction::New);
    ws.close_tab(0);
    assert_eq!(
        ws.close_prompt,
        Some(0),
        "a tab that is not on show asks too"
    );
    ws.apply_close_choice(0, Choice::Discard);
    assert_eq!(ws.tabs().len(), 1);
    assert!(!ws.any_dirty());
    assert_eq!(
        layer_count(&ws),
        1,
        "the one left is the new, empty document"
    );

    let mut only = drawn();
    only.perform(MenuAction::Close);
    only.apply_close_choice(0, Choice::Discard);
    assert_eq!(only.tabs().len(), 1, "the last tab becomes an empty one");
    assert_eq!(layer_count(&only), 1);
    assert!(!only.is_dirty());
}

#[test]
fn save_writes_the_file_and_then_closes_the_tab() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("kept.flare");
    let mut ws = Workspace::new(document(), Some(path.clone()));
    rect(&mut ws);
    ws.perform(MenuAction::New);
    ws.close_tab(0);
    ws.apply_close_choice(0, Choice::Save);
    assert_eq!(ws.position, 0, "the tab is brought forward to be saved");
    let ctx = egui::Context::default();
    ws.settle_inline();
    run(&mut ws, &ctx, 0.0);
    assert!(path.exists(), "the project was written");
    assert_eq!(ws.tabs().len(), 1, "and its tab closed");
    assert_eq!(ws.path(), None, "the empty one is what is left");
}

#[test]
fn a_save_that_is_not_made_leaves_the_tab_open() {
    // An untitled project asks for a place to save it; if none is given the work is not closed.
    let mut ws = drawn();
    ws.apply_close_choice(0, Choice::Save);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::SaveAs));
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    assert!(
        !ws.close_after_save,
        "the wish to close ended with the dialog"
    );
    assert!(ws.is_dirty());
    assert_eq!(drawing_count(&ws), 1);
}

#[test]
fn the_prompt_is_drawn_without_answering_itself() {
    let mut ws = drawn();
    ws.perform(MenuAction::Close);
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    assert_eq!(ws.close_prompt, Some(0));
    assert_eq!(ws.tabs().len(), 1);
}
