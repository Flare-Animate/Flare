//! Save and Save As write Animate's formats as well as Flare's, and a document opened from a
//! `.fla` or `.xfl` is saved back to it. The files are real ones in temporary folders.

use super::*;
use crate::workspace::close_prompt::Choice;
use std::collections::BTreeMap;
use std::path::Path;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../io/flare-import/tests/fixtures")
        .join(name)
}

/// A copy of the `sample.fla` fixture in a new folder, and the folder.
fn fla_copy() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("sample.fla");
    std::fs::copy(fixture("sample.fla"), &path).expect("copy");
    (dir, path)
}

/// A rectangle away from anything the fixtures hold, which would otherwise merge with it.
fn far_rect(ws: &mut Workspace) {
    ws.commit_rectangle(Pos2::new(900.0, 900.0), Pos2::new(960.0, 950.0));
}

/// How many shapes the document holds. (An empty keyframe holds one element that is no shape, so
/// the element count cannot tell a drawing from no drawing.)
fn shapes(ws: &Workspace) -> usize {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .flat_map(|l| l.spans())
        .flat_map(|s| s.elements())
        .filter(|c| matches!(c, Content::Shape(_)))
        .count()
}

fn opened(path: &Path) -> Workspace {
    let mut ws = workspace();
    ws.open_any(path);
    assert_eq!(ws.path(), Some(path), "{:?}", ws.status());
    ws
}

fn entries(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
    (0..zip.len())
        .map(|i| {
            let mut file = zip.by_index(i).expect("entry");
            let mut body = Vec::new();
            file.read_to_end(&mut body).expect("read");
            (file.name().expect("name").into_owned(), body)
        })
        .collect()
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn saving_an_unchanged_fla_writes_the_same_bytes_and_creates_no_flare() {
    let (dir, path) = fla_copy();
    let original = std::fs::read(&path).expect("read");
    let mut ws = opened(&path);
    ws.save();
    assert_eq!(std::fs::read(&path).expect("read"), original);
    assert_eq!(file_names(dir.path()), ["sample.fla"]);
    assert!(
        ws.status()
            .is_some_and(|s| s.contains("unchanged original")),
        "{:?}",
        ws.status()
    );
    assert!(!ws.is_dirty());
}

#[test]
fn an_edited_fla_is_saved_back_with_the_edit_and_every_untouched_file_as_it_was() {
    let (_dir, path) = fla_copy();
    let before = entries(&std::fs::read(&path).expect("read"));
    let mut ws = opened(&path);
    let drawn = shapes(&ws);
    far_rect(&mut ws);
    assert!(ws.is_dirty());
    ws.save();
    assert!(!ws.is_dirty(), "{:?}", ws.status());
    let after = entries(&std::fs::read(&path).expect("read"));
    let changed: Vec<&String> = before
        .iter()
        .filter(|(name, body)| after.get(*name) != Some(*body))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        changed,
        ["DOMDocument.xml"],
        "only what was edited is rebuilt"
    );
    let added: Vec<&String> = after.keys().filter(|n| !before.contains_key(*n)).collect();
    assert_eq!(
        added,
        ["mimetype"],
        "an archive that had none gets the marker"
    );
    let again = opened(&path);
    assert_eq!(shapes(&again), drawn + 1, "the edit is in the file");
    assert!(
        ws.status().is_some_and(|s| s.contains("rebuilt")),
        "{:?}",
        ws.status()
    );
    assert!(
        ws.log.iter().any(|l| l.contains("DOMDocument.xml")),
        "the Output panel names what was rebuilt: {:?}",
        ws.log
    );
}

#[cfg(unix)]
#[test]
fn a_save_that_fails_keeps_the_old_file_and_the_unsaved_work() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, path) = fla_copy();
    let original = std::fs::read(&path).expect("read");
    let mut ws = opened(&path);
    far_rect(&mut ws);
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).expect("chmod");
    ws.save();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).expect("chmod");
    if std::fs::write(dir.path().join("probe"), b"").is_ok() {
        return; // the permission does not bind this user (root)
    }
    assert!(ws.is_dirty(), "a failed save leaves the work unsaved");
    assert!(
        ws.status().is_some_and(|s| s.starts_with("Cannot save")),
        "{:?}",
        ws.status()
    );
    assert_eq!(std::fs::read(&path).expect("read"), original);
    assert_eq!(ws.path(), Some(path.as_path()));
}

#[test]
fn a_read_only_fla_is_not_replaced_and_says_so() {
    let (_dir, path) = fla_copy();
    let original = std::fs::read(&path).expect("read");
    let mut ws = opened(&path);
    far_rect(&mut ws);
    let mut permissions = std::fs::metadata(&path).expect("meta").permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).expect("chmod");
    ws.save();
    assert!(ws.is_dirty());
    assert!(
        ws.status().is_some_and(|s| s.contains("read-only")),
        "{:?}",
        ws.status()
    );
    assert_eq!(std::fs::read(&path).expect("read"), original);
}

#[test]
fn save_as_chooses_the_format_by_the_extension_and_save_then_writes_that_format() {
    let (dir, path) = fla_copy();
    let mut ws = opened(&path);
    far_rect(&mut ws);
    let flare = dir.path().join("work.flare");
    ws.save_as(&flare);
    assert_eq!(ws.path(), Some(flare.as_path()));
    assert!(flare_storage::load(&flare).is_ok(), "a project file");
    far_rect(&mut ws);
    ws.save();
    assert_eq!(shapes(&opened(&flare)), 2, "Save wrote the project again");
    let fla = dir.path().join("Out.FLA");
    ws.save_as(&fla);
    assert_eq!(
        ws.path(),
        Some(fla.as_path()),
        "an upper-case ending is a .fla"
    );
    assert!(flare_import::import_path(&fla).is_ok(), "a Flash document");
    assert_eq!(ws.title(), "Out.FLA - Flare");
    assert_eq!(shapes(&opened(&fla)), 2);
}

#[test]
fn a_name_with_no_extension_gets_fla_and_a_double_extension_keeps_the_last() {
    let dir = tempfile::tempdir().expect("dir");
    let mut ws = workspace();
    far_rect(&mut ws);
    ws.save_as(&dir.path().join("plain"));
    assert_eq!(ws.path(), Some(dir.path().join("plain.fla").as_path()));
    ws.save_as(&dir.path().join("a.flare.fla"));
    assert_eq!(ws.path(), Some(dir.path().join("a.flare.fla").as_path()));
    assert!(flare_storage::load(&dir.path().join("a.flare.fla")).is_err());
    assert_eq!(
        file_names(dir.path()),
        ["a.flare.fla", "plain.fla"],
        "no stray files"
    );
}

#[test]
fn an_untitled_document_asks_where_to_save_and_offers_fla() {
    let mut ws = workspace();
    far_rect(&mut ws);
    ws.save();
    assert_eq!(ws.take_dialog(), Some(DialogRequest::SaveAs));
    let (folder, name) = ws.save_as_start();
    assert_eq!(folder, None);
    assert_eq!(name, "Untitled.fla");
}

#[test]
fn save_as_starts_from_the_documents_own_folder_and_name() {
    let (dir, path) = fla_copy();
    let ws = opened(&path);
    let (folder, name) = ws.save_as_start();
    assert_eq!(folder.as_deref(), Some(dir.path()));
    assert_eq!(name, "sample.fla");
}

#[test]
fn xfl_is_saved_as_a_folder_with_a_stub_and_saved_back_to_it() {
    let dir = tempfile::tempdir().expect("dir");
    let mut ws = workspace();
    far_rect(&mut ws);
    ws.save_as(&dir.path().join("scene.xfl"));
    let stub = dir.path().join("scene").join("scene.xfl");
    assert_eq!(ws.path(), Some(stub.as_path()));
    assert!(dir.path().join("scene").join("DOMDocument.xml").is_file());
    far_rect(&mut ws);
    ws.save();
    assert!(!ws.is_dirty(), "{:?}", ws.status());
    assert_eq!(
        file_names(dir.path()),
        ["scene"],
        "nothing nested or left over"
    );
    let again = opened(&stub);
    assert_eq!(shapes(&again), 2);
}

#[test]
fn the_recent_list_holds_fla_files_and_reopens_them_after_a_restart() {
    let (dir, path) = fla_copy();
    let mut ws = opened(&path);
    let saved = dir.path().join("copy.fla");
    ws.save_as(&saved);
    assert_eq!(ws.recent_files(), [saved.clone(), path.clone()]);
    let json = ws.preferences_json().expect("json");
    let mut restarted = workspace();
    restarted.restore_preferences(&json).expect("restored");
    assert_eq!(restarted.recent_files(), [saved.clone(), path]);
    restarted.open(&saved);
    assert_eq!(restarted.path(), Some(saved.as_path()));
    assert!(!restarted.is_dirty());
}

#[test]
fn revert_goes_back_to_the_fla_on_disk() {
    let (_dir, path) = fla_copy();
    let mut ws = opened(&path);
    let drawn = shapes(&ws);
    far_rect(&mut ws);
    assert_eq!(shapes(&ws), drawn + 1, "{:?}", ws.status());
    ws.perform(MenuAction::Revert);
    assert_eq!(shapes(&ws), drawn);
    assert!(!ws.is_dirty());
    assert_eq!(ws.path(), Some(path.as_path()));
}

#[test]
fn the_close_prompts_save_writes_the_documents_own_format() {
    let (dir, path) = fla_copy();
    let mut ws = opened(&path);
    far_rect(&mut ws);
    ws.perform(MenuAction::Close);
    assert_eq!(ws.close_prompt, Some(0));
    ws.apply_close_choice(0, Choice::Save);
    let ctx = egui::Context::default();
    ws.settle_inline();
    run(&mut ws, &ctx, 0.0);
    assert_eq!(file_names(dir.path()), ["sample.fla"], "no .flare appears");
    assert_eq!(shapes(&opened(&path)), 1, "the drawing was saved");
    assert_eq!(ws.tabs().len(), 1);
    assert_eq!(ws.path(), None, "the tab was closed after the save");
}

#[test]
fn autosave_works_for_a_fla_backed_document_and_a_save_removes_the_recovery_file() {
    let (dir, path) = fla_copy();
    let store = dir.path().join("recovery");
    let mut ws = opened(&path);
    ws.enable_recovery(store.clone());
    ws.set_autosave_interval(1.0);
    let file = flare_storage::recovery_path(&store, ws.document().id());
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    far_rect(&mut ws);
    run(&mut ws, &ctx, 5.0);
    assert!(file.exists(), "a recovery snapshot is written for the .fla");
    assert!(
        flare_storage::load(&file).is_ok(),
        "and it is a project snapshot"
    );
    ws.save();
    assert!(!file.exists(), "saving supersedes it");
}
