use super::*;

#[test]
fn tampered_or_invented_attachments_are_refused_on_load() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let edit = |find: &str, replace: &str| {
        save(&sample(), &path).expect("save");
        let (id, payload) = attachment_rows(&path).into_iter().next().expect("a row");
        assert!(payload.contains(find), "{payload}");
        let conn = Connection::open(&path).expect("open");
        conn.execute(
            "UPDATE entity SET payload_json = ?1 WHERE id = ?2",
            [payload.replace(find, replace), id],
        )
        .expect("tamper");
        drop(conn);
        load(&path)
    };
    // Content changed but the digest was not: refused.
    assert!(matches!(
        edit("<media>", "<evil>"),
        Err(StorageError::Corrupt(_))
    ));
    // A digest that is not even hex.
    let digest = {
        save(&sample(), &path).expect("save");
        let payload = attachment_rows(&path).remove(0).1;
        let value: serde_json::Value = serde_json::from_str(&payload).expect("json");
        value["digest"].as_str().expect("digest").to_owned()
    };
    assert!(matches!(edit(&digest, "zz"), Err(StorageError::Corrupt(_))));
    // An anchor to a layer that does not exist is an invalid document, not a trusted one.
    save(&sample(), &path).expect("save");
    let rows = attachment_rows(&path);
    let (id, payload) = rows
        .iter()
        .find(|(_, p)| p.contains("\"Layer\""))
        .expect("layer note")
        .clone();
    let value: serde_json::Value = serde_json::from_str(&payload).expect("json");
    let real = value["anchor"]["id"].as_str().expect("id").to_owned();
    let conn = Connection::open(&path).expect("open");
    let moved = payload.replace(&real, "00000000-0000-0000-0000-00000000dead");
    conn.execute(
        "UPDATE entity SET payload_json = ?1 WHERE id = ?2",
        [moved, id],
    )
    .expect("tamper");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Invalid(_))));
}

#[test]
fn projects_without_attachments_still_load() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute("DELETE FROM entity WHERE kind = 'attachment'", [])
        .expect("strip");
    drop(conn);
    assert!(load(&path).expect("legacy load").attachments().is_empty());
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
fn older_files_without_sounds_load_and_bad_sound_rows_are_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    // No cues key and no sound rows: how every earlier file looks.
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute("DELETE FROM entity WHERE kind = 'sound'", [])
        .expect("strip");
    conn.execute(
        "UPDATE entity SET payload_json = replace(payload_json, '\"cues\":', '\"legacy\":')",
        [],
    )
    .expect("strip cues");
    drop(conn);
    let loaded = load(&path).expect("an old file");
    assert!(loaded.sounds().is_empty());

    // A cue naming a sound that is not in the file is a damaged file.
    save(&sample(), &path).expect("save");
    let conn = Connection::open(&path).expect("open");
    conn.execute("DELETE FROM entity WHERE kind = 'sound'", [])
        .expect("strip sound");
    drop(conn);
    assert!(matches!(load(&path), Err(StorageError::Invalid(_))));

    // Bad base64, and zero channels, are refused rather than trusted.
    for (from, to) in [
        ("\"data\":\"", "\"data\":\"!!!"),
        ("\"channels\":2", "\"channels\":0"),
    ] {
        save(&sample(), &path).expect("save");
        let (id, payload) = sound_row(&path);
        let conn = Connection::open(&path).expect("open");
        conn.execute(
            "UPDATE entity SET payload_json = ?1 WHERE id = ?2",
            params![payload.replace(from, to), id],
        )
        .expect("tamper");
        drop(conn);
        assert!(
            matches!(load(&path), Err(StorageError::Corrupt(_))),
            "{from}"
        );
    }
}
