//! The legacy `.fla` reader against the genuine legacy files in the vendored Ruffle tests, whose
//! sibling `.swf` (compiled from the same document) is the ground truth for stage, frame rate and
//! background. Run with `--nocapture` to see the table.
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use flare_import::legacy::{self, Era, ItemKind};
use flare_import::{import_path, import_swf_bytes};

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../third_party/ruffle")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "fla")
            && fs::read(&path).is_ok_and(|b| b.starts_with(&legacy::CFB_MAGIC))
        {
            out.push(path);
        }
    }
}

/// Every legacy `.fla` in the vendored suite, or `None` when the suite is not there.
fn corpus() -> Option<Vec<PathBuf>> {
    let root = corpus_root();
    if !root.is_dir() {
        eprintln!("skipped: {} is absent", root.display());
        return None;
    }
    let mut files = Vec::new();
    collect(&root, &mut files);
    files.sort();
    Some(files)
}

struct Truth {
    width: u32,
    height: u32,
    rate: f64,
    background: [u8; 3],
    buttons: usize,
    sprites: usize,
}

fn truth(fla: &Path) -> Option<Truth> {
    let bytes = fs::read(fla.with_extension("swf")).ok()?;
    let imported = import_swf_bytes(&bytes).ok()?;
    let stage = imported.document.stage();
    let buf = swf::decompress_swf(&bytes[..]).ok()?;
    let movie = swf::parse_swf(&buf).ok()?;
    let (mut buttons, mut sprites) = (0, 0);
    for tag in &movie.tags {
        match tag {
            swf::Tag::DefineButton(_) | swf::Tag::DefineButton2(_) => buttons += 1,
            swf::Tag::DefineSprite(_) => sprites += 1,
            _ => {}
        }
    }
    let bg = stage.background();
    Some(Truth {
        width: stage.width(),
        height: stage.height(),
        rate: imported.document.frame_rate().per_second(),
        background: [bg.r, bg.g, bg.b],
        buttons,
        sprites,
    })
}

#[test]
fn the_corpus_agrees_with_its_movies() {
    let Some(files) = corpus() else { return };
    assert!(
        files.len() >= 100,
        "expected the vendored suite, found {}",
        files.len()
    );
    let (mut with_swf, mut agree_size_rate, mut agree_bg) = (0, 0, 0);
    let (mut unreadable, mut eras) = (Vec::new(), [0_usize; 3]);
    let mut disagreements = Vec::new();
    let (mut button_files, mut button_ok, mut sprite_files, mut sprite_ok) = (0, 0, 0, 0);
    for path in &files {
        let name = path
            .strip_prefix(corpus_root())
            .unwrap_or(path)
            .display()
            .to_string();
        let bytes = fs::read(path).expect("read");
        let legacy = legacy::read(&bytes).expect("a container that opens");
        eras[match legacy.info.era {
            Era::Numbered => 0,
            Era::Stamped => 1,
            Era::Unknown => 2,
        }] += 1;
        let Ok(props) = legacy.properties else {
            unreadable.push(name);
            continue;
        };
        let Some(truth) = truth(path) else { continue };
        with_swf += 1;
        let size = (props.width, props.height) == (truth.width, truth.height);
        let rate = (props.frame_rate - truth.rate).abs() < 0.01;
        let bg = props.background == truth.background;
        agree_size_rate += usize::from(size && rate);
        agree_bg += usize::from(bg);
        if !(size && rate && bg) {
            disagreements.push(format!(
                "{name}: fla {}x{} @{} bg {:?}; swf {}x{} @{} bg {:?}",
                props.width,
                props.height,
                props.frame_rate,
                props.background,
                truth.width,
                truth.height,
                truth.rate,
                truth.background
            ));
        }
        let kinds = |k: ItemKind| legacy.library.iter().filter(|i| i.kind == k).count();
        if (truth.buttons > 0 && kinds(ItemKind::Button) == 0)
            || (truth.sprites > 0 && kinds(ItemKind::MovieClip) == 0)
        {
            eprintln!(
                "KINDS {name}: swf buttons {} sprites {}; library {:?}",
                truth.buttons, truth.sprites, legacy.library
            );
        }
        if truth.buttons > 0 {
            button_files += 1;
            button_ok += usize::from(kinds(ItemKind::Button) > 0);
        }
        if truth.sprites > 0 {
            sprite_files += 1;
            sprite_ok += usize::from(kinds(ItemKind::MovieClip) > 0);
        }
    }
    eprintln!(
        "legacy corpus: {} files; eras numbered/stamped/unknown {:?}; {} with a sibling swf; \
         size+rate agree {agree_size_rate}; background agrees {agree_bg}; settings unreadable {:?}; \
         swf has buttons {button_files} (library has one {button_ok}); swf has sprites {sprite_files} \
         (library has a movie clip {sprite_ok})",
        files.len(),
        eras,
        with_swf,
        unreadable
    );
    for line in &disagreements {
        eprintln!("DISAGREES {line}");
    }
    assert!(
        unreadable.is_empty(),
        "settings not found in {unreadable:?}"
    );
    assert!(
        with_swf >= 100,
        "only {with_swf} files have a movie to compare with"
    );
    assert!(
        agree_size_rate * 10 >= with_swf * 9,
        "size and rate agree in {agree_size_rate} of {with_swf}"
    );
    assert!(
        agree_bg * 10 >= with_swf * 9,
        "background agrees in {agree_bg} of {with_swf}"
    );
    // Not all: a movie also gets clips from compiled components (a "MenuBar" is a media item here,
    // not a symbol) and from files other than its document, so this is a floor, not an equality.
    assert!(
        button_ok * 10 >= button_files * 7,
        "buttons: {button_ok} of {button_files}"
    );
    assert!(
        sprite_ok * 10 >= sprite_files * 7,
        "movie clips: {sprite_ok} of {sprite_files}"
    );
}

#[test]
fn every_symbol_stream_is_found_in_the_library_table() {
    let Some(files) = corpus() else { return };
    for path in files {
        let bytes = fs::read(&path).expect("read");
        let legacy = legacy::read(&bytes).expect("opens");
        let unfound: Vec<_> = legacy.library.iter().filter(|i| !i.found).collect();
        assert!(unfound.is_empty(), "{}: {unfound:?}", path.display());
        assert!(
            legacy.library.iter().all(|i| i.kind != ItemKind::Unknown),
            "{}: a kind was not recognised in {:?}",
            path.display(),
            legacy.library
        );
        assert_eq!(
            legacy.library.len(),
            legacy.info.symbols + legacy.info.media
        );
    }
}

#[test]
fn a_known_file_reads_exactly() {
    let Some(_) = corpus() else { return };
    let path = corpus_root().join("swf/tests/swfs/DefineButtonCxformSound-MX.fla");
    let bytes = fs::read(path).expect("read");
    let legacy = legacy::read(&bytes).expect("opens");
    assert_eq!(legacy.info.era, Era::Numbered);
    assert_eq!(legacy.info.format_version, Some(0x129));
    assert_eq!(
        (legacy.info.pages, legacy.info.symbols, legacy.info.media),
        (1, 2, 1)
    );
    let props = legacy.properties.expect("settings");
    assert_eq!(
        (props.width, props.height, props.frame_rate),
        (550, 400, 12.0)
    );
    let items: Vec<_> = legacy
        .library
        .iter()
        .map(|i| (i.stream.as_str(), i.name.as_str(), i.kind))
        .collect();
    assert_eq!(
        items,
        [
            ("Symbol 1", "TestButton", ItemKind::Button),
            ("Media 1", "test-sound.wav", ItemKind::Sound),
            ("Symbol 3", "Test", ItemKind::Graphic),
        ]
    );
}

#[test]
fn import_path_opens_a_legacy_file_as_a_skeleton() {
    let Some(_) = corpus() else { return };
    let path = corpus_root().join("swf/tests/swfs/DefineButton-MX.fla");
    let imported = import_path(&path).expect("imports");
    let document = &imported.document;
    assert_eq!(
        (document.stage().width(), document.stage().height()),
        (550, 400)
    );
    assert_eq!(document.frame_rate().per_second(), 12.0);
    assert_eq!(document.scenes().len(), 1);
    assert_eq!(document.symbols().len(), 1);
    assert_eq!(document.symbols()[0].name(), "TestButton");
    assert_eq!(
        document.symbols()[0].kind(),
        flare_document::SymbolKind::Button
    );
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.severity == flare_import::Severity::Warning
                && d.message.contains("not read yet")),
        "{:?}",
        imported.diagnostics
    );
}
