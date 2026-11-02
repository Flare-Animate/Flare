//! The real files vendored with Ruffle: opening each in Flare leaves it untouched, and exporting
//! the unchanged import writes it back byte for byte. The counts are printed so they can be read
//! (`cargo test -p flare-publish --test lossless_corpus -- --nocapture`).
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::io::Read;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use flare_commands::Command;
use flare_export::{export_xfl, fla_bytes, write_directory};
use flare_history::EditorSession;
use flare_import::import_path;
use flare_publish::{PublishOptions, publish_swf};

fn corpus() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../third_party/ruffle");
    if root.join("swf").is_dir() {
        Some(root)
    } else {
        eprintln!(
            "SKIPPED: {} is not checked out, there is no corpus to read",
            root.display()
        );
        None
    }
}

fn find(root: &Path, extension: &str) -> Vec<PathBuf> {
    fn walk(at: &Path, extension: &str, out: &mut Vec<PathBuf>) {
        let Ok(list) = std::fs::read_dir(at) else {
            return;
        };
        for entry in list.flatten() {
            let path = entry.path();
            let kind = entry.file_type().expect("type");
            if kind.is_dir() {
                walk(&path, extension, out);
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case(extension))
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, extension, &mut out);
    out.sort();
    out
}

/// What a read could not have changed: bytes, size, time and permissions.
fn stamp(path: &Path) -> (Vec<u8>, u64, Option<std::time::SystemTime>, bool) {
    let meta = std::fs::metadata(path).expect("meta");
    (
        std::fs::read(path).expect("read"),
        meta.len(),
        meta.modified().ok(),
        meta.permissions().readonly(),
    )
}

fn head(path: &Path) -> [u8; 4] {
    let mut head = [0_u8; 4];
    let mut file = std::fs::File::open(path).expect("open");
    let _ = file.read(&mut head).expect("read");
    head
}

#[derive(Default)]
struct Counts {
    files: usize,
    imported: usize,
    refused: BTreeMap<String, usize>,
    identical: usize,
    panicked: Vec<String>,
    mutated: Vec<String>,
}

impl Counts {
    fn refuse(&mut self, why: &str) {
        let short: String = why.chars().take(70).collect();
        *self.refused.entry(short).or_default() += 1;
    }

    fn report(&self, what: &str) {
        let refused: usize = self.refused.values().sum();
        eprintln!(
            "{what}: {} files, {} imported, {refused} refused, {} byte-identical on export",
            self.files, self.imported, self.identical
        );
        for (why, n) in &self.refused {
            eprintln!("    refused x{n}: {why}");
        }
    }

    /// `allowed` are the kinds of refusal that are justified for this format: a refused file is
    /// damaged or not what its name says, never a good file Flare gave up on.
    fn check(&self, allowed: &[&str]) {
        for why in self.refused.keys() {
            assert!(
                allowed.iter().any(|a| why.starts_with(a)),
                "an unexplained refusal: {why}"
            );
        }
        assert!(self.panicked.is_empty(), "panicked on {:?}", self.panicked);
        assert!(
            self.mutated.is_empty(),
            "a source changed: {:?}",
            self.mutated
        );
        assert_eq!(
            self.identical, self.imported,
            "every import exports as the original"
        );
    }
}

/// The entries of a zip that are safe to unpack, by name.
fn safe_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else {
            continue;
        };
        let Ok(name) = entry.name().map(|n| n.into_owned()) else {
            continue;
        };
        if !flare_document::is_safe_entry_name(&name) || entry.is_dir() {
            continue;
        }
        let mut body = Vec::new();
        if entry.read_to_end(&mut body).is_ok() {
            out.push((name, body));
        }
    }
    out
}

#[test]
fn every_zipped_fla_in_the_corpus_imports_untouched_and_exports_as_itself() {
    let Some(root) = corpus() else { return };
    let (mut zips, mut legacy, mut other) = (Vec::new(), 0, 0);
    for path in find(&root, "fla") {
        match head(&path) {
            [b'P', b'K', 3, 4] => zips.push(path),
            [0xd0, 0xcf, 0x11, 0xe0] => legacy += 1,
            _ => other += 1,
        }
    }
    let mut counts = Counts::default();
    let (mut carried_ok, mut carried_total) = (0, 0);
    for path in &zips {
        counts.files += 1;
        let before = stamp(path);
        let result = catch_unwind(AssertUnwindSafe(|| import_path(path)));
        if stamp(path) != before {
            counts.mutated.push(path.display().to_string());
        }
        let imported = match result {
            Err(_) => {
                counts.panicked.push(path.display().to_string());
                continue;
            }
            Ok(Err(error)) => {
                counts.refuse(&error.to_string());
                continue;
            }
            Ok(Ok(imported)) => imported,
        };
        counts.imported += 1;
        let document = imported.document;
        let output = export_xfl(&document).expect("export");
        if output.lossless.exact && fla_bytes(&output).expect("zip") == before.0 {
            counts.identical += 1;
        } else {
            eprintln!(
                "NOT IDENTICAL: {}: {}",
                path.display(),
                output.lossless.reason
            );
        }

        // An edit to the scene keeps every other file of the archive as it was, and undoing it
        // gives the file back.
        let mut session = EditorSession::new(document);
        let scene = session.document().scenes()[0].id();
        session
            .apply(vec![Command::RenameScene {
                scene,
                name: "Edited by the corpus test".into(),
            }])
            .expect("renamed");
        let edited = export_xfl(session.document()).expect("export");
        let after: BTreeMap<String, Vec<u8>> = safe_entries(&fla_bytes(&edited).expect("zip"))
            .into_iter()
            .collect();
        carried_total += 1;
        let all_but_main = safe_entries(&before.0)
            .into_iter()
            .filter(|(name, _)| name != "DOMDocument.xml")
            .all(|(name, body)| after.get(&name) == Some(&body));
        if all_but_main && !edited.lossless.exact {
            carried_ok += 1;
        } else {
            eprintln!(
                "NOT CARRIED: {}: {}",
                path.display(),
                edited.lossless.reason
            );
        }
        assert!(session.undo());
        assert_eq!(
            fla_bytes(&export_xfl(session.document()).expect("export")).expect("zip"),
            before.0,
            "{}: undo",
            path.display()
        );
    }
    counts.report("zipped .fla");
    eprintln!("    legacy binary .fla (a skeleton, no source kept): {legacy}; other: {other}");
    eprintln!(
        "    after a scene rename, every other entry carried untouched: {carried_ok}/{carried_total}"
    );
    counts.check(&[]);
    assert_eq!(counts.imported, counts.files, "every zipped .fla imports");
    assert_eq!(carried_ok, carried_total);
    assert!(counts.imported > 0);
}

#[test]
fn every_zipped_fla_unpacked_as_a_folder_imports_untouched_and_exports_as_itself() {
    let Some(root) = corpus() else { return };
    let mut counts = Counts::default();
    for path in find(&root, "fla") {
        if head(&path) != [b'P', b'K', 3, 4] {
            continue;
        }
        counts.files += 1;
        let bytes = std::fs::read(&path).expect("read");
        let dir = tempfile::tempdir().expect("dir");
        let project = dir.path().join("Project");
        let entries = safe_entries(&bytes);
        let mut expected = BTreeMap::new();
        for (name, body) in &entries {
            let target = project.join(name);
            std::fs::create_dir_all(target.parent().expect("parent")).expect("dir");
            std::fs::write(&target, body).expect("file");
            expected.insert(name.clone(), body.clone());
        }
        if entries.is_empty() {
            counts.refuse("no safe entries to unpack");
            continue;
        }
        let before = tree(&project);
        let result = catch_unwind(AssertUnwindSafe(|| import_path(&project)));
        // Neither the folder nor anything beside it (the scratch directory) may have changed.
        if tree(&project) != before || tree(dir.path()).len() != before.len() {
            counts.mutated.push(path.display().to_string());
        }
        let imported = match result {
            Err(_) => {
                counts.panicked.push(path.display().to_string());
                continue;
            }
            Ok(Err(error)) => {
                counts.refuse(&error.to_string());
                continue;
            }
            Ok(Ok(imported)) => imported,
        };
        counts.imported += 1;
        let output = export_xfl(&imported.document).expect("export");
        let out = dir.path().join("Out");
        write_directory(&output, &out).expect("written");
        if output.lossless.exact && tree(&out) == before {
            counts.identical += 1;
        } else {
            eprintln!(
                "NOT IDENTICAL (folder): {}: {}",
                path.display(),
                output.lossless.reason
            );
        }
    }
    counts.report("unpacked XFL folders");
    counts.check(&[]);
    assert_eq!(
        counts.imported, counts.files,
        "every unpacked project imports"
    );
}

fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let mut any = false;
        for entry in std::fs::read_dir(at).expect("list").flatten() {
            any = true;
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let name = path
                    .strip_prefix(root)
                    .expect("in")
                    .to_string_lossy()
                    .into_owned();
                out.insert(name, std::fs::read(&path).expect("read"));
            }
        }
        if !any && at != root {
            out.insert(
                format!("{}/", at.strip_prefix(root).expect("in").display()),
                Vec::new(),
            );
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn every_swf_in_the_corpus_imports_untouched_and_publishes_as_itself() {
    let Some(root) = corpus() else { return };
    let mut counts = Counts::default();
    let mut edited_differs = 0;
    for path in find(&root, "swf") {
        counts.files += 1;
        let before = stamp(&path);
        let result = catch_unwind(AssertUnwindSafe(|| import_path(&path)));
        if stamp(&path) != before {
            counts.mutated.push(path.display().to_string());
        }
        let imported = match result {
            Err(_) => {
                counts.panicked.push(path.display().to_string());
                continue;
            }
            Ok(Err(error)) => {
                counts.refuse(&error.to_string());
                continue;
            }
            Ok(Ok(imported)) => imported,
        };
        counts.imported += 1;
        let published = publish_swf(&imported.document, &PublishOptions::default());
        match published {
            Ok(p) if p.original && p.bytes == before.0 => counts.identical += 1,
            Ok(p) => eprintln!(
                "NOT IDENTICAL: {} (original: {})",
                path.display(),
                p.original
            ),
            Err(error) => eprintln!("PUBLISH FAILED: {}: {error}", path.display()),
        }
        // Changed, it is built again, and unchanged again it is the movie.
        let mut session = EditorSession::new(imported.document);
        let scene = session.document().scenes()[0].id();
        session
            .apply(vec![Command::RenameScene {
                scene,
                name: "Edited".into(),
            }])
            .expect("renamed");
        if let Ok(p) = publish_swf(session.document(), &PublishOptions::default())
            && !p.original
            && p.bytes != before.0
        {
            edited_differs += 1;
        }
        assert!(session.undo());
        if let Ok(p) = publish_swf(session.document(), &PublishOptions::default()) {
            assert!(
                p.original && p.bytes == before.0,
                "{}: undo",
                path.display()
            );
        }
    }
    counts.report(".swf");
    eprintln!(
        "    an edited import is rebuilt, not the original: {edited_differs}/{}",
        counts.imported
    );
    // Truncated movies, tags cut short on purpose, and movies whose header holds a stage or frame
    // rate that cannot exist are refused; so are the few `.swf` names that are not movies.
    counts.check(&[
        "invalid frame rate",
        "invalid stage",
        "not a Flash movie",
        "not an XFL document",
        "the movie once decompressed is larger",
    ]);
    assert_eq!(edited_differs, counts.imported);
    assert!(counts.imported > 0);
}
