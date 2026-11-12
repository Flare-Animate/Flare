use std::path::Path;

use crate::app_icon::{in_app_bundle, show_in_dock};

#[test]
fn a_program_inside_an_app_bundle_is_known_and_a_bare_one_is_not() {
    assert!(in_app_bundle(Path::new(
        "/Applications/Flare.app/Contents/MacOS/flare"
    )));
    assert!(!in_app_bundle(Path::new(
        "/Users/me/Flare/target/debug/flare-desktop"
    )));
    assert!(!in_app_bundle(Path::new("/tmp/Contents/MacOS/flare")));
    assert!(!in_app_bundle(Path::new("/Flare.app/MacOS/flare")));
    assert!(!in_app_bundle(Path::new("flare")));
}

#[test]
fn a_picture_that_cannot_be_read_or_no_main_thread_changes_nothing() {
    // A test runs off the main thread, and these bytes are no picture: neither may panic.
    show_in_dock(b"not a png");
    show_in_dock(&[]);
}
