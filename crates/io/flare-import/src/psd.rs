//! Photoshop (PSD) files. Each visible layer comes in as a bitmap layer at its place on the canvas;
//! a file with no such layer comes in as its flattened picture, one frame. The `psd` crate reads
//! the file. Sizes are checked before any pixel is made, and the reading runs behind a panic
//! boundary, because a damaged file can make that parser index past its end.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::panic::{AssertUnwindSafe, catch_unwind};

use flare_core::{
    BitmapId, DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, Transform2D,
};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, MAX_STAGE_SIDE, Scene, Stage,
    Timeline,
};
use glamx::{DAffine2, DVec2};

use crate::bitmap::{MAX_DECODED_BYTES, bad, check_size};
use crate::xfl::{Diagnostic, ImportError, Imported, Severity};

/// One layer's picture and where it sits on the canvas.
struct LayerPicture {
    name: String,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    rgba: Vec<u8>,
}

/// Most layers a PSD may hold.
const MAX_PSD_LAYERS: u64 = 1_000;

/// Runs a call into the `psd` crate, turning a panic into an import error.
fn guarded<T>(name: &str, call: impl FnOnce() -> Result<T, String>) -> Result<T, ImportError> {
    match catch_unwind(AssertUnwindSafe(call)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(why)) => Err(bad(name, why)),
        Err(_) => Err(bad(name, "the file is damaged: the reader stopped at it")),
    }
}

pub(crate) fn import(name: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let psd = guarded(name, || {
        psd::Psd::from_bytes(bytes).map_err(|e| e.to_string())
    })?;
    let (width, height) = (psd.width(), psd.height());
    check_size(name, width, height)?;
    // The crate returns every layer as a whole canvas, transparent outside the layer, so the work is
    // the canvas times the layers, and that is bounded before the first one is made.
    let canvas_bytes = u64::from(width) * u64::from(height) * 4;
    let layer_count = psd.layers().len() as u64;
    if layer_count > MAX_PSD_LAYERS || canvas_bytes * layer_count > MAX_DECODED_BYTES {
        return Err(bad(name, "too many layers for the picture's size"));
    }
    let mut diagnostics = Vec::new();
    let mut pictures = Vec::new();
    for layer in psd.layers() {
        // A group or an empty layer has no picture of its own.
        if layer.width() == 0 || layer.height() == 0 {
            continue;
        }
        // Bit 1 of a layer's flags marks it hidden (as ImageMagick and ag-psd read it). The crate's
        // `visible()` reports that same bit, so it is read here as hidden.
        if layer.visible() {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                location: name.to_owned(),
                message: format!(
                    "the hidden layer \"{}\" is not imported: a layer has no visibility yet",
                    layer.name()
                ),
            });
            continue;
        }
        let canvas_rgba = guarded(name, || Ok(layer.rgba()))?;
        if canvas_rgba.len() as u64 != canvas_bytes {
            return Err(bad(name, "a layer does not match the picture's size"));
        }
        // The part of the layer on the canvas, cut out of the whole-canvas picture.
        let (left, top) = (layer.layer_left(), layer.layer_top());
        let x0 = left.max(0);
        let y0 = top.max(0);
        let x1 = (left + i32::from(layer.width())).min(width as i32);
        let y1 = (top + i32::from(layer.height())).min(height as i32);
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
        let row = w as usize * 4;
        let mut rgba = Vec::with_capacity(row * h as usize);
        for y in y0..y1 {
            let start = (y as usize * width as usize + x0 as usize) * 4;
            rgba.extend_from_slice(&canvas_rgba[start..start + row]);
        }
        pictures.push(LayerPicture {
            name: layer.name().to_owned(),
            width: w,
            height: h,
            x: x0,
            y: y0,
            rgba,
        });
    }
    if pictures.is_empty() {
        let rgba = guarded(name, || Ok(psd.rgba()))?;
        if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return Err(bad(name, "the picture does not match its size"));
        }
        pictures.push(LayerPicture {
            name: name.to_owned(),
            width,
            height,
            x: 0,
            y: 0,
            rgba,
        });
    }
    build_layers(name, width, height, pictures, diagnostics)
}

/// The layers as one-frame bitmap layers in the file's order, on a stage of the file's size.
fn build_layers(
    name: &str,
    width: u32,
    height: u32,
    pictures: Vec<LayerPicture>,
    diagnostics: Vec<Diagnostic>,
) -> Result<Imported, ImportError> {
    let mut library: Vec<Bitmap> = Vec::new();
    let mut seen: HashMap<[u8; 32], BitmapId> = HashMap::new();
    let mut layers = Vec::new();
    for (index, picture) in pictures.into_iter().enumerate() {
        let layer_name = if picture.name.trim().is_empty() {
            format!("Layer {}", index + 1)
        } else {
            picture.name
        };
        let bitmap = Bitmap::new(
            BitmapId::new(),
            layer_name.clone(),
            picture.width,
            picture.height,
            picture.rgba,
        )
        .map_err(|e| bad(name, e))?;
        let id = *seen.entry(*bitmap.content_hash()).or_insert_with(|| {
            let id = bitmap.id();
            library.push(bitmap);
            id
        });
        let offset =
            DAffine2::from_translation(DVec2::new(f64::from(picture.x), f64::from(picture.y)));
        let transform = Transform2D::new(offset).map_err(|e| bad(name, e))?;
        let content = Content::Bitmap(BitmapInstance::new(InstanceId::new(), id, transform));
        let span = FrameSpan::new(0, NonZeroU32::MIN, content);
        layers.push(Layer::new(LayerId::new(), layer_name, vec![span]).map_err(|e| bad(name, e))?);
    }
    let scene = Scene::new(SceneId::new(), name, Timeline::new(layers));
    let stage = Stage::new(
        width.min(MAX_STAGE_SIDE),
        height.min(MAX_STAGE_SIDE),
        Rgba8::WHITE,
    )
    .map_err(|e| bad(name, e))?;
    let rate = FrameRate::new(24, 1).map_err(|e| bad(name, e))?;
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .map_err(ImportError::Invalid)?
        .with_stage(stage)
        .with_bitmaps(library)
        .map_err(ImportError::Invalid)?;
    let mut all = vec![Diagnostic {
        severity: Severity::Info,
        location: name.to_owned(),
        message: "imported as bitmap layers".into(),
    }];
    all.extend(diagnostics);
    Ok(Imported {
        document,
        diagnostics: all,
    })
}
