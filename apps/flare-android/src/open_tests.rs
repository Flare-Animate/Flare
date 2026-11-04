//! The start of the app: what a saved file that cannot be read does to it.
#![allow(clippy::expect_used)]

use super::start;

#[test]
fn a_start_with_nothing_saved_still_opens_a_document() {
    let workspace = start(None, None).expect("a first run starts");
    assert!(
        workspace.document().scenes().len() >= 1,
        "there is a document to show"
    );
}

#[test]
fn saved_layouts_and_preferences_that_cannot_be_read_are_ignored_not_fatal() {
    // Junk in a saved file is the same start as no saved file: it must never stop the app.
    let with_junk = start(
        Some("{ not json".to_owned()),
        Some("{\"version\": \"no version\"}".to_owned()),
    )
    .expect("a start with junk still goes on");
    assert!(
        !with_junk.is_dirty(),
        "junk is read as \"nothing saved\", so the start opens at Home"
    );
}

#[test]
fn a_clean_start_is_a_start_at_home_because_there_is_no_work_to_return_to() {
    let workspace = start(None, None).expect("the first run");
    assert!(
        !workspace.is_dirty(),
        "a first run is clean, so it opens at Home"
    );
}
