use thiserror::Error;

/// Why a Toonz file was refused or could not be read. Nothing in this crate panics on file
/// content; every problem is one of these.
#[derive(Debug, Error)]
pub enum ToonzError {
    /// The bytes are not a file of the kind asked for (for example, a scene parser given a palette).
    #[error("not a Toonz file of this kind: {0}")]
    UnknownFormat(&'static str),
    /// The file is of the right kind but its content is inconsistent or cannot be read.
    #[error("malformed {file}: {detail}")]
    Malformed { file: &'static str, detail: String },
    /// A bound from [`crate::limits`] was exceeded. Nothing is read past it.
    #[error("{what} is over the limit of {max}")]
    Limit { what: &'static str, max: usize },
    /// The format is recognised but this part of it is not built yet.
    #[error("{what} in {format} is not supported yet")]
    NotYetImplemented {
        format: &'static str,
        what: &'static str,
    },
    /// The XML itself is broken (unbalanced tags, bad syntax).
    #[error("invalid XML: {0}")]
    Xml(#[from] quick_xml::Error),
    /// Text in the file is not UTF-8.
    #[error("text is not UTF-8")]
    Utf8,
}

impl ToonzError {
    pub(crate) fn malformed(file: &'static str, detail: impl Into<String>) -> Self {
        Self::Malformed {
            file,
            detail: detail.into(),
        }
    }
}
