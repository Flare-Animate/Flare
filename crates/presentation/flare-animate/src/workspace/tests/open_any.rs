use super::*;
use crate::workspace::open_any::{PathKind, classify};
use std::path::Path;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../io/flare-import/tests/fixtures")
        .join(name)
}

/// A file as a window system hands it over.
#[derive(Debug)]
struct Dropped(PathBuf);

impl egui::DroppedFile for Dropped {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

fn dropped(path: &Path) -> egui::DroppedFileHandle {
    std::sync::Arc::new(Dropped(path.to_owned()))
}

#[test]
fn paths_are_told_apart_by_extension_in_any_case_and_folders_are_xfl() {
    let kind = |name: &str, dir: bool| classify(Path::new(name), dir);
    assert_eq!(kind("a.flare", false), Some(PathKind::Project));
    assert_eq!(kind("A.FLARE", false), Some(PathKind::Project));
    assert_eq!(kind("a.fla", false), Some(PathKind::Import));
    assert_eq!(kind("a.XFL", false), Some(PathKind::Import));
    assert_eq!(
        kind("MyProject", true),
        Some(PathKind::Import),
        "an XFL folder has any name"
    );
    assert_eq!(kind("a.png", false), None);
    assert_eq!(kind("a", false), None);
    assert_eq!(kind("a.flare.txt", false), None);
}

#[test]
fn a_flare_project_given_to_open_any_is_opened() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("Saved.flare");
    let mut source = workspace();
    rect(&mut source);
    flare_storage::save(source.document(), &path).expect("save");
    let mut ws = workspace();
    ws.open_any(&path);
    assert_eq!(ws.path(), Some(path.as_path()));
    assert_eq!(layer_count(&ws), 2);
}

#[test]
fn an_animate_file_given_to_open_any_is_imported_as_an_unsaved_project() {
    let mut ws = workspace();
    ws.open_any(&fixture("sample.fla"));
    assert!(
        ws.status().is_some_and(|s| s.starts_with("Imported")),
        "{:?}",
        ws.status()
    );
    assert!(
        ws.path().is_none(),
        "an import is a new project, not the .fla itself"
    );
    assert!(ws.is_dirty());
    let mut folder = workspace();
    folder.open_any(&fixture("sample_xfl"));
    assert!(
        folder.status().is_some_and(|s| s.starts_with("Imported")),
        "{:?}",
        folder.status()
    );
    assert!(folder.is_dirty());
}

#[test]
fn another_kind_of_file_is_reported_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("picture.png");
    std::fs::write(&path, b"not a project").expect("write");
    let mut ws = workspace();
    ws.open_any(&path);
    assert!(ws.status().is_some_and(|s| s.contains("picture.png")));
    assert!(!ws.is_dirty());
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn files_dropped_on_the_window_are_opened() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("Dropped.flare");
    let mut source = workspace();
    rect(&mut source);
    flare_storage::save(source.document(), &path).expect("save");
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let mut raw = input(0.0, vec![]);
    raw.dropped_files = vec![dropped(&path)];
    pass(&mut ws, &ctx, raw);
    assert_eq!(ws.path(), Some(path.as_path()));
}

#[test]
fn a_drop_of_too_many_or_unknown_files_is_harmless() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let mut raw = input(1.0, vec![]);
    raw.dropped_files = (0..100)
        .map(|i| dropped(Path::new(&format!("/nowhere/{i}.png"))))
        .collect();
    pass(&mut ws, &ctx, raw);
    assert!(ws.status().is_some());
}
