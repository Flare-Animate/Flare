//! Save writes a `.flare` project that was opened from disk, and a failure is told to the person.

use super::*;

#[test]
fn save_writes_the_edited_project_back_to_the_opened_flare_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("open.flare");
    flare_storage::save(&document(), &path).expect("write the project");
    let mut ws = workspace();
    ws.open_any(&path);
    assert_eq!(ws.path(), Some(path.as_path()), "{:?}", ws.status());
    ws.commit_rectangle(Pos2::new(900.0, 900.0), Pos2::new(960.0, 950.0));
    assert!(ws.is_dirty());
    ws.save();
    assert!(!ws.is_dirty(), "saved: {:?}", ws.status());
    let reloaded = flare_storage::load(&path).expect("reload");
    assert_eq!(
        &reloaded,
        ws.document(),
        "the bytes on disk are the document"
    );
}

#[test]
fn a_failed_save_is_reported_and_leaves_the_tab_dirty() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("gone").join("open.flare");
    let mut ws = workspace();
    ws.commit_rectangle(Pos2::new(900.0, 900.0), Pos2::new(960.0, 950.0));
    ws.save_as(&path);
    assert!(ws.is_dirty());
    assert!(ws.status().is_some(), "the person is told");
}

#[test]
fn save_with_worker_threads_writes_the_file() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("open.flare");
    flare_storage::save(&document(), &path).expect("write the project");
    let mut ws = workspace();
    ws.background.start_threads(std::sync::Arc::new(|| {}));
    ws.open_any(&path);
    for done in ws.background.finish_io() {
        ws.handle_done(done);
    }
    assert_eq!(ws.path(), Some(path.as_path()), "{:?}", ws.status());
    ws.commit_rectangle(Pos2::new(900.0, 900.0), Pos2::new(960.0, 950.0));
    ws.save();
    for done in ws.background.finish_io() {
        ws.handle_done(done);
    }
    assert!(!ws.is_dirty(), "saved: {:?} {:?}", ws.status(), ws.log);
    assert_eq!(&flare_storage::load(&path).expect("reload"), ws.document());
}
