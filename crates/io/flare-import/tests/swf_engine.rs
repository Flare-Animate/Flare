//! Importing Flash movies whose scripts are written against a 3D engine: the import names the
//! engine and counts its embedded data, and leaves running the movie to Ruffle.

#![allow(clippy::expect_used)]

use flare_import::{Severity, import_swf_bytes};
use swf::avm2::types::{AbcFile, ConstantPool};
use swf::{DefineBinaryData, DoAbc2, DoAbc2Flag, SwfStr, Tag};

mod swf_support;
use swf_support::{bytes, header, moving_square};

/// An ABC payload whose constant pool carries `strings` and nothing else, serialised by the
/// swf crate's own writer so the reader is exercised on well-formed data.
fn abc(strings: &[&str]) -> Vec<u8> {
    let file = AbcFile {
        major_version: 46,
        minor_version: 16,
        constant_pool: ConstantPool {
            ints: vec![],
            uints: vec![],
            doubles: vec![],
            strings: strings.iter().map(|s| s.as_bytes().to_vec()).collect(),
            namespaces: vec![],
            namespace_sets: vec![],
            multinames: vec![],
        },
        methods: vec![],
        metadata: vec![],
        instances: vec![],
        classes: vec![],
        scripts: vec![],
        method_bodies: vec![],
    };
    let mut out = Vec::new();
    swf::avm2::write::Writer::new(&mut out)
        .write(file)
        .expect("write abc");
    out
}

fn do_abc2<'a>(data: &'a [u8]) -> Tag<'a> {
    Tag::DoAbc2(DoAbc2 {
        flags: DoAbc2Flag::empty(),
        name: SwfStr::from_utf8_str("main"),
        data,
    })
}

fn script_reports(imported: &flare_import::Imported) -> Vec<&str> {
    imported
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info && d.location == "scripts")
        .map(|d| d.message.as_str())
        .collect()
}

#[test]
fn a_movie_names_the_3d_engine_its_scripts_are_written_against() {
    let payload = abc(&["flare.basic.Scene3D", "flare.core.Mesh3D"]);
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&payload), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(
        reports[0].contains("Flare3D engine") && reports[0].contains("flare.basic.Scene3D"),
        "{reports:?}"
    );
}

#[test]
fn an_alternativa3d_movie_is_reported_by_its_name() {
    let payload = abc(&["alternativa.engine3d.core.Camera3D", "alternativa.utils"]);
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&payload), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(reports[0].contains("Alternativa3D engine"), "{reports:?}");
}

#[test]
fn stage3d_is_reported_only_when_no_engine_names_it() {
    let alone = abc(&["flash.display3D.Context3D"]);
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&alone), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(reports[0].contains("Stage 3D"), "{reports:?}");

    let with_engine = abc(&["flare.basic.Scene3D", "flash.display3D.Context3D"]);
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&with_engine), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(reports[0].contains("Flare3D engine"), "{reports:?}");
}

#[test]
fn embedded_binary_data_is_counted_when_an_engine_is_present() {
    let payload = abc(&["flare.core.Pivot3D"]);
    let binary = Tag::DefineBinaryData(DefineBinaryData {
        id: 9,
        data: &[0, 1, 2],
    });
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[do_abc2(&payload), binary, Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert!(
        reports.iter().any(|m| m.contains("embedded binary data")),
        "{reports:?}"
    );
}

#[test]
fn an_unreadable_script_block_is_reported_and_does_not_stop_the_import() {
    let data = bytes(
        &header(100.0, 100.0, 12.0),
        &[Tag::DoAbc(&[0xff; 16]), Tag::ShowFrame, Tag::End],
    );
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(reports[0].contains("could not be read"), "{reports:?}");
    // The import itself still carried the movie over.
    assert_eq!(imported.document.scenes().len(), 1);
}

#[test]
fn only_the_first_script_blocks_are_scanned_and_the_rest_are_counted() {
    let payload = abc(&[]);
    let mut tags: Vec<Tag<'_>> = (0..40).map(|_| do_abc2(&payload)).collect();
    tags.push(Tag::ShowFrame);
    tags.push(Tag::End);
    let data = bytes(&header(100.0, 100.0, 12.0), &tags);
    let imported = import_swf_bytes(&data).expect("import");
    let reports = script_reports(&imported);
    assert_eq!(reports.len(), 1, "{:?}", imported.diagnostics);
    assert!(
        reports[0].contains("8 further script blocks were not scanned"),
        "{reports:?}"
    );
}

#[test]
fn a_movie_without_3d_scripts_gains_no_engine_report() {
    let imported = import_swf_bytes(&moving_square()).expect("import");
    assert!(
        imported
            .diagnostics
            .iter()
            .all(|d| !d.message.contains("engine")),
        "{:?}",
        imported.diagnostics
    );
}
