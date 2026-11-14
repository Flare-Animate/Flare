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
    /// Colors for the ghosts before and after `frame`, as Animate's tinted onion skin has them;
    /// `None` draws them in their own colors.
    pub tints: Option<[Rgba8; 2]>,
    /// Outline Mode: every shape as a line in its layer's color, no ghosts.
    pub outline_mode: bool,
    /// Draw what the scene's camera sees rather than the scene as it is edited.
    pub through_camera: bool,
    pub viewport: flare_render::Viewport,
    pub size: [u32; 2],
    pub background: Rgba8,
    /// Text being typed, shown in place of the text object it edits (or on top of the scene when
    /// it is new), so the stage never shows the old letters and the new ones together.
    pub text_preview: Option<TextPreviewRender>,
}

/// How the stage is drawn while text is edited.
#[derive(Clone)]
pub struct TextPreviewRender {
    /// The text object being edited: its shape is replaced, in its place in the paint order.
    pub replaces: Option<flare_core::ShapeId>,
    /// What to draw there instead; none for text that has no letters yet.
    pub shape: Option<flare_document::Shape>,
}

/// Sound work, on a worker of its own because decoding a long file takes a while.
pub enum AudioJob {
    /// Read a file, decode it to learn its length and to be ready to play.
    Import { id: u64, path: PathBuf },
    /// Decode an audio file already held in memory (for a browser file picker).
    ImportBytes {
        id: u64,
        name: String,
        bytes: Vec<u8>,
    },
    /// Decode a sound already in the document.
    Decode { sound: SoundId, data: Arc<[u8]> },
}

/// Work on text. Building the font database takes long enough that this has a worker of its
/// own, so a first use never holds up a save or a redraw.
pub enum TextJob {
    /// Set `source` as a shape with identity `shape`, filled with `fill`.
    Set {
        id: u64,
        source: flare_document::TextSource,
        shape: flare_core::ShapeId,
        fill: flare_core::Rgba8,
    },
    /// List the font families there are to set text in.
    Families,
    /// What `text` looks like while it is being typed, and where the caret goes with the cursor
    /// before byte `cursor`. The shape has identity `shape` and the corner `origin`.
    Preview {
        id: u64,
        text: String,
        style: flare_text::TextStyle,
        origin: flare_core::DocumentPoint,
        cursor: usize,
        shape: flare_core::ShapeId,
        fill: flare_core::Rgba8,
    },
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
        /// The file's bytes and what decoding them gave, with its waveform.
        result: Result<(Vec<u8>, flare_audio::Decoded), String>,
    },
    SoundDecoded {
        sound: SoundId,
        result: Result<flare_audio::Decoded, String>,
    },
    Text {
        id: u64,
        result: Result<flare_document::Shape, String>,
    },
    /// The font families there are, sorted.
    Families(Vec<String>),
    /// Text being typed: its shape so far (none while it has no letters) and its caret.
    TextPreview {
        id: u64,
        result: Result<(Option<flare_document::Shape>, flare_text::Caret), String>,
    },
    Rendered {
        id: u64,
        result: Result<Rendered, String>,
    },
    Saved {
        path: PathBuf,
        document: Arc<Document>,
        result: Result<crate::Saved, String>,
    },
    Opened {
        id: u64,
        path: PathBuf,
        recovery: bool,
        result: Result<crate::Loaded, String>,
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
    /// What a wand click picked; `None` when it took every pixel away.
    WandPicked {
        id: u64,
        result: Result<Option<crate::WandPicked>, String>,
    },
    /// The commands that clear the picked pixels, made from `document`: they apply to that
    /// document only.
    WandCleared {
        id: u64,
        document: Arc<Document>,
        result: Result<Vec<flare_commands::Command>, String>,
    },
    /// The next edit of a Blur or Smudge stroke, made for `document`: it applies to that document
    /// only, and the stroke waits until the window sends a document with it applied.
    BlendEdited {
        id: u64,
        document: Arc<Document>,
        result: flare_tools::blend::BlendEdit,
    },
    /// What ffprobe said about a movie to bring in.
    VideoProbed {
        id: u64,
        path: PathBuf,
        result: Result<flare_tools::video::Source, String>,
    },
    /// A movie brought in has been read, or stopped, or could not be.
    VideoRead {
        id: u64,
        outcome: crate::video_import::VideoOutcome,
    },
    /// A Blur or Smudge stroke is over: every change it made has been handed back, or `error`
    /// says why it stopped (no bitmap where it started, or the bitmap changed under it).
    BlendEnded { id: u64, error: Option<String> },
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
