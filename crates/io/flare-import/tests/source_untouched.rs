//! Opening a file never changes it, and never writes beside or inside it.
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use flare_import::{import_bytes, import_path};

#[derive(Debug, PartialEq, Eq)]
struct Seen {
    folder: bool,
    len: u64,
    modified: Option<SystemTime>,
    readonly: bool,
    bytes: Vec<u8>,
}

/// Everything about a tree that a write would change: names, kinds, sizes, times, permissions
/// and bytes, for the path itself and everything under it (and its parent's listing).
fn snapshot(path: &Path) -> BTreeMap<PathBuf, Seen> {
    fn walk(path: &Path, out: &mut BTreeMap<PathBuf, Seen>) {
        let meta = std::fs::symlink_metadata(path).expect("meta");
        let folder = meta.is_dir();
        out.insert(
            path.to_owned(),
            Seen {
                folder,
                len: if folder { 0 } else { meta.len() },
                modified: meta.modified().ok(),
                readonly: meta.permissions().readonly(),
                bytes: if folder || meta.file_type().is_symlink() {
                    Vec::new()
                } else {
                    std::fs::read(path).expect("read")
                },
            },
        );
        if folder {
            for entry in std::fs::read_dir(path).expect("list") {
                walk(&entry.expect("entry").path(), out);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(path, &mut out);
    out
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("dir");
    for entry in std::fs::read_dir(from).expect("list") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy");
        }
    }
}

/// A scratch directory holding a copy of every fixture, so the check can be made on files we may
/// stat and re-read without touching the repository.
fn scratch() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("dir");
    copy_tree(&fixtures(), dir.path());
    dir
}

#[test]
fn importing_any_fixture_leaves_the_file_or_folder_and_its_neighbours_untouched() {
    let dir = scratch();
    let before = snapshot(dir.path());
    let mut imported = 0;
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir.path())
        .expect("list")
        .map(|e| e.expect("entry").path())
        .collect();
    paths.sort();
    // The stub and the document file of a folder open the folder, too.
    for folder in ["sample_xfl", "handmade_xfl"] {
        for entry in std::fs::read_dir(dir.path().join(folder)).expect("list") {
            let path = entry.expect("entry").path();
            if path.is_file() {
                paths.push(path);
            }
        }
    }
    for path in &paths {
        if import_path(path).is_ok() {
            imported += 1;
        }
        assert_eq!(
            snapshot(dir.path()),
            before,
            "after importing {}",
            path.display()
        );
    }
    assert!(imported >= 5, "only {imported} of {} imported", paths.len());
}

#[cfg(unix)]
#[test]
fn a_source_that_cannot_be_written_still_imports() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch();
    let file = dir.path().join("sample.fla");
    let folder = dir.path().join("sample_xfl");
    for path in [&file, &folder, dir.path()] {
        let mode = if path.is_dir() { 0o555 } else { 0o444 };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
    }
    let before = snapshot(dir.path());
    let a = import_path(&file);
    let b = import_path(&folder);
    // Put the permissions back so the scratch directory can be removed.
    for path in [dir.path(), &folder, &file] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    assert!(a.is_ok() && b.is_ok());
    let after = snapshot(dir.path());
    assert_eq!(after.len(), before.len(), "nothing was created");
}

#[test]
fn importing_from_memory_keeps_the_bytes_it_was_given() {
    for name in [
        "sample.fla",
        "handmade.fla",
        "orphan_local.fla",
        "stale_trailer.fla",
    ] {
        let bytes = std::fs::read(fixtures().join(name)).expect("read");
        if let Ok(imported) = import_bytes(bytes.clone()) {
            let origin = imported.document.origin().expect("kept");
            assert_eq!(
                origin.data(),
                &flare_document::OriginData::Bytes(bytes),
                "{name}"
            );
        }
    }
}

#[test]
fn a_folder_is_read_once_so_the_source_and_the_document_agree() {
    let dir = scratch();
    let folder = dir.path().join("handmade_xfl");
    let imported = import_path(&folder).expect("imports");
    let origin = imported.document.origin().expect("kept");
    let flare_document::OriginData::Files(files) = origin.data() else {
        panic!("a folder is kept as files");
    };
    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "in a fixed order, not the disk's");
    assert!(names.contains(&"DOMDocument.xml"));
    for file in files {
        if !file.name.ends_with('/') {
            assert_eq!(
                std::fs::read(folder.join(&file.name)).expect("read"),
                file.bytes
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn a_folder_with_a_link_in_it_imports_but_says_its_original_was_not_kept() {
    let dir = scratch();
    let folder = dir.path().join("handmade_xfl");
    std::os::unix::fs::symlink("DOMDocument.xml", folder.join("alias.xml")).expect("link");
    let imported = import_path(&folder).expect("still imports");
    assert!(imported.document.origin().is_none());
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.location == "source" && d.message.contains("symbolic link")),
        "{:?}",
        imported.diagnostics
    );
}

#[test]
fn a_file_that_is_not_a_project_is_refused_and_keeps_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.fla");
    std::fs::write(&path, b"PK\x03\x04 but not a zip").expect("write");
    assert!(import_path(&path).is_err());
    assert_eq!(
        std::fs::read(&path).expect("read"),
        b"PK\x03\x04 but not a zip"
    );
    assert_eq!(std::fs::read_dir(dir.path()).expect("list").count(), 1);
}
