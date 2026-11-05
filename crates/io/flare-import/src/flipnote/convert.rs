//! A PPM as a Flare project: the two Flipnote layers and a paper layer under them, one frame for
//! each Flipnote frame. A frame's pixels are a bitmap in the document's library; identical
//! pictures share one bitmap, and a run of one bitmap is one span.

use std::collections::HashMap;

use flare_core::{
    BitmapId, DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, Transform2D,
};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Stage, Timeline,
};

use super::error::FlipnoteError;
use super::ppm::{FrameDecoder, HEIGHT, PALETTE, Ppm, WIDTH};
use crate::xfl::{Diagnostic, ImportError, Imported, Severity};

/// Most bytes of distinct pictures one import may hold; the same limit the image importers use.
const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;
const PICTURE_BYTES: u64 = (WIDTH * HEIGHT * 4) as u64;

fn foreign(file: &str, e: impl std::fmt::Display) -> ImportError {
    ImportError::Foreign(format!("{file}: {e}"))
}

fn colour(index: u8) -> [u8; 4] {
    PALETTE
        .get(usize::from(index))
        .copied()
        .unwrap_or([0, 0, 0, 0xFF])
}

/// One layer's pixels as RGBA: pen colour where the bit is set, transparent elsewhere.
fn pen_picture(bits: &[u8], pen: [u8; 4]) -> Vec<u8> {
    bits.iter()
        .flat_map(|&bit| if bit == 0 { [0, 0, 0, 0] } else { pen })
        .collect()
}

fn paper_picture(paper: [u8; 4]) -> Vec<u8> {
    paper.repeat(WIDTH * HEIGHT)
}

/// The document's bitmap library, with identical pictures stored once.
struct Library {
    file: String,
    bitmaps: Vec<Bitmap>,
    seen: HashMap<[u8; 32], BitmapId>,
    bytes: u64,
}

impl Library {
    fn intern(&mut self, rgba: Vec<u8>) -> Result<BitmapId, ImportError> {
        let bitmap = Bitmap::new(
            BitmapId::new(),
            format!("{} frame", self.file),
            WIDTH as u32,
            HEIGHT as u32,
            rgba,
        )
        .map_err(|e| foreign(&self.file, e))?;
        if let Some(id) = self.seen.get(bitmap.content_hash()) {
            return Ok(*id);
        }
        self.bytes += PICTURE_BYTES;
        if self.bytes > MAX_DECODED_BYTES {
            return Err(foreign(
                &self.file,
                FlipnoteError::TooLarge(MAX_DECODED_BYTES),
            ));
        }
        let id = bitmap.id();
        self.seen.insert(*bitmap.content_hash(), id);
        self.bitmaps.push(bitmap);
        Ok(id)
    }
}

/// Runs of one bitmap, as spans; a frame is one unit of the timeline.
fn spans(ids: &[BitmapId], file: &str) -> Result<Vec<FrameSpan>, ImportError> {
    let mut out = Vec::new();
    let mut start = 0_u32;
    for run in ids.chunk_by(|a, b| a == b) {
        let Some(&id) = run.first() else { continue };
        let len = u32::try_from(run.len()).map_err(|e| foreign(file, e))?;
        let duration = std::num::NonZeroU32::new(len)
            .ok_or_else(|| foreign(file, "an empty run of frames"))?;
        out.push(FrameSpan::new(
            start,
            duration,
            Content::Bitmap(BitmapInstance::new(
                InstanceId::new(),
                id,
                Transform2D::IDENTITY,
            )),
        ));
        start = start.saturating_add(len);
    }
    Ok(out)
}

pub(crate) fn convert(ppm: &Ppm<'_>, file: &str) -> Result<Imported, ImportError> {
    let (num, den) = ppm.frame_rate();
    let rate = FrameRate::new(num, den).map_err(|e| foreign(file, e))?;
    let mut library = Library {
        file: file.to_owned(),
        bitmaps: Vec::new(),
        seen: HashMap::new(),
        bytes: 0,
    };
    // Per frame: layer 1, layer 2, paper. Layer 1 is the top one.
    let mut per_layer: [Vec<BitmapId>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut dec = FrameDecoder::new();
    for index in 0..ppm.frame_count() {
        let info = ppm
            .decode_frame(&mut dec, index)
            .map_err(|e| foreign(file, e))?;
        let [paper, pen1, pen2] = info.palette.map(colour);
        let layers = dec.layers();
        let first = library.intern(pen_picture(&layers[0], pen1))?;
        let second = library.intern(pen_picture(&layers[1], pen2))?;
        let under = library.intern(paper_picture(paper))?;
        per_layer[0].push(first);
        per_layer[1].push(second);
        per_layer[2].push(under);
    }

    let visible = ppm.meta().map_or([true, true], |m| m.layer_visible);
    let mut layers = Vec::new();
    for (k, name) in ["Layer 1", "Layer 2", "Paper"].into_iter().enumerate() {
        let ids = per_layer.get(k).map(Vec::as_slice).unwrap_or_default();
        let shown = visible.get(k).copied().unwrap_or(true);
        let layer = Layer::new(LayerId::new(), name, spans(ids, file)?)
            .map_err(|e| foreign(file, e))?
            .with_visible(shown);
        layers.push(layer);
    }
    let scene = Scene::new(SceneId::new(), file, Timeline::new(layers));
    let stage =
        Stage::new(WIDTH as u32, HEIGHT as u32, Rgba8::WHITE).map_err(|e| foreign(file, e))?;
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .map_err(ImportError::Invalid)?
        .with_stage(stage)
        .with_bitmaps(library.bitmaps)
        .map_err(ImportError::Invalid)?;

    Ok(Imported {
        document,
        diagnostics: diagnostics(ppm, file),
    })
}

fn note(severity: Severity, file: &str, message: String) -> Diagnostic {
    Diagnostic {
        severity,
        location: file.to_owned(),
        message,
    }
}

fn diagnostics(ppm: &Ppm<'_>, file: &str) -> Vec<Diagnostic> {
    let (num, den) = ppm.frame_rate();
    let mut out = vec![note(
        Severity::Info,
        file,
        format!(
            "{} frames at {} a second, as two layers over the paper colour",
            ppm.frame_count(),
            if den == 1 {
                num.to_string()
            } else {
                format!("{num}/{den}")
            }
        ),
    )];
    if let Some(meta) = ppm.meta() {
        out.push(note(
            Severity::Info,
            file,
            format!(
                "made by {} (FSID {}), first made by {} (FSID {}), created at Unix time {}",
                meta.current.name,
                meta.current.fsid,
                meta.root.name,
                meta.root.fsid,
                meta.created_unix
            ),
        ));
        if meta.locked {
            out.push(note(
                Severity::Info,
                file,
                "locked by its author in Flipnote Studio; Flare opens it for editing".into(),
            ));
        }
    }
    let sound = ppm.sound();
    let tracks: Vec<usize> = std::iter::once(sound.bgm.len)
        .chain(sound.effects.iter().map(|s| s.len))
        .filter(|&len| len > 0)
        .collect();
    if !tracks.is_empty() {
        out.push(note(
            Severity::Unsupported,
            file,
            format!(
                "the sound is not imported: {} audio tracks have no Flare sound layer yet",
                tracks.len()
            ),
        ));
    }
    out.push(note(
        Severity::Info,
        file,
        "the thumbnail is not imported; the RSA signature is not checked".into(),
    ));
    out
}
