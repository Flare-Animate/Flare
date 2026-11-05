use thiserror::Error;

/// Why a Moho project could not be read or converted.
#[derive(Debug, Error)]
pub enum MohoError {
    #[error("not a Moho project")]
    NotMoho,
    #[error(
        "legacy .anme projects (Anime Studio before version 11) are not supported; \
         re-save the project from Moho 11 or later"
    )]
    LegacyFormat,
    #[error("{what} is {size} bytes, over the limit of {limit} bytes")]
    TooLarge {
        what: &'static str,
        size: u64,
        limit: u64,
    },
    #[error("the ZIP container has no Project.mohoproj or Project.animeproj")]
    MissingDocument,
    #[error("the ZIP container is invalid: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("the project document is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the project document is not a JSON object")]
    NotObject,
    #[error("the project exceeds the limit of {limit} for {what}")]
    Limit { what: &'static str, limit: usize },
    #[error("cannot read the project: {0}")]
    Io(#[from] std::io::Error),
    #[error("the project cannot become a Flare document: {0}")]
    Document(String),
}
