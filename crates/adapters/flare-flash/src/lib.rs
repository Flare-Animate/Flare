//! Bounded prefix inspection only. No decompression, parsing, scripts, or I/O.
//!
//! A ZIP or compound-file signature identifies a container, not a proven FLA.
//! A plausible SWF header does not validate the rest of the file.
#![forbid(unsafe_code)]

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SwfCompression {
    None,
    Zlib,
    Lzma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PrefixKind {
    SwfHeader {
        compression: SwfCompression,
        version: u8,
        /// SWF header's declared uncompressed file length; not an allocation size.
        declared_uncompressed_length: u32,
    },
    ZipContainerCandidate,
    CompoundFileCandidate,
    Unknown,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InspectError {
    #[error("SWF signature is present, but its eight-byte header is truncated")]
    TruncatedSwfHeader,
    #[error("SWF declared length {0} is shorter than its fixed header")]
    InvalidSwfLength(u32),
    #[error("SWF version zero is invalid")]
    InvalidSwfVersion,
}

/// Inspects an already-bounded byte prefix without allocating.
///
/// It deliberately does not guess a Flash authoring version from an extension.
/// Decompression budgets and full format validation belong to later readers.
/// Unknown/future nonzero SWF versions remain inspectable, not 'supported'.
pub fn inspect_prefix(prefix: &[u8]) -> Result<PrefixKind, InspectError> {
    let compression = if prefix.starts_with(b"FWS") {
        Some(SwfCompression::None)
    } else if prefix.starts_with(b"CWS") {
        Some(SwfCompression::Zlib)
    } else if prefix.starts_with(b"ZWS") {
        Some(SwfCompression::Lzma)
    } else {
        None
    };
    if let Some(compression) = compression {
        let header = prefix.get(..8).ok_or(InspectError::TruncatedSwfHeader)?;
        let version = header[3];
        if version == 0 {
            return Err(InspectError::InvalidSwfVersion);
        }
        let declared_uncompressed_length =
            u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        if declared_uncompressed_length < 8 {
            return Err(InspectError::InvalidSwfLength(declared_uncompressed_length));
        }
        return Ok(PrefixKind::SwfHeader {
            compression,
            version,
            declared_uncompressed_length,
        });
    }
    if prefix.starts_with(b"PK\x03\x04")
        || prefix.starts_with(b"PK\x05\x06")
        || prefix.starts_with(b"PK\x07\x08")
    {
        return Ok(PrefixKind::ZipContainerCandidate);
    }
    if prefix.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) {
        return Ok(PrefixKind::CompoundFileCandidate);
    }
    Ok(PrefixKind::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swf_header_is_inspected_not_loaded() -> Result<(), InspectError> {
        assert_eq!(
            inspect_prefix(b"FWS\x09\x10\x00\x00\x00")?,
            PrefixKind::SwfHeader {
                compression: SwfCompression::None,
                version: 9,
                declared_uncompressed_length: 16,
            }
        );
        Ok(())
    }

    #[test]
    fn invalid_headers_are_errors() {
        assert_eq!(
            inspect_prefix(b"FWS"),
            Err(InspectError::TruncatedSwfHeader)
        );
        assert_eq!(
            inspect_prefix(b"CWS\x09\x07\x00\x00\x00"),
            Err(InspectError::InvalidSwfLength(7))
        );
        assert_eq!(
            inspect_prefix(b"FWS\x00\x10\x00\x00\x00"),
            Err(InspectError::InvalidSwfVersion)
        );
    }

    #[test]
    fn containers_are_only_candidates() -> Result<(), InspectError> {
        assert_eq!(
            inspect_prefix(b"PK\x03\x04anything")?,
            PrefixKind::ZipContainerCandidate
        );
        assert_eq!(
            inspect_prefix(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1])?,
            PrefixKind::CompoundFileCandidate
        );
        assert_eq!(inspect_prefix(b"<DOMDocument/>")?, PrefixKind::Unknown);
        assert_eq!(inspect_prefix(&[])?, PrefixKind::Unknown);
        Ok(())
    }
}
