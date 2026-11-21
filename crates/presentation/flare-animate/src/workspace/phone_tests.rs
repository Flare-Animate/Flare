//! The phone arrangement: which windows get it, what it puts away, and that a wider window gives
//! back what was there.
#![allow(clippy::expect_used)]

use super::*;

#[test]
fn only_a_touch_window_narrower_than_the_limit_is_a_phone() {
    assert!(is_phone(true, 390.0));
    assert!(is_phone(true, PHONE_MAX_WIDTH - 0.5));
    // The limit itself is a wide window.
    assert!(!is_phone(true, PHONE_MAX_WIDTH));
    assert!(!is_phone(true, 744.0), "a portrait tablet keeps its panels");
    // A pointer never gets the phone arrangement, however narrow.
    assert!(!is_phone(false, 390.0));
}

#[test]
fn a_phone_puts_every_collapsible_panel_in_the_strip_and_keeps_the_stage_and_the_rest_docked() {
    let mut ws = Workspace::untitled().expect("workspace");
    ws.set_touch(true);
    let strip_before = ws.panels.layout.strip.len();
    ws.apply_phone_arrangement(390.0);
    for pane in flare_dock::Pane::ALL {
        if pane.can_collapse() {
            assert!(
                ws.panels.layout.in_strip(pane),
                "{pane:?} is in the strip on a phone"
            );
        } else {
            assert!(!ws.panels.layout.in_strip(pane), "{pane:?} stays docked");
        }
    }
    assert!(
        flare_dock::check(&ws.panels.layout).is_ok(),
        "the phone arrangement is valid"
    );
    assert!(
        ws.phone_stash.is_some(),
        "what the person had is kept to come back"
    );
    assert!(
        ws.panels.layout.strip.len() > strip_before,
        "the docked side panels moved into the strip"
    );
}

#[test]
fn a_wider_window_gives_the_arrangement_back_and_a_second_narrow_frame_changes_nothing() {
    let mut ws = Workspace::untitled().expect("workspace");
    ws.set_touch(true);
    let before = ws.panels.layout.strip.clone();
    ws.apply_phone_arrangement(390.0);
    ws.apply_phone_arrangement(360.0);
    assert!(
        ws.phone_stash.is_some(),
        "still a phone: nothing is put away twice"
    );
    ws.apply_phone_arrangement(1200.0);
    assert!(ws.phone_stash.is_none());
    assert_eq!(
        ws.panels.layout.strip, before,
        "the arrangement is back as it was"
    );
}

#[test]
fn the_saved_layout_is_the_persons_own_while_a_phone_shows_another() {
    let mut ws = Workspace::untitled().expect("workspace");
    ws.set_touch(true);
    let saved_before = ws.layout_json().expect("json");
    ws.apply_phone_arrangement(390.0);
    let saved_on_phone = ws.layout_json().expect("json");
    assert_eq!(
        saved_before, saved_on_phone,
        "a phone never changes what is saved"
    );
}

#[test]
fn a_pointer_window_is_never_rearranged() {
    let mut ws = Workspace::untitled().expect("workspace");
    let before = ws.panels.layout.strip.clone();
    ws.apply_phone_arrangement(390.0);
    assert!(ws.phone_stash.is_none());
    assert_eq!(ws.panels.layout.strip, before);
}
