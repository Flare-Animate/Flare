//! Raster (`.tlv`) levels. Only the header is recognised; decoding is not built yet, so this
//! returns [`ToonzError::NotYetImplemented`] for a file of the right kind and
//! [`ToonzError::UnknownFormat`] for anything else. Vector levels are in [`crate::pli`].

use std::convert::Infallible;

use crate::error::ToonzError;
use crate::sniff::{Format, sniff};

pub fn decode_tlv(bytes: &[u8]) -> Result<Infallible, ToonzError> {
    match sniff(bytes) {
        Format::Tlv { .. } => Err(ToonzError::NotYetImplemented {
            format: "tlv",
            what: "raster pixel decoding (the header is recognised)",
        }),
        _ => Err(ToonzError::UnknownFormat("not a TLV raster level")),
    }
}
