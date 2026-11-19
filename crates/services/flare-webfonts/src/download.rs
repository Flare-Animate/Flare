//! One blocking download per file.

use std::time::Duration;

use flare_text::GoogleFamily;

use crate::{Cache, FetchError};

/// How long one file may take, start to finish. The largest the catalog has (32 MB) arrives
/// well inside it on a slow connection; a stalled one is given up on.
const TIMEOUT: Duration = Duration::from_secs(120);

/// The bytes at `url`, at most [`flare_text::MAX_FONT_BYTES`] of them.
pub fn download(url: &str) -> Result<Vec<u8>, FetchError> {
    let failed = |reason: String| FetchError::Download {
        url: url.to_owned(),
        reason,
    };
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(|e| failed(e.to_string()))?;
    response
        .body_mut()
        .with_config()
        .limit(flare_text::MAX_FONT_BYTES as u64)
        .read_to_vec()
        .map_err(|e| failed(e.to_string()))
}

/// The files of the family named `name`: from `cache` when they are there, otherwise downloaded
/// with `get` (normally [`download`]), checked, and kept in `cache`. Blocks, so it runs on a
/// worker or in a program with no window.
pub fn fetch_family(
    name: &str,
    cache: Option<&Cache>,
    get: impl Fn(&str) -> Result<Vec<u8>, FetchError>,
) -> Result<(&'static GoogleFamily, Vec<Vec<u8>>), FetchError> {
    let family = flare_text::google::family_named(name).ok_or_else(|| FetchError::Unknown {
        family: name.to_owned(),
    })?;
    if let Some(files) = cache.and_then(|cache| cache.read(family)) {
        return Ok((family, files));
    }
    let files = family
        .urls()
        .iter()
        .map(|url| get(url))
        .collect::<Result<Vec<_>, _>>()?;
    flare_text::check_google_family(family, &files).map_err(|reason| match reason {
        flare_text::TextError::FontTooLarge => FetchError::TooLarge {
            family: family.name().to_owned(),
        },
        reason => FetchError::NotTheFont {
            family: family.name().to_owned(),
            reason,
        },
    })?;
    if let Some(cache) = cache {
        let _ = cache.write(family, &files);
    }
    Ok((family, files))
}

/// The environment variable that names where downloaded fonts are kept, in place of
/// [`crate::default_cache_dir`], for a program with no window (and its tests).
pub const CACHE_ENV: &str = "FLARE_FONT_CACHE";

/// How one file is downloaded: [`download`], or a stand-in a test gives.
pub type Transport = fn(&str) -> Result<Vec<u8>, FetchError>;

/// Downloads families into one cache, for a program with no window.
#[derive(Debug, Clone)]
pub struct Downloader {
    cache: Option<Cache>,
    get: Transport,
}

impl Downloader {
    pub fn new(cache: Option<Cache>) -> Self {
        Self {
            cache,
            get: download,
        }
    }

    /// Downloads with `get` instead of over the network. Tests use this so they do not depend
    /// on a connection.
    #[must_use]
    pub fn with_transport(mut self, get: Transport) -> Self {
        self.get = get;
        self
    }

    /// Keeping files where [`CACHE_ENV`] says, else in the platform's cache directory, else
    /// nowhere (every run downloads again).
    pub fn for_programs() -> Self {
        let dir = std::env::var_os(CACHE_ENV)
            .filter(|dir| !dir.is_empty())
            .map(std::path::PathBuf::from)
            .or_else(crate::default_cache_dir);
        Self::new(dir.map(Cache::new))
    }

    pub fn cache(&self) -> Option<&Cache> {
        self.cache.as_ref()
    }

    /// The files of the family named `name`, from the cache or downloaded. Blocks.
    pub fn fetch(&self, name: &str) -> Result<Vec<Vec<u8>>, String> {
        fetch_family(name, self.cache.as_ref(), self.get)
            .map(|(_, files)| files)
            .map_err(|e| e.to_string())
    }
}
