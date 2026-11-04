//! Identifies a Toonz file from its first bytes. Only the header is looked at; nothing is decoded.

/// `PLI` vector magic as the little-endian writer lays it out (`c_magicNt` in `pli_io.cpp`).
const PLI_MAGIC: &[u8] = b"PLIM";
/// The same magic written by the big-endian (Irix) writer (`c_magicIrix`).
const PLI_MAGIC_IRIX: &[u8] = b"MILP";
/// Raster level versions: the first five bytes are `TLV10`, `TLV11` or `TLV12`.
const TLV_MAGICS: [(&[u8], u8); 3] = [(b"TLV10", 10), (b"TLV11", 11), (b"TLV12", 12)];
/// How far into an XML file the root element is searched for.
const XML_HEAD: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A `.tnz` scene (XML with a `<tnz>` root).
    Scene,
    /// A palette (XML with a `<palette>` root).
    Palette,
    /// A `.pli` vector level. `legacy_byte_order` is set for the Irix-written magic.
    Pli {
        legacy_byte_order: bool,
    },
    /// A `.tlv` raster level of the given format version (10, 11 or 12).
    Tlv {
        version: u8,
    },
    Unknown,
}

pub fn sniff(bytes: &[u8]) -> Format {
    if bytes.starts_with(PLI_MAGIC) {
        return Format::Pli {
            legacy_byte_order: false,
        };
    }
    if bytes.starts_with(PLI_MAGIC_IRIX) {
        return Format::Pli {
            legacy_byte_order: true,
        };
    }
    for (magic, version) in TLV_MAGICS {
        if bytes.starts_with(magic) {
            return Format::Tlv { version };
        }
    }
    match xml_root_name(bytes) {
        Some("tnz") => Format::Scene,
        Some("palette") => Format::Palette,
        _ => Format::Unknown,
    }
}

/// The name of the root element if `bytes` starts with XML, looking at the first few KiB.
fn xml_root_name(bytes: &[u8]) -> Option<&str> {
    let head = &bytes[..bytes.len().min(XML_HEAD)];
    let text = match std::str::from_utf8(head) {
        Ok(text) => text,
        // The cut may split a character at the end of the window; keep what is valid.
        Err(err) => std::str::from_utf8(&head[..err.valid_up_to()]).ok()?,
    };
    let mut rest = text.trim_start_matches('\u{feff}').trim_start();
    loop {
        let start = rest.find('<')?;
        rest = &rest[start..];
        if let Some(after) = rest.strip_prefix("<?") {
            rest = &after[after.find("?>")? + 2..];
        } else if let Some(after) = rest.strip_prefix("<!--") {
            rest = &after[after.find("-->")? + 3..];
        } else if rest.starts_with("<!") {
            rest = &rest[rest.find('>')? + 1..];
        } else {
            let name = &rest[1..];
            let end = name
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .unwrap_or(name.len());
            return Some(&name[..end]);
        }
        rest = rest.trim_start();
    }
}
