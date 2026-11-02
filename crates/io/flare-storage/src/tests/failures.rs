use super::*;

#[test]
fn tampered_or_invented_attachments_are_refused_on_load() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    // Content changed but the digest was not: refused.
    save(&sample(), &path).expect("save");
    edit_text(&path, &["attachment"], |t| t.replace("<media>", "<evil>"));
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
    // A digest that is not even hex.
    save(&sample(), &path).expect("save");
    edit_values(&path, &["attachment"], |v| {
        set_key(v, "digest", &serde_json::json!("zz"));
    });
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
    // An anchor to something that does not exist is an invalid document, not a trusted one.
    save(&sample(), &path).expect("save");
    edit_values(&path, &["attachment"], |v| {
        set_key(
            v,
            "id",
            &serde_json::json!("00000000-0000-0000-0000-00000000dead"),
        );
    });
    assert!(matches!(load(&path), Err(StorageError::Invalid(_))));
}

#[test]
fn projects_without_attachments_load() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute("DELETE FROM entity WHERE kind = 'attachment'", [])
        .expect("strip");
    drop(conn);
    assert!(load(&path).expect("load").attachments().is_empty());
}

#[test]
fn only_not_a_database_failures_are_reported_as_foreign_files() {
    let failure = |code| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None);
    assert!(matches!(
        not_flare(failure(rusqlite::ffi::SQLITE_NOTADB)),
        StorageError::NotFlareFile
    ));
    assert!(matches!(
        not_flare(failure(rusqlite::ffi::SQLITE_CORRUPT)),
        StorageError::NotFlareFile
    ));
    for code in [
        rusqlite::ffi::SQLITE_BUSY,
        rusqlite::ffi::SQLITE_PERM,
        rusqlite::ffi::SQLITE_IOERR,
        rusqlite::ffi::SQLITE_CANTOPEN,
    ] {
        assert!(
            matches!(not_flare(failure(code)), StorageError::Sqlite(_)),
            "code {code} is a real error, not a foreign file"
        );
    }
}

#[test]
fn sounds_and_cues_round_trip_byte_for_byte() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let document = sample();
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded.sounds(), document.sounds());
    assert_eq!(loaded.sounds()[0].data(), &[0, 1, 2, 254, 255]);
    assert_eq!(loaded.scenes()[0].timeline().layers()[0].cues().len(), 1);
}

#[test]
fn a_cue_for_a_missing_sound_and_a_damaged_sound_record_are_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    // A cue naming a sound that is not in the file is a damaged file.
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute("DELETE FROM entity WHERE kind = 'sound'", [])
        .expect("strip sound");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Invalid(_))));

    // Zero channels is refused rather than trusted.
    save(&sample(), &path).expect("save");
    edit_sound(&path, |v| set_key(v, "channels", &serde_json::json!(0)));
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));

    // So is a record too short to hold its fields, and one whose fields claim more than there is.
    for record in [
        &[1u8][..],
        &[0xff, 0xff, 0xff, 0xff, 1, 2, 3][..],
        &[8, 0, 0, 0, 1][..],
    ] {
        save(&sample(), &path).expect("save");
        set_raw(&path, "sound", record);
        assert!(
            matches!(load(&path), Err(StorageError::Corrupt(_))),
            "{record:?}"
        );
    }
}
