use super::*;

#[test]
fn a_quick_press_and_release_alone_is_a_tap() {
    let mut tap = ModifierTap::default();
    assert!(!tap.update(true, false, 1.0));
    assert!(!tap.update(true, false, 1.1));
    assert!(tap.update(false, false, 1.2));
    assert!(!tap.update(false, false, 1.3), "only once");
}

#[test]
fn a_long_hold_or_anything_else_pressed_is_not_a_tap() {
    let mut tap = ModifierTap::default();
    tap.update(true, false, 0.0);
    assert!(
        !tap.update(false, false, TAP_SECONDS + 0.1),
        "held too long"
    );
    // Alt-click: a button went down while it was held.
    tap.update(true, false, 2.0);
    tap.update(true, true, 2.1);
    assert!(!tap.update(false, false, 2.2));
    // Already held with something else when it went down.
    tap.update(true, true, 3.0);
    assert!(!tap.update(false, false, 3.1));
    // A key let go on the very frame the modifier is.
    tap.update(true, false, 4.0);
    assert!(!tap.update(false, true, 4.1));
    // A clock that goes backwards is no tap either.
    tap.update(true, false, 5.0);
    assert!(!tap.update(false, false, 4.0));
    // And after all that, a clean tap still works.
    tap.update(true, false, 6.0);
    assert!(tap.update(false, false, 6.2));
}
