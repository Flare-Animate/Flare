//! Flash Local Shared Object (`.sol`) reader/writer: the SOL container header
//! followed by an AMF0 body.
//!
//! Absorbed from `ruffle-rs/rust-flash-lso` (`flash-lso` crate, MIT); see
//! `ORIGIN.md`. The container layout, the header constants and the AMF0 type
//! handling mirror `flash-lso/src/{read,write}.rs` and `amf0/{read,write}.rs`,
//! re-expressed with safe bounds-checked reads instead of `nom`, so it compiles
//! with no external parser dependency and can be reused by the `ac-xfl` crate.
//!
//! Layout (offsets from the start of the file):
//!
//! ```text
//! 00 BF                 header version
//! u32 BE length         body length + 16 + name length (written, not validated)
//! "TCSO" + 6 bytes       header signature
//! u16 BE + bytes         the shared object name (AMF0 short string)
//! 00 00 00               padding
//! 00 | 03                format version: 00 = AMF0, 03 = AMF3
//! body                   AMF0: (u16 name + value + 00) repeated
//! ```
//!
//! The body of a normal `.sol` is a single element with an empty name whose
//! value is an anonymous ECMA array — that is what [`Lso::parse`] returns and
//! what [`Lso::to_bytes`] writes.

use std::fmt;

use serde_json::{json, Map, Value};

/// First two bytes of every SOL file.
pub const HEADER_VERSION: [u8; 2] = [0x00, 0xbf];
/// `TCSO` + the fixed 6-byte trailer that follows it.
pub const HEADER_SIGNATURE: [u8; 10] =
    [0x54, 0x43, 0x53, 0x4f, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00];
/// Separator byte between top-level body elements.
pub const PADDING: u8 = 0x00;
/// AMF0 body.
pub const FORMAT_VERSION_AMF0: u8 = 0x00;
/// AMF3 body (container only; the body itself is not decoded here).
pub const FORMAT_VERSION_AMF3: u8 = 0x03;

// AMF0 type markers (ActionScript 3 AMF0 spec).
mod marker {
    pub const NUMBER: u8 = 0x00;
    pub const BOOLEAN: u8 = 0x01;
    pub const STRING: u8 = 0x02;
    pub const OBJECT: u8 = 0x03;
    pub const MOVIE_CLIP: u8 = 0x04;
    pub const NULL: u8 = 0x05;
    pub const UNDEFINED: u8 = 0x06;
    pub const REFERENCE: u8 = 0x07;
    pub const ECMA_ARRAY: u8 = 0x08;
    pub const OBJECT_END: u8 = 0x09;
    pub const STRICT_ARRAY: u8 = 0x0a;
    pub const DATE: u8 = 0x0b;
    pub const LONG_STRING: u8 = 0x0c;
    pub const UNSUPPORTED: u8 = 0x0d;
    pub const RECORD_SET: u8 = 0x0e;
    pub const XML: u8 = 0x0f;
    pub const TYPED_OBJECT: u8 = 0x10;
    pub const AMF3: u8 = 0x11;
}

/// Why an LSO could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsoError {
    /// Ran past the end of the buffer.
    UnexpectedEof,
    /// The first two bytes were not `00 BF`.
    BadMagic,
    /// The 10-byte `TCSO` signature was not where it should be.
    BadSignature,
    /// Format version byte was neither `00` (AMF0) nor `03` (AMF3).
    BadVersion(u8),
    /// AMF3 bodies are recognised but not decoded (yet).
    Amf3Body,
    /// A marker byte this reader does not implement.
    UnsupportedMarker(u8),
    /// A string was not valid UTF-8.
    Utf8,
    /// Bytes left over after the body was read.
    TrailingData,
}

impl fmt::Display for LsoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LsoError::UnexpectedEof => write!(f, "unexpected end of LSO data"),
            LsoError::BadMagic => write!(f, "not an LSO file (bad header magic)"),
            LsoError::BadSignature => write!(f, "not an LSO file (bad TCSO signature)"),
            LsoError::BadVersion(v) => write!(f, "unsupported LSO format version {:#04x}", v),
            LsoError::Amf3Body => write!(f, "AMF3 LSO bodies are not decoded"),
            LsoError::UnsupportedMarker(m) => write!(f, "unsupported AMF0 marker {:#04x}", m),
            LsoError::Utf8 => write!(f, "AMF0 string is not valid UTF-8"),
            LsoError::TrailingData => write!(f, "trailing bytes after the LSO body"),
        }
    }
}

/// An AMF0 value. Mirrors the subset of `flash_lso::types::Value` that SOL
/// files use.
#[derive(Debug, Clone, PartialEq)]
pub enum AmfValue {
    Number(f64),
    Boolean(bool),
    String(String),
    /// An anonymous object (`0x03`).
    Object(Vec<(String, AmfValue)>),
    Null,
    Undefined,
    /// A back-reference to an earlier value (`0x07`).
    Reference(u16),
    /// An associative array (`0x08`); the declared length is kept for fidelity.
    EcmaArray { length: u32, entries: Vec<(String, AmfValue)> },
    StrictArray(Vec<AmfValue>),
    Date { time_ms: f64, timezone: u16 },
    LongString(String),
    Xml(String),
    TypedObject { class_name: String, fields: Vec<(String, AmfValue)> },
    /// A marker this reader does not implement (MovieClip, RecordSet, ...).
    Unsupported(u8),
    /// An embedded AMF3 payload (`0x11`); not decoded.
    Amf3,
}

/// A decoded SOL file.
#[derive(Debug, Clone, PartialEq)]
pub struct Lso {
    /// The shared object name stored in the header.
    pub name: String,
    /// `0` for AMF0, `3` for AMF3.
    pub format_version: u8,
    /// Top-level body elements: normally one entry with an empty name holding
    /// the anonymous ECMA array.
    pub body: Vec<(String, AmfValue)>,
}

/// Cheap check used by format sniffing; does not decode the body.
pub fn is_lso(d: &[u8]) -> bool {
    d.len() >= 16 && d[0..2] == HEADER_VERSION && d[6..16] == HEADER_SIGNATURE
}

/// Decode a `.sol` file.
pub fn parse(d: &[u8]) -> Result<Lso, LsoError> {
    let mut r = Reader::new(d);
    r.expect(&HEADER_VERSION).map_err(|_| LsoError::BadMagic)?;
    let _length = r.u32()?;
    r.expect(&HEADER_SIGNATURE)
        .map_err(|_| LsoError::BadSignature)?;
    let name = r.str_u16()?;
    r.expect(&[PADDING, PADDING, PADDING])?;
    let format_version = r.u8()?;
    match format_version {
        FORMAT_VERSION_AMF0 => {}
        FORMAT_VERSION_AMF3 => return Err(LsoError::Amf3Body),
        v => return Err(LsoError::BadVersion(v)),
    }
    let body = parse_body(&mut r)?;
    if r.remaining() != 0 {
        return Err(LsoError::TrailingData);
    }
    Ok(Lso { name, format_version, body })
}

fn parse_body(r: &mut Reader<'_>) -> Result<Vec<(String, AmfValue)>, LsoError> {
    let mut out = Vec::new();
    while r.remaining() >= 2 {
        let name = r.str_u16()?;
        let value = parse_value(r)?;
        r.expect(&[PADDING])?;
        out.push((name, value));
    }
    Ok(out)
}

/// `0x03` / `0x08` / `0x10` share the "key, value, ... , 00 00 09" encoding.
fn parse_keyed(r: &mut Reader<'_>) -> Result<Vec<(String, AmfValue)>, LsoError> {
    let mut out = Vec::new();
    loop {
        let name = r.str_u16()?;
        if r.peek()? == marker::OBJECT_END {
            r.u8()?;
            break;
        }
        let value = parse_value(r)?;
        out.push((name, value));
    }
    Ok(out)
}

fn parse_value(r: &mut Reader<'_>) -> Result<AmfValue, LsoError> {
    Ok(match r.u8()? {
        marker::NUMBER => AmfValue::Number(r.f64()?),
        marker::BOOLEAN => AmfValue::Boolean(r.u8()? > 0),
        marker::STRING => AmfValue::String(r.str_u16()?),
        marker::OBJECT => AmfValue::Object(parse_keyed(r)?),
        marker::NULL => AmfValue::Null,
        marker::UNDEFINED => AmfValue::Undefined,
        marker::REFERENCE => AmfValue::Reference(r.u16()?),
        marker::ECMA_ARRAY => {
            let length = r.u32()?;
            AmfValue::EcmaArray { length, entries: parse_keyed(r)? }
        }
        marker::STRICT_ARRAY => {
            let n = r.u32()? as usize;
            // Guard against a huge declared count by needing one byte per entry.
            if n > r.remaining() {
                return Err(LsoError::UnexpectedEof);
            }
            let mut values = Vec::with_capacity(n);
            for _ in 0..n {
                values.push(parse_value(r)?);
            }
            AmfValue::StrictArray(values)
        }
        marker::DATE => {
            let time_ms = r.f64()?;
            let timezone = r.u16()?;
            AmfValue::Date { time_ms, timezone }
        }
        marker::LONG_STRING => AmfValue::LongString(r.str_u32()?),
        marker::XML => AmfValue::Xml(r.str_u32()?),
        marker::TYPED_OBJECT => {
            let class_name = r.str_u16()?;
            AmfValue::TypedObject { class_name, fields: parse_keyed(r)? }
        }
        marker::AMF3 => AmfValue::Amf3,
        marker::MOVIE_CLIP | marker::UNSUPPORTED | marker::RECORD_SET => {
            return Err(LsoError::UnsupportedMarker(r.peek_back()?))
        }
        m => return Err(LsoError::UnsupportedMarker(m)),
    })
}

impl Lso {
    /// Serialize back to `.sol` bytes. Inverse of [`parse`].
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut body = Vec::new();
        for (name, value) in &self.body {
            write_str_u16(&mut body, name);
            write_value(&mut body, value);
            body.push(PADDING);
        }
        // header_length() + body, as in flash-lso/src/write.rs.
        let header_len = 10 + (2 + self.name.len() + 3 + 1);
        let length = (body.len() + header_len) as u32;

        let mut out = Vec::with_capacity(6 + header_len + body.len());
        out.extend_from_slice(&HEADER_VERSION);
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(&HEADER_SIGNATURE);
        write_str_u16(&mut out, &self.name);
        out.extend_from_slice(&[PADDING, PADDING, PADDING]);
        out.push(self.format_version);
        out.extend_from_slice(&body);
        out
    }

    /// A JSON view of the shared object, for the importer log / tooling.
    pub fn to_json(&self) -> Value {
        let mut root = Map::new();
        root.insert("name".into(), Value::String(self.name.clone()));
        root.insert("formatVersion".into(), json!(self.format_version));
        let body: Map<String, Value> = self
            .body
            .iter()
            .map(|(k, v)| (k.clone(), amf_to_json(v)))
            .collect();
        root.insert("body".into(), Value::Object(body));
        Value::Object(root)
    }
}

fn amf_to_json(v: &AmfValue) -> Value {
    match v {
        AmfValue::Number(n) => json!(n),
        AmfValue::Boolean(b) => json!(b),
        AmfValue::String(s) | AmfValue::LongString(s) => json!(s),
        AmfValue::Object(_) | AmfValue::EcmaArray { .. } | AmfValue::TypedObject { .. } => {
            Value::Object(pairs_to_json(v))
        }
        AmfValue::StrictArray(a) => Value::Array(a.iter().map(amf_to_json).collect()),
        AmfValue::Null => Value::Null,
        AmfValue::Undefined => json!({ "$undefined": true }),
        AmfValue::Reference(i) => json!({ "$ref": i }),
        AmfValue::Date { time_ms, timezone } => {
            json!({ "$date": time_ms, "$timezone": timezone })
        }
        AmfValue::Xml(s) => json!({ "$xml": s }),
        AmfValue::Unsupported(m) => json!({ "$unsupported": m }),
        AmfValue::Amf3 => json!({ "$amf3": true }),
    }
}

fn pairs_to_json(v: &AmfValue) -> Map<String, Value> {
    let mut m = Map::new();
    match v {
        AmfValue::Object(p) | AmfValue::EcmaArray { entries: p, .. } => {
            for (k, val) in p {
                m.insert(k.clone(), amf_to_json(val));
            }
        }
        AmfValue::TypedObject { class_name, fields } => {
            m.insert("$class".into(), json!(class_name));
            for (k, val) in fields {
                m.insert(k.clone(), amf_to_json(val));
            }
        }
        _ => {}
    }
    m
}

fn write_value(out: &mut Vec<u8>, v: &AmfValue) {
    match v {
        AmfValue::Number(n) => {
            out.push(marker::NUMBER);
            out.extend_from_slice(&n.to_be_bytes());
        }
        AmfValue::Boolean(b) => {
            out.push(marker::BOOLEAN);
            out.push(u8::from(*b));
        }
        AmfValue::String(s) => {
            if s.len() > 0xffff {
                write_long_string(out, s);
            } else {
                out.push(marker::STRING);
                write_str_u16(out, s);
            }
        }
        AmfValue::Object(p) => {
            out.push(marker::OBJECT);
            write_keyed(out, p);
        }
        AmfValue::Null => out.push(marker::NULL),
        AmfValue::Undefined => out.push(marker::UNDEFINED),
        AmfValue::Reference(i) => {
            out.push(marker::REFERENCE);
            out.extend_from_slice(&i.to_be_bytes());
        }
        AmfValue::EcmaArray { length, entries } => {
            out.push(marker::ECMA_ARRAY);
            out.extend_from_slice(&length.to_be_bytes());
            write_keyed(out, entries);
        }
        AmfValue::StrictArray(a) => {
            out.push(marker::STRICT_ARRAY);
            out.extend_from_slice(&(a.len() as u32).to_be_bytes());
            for v in a {
                write_value(out, v);
            }
        }
        AmfValue::Date { time_ms, timezone } => {
            out.push(marker::DATE);
            out.extend_from_slice(&time_ms.to_be_bytes());
            out.extend_from_slice(&timezone.to_be_bytes());
        }
        AmfValue::LongString(s) => write_long_string(out, s),
        AmfValue::Xml(s) => {
            out.push(marker::XML);
            out.extend_from_slice(&(s.len() as u32).to_be_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        AmfValue::TypedObject { class_name, fields } => {
            out.push(marker::TYPED_OBJECT);
            write_str_u16(out, class_name);
            write_keyed(out, fields);
        }
        AmfValue::Unsupported(m) => out.push(*m),
        AmfValue::Amf3 => out.push(marker::AMF3),
    }
}

fn write_keyed(out: &mut Vec<u8>, pairs: &[(String, AmfValue)]) {
    for (k, v) in pairs {
        write_str_u16(out, k);
        write_value(out, v);
    }
    out.extend_from_slice(&[PADDING, PADDING, marker::OBJECT_END]);
}

fn write_str_u16(out: &mut Vec<u8>, s: &str) {
    let len = s.len().min(0xffff) as u16;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&s.as_bytes()[..len as usize]);
}

fn write_long_string(out: &mut Vec<u8>, s: &str) {
    out.push(marker::LONG_STRING);
    out.extend_from_slice(&(s.len() as u32).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

/// Bounds-checked cursor over the input, standing in for `nom`.
struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn new(d: &'a [u8]) -> Self {
        Reader { d, p: 0 }
    }

    fn remaining(&self) -> usize {
        self.d.len() - self.p
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], LsoError> {
        let end = self.p.checked_add(n).ok_or(LsoError::UnexpectedEof)?;
        let s = self.d.get(self.p..end).ok_or(LsoError::UnexpectedEof)?;
        self.p = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, LsoError> {
        Ok(self.take(1)?[0])
    }

    fn peek(&self) -> Result<u8, LsoError> {
        self.d.get(self.p).copied().ok_or(LsoError::UnexpectedEof)
    }

    fn peek_back(&self) -> Result<u8, LsoError> {
        self.d.get(self.p - 1).copied().ok_or(LsoError::UnexpectedEof)
    }

    fn u16(&mut self) -> Result<u16, LsoError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, LsoError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f64(&mut self) -> Result<f64, LsoError> {
        let b = self.take(8)?;
        Ok(f64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    fn expect(&mut self, want: &[u8]) -> Result<(), LsoError> {
        if self.take(want.len())? == want {
            Ok(())
        } else {
            Err(LsoError::BadSignature)
        }
    }

    fn str_u16(&mut self) -> Result<String, LsoError> {
        let n = self.u16()? as usize;
        let b = self.take(n)?;
        std::str::from_utf8(b)
            .map(|s| s.to_string())
            .map_err(|_| LsoError::Utf8)
    }

    fn str_u32(&mut self) -> Result<String, LsoError> {
        let n = self.u32()? as usize;
        let b = self.take(n)?;
        std::str::from_utf8(b)
            .map(|s| s.to_string())
            .map_err(|_| LsoError::Utf8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the bytes of a minimal but realistic SOL file by hand, so the test
    /// pins the on-disk layout rather than trusting the writer.
    fn sample_bytes() -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&HEADER_VERSION);
        d.extend_from_slice(&0u32.to_be_bytes()); // length (not validated on read)
        d.extend_from_slice(&HEADER_SIGNATURE);
        write_str_u16(&mut d, "SO");
        d.extend_from_slice(&[0, 0, 0]);
        d.push(FORMAT_VERSION_AMF0);
        // body: anonymous ECMA array with one string and one number entry
        write_str_u16(&mut d, ""); // element name
        d.push(marker::ECMA_ARRAY);
        d.extend_from_slice(&2u32.to_be_bytes());
        write_str_u16(&mut d, "hi");
        d.push(marker::STRING);
        write_str_u16(&mut d, "there");
        write_str_u16(&mut d, "n");
        d.push(marker::NUMBER);
        d.extend_from_slice(&3.5f64.to_be_bytes());
        d.extend_from_slice(&[0, 0, marker::OBJECT_END]);
        d.push(PADDING); // element separator
        d
    }

    #[test]
    fn parses_the_hand_built_layout() {
        let lso = parse(&sample_bytes()).unwrap();
        assert_eq!(lso.name, "SO");
        assert_eq!(lso.format_version, FORMAT_VERSION_AMF0);
        assert_eq!(lso.body.len(), 1);
        let (name, value) = &lso.body[0];
        assert_eq!(name, "");
        match value {
            AmfValue::EcmaArray { length, entries } => {
                assert_eq!(*length, 2);
                assert_eq!(entries[0], ("hi".to_string(), AmfValue::String("there".into())));
                assert_eq!(entries[1], ("n".to_string(), AmfValue::Number(3.5)));
            }
            other => panic!("expected ECMA array, got {:?}", other),
        }
    }

    #[test]
    fn round_trips_through_the_writer() {
        let original = parse(&sample_bytes()).unwrap();
        let bytes = original.to_bytes();
        let again = parse(&bytes).unwrap();
        assert_eq!(original, again);
        // The written header length covers header+body but not the magic (2)
        // or the length field itself (4): bytes.len() - 6, i.e. body + 16 +
        // name length.
        let length = u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]) as usize;
        assert_eq!(length, bytes.len() - 6);
    }

    #[test]
    fn handles_every_common_amf0_type() {
        let lso = Lso {
            name: "types".into(),
            format_version: FORMAT_VERSION_AMF0,
            body: vec![(
                String::new(),
                AmfValue::Object(vec![
                    ("bool".into(), AmfValue::Boolean(true)),
                    ("null".into(), AmfValue::Null),
                    ("undef".into(), AmfValue::Undefined),
                    ("ref".into(), AmfValue::Reference(7)),
                    ("date".into(), AmfValue::Date { time_ms: 1.0e12, timezone: 0 }),
                    (
                        "arr".into(),
                        AmfValue::StrictArray(vec![AmfValue::Number(1.0), AmfValue::Number(2.0)]),
                    ),
                    (
                        "inner".into(),
                        AmfValue::Object(vec![("k".into(), AmfValue::String("v".into()))]),
                    ),
                ]),
            )],
        };
        assert_eq!(parse(&lso.to_bytes()).unwrap(), lso);
    }

    #[test]
    fn json_view_is_stable() {
        let lso = parse(&sample_bytes()).unwrap();
        let j = lso.to_json();
        assert_eq!(j["name"], "SO");
        assert_eq!(j["body"][""]["hi"], "there");
        assert_eq!(j["body"][""]["n"], 3.5);
    }

    #[test]
    fn sniff_and_reject_bad_input() {
        assert!(is_lso(&sample_bytes()));
        assert!(!is_lso(b"not a sol file at all"));
        assert!(!is_lso(&[]));
        assert_eq!(parse(b"junk").unwrap_err(), LsoError::BadMagic);
        let mut bad = sample_bytes();
        bad[6] = 0x00; // corrupt TCSO
        assert_eq!(parse(&bad).unwrap_err(), LsoError::BadSignature);
        let mut amf3 = sample_bytes();
        amf3[23] = FORMAT_VERSION_AMF3; // header(16) + name len(2) + "SO"(2) + 3 padding
        assert_eq!(parse(&amf3).unwrap_err(), LsoError::Amf3Body);
    }

    #[test]
    fn truncated_input_never_panics() {
        let full = sample_bytes();
        for n in 0..full.len() {
            let _ = parse(&full[..n]); // must not panic
        }
    }
}
