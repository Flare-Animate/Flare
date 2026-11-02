//! The record of the file a project was imported from: kept, checked, and optional.

use super::*;
use crate::codec::{from_cbor, to_cbor};
use flare_document::{Baseline, Origin, OriginData, OriginFile, OriginKind, SymbolBaseline};
use std::sync::Arc;

fn with_origin(document: Document, kind: OriginKind, data: OriginData) -> Document {
    let print = fingerprint(&document).expect("fingerprint");
    let baseline = Baseline {
        main: print.main,
        symbols: print
            .symbols
            .iter()
            .map(|(id, digest)| SymbolBaseline {
                id: *id,
                name: "n".into(),
                digest: *digest,
                file: "LIBRARY/a.xml".into(),
            })
            .collect(),
    };
    let origin = Origin::new(kind, "Name", data, baseline).expect("origin");
    document.with_origin(Some(Arc::new(origin)))
}

fn zip_like() -> OriginData {
    OriginData::Bytes((0..5000_u32).map(|i| (i * 7) as u8).collect())
}

fn folder() -> OriginData {
    OriginData::Files(vec![
        OriginFile {
            name: "DOMDocument.xml".into(),
            bytes: b"<a/>".to_vec(),
        },
        OriginFile {
            name: "bin/x.dat".into(),
            bytes: vec![0, 1, 2, 255],
        },
        OriginFile {
            name: "empty/".into(),
            bytes: vec![],
        },
    ])
}

fn origin_row(path: &Path) -> super::records::Row {
    rows(path)
        .into_iter()
        .find(|r| r.kind == "origin")
        .expect("an origin row")
}

#[test]
fn a_source_is_saved_and_comes_back_with_the_same_content_and_baseline() {
    for (kind, data) in [
        (OriginKind::Fla, zip_like()),
        (OriginKind::Swf, zip_like()),
        (OriginKind::Xfl, folder()),
    ] {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        let document = with_origin(sample(), kind, data);
        save(&document, &path).expect("save");
        let loaded = load(&path).expect("load");
        let (a, b) = (document.origin().expect("o"), loaded.origin().expect("o"));
        assert_eq!(a.data(), b.data());
        assert_eq!(a.baseline(), b.baseline());
        assert_eq!(
            (a.kind(), a.name(), a.digest()),
            (b.kind(), b.name(), b.digest())
        );
        // And it is the same document, so its fingerprint is the baseline's.
        assert!(fingerprint(&loaded).expect("fp").matches(b.baseline()));
    }
}

#[test]
fn a_document_without_a_source_writes_no_record_and_one_from_before_still_loads() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    assert!(rows(&path).iter().all(|r| r.kind != "origin"));
    assert!(load(&path).expect("load").origin().is_none());

    // A project saved with a source and then stripped of it is a project from before sources.
    save(&with_origin(sample(), OriginKind::Fla, zip_like()), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute("DELETE FROM entity WHERE kind = 'origin'", [])
        .expect("strip");
    drop(conn);
    let loaded = load(&path).expect("loads without it");
    assert!(loaded.origin().is_none());
    assert_eq!(loaded.scenes().len(), sample().scenes().len());
}

#[test]
fn a_source_that_does_not_match_its_digest_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&with_origin(sample(), OriginKind::Fla, zip_like()), &path).expect("save");
    // A record whose own digest is right but whose bytes are not the source it names.
    let mut record = origin_row(&path).record;
    let last = record.len() - 1;
    record[last] ^= 0xff;
    set_raw(&path, "origin", &record);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(m)) if m.contains("digest")));
}

#[test]
fn a_record_whose_stored_digest_is_wrong_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&with_origin(sample(), OriginKind::Fla, zip_like()), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute(
        "UPDATE entity SET digest = zeroblob(32) WHERE kind = 'origin'",
        [],
    )
    .expect("damage");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn a_source_declaring_more_than_the_limit_is_refused_before_anything_is_allocated() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&with_origin(sample(), OriginKind::Fla, zip_like()), &path).expect("save");
    let too_big = crate::wire::MAX_ORIGIN_RECORD_BYTES as i64 + 1;
    let conn = Connection::open(&path).expect("open");
    // The payload is a few kilobytes; the record claims over 256 MiB of it.
    conn.execute(
        "UPDATE entity SET size = ?1 WHERE kind = 'origin'",
        [too_big],
    )
    .expect("damage");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Limit(_))));
    // Whereas a size within the limit that the bytes do not have is plain corruption.
    let conn = Connection::open(&path).expect("open");
    conn.execute("UPDATE entity SET size = 1000000 WHERE kind = 'origin'", [])
        .expect("damage");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}

#[test]
fn damaged_source_fields_are_refused_not_trusted() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&with_origin(sample(), OriginKind::Xfl, folder()), &path).expect("save");
    let good = origin_row(&path).record;
    let meta_len = u32::from_le_bytes(good[..4].try_into().expect("four")) as usize;
    let meta: serde_json::Value = from_cbor(&good[4..4 + meta_len]).expect("cbor");
    let reframe = |meta: &serde_json::Value, payload: &[u8]| {
        let meta = to_cbor(meta).expect("cbor");
        let mut out = (meta.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(&meta);
        out.extend_from_slice(payload);
        out
    };
    let payload = &good[4 + meta_len..];

    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("too short", vec![1, 2]),
        ("fields that do not fit", {
            let mut r = vec![0xff, 0xff, 0xff, 0x7f];
            r.extend_from_slice(b"abc");
            r
        }),
        ("truncated bytes", good[..good.len() - 3].to_vec()),
        ("extra bytes", [good.clone(), vec![9, 9]].concat()),
        ("a file longer than the bytes", {
            let mut m = meta.clone();
            m["files"][0]["len"] = serde_json::json!(1_000_000_000_000_u64);
            reframe(&m, payload)
        }),
        ("an unsafe name", {
            let mut m = meta.clone();
            m["files"][0]["name"] = serde_json::json!("../escape");
            reframe(&m, payload)
        }),
        ("a repeated name", {
            let mut m = meta.clone();
            m["files"][1]["name"] = m["files"][0]["name"].clone();
            reframe(&m, payload)
        }),
        ("an unknown kind", {
            let mut m = meta.clone();
            m["kind"] = serde_json::json!("rar");
            reframe(&m, payload)
        }),
        ("a bad digest", {
            let mut m = meta.clone();
            m["digest"] = serde_json::json!("zz");
            reframe(&m, payload)
        }),
        ("a bad symbol id", {
            let mut m = meta.clone();
            m["symbols"] =
                serde_json::json!([{"id": "nope", "name": "", "digest": "00", "file": ""}]);
            reframe(&m, payload)
        }),
        ("a kind that wants bytes with files", {
            let mut m = meta.clone();
            m["kind"] = serde_json::json!("fla");
            reframe(&m, payload)
        }),
    ];
    for (what, record) in cases {
        save(&with_origin(sample(), OriginKind::Xfl, folder()), &path).expect("save");
        set_raw(&path, "origin", &record);
        let result = load(&path);
        assert!(
            matches!(
                result,
                Err(StorageError::Corrupt(_) | StorageError::Limit(_))
            ),
            "{what}: {result:?}"
        );
    }
}

#[test]
fn two_source_records_are_refused() {
    let mut zstd = NativeZstd::new().expect("zstd");
    let mut parts = to_parts(
        &with_origin(sample(), OriginKind::Fla, zip_like()),
        &mut zstd,
    )
    .expect("parts");
    let copy = parts
        .records
        .iter()
        .find(|r| r.kind == "origin")
        .expect("origin")
        .clone();
    parts.records.push(crate::Record {
        id: "second".into(),
        ordinal: 1,
        ..copy
    });
    assert!(matches!(
        from_parts(parts, &mut zstd),
        Err(StorageError::Corrupt(_))
    ));
}

#[test]
fn the_fingerprint_is_stable_and_sees_every_part() {
    let document = sample();
    let a = fingerprint(&document).expect("fp");
    assert_eq!(a, fingerprint(&document.clone()).expect("fp"));
    // Through a save and a load it is the same.
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&document, &path).expect("save");
    assert_eq!(a, fingerprint(&load(&path).expect("load")).expect("fp"));
    // A different stage changes the main part and no symbol's.
    let moved = document
        .clone()
        .with_stage(Stage::new(77, 66, Rgba8::new(1, 2, 3, 255)).expect("stage"));
    let b = fingerprint(&moved).expect("fp");
    assert_ne!(a.main, b.main);
    assert_eq!(a.symbols, b.symbols);
    // A renamed symbol changes the main part (its name is listed there) and its own part.
    let symbol = document.symbols()[0].clone();
    let renamed = Document::new(
        document.id(),
        document.frame_rate(),
        document.scenes().to_vec(),
        vec![symbol.clone().with_name("other")],
    );
    if let Ok(renamed) = renamed {
        let c = fingerprint(&renamed.with_stage(document.stage())).expect("fp");
        assert_ne!(a.main, c.main);
        assert_ne!(a.symbols[0].1, c.symbols[0].1);
    }
}
