//! Crash recovery files.
//!
//! Autosave writes the current document to a separate recovery store, never over the
//! user's project file, so "discard changes" really discards them. A recovery file is a
//! normal container (same atomic write, same untrusted-load checks) named after the
//! document ID, so each document has at most one. It is removed when the work is saved or
//! deliberately discarded.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use flare_core::DocumentId;
use flare_document::Document;

use crate::{StorageError, load, save};

const SUFFIX: &str = ".recovery.flare";

/// A usable recovery file found in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryEntry {
    pub path: PathBuf,
    pub modified: SystemTime,
}

pub fn recovery_path(dir: &Path, id: DocumentId) -> PathBuf {
    dir.join(format!("{}{SUFFIX}", id.as_uuid()))
}

/// Writes (replacing) the recovery file for `document`.
pub fn save_recovery(document: &Document, dir: &Path) -> Result<PathBuf, StorageError> {
    fs::create_dir_all(dir)?;
    let path = recovery_path(dir, document.id());
    save(document, &path)?;
    Ok(path)
}

/// Removes a document's recovery file. A missing file is fine.
pub fn discard_recovery(dir: &Path, id: DocumentId) -> io::Result<()> {
    match fs::remove_file(recovery_path(dir, id)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Recovery files that still load, newest first. Corrupt or foreign files are skipped, never
/// deleted: they may be worth inspecting, and a crash must not cost more data.
pub fn list_recoveries(dir: &Path) -> Vec<RecoveryEntry> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<RecoveryEntry> = read
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(SUFFIX))
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            load(&e.path()).ok()?;
            Some(RecoveryEntry {
                path: e.path(),
                modified,
            })
        })
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.modified));
    entries
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use flare_core::{FrameRate, LayerId, SceneId};
    use flare_document::{Layer, Scene, Timeline};

    use super::*;

    fn document() -> Document {
        let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
        let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
        let rate = FrameRate::new(24, 1).expect("rate");
        Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc")
    }

    #[test]
    fn recovery_files_round_trip_and_are_replaced_in_place() {
        let dir = tempfile::tempdir().expect("dir");
        let doc = document();
        let path = save_recovery(&doc, dir.path()).expect("save");
        assert_eq!(path, recovery_path(dir.path(), doc.id()));
        assert_eq!(load(&path).expect("load"), doc);
        save_recovery(&doc, dir.path()).expect("save again");
        assert_eq!(
            list_recoveries(dir.path()).len(),
            1,
            "one file per document"
        );
    }

    #[test]
    fn the_store_is_created_on_demand_and_listing_a_missing_store_is_empty() {
        let dir = tempfile::tempdir().expect("dir");
        let store = dir.path().join("a").join("b");
        assert!(list_recoveries(&store).is_empty());
        save_recovery(&document(), &store).expect("creates the directories");
        assert_eq!(list_recoveries(&store).len(), 1);
    }

    #[test]
    fn corrupt_and_unrelated_files_are_skipped_but_never_deleted() {
        let dir = tempfile::tempdir().expect("dir");
        let good = document();
        save_recovery(&good, dir.path()).expect("save");
        let broken = dir.path().join(format!("{}{SUFFIX}", uuid_text()));
        fs::write(
            &broken,
            b"not a database at all, just text that is long enough....",
        )
        .expect("w");
        let unrelated = dir.path().join("notes.txt");
        fs::write(&unrelated, b"hello").expect("w");
        let found = list_recoveries(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, recovery_path(dir.path(), good.id()));
        assert!(broken.exists() && unrelated.exists());
    }

    fn uuid_text() -> String {
        DocumentId::new().as_uuid().to_string()
    }

    #[test]
    fn discarding_removes_only_that_document_and_tolerates_a_missing_file() {
        let dir = tempfile::tempdir().expect("dir");
        let (a, b) = (document(), document());
        save_recovery(&a, dir.path()).expect("a");
        save_recovery(&b, dir.path()).expect("b");
        discard_recovery(dir.path(), a.id()).expect("discard");
        discard_recovery(dir.path(), a.id()).expect("already gone");
        let left = list_recoveries(dir.path());
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].path, recovery_path(dir.path(), b.id()));
    }

    #[test]
    fn newest_recovery_comes_first() {
        let dir = tempfile::tempdir().expect("dir");
        let (old, new) = (document(), document());
        let old_path = save_recovery(&old, dir.path()).expect("old");
        let file = fs::File::options()
            .write(true)
            .open(&old_path)
            .expect("open");
        file.set_modified(SystemTime::UNIX_EPOCH).expect("backdate");
        save_recovery(&new, dir.path()).expect("new");
        let found = list_recoveries(dir.path());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].path, recovery_path(dir.path(), new.id()));
    }

    #[test]
    fn a_failed_recovery_write_leaves_the_previous_one_intact() {
        let dir = tempfile::tempdir().expect("dir");
        let doc = document();
        let path = save_recovery(&doc, dir.path()).expect("save");
        let before = fs::read(&path).expect("read");
        // A file where the store directory should be makes the next write fail.
        let blocked = dir.path().join("blocked");
        fs::write(&blocked, b"x").expect("w");
        assert!(save_recovery(&doc, &blocked).is_err());
        assert_eq!(fs::read(&path).expect("read"), before);
    }
}
