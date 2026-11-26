//! The undo steps hold at most so many bytes of pictures that no later state shows.

use flare_core::BitmapId;
use flare_document::Bitmap;

use super::*;

/// A 10 by 10 picture: 400 bytes.
fn picture(shade: u8) -> Bitmap {
    Bitmap::new(BitmapId::new(), "p", 10, 10, vec![shade; 400]).expect("bitmap")
}

/// Puts a new picture in the library in place of `old`, as a stroke on a painting does.
fn replace(session: &mut EditorSession, old: Option<BitmapId>, shade: u8) -> BitmapId {
    let new = picture(shade);
    let id = new.id();
    let mut commands = vec![Command::AddBitmap(new)];
    commands.extend(old.map(Command::RemoveBitmap));
    session.apply(commands).expect("apply");
    id
}

#[test]
fn the_oldest_steps_go_once_their_pictures_pass_the_budget() {
    let f = fixture();
    // Room for two replaced pictures, not three.
    let mut session = EditorSession::with_history_limits(f.session.document().clone(), 100, 1000);
    let mut last = replace(&mut session, None, 1);
    assert_eq!(
        session.held_picture_bytes(),
        0,
        "adding keeps nothing behind"
    );
    for shade in 2..=5 {
        last = replace(&mut session, Some(last), shade);
    }
    // Five steps: the first left nothing behind, each later one a 400-byte picture, and 1000
    // bytes hold two of those.
    assert_eq!(session.undo_labels().len(), 2, "the first three went");
    assert!(session.held_picture_bytes() <= 1000);
    assert_eq!(session.held_picture_bytes(), 800);

    // Undo and redo carry the count with them.
    assert!(session.undo());
    assert_eq!(session.held_picture_bytes(), 400);
    assert!(session.redo());
    assert_eq!(session.held_picture_bytes(), 800);
    assert_eq!(session.document().bitmap(last).map(Bitmap::id), Some(last));
}

#[test]
fn the_newest_step_is_kept_however_large() {
    let f = fixture();
    let mut session = EditorSession::with_history_limits(f.session.document().clone(), 100, 10);
    let first = replace(&mut session, None, 1);
    replace(&mut session, Some(first), 2);
    assert_eq!(session.undo_labels().len(), 1);
    assert!(session.undo(), "the one step is still there");
    assert!(session.document().bitmap(first).is_some());
}

#[test]
fn clearing_the_history_frees_what_it_held() {
    let f = fixture();
    let mut session = EditorSession::new(f.session.document().clone());
    let first = replace(&mut session, None, 1);
    replace(&mut session, Some(first), 2);
    assert_eq!(session.held_picture_bytes(), 400);
    session.clear_history();
    assert_eq!(session.held_picture_bytes(), 0);
}
