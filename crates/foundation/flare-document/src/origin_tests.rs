#![allow(clippy::expect_used)]

use std::sync::Arc;

use super::*;

fn file(name: &str, bytes: &[u8]) -> OriginFile {
    OriginFile {
        name: name.into(),
        bytes: bytes.to_vec(),
    }
}

fn folder(files: Vec<OriginFile>) -> Result<Origin, OriginError> {
    Origin::new(
        OriginKind::Xfl,
        "P",
        OriginData::Files(files),
        Baseline::default(),
    )
}

#[test]
fn entry_names_must_stay_inside_the_document() {
    for ok in ["a", "a/b.xml", "a/", "LIBRARY/Symbol 1.xml", ".hidden"] {
        assert!(is_safe_entry_name(ok), "{ok}");
    }
    for bad in [
        "", "/", "/a", "../a", "a/../b", "a//b", "./a", "a\\b", "a\0b", "//",
    ] {
        assert!(!is_safe_entry_name(bad), "{bad:?}");
    }
    assert!(!is_safe_entry_name(&"a".repeat(MAX_ORIGIN_NAME_BYTES + 1)));
}

#[test]
fn a_source_is_checked_when_it_is_made() {
    assert!(folder(vec![file("a", b"x"), file("d/", b"")]).is_ok());
    assert_eq!(
        folder(vec![file("../x", b"")]).expect_err("refused"),
        OriginError::UnsafeName("../x".into())
    );
    assert_eq!(
        folder(vec![file("a", b""), file("a", b"")]).expect_err("refused"),
        OriginError::DuplicateName("a".into())
    );
    let many = (0..=MAX_ORIGIN_FILES)
        .map(|i| file(&format!("f{i}"), b""))
        .collect();
    assert_eq!(
        folder(many).expect_err("refused"),
        OriginError::TooManyFiles
    );
    // A single file cannot be a list and the other way round.
    assert!(matches!(
        Origin::new(
            OriginKind::Fla,
            "P",
            OriginData::Files(vec![]),
            Baseline::default()
        ),
        Err(OriginError::WrongData(..))
    ));
    assert!(matches!(
        Origin::new(
            OriginKind::Xfl,
            "P",
            OriginData::Bytes(vec![]),
            Baseline::default()
        ),
        Err(OriginError::WrongData(..))
    ));
    // A name with a slash would make the stub's path a lie.
    assert!(matches!(
        Origin::new(
            OriginKind::Swf,
            "a/b",
            OriginData::Bytes(vec![]),
            Baseline::default()
        ),
        Err(OriginError::UnsafeName(_))
    ));
}

#[test]
fn a_source_over_the_limit_is_refused_without_being_read() {
    // Zeroed memory is not touched until it is read, and the size is checked before any hashing.
    let too_big = vec![0_u8; MAX_ORIGIN_BYTES + 1];
    assert_eq!(
        Origin::new(
            OriginKind::Fla,
            "P",
            OriginData::Bytes(too_big),
            Baseline::default()
        )
        .expect_err("refused"),
        OriginError::TooLarge(MAX_ORIGIN_BYTES + 1)
    );
}

#[test]
fn the_digest_follows_the_content_and_the_names() {
    let a = folder(vec![file("a", b"1"), file("b", b"2")]).expect("ok");
    let same = folder(vec![file("a", b"1"), file("b", b"2")]).expect("ok");
    let moved_byte = folder(vec![file("a", b"12"), file("b", b"")]).expect("ok");
    let renamed = folder(vec![file("a", b"1"), file("c", b"2")]).expect("ok");
    assert_eq!(a.digest(), same.digest());
    assert_eq!(a, same);
    assert_ne!(a.digest(), moved_byte.digest(), "where a file ends matters");
    assert_ne!(a.digest(), renamed.digest());
    let stored = Origin::from_stored(
        OriginKind::Xfl,
        "P",
        OriginData::Files(vec![file("a", b"1"), file("b", b"2")]),
        a.digest(),
        Baseline::default(),
    );
    assert_eq!(stored.expect("matches"), a);
    let wrong = Origin::from_stored(
        OriginKind::Xfl,
        "P",
        OriginData::Files(vec![file("a", b"1"), file("b", b"3")]),
        a.digest(),
        Baseline::default(),
    );
    assert_eq!(wrong.expect_err("refused"), OriginError::DigestMismatch);
}

#[test]
fn a_document_carries_its_source_and_compares_by_it() {
    let origin = Arc::new(folder(vec![file("a", b"1")]).expect("ok"));
    let plain = Document::new(
        flare_core::DocumentId::new(),
        flare_core::FrameRate::new(24, 1).expect("rate"),
        vec![crate::Scene::new(
            flare_core::SceneId::new(),
            "S",
            crate::Timeline::default(),
        )],
        vec![],
    )
    .expect("document");
    let with = plain.clone().with_origin(Some(origin.clone()));
    assert!(plain.origin().is_none() && with.origin().is_some());
    assert_ne!(plain, with);
    assert_eq!(with, with.clone());
    assert!(Arc::ptr_eq(with.origin().expect("o"), &origin));
}
