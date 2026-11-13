#![allow(clippy::expect_used)]

use super::super::*;

#[test]
fn touch_sizes_are_larger_than_pointer_sizes() {
    assert!(super::rail_width(true) > super::rail_width(false));
    assert!(super::rail_button(true) > super::rail_button(false));
    assert!(super::rail_icon(true) > super::rail_icon(false));
    assert!(super::tab_bar(true) > super::tab_bar(false));
}

#[test]
fn restyling_keeps_the_choice() {
    let mut workspace = Workspace::untitled().expect("a workspace");
    let ctx = egui::Context::default();
    workspace.set_touch(true);
    workspace.ensure_touch_style(&ctx);
    // Applying it again changes nothing, and the style says touch was applied.
    workspace.ensure_touch_style(&ctx);
    assert_eq!(workspace.styled_touch, Some(true));
    // Back to the pointer, the choice is re-applied again.
    workspace.set_touch(false);
    workspace.ensure_touch_style(&ctx);
    assert_eq!(workspace.styled_touch, Some(false));
}
