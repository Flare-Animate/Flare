//! The jobs a front end hands over and the results it gets back.

use std::path::PathBuf;
use std::sync::Arc;

use flare_core::{Rgba8, SceneId, SoundId};
use flare_document::Document;

/// One frame to draw: the part of the stage in `viewport`, at `size` pixels, with onion-skin
/// ghosts of the given frames beneath it.
pub struct RenderJob {
    pub id: u64,
    pub document: Arc<Document>,
    pub scene: SceneId,
    pub frame: u32,
    /// (frame, opacity), farthest first.
    pub ghosts: Vec<(u32, f32)>,
    /// Draw the ghosts as thin outlines rather than faded pictures.
    pub outlines: bool,
    /// Outline Mode: every shape as a line in its layer's color, no ghosts.
    pub outline_mode: bool,
    pub viewport: flare_render::Viewport,
    pub size: [u32; 2],
    pub background: Rgba8,
}

/// Sound work, on a worker of its own because decoding a long file takes a while.
pub enum AudioJob {
    /// Read a file, decode it to learn its length and to be ready to play.
    Import { id: u64, path: PathBuf },
    /// Decode a sound already in the document.
    Decode { sound: SoundId, data: Arc<[u8]> },
}

/// Text to turn into outlines. Building the font database takes long enough that this has a
/// worker of its own, so a first use never holds up a save or a redraw.
pub struct TextJob {
    pub id: u64,
    pub text: String,
    pub style: flare_text::TextStyle,
}

pub enum IoJob {
    Save {
        path: PathBuf,
        document: Arc<Document>,
    },
    Open {
        id: u64,
        path: PathBuf,
        /// Opening a recovery file rather than a project.
        recovery: bool,
    },
    Autosave {
        id: u64,
        dir: PathBuf,
        document: Arc<Document>,
    },
    /// Read an XFL folder or a `.fla` and turn it into a document.
    Import { id: u64, path: PathBuf },
    /// The same from a `.fla` already in memory (a browser has no path to read).
    ImportBytes {
        id: u64,
        name: String,
        bytes: Vec<u8>,
    },
}

/// A drawn frame, in whichever form the rasterizer produced it: the common one (no ghosts)
/// goes straight from premultiplied pixels to the front end with no conversion in between.
pub enum Rendered {
    Premultiplied(flare_render::PremultipliedImage),
    Straight(flare_render::Image),
}

impl Rendered {
    pub fn size(&self) -> [usize; 2] {
        let (w, h) = match self {
            Self::Premultiplied(i) => (i.width(), i.height()),
            Self::Straight(i) => (i.width(), i.height()),
        };
        [w as usize, h as usize]
    }
}

/// What a worker reports back. Errors are already text: the front end only shows them.
pub enum Done {
    SoundImported {
        id: u64,
        path: PathBuf,
        /// The file's bytes and what decoding them gave.
        result: Result<(Vec<u8>, Arc<flare_audio::Audio>), String>,
    },
    SoundDecoded {
        sound: SoundId,
        result: Result<Arc<flare_audio::Audio>, String>,
    },
    Text {
        id: u64,
        result: Result<flare_text::TextOutline, String>,
    },
    Rendered {
        id: u64,
        result: Result<Rendered, String>,
    },
    Saved {
        path: PathBuf,
        document: Arc<Document>,
        result: Result<(), String>,
    },
    Opened {
        id: u64,
        path: PathBuf,
        recovery: bool,
        result: Result<Document, String>,
    },
    Autosaved {
        id: u64,
        result: Result<PathBuf, String>,
    },
    Imported {
        id: u64,
        path: PathBuf,
        /// The document and what could not be carried over, one line each.
        result: Result<(Document, Vec<String>), String>,
    },
}

impl Done {
    /// Whether this finished a save, open or autosave (what `Runtime::io_pending` counts).
    pub(crate) fn is_io(&self) -> bool {
        matches!(
            self,
            Done::Saved { .. }
                | Done::Opened { .. }
                | Done::Autosaved { .. }
                | Done::Imported { .. }
        )
    }
}
