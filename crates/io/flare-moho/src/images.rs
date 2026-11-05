//! The pictures an image layer shows. The reader never opens a file: the caller that knows the
//! project's folder supplies each picture by its path, relative to that folder, and says why a
//! picture cannot be used.

/// A decoded picture: RGBA8, straight alpha, row by row from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Supplies the picture at a relative path. An error is a reason for the report, not a failure of
/// the whole import.
pub trait ImageSource {
    fn load(&mut self, path: &str) -> Result<Picture, String>;
}

/// No folder to read from: every image layer is reported as having no picture.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoImages;

impl ImageSource for NoImages {
    fn load(&mut self, _path: &str) -> Result<Picture, String> {
        Err("no project folder to read the picture from".to_owned())
    }
}
