//! The frame picker moves the playhead the way `flare goto` does, by number and by keyframe.

use flare_editor::frame_picker::{Goto, GotoError};

use super::two_exposures;

#[test]
fn the_picker_goes_by_number_and_by_keyframe_and_refuses_what_is_not_there() {
    // Two keyframes: frames 0 to 11 and 12 to 23.
    let (mut ws, _layer) = two_exposures();
    assert_eq!(ws.go_to(Goto::Frame(5)), Ok(5));
    assert_eq!(ws.frame(), 5);
    assert_eq!(ws.go_to(Goto::NextKeyframe), Ok(12));
    assert_eq!(ws.frame(), 12);
    assert_eq!(ws.go_to(Goto::PreviousKeyframe), Ok(0));
    assert_eq!(ws.go_to(Goto::Last), Ok(23));

    // A frame past the end, and no keyframe after the last one: refused, and the playhead stays.
    assert!(matches!(
        ws.go_to(Goto::Frame(24)),
        Err(GotoError::PastTheEnd { frame: 24, .. })
    ));
    assert_eq!(ws.go_to(Goto::NextKeyframe), Err(GotoError::NothingThere));
    assert_eq!(ws.frame(), 23);
}
