//! Reading and writing `.flare` files in the page.
//!
//! A page has neither SQLite nor zstd, so it brings them as WebAssembly (`src/lib/flare-file.js`,
//! over `@sqlite.org/sqlite-wasm` and `@bokuweb/zstd-wasm`) and registers the result as
//! `globalThis.flareFile` before this module starts. Everything that decides whether a file is
//! good is `flare-storage`'s, the code the desktop app runs: the page only opens the database and
//! compresses, and hands over rows.

use flare_document::Document;
use flare_storage::{Parts, Record, StorageError, Zstd, from_parts, to_parts};
use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};

fn host_error(what: &str, error: JsValue) -> StorageError {
    let text = error
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "an unknown error".to_owned());
    StorageError::Host(format!("{what}: {text}"))
}

/// The registered library, or the reason there is none.
pub struct FlareFile {
    host: JsValue,
}

impl FlareFile {
    /// The library the page registered as `globalThis.flareFile`.
    pub fn registered() -> Result<Self, StorageError> {
        let host = Reflect::get(&js_sys::global(), &JsValue::from_str("flareFile"))
            .ok()
            .filter(|v| !v.is_undefined() && !v.is_null())
            .ok_or_else(|| {
                StorageError::Host("the page did not load its .flare file library".to_owned())
            })?;
        Ok(Self { host })
    }

    /// A library object given directly (for tests, which build it themselves).
    pub fn from_host(host: JsValue) -> Self {
        Self { host }
    }

    fn function(&self, name: &str) -> Result<Function, StorageError> {
        Reflect::get(&self.host, &JsValue::from_str(name))
            .ok()
            .and_then(|f| f.dyn_into::<Function>().ok())
            .ok_or_else(|| StorageError::Host(format!("the file library has no {name}")))
    }

    fn call(&self, name: &str, argument: &JsValue) -> Result<JsValue, StorageError> {
        self.function(name)?
            .call1(&self.host, argument)
            .map_err(|e| host_error(name, e))
    }

    /// The document in the bytes of a `.flare` file.
    pub fn read(&self, bytes: &[u8]) -> Result<Document, StorageError> {
        let rows = self.call("read", &Uint8Array::from(bytes).into())?;
        from_parts(parts_from_js(&rows)?, &mut BrowserZstd(self))
    }

    /// The bytes of a `.flare` file holding `document`.
    pub fn write(&self, document: &Document) -> Result<Vec<u8>, StorageError> {
        let parts = to_parts(document, &mut BrowserZstd(self))?;
        let bytes = self.call("write", &parts_to_js(&parts))?;
        Ok(Uint8Array::new(&bytes).to_vec())
    }
}

/// zstd through the page's WebAssembly module.
struct BrowserZstd<'a>(&'a FlareFile);

impl Zstd for BrowserZstd<'_> {
    fn compress(&mut self, raw: &[u8]) -> Result<Vec<u8>, StorageError> {
        let packed = self.0.call("zstdCompress", &Uint8Array::from(raw).into())?;
        Ok(Uint8Array::new(&packed).to_vec())
    }

    fn decompress(&mut self, frame: &[u8], size: usize) -> Result<Vec<u8>, StorageError> {
        let raw = self
            .0
            .call("zstdDecompress", &Uint8Array::from(frame).into())?;
        let raw = Uint8Array::new(&raw);
        // The frame's header was checked to declare `size`; a decompressor that made more is
        // refused here whatever it was told.
        if raw.length() as usize != size {
            return Err(StorageError::Corrupt(
                "a record is not its stated size".to_owned(),
            ));
        }
        Ok(raw.to_vec())
    }
}

fn get(object: &JsValue, key: &str) -> Result<JsValue, StorageError> {
    Reflect::get(object, &JsValue::from_str(key))
        .map_err(|_| StorageError::Host(format!("the file library's rows have no {key}")))
}

fn text(object: &JsValue, key: &str) -> Result<String, StorageError> {
    get(object, key)?
        .as_string()
        .ok_or_else(|| StorageError::Host(format!("{key} is not text")))
}

fn number(object: &JsValue, key: &str) -> Result<i64, StorageError> {
    get(object, key)?
        .as_f64()
        .filter(|n| n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15)
        .map(|n| n as i64)
        .ok_or_else(|| StorageError::Host(format!("{key} is not a whole number")))
}

fn bytes(object: &JsValue, key: &str) -> Result<Vec<u8>, StorageError> {
    let value = get(object, key)?;
    if !value.is_instance_of::<Uint8Array>() {
        return Err(StorageError::Host(format!("{key} is not bytes")));
    }
    Ok(Uint8Array::new(&value).to_vec())
}

fn parts_from_js(rows: &JsValue) -> Result<Parts, StorageError> {
    let records = get(rows, "records")?;
    let records = records
        .dyn_ref::<Array>()
        .ok_or_else(|| StorageError::Host("records is not a list".to_owned()))?;
    let mut out = Vec::with_capacity(records.length() as usize);
    for row in records.iter() {
        out.push(Record {
            id: text(&row, "id")?,
            kind: text(&row, "kind")?,
            ordinal: number(&row, "ordinal")?,
            codec: text(&row, "codec")?,
            size: number(&row, "size")?,
            digest: bytes(&row, "digest")?,
            payload: bytes(&row, "payload")?,
        });
    }
    Ok(Parts {
        project_id: text(rows, "projectId")?,
        settings: text(rows, "settings")?,
        records: out,
    })
}

fn parts_to_js(parts: &Parts) -> JsValue {
    let set = |object: &Object, key: &str, value: &JsValue| {
        // Setting a property on a plain object cannot fail.
        let _ = Reflect::set(object, &JsValue::from_str(key), value);
    };
    let records = Array::new();
    for r in &parts.records {
        let row = Object::new();
        set(&row, "id", &JsValue::from_str(&r.id));
        set(&row, "kind", &JsValue::from_str(&r.kind));
        set(&row, "ordinal", &JsValue::from_f64(r.ordinal as f64));
        set(&row, "codec", &JsValue::from_str(&r.codec));
        set(&row, "size", &JsValue::from_f64(r.size as f64));
        set(
            &row,
            "digest",
            &Uint8Array::from(r.digest.as_slice()).into(),
        );
        set(
            &row,
            "payload",
            &Uint8Array::from(r.payload.as_slice()).into(),
        );
        records.push(&row);
    }
    let out = Object::new();
    set(&out, "projectId", &JsValue::from_str(&parts.project_id));
    set(&out, "settings", &JsValue::from_str(&parts.settings));
    set(&out, "records", &records);
    out.into()
}
