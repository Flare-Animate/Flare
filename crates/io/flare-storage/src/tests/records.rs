//! Reading and rewriting the stored records, for the tests that damage a file on purpose.
//!
//! A record is CBOR (maybe compressed) with a digest, so a test cannot just run text replacement
//! over a column. These helpers decode each record to JSON, let the test change it, and write it
//! back as a valid record (fresh digest, size and codec): what comes out is damaged in the way the
//! test chose and in no other way.

use super::*;
use crate::codec::{NativeZstd, Packer, from_cbor, to_cbor, unpack};
use serde_json::Value;
use std::path::Path;

/// One stored record, decoded.
pub(super) struct Row {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) record: Vec<u8>,
}

fn limit(kind: &str) -> usize {
    if kind == "sound" {
        MAX_SOUND_PAYLOAD_BYTES
    } else {
        MAX_PAYLOAD_BYTES
    }
}

pub(super) fn rows(path: &Path) -> Vec<Row> {
    let conn = Connection::open(path).expect("open");
    let mut statement = conn
        .prepare("SELECT id, kind, codec, size, digest, payload FROM entity ORDER BY kind, ordinal")
        .expect("prepare");
    statement
        .query_map([], |row| {
            let kind: String = row.get(1)?;
            let record = unpack(
                &mut NativeZstd::new().expect("zstd"),
                &row.get::<_, String>(2)?,
                row.get(3)?,
                &row.get::<_, Vec<u8>>(4)?,
                &row.get::<_, Vec<u8>>(5)?,
                limit(&kind),
            )
            .expect("a stored record that unpacks");
            Ok(Row {
                id: row.get(0)?,
                kind,
                record,
            })
        })
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

/// The records of `kind` (not sound) as JSON text, with their ids.
pub(super) fn json(path: &Path, kind: &str) -> Vec<(String, String)> {
    rows(path)
        .into_iter()
        .filter(|row| row.kind == kind)
        .map(|row| {
            let value: Value = from_cbor(&row.record).expect("cbor");
            (row.id, value.to_string())
        })
        .collect()
}

/// Stores `record` as row `id`'s payload, properly packed and digested.
pub(super) fn put_record(path: &Path, id: &str, record: &[u8]) {
    let conn = Connection::open(path).expect("open");
    let mut zstd = NativeZstd::new().expect("zstd");
    let stored = Packer::new(&mut zstd).pack(record).expect("pack");
    conn.execute(
        "UPDATE entity SET codec = ?1, size = ?2, digest = ?3, payload = ?4 WHERE id = ?5",
        params![
            stored.codec.name(),
            stored.size as i64,
            stored.digest.as_slice(),
            stored.bytes.as_ref(),
            id
        ],
    )
    .expect("update");
}

/// Changes every record of the given kinds (not sound) as JSON. Panics if nothing changed, so a
/// test cannot pass because its edit missed.
pub(super) fn edit_values(path: &Path, kinds: &[&str], edit: impl Fn(&mut Value)) {
    let mut changed = false;
    for row in rows(path) {
        if !kinds.contains(&row.kind.as_str()) {
            continue;
        }
        let before: Value = from_cbor(&row.record).expect("cbor");
        let mut after = before.clone();
        edit(&mut after);
        if after != before {
            changed = true;
            put_record(path, &row.id, &to_cbor(&after).expect("cbor"));
        }
    }
    assert!(changed, "the edit changed no record");
}

/// Like [`edit_values`], on each record's JSON text.
pub(super) fn edit_text(path: &Path, kinds: &[&str], edit: impl Fn(&str) -> String) {
    edit_values(path, kinds, |value| {
        let text = edit(&value.to_string());
        *value = serde_json::from_str(&text).expect("the edited text is still JSON");
    });
}

/// Replaces every record of `kind` with the CBOR of `json`.
pub(super) fn set_json(path: &Path, kind: &str, json: &str) {
    let value: Value = serde_json::from_str(json).expect("json");
    for row in rows(path).into_iter().filter(|r| r.kind == kind) {
        put_record(path, &row.id, &to_cbor(&value).expect("cbor"));
    }
}

/// Replaces every record of `kind` with these bytes, which are not CBOR of anything.
pub(super) fn set_raw(path: &Path, kind: &str, bytes: &[u8]) {
    for row in rows(path).into_iter().filter(|r| r.kind == kind) {
        put_record(path, &row.id, bytes);
    }
}

/// Changes the fields of the sound (not its file).
pub(super) fn edit_sound(path: &Path, edit: impl Fn(&mut Value)) {
    for row in rows(path).into_iter().filter(|r| r.kind == "sound") {
        let length = u32::from_le_bytes(row.record[..4].try_into().expect("four")) as usize;
        let mut meta: Value = from_cbor(&row.record[4..4 + length]).expect("cbor");
        edit(&mut meta);
        let meta = to_cbor(&meta).expect("cbor");
        let mut record = (meta.len() as u32).to_le_bytes().to_vec();
        record.extend_from_slice(&meta);
        record.extend_from_slice(&row.record[4 + length..]);
        put_record(path, &row.id, &record);
    }
}

/// Removes `key` wherever it appears in `value`.
pub(super) fn strip_key(value: &mut Value, key: &str) {
    match value {
        Value::Object(map) => {
            map.remove(key);
            map.values_mut().for_each(|v| strip_key(v, key));
        }
        Value::Array(items) => items.iter_mut().for_each(|v| strip_key(v, key)),
        _ => {}
    }
}

/// Sets `key` to `to` wherever it appears in `value`.
pub(super) fn set_key(value: &mut Value, key: &str, to: &Value) {
    match value {
        Value::Object(map) => {
            if map.contains_key(key) {
                map.insert(key.to_owned(), to.clone());
            }
            map.values_mut().for_each(|v| set_key(v, key, to));
        }
        Value::Array(items) => items.iter_mut().for_each(|v| set_key(v, key, to)),
        _ => {}
    }
}
