//! Vector (`.pli`) and raster (`.tlv`) levels. Only the header is recognised. Decoding is not
//! built yet, so these return [`ToonzError::NotYetImplemented`] for a file of the right kind and
//! [`ToonzError::UnknownFormat`] for anything else.

use std::convert::Infallible;

use crate::error::ToonzError;
use crate::sniff::{Format, sniff};

pub fn decode_pli(bytes: &[u8]) -> Result<Infallible, ToonzError> {
    match sniff(bytes) {
        Format::Pli { .. } => Err(ToonzError::NotYetImplemented {
            format: "pli",
            what: "stroke and region decoding (the header is recognised)",
        }),
        _ => Err(ToonzError::UnknownFormat("not a PLI vector level")),
    }
}

pub fn decode_tlv(bytes: &[u8]) -> Result<Infallible, ToonzError> {
    match sniff(bytes) {
        Format::Tlv { .. } => Err(ToonzError::NotYetImplemented {
            format: "tlv",
            what: "raster pixel decoding (the header is recognised)",
        }),
        _ => Err(ToonzError::UnknownFormat("not a TLV raster level")),
    }
}
