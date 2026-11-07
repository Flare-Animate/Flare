//! Real Wick Editor projects. Point `WICK_SAMPLES` at a folder of `.wick` files, in folders of
//! their own, and run `cargo test -p flare-import --test wick -- --ignored --nocapture`. Each file
//! is imported and one line is printed for it; the test fails if any file is refused. The files
//! are never run or written.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use flare_import::import_foreign_path;

fn wick_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(wick_files(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("wick") {
            found.push(path);
        }
    }
    found.sort();
    found
}

#[test]
#[ignore = "needs real .wick files in WICK_SAMPLES"]
fn every_real_wick_project_imports() {
    let root = std::env::var("WICK_SAMPLES").expect("set WICK_SAMPLES to a folder of .wick files");
    let files = wick_files(Path::new(&root));
    assert!(!files.is_empty(), "no .wick files in {root}");
    let mut refused = Vec::new();
    for path in &files {
        match import_foreign_path(path) {
            Ok(imported) => {
                let document = &imported.document;
                let unsupported = imported
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == flare_import::Severity::Unsupported)
                    .count();
                println!(
                    "OK {} symbols={} layers={} bitmaps={} attachments={} unsupported={}",
                    path.display(),
                    document.symbols().len(),
                    document.scenes()[0].timeline().layers().len(),
                    document.bitmaps().len(),
                    document.attachments().len(),
                    unsupported,
                );
            }
            Err(error) => {
                println!("REFUSED {}: {error}", path.display());
                refused.push(path.display().to_string());
            }
        }
    }
    println!("{} files, {} refused", files.len(), refused.len());
    assert!(refused.is_empty(), "refused: {refused:?}");
}
