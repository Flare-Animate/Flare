//! Downloading the Google Fonts families text names, and keeping them for next time.
//!
//! Which files a family has, and where they are, is `flare_text::google`'s; whether bytes are the
//! family's fonts is `flare_text`'s too. This crate is the transport on the platforms that have
//! sockets: one blocking download per file, bounded in size and time, run off the UI thread by
//! whoever calls it, with the files kept in a cache directory so a family is downloaded once. A
//! browser downloads with its own `fetch` instead, and has nothing here.
#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
mod cache;
#[cfg(not(target_arch = "wasm32"))]
mod download;

#[cfg(not(target_arch = "wasm32"))]
pub use cache::{Cache, default_cache_dir};
#[cfg(not(target_arch = "wasm32"))]
pub use download::{CACHE_ENV, Downloader, Transport, download, fetch_family};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("{family} is not a Google Fonts family Flare knows")]
    Unknown { family: String },
    #[error("could not download {url}: {reason}")]
    Download { url: String, reason: String },
    #[error("a file of {family} is larger than {} MB", flare_text::MAX_FONT_BYTES / (1024 * 1024))]
    TooLarge { family: String },
    #[error("the download of {family} is not its font: {reason}")]
    NotTheFont {
        family: String,
        reason: flare_text::TextError,
    },
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
