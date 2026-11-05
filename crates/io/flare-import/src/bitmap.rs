//! Raster images as projects: a PNG as a one-frame project, a GIF as its frames (delays become
//! the frame rate and hold counts), and a sequence of numbered files as one layer with a
//! keyframe per file. Each distinct image is stored once in the document's bitmap library.

use std::collections::HashMap;
use std::num::NonZeroU32;

use flare_core::{
    BitmapId, DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, Transform2D,
};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, MAX_BITMAP_SIDE, MAX_STAGE_SIDE,
    Scene, Stage, Timeline,
};

use crate::xfl::{Diagnostic, ImportError, Imported, Severity};

/// Most frames an animated GIF or a sequence may hold.
pub const MAX_IMAGE_FRAMES: usize = 2_000;
/// Most decoded pixel bytes one import may hold, across every frame, before duplicates merge.
const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;

/// A decoded picture: RGBA8 straight alpha, and the time it shows (GIF only), in hundredths.
struct Picture {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    delay_cs: u32,
}

fn bad(file: &str, why: impl std::fmt::Display) -> ImportError {
    ImportError::Foreign(format!("{file}: {why}"))
}

fn check_size(file: &str, width: u32, height: u32) -> Result<(), ImportError> {
    if width == 0 || height == 0 || width > MAX_BITMAP_SIDE || height > MAX_BITMAP_SIDE {
        return Err(bad(
            file,
            format!("{width}x{height} pixels; a bitmap is at most {MAX_BITMAP_SIDE} on a side"),
        ));
    }
    if u64::from(width) * u64::from(height) > flare_document::MAX_BITMAP_PIXELS {
        return Err(bad(file, "too many pixels"));
    }
    Ok(())
}

fn decode_png(file: &str, bytes: &[u8]) -> Result<Picture, ImportError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| bad(file, e))?;
    let (w, h) = (reader.info().width, reader.info().height);
    // Checked before the buffer is sized from the header.
    check_size(file, w, h)?;
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| bad(file, "too large"))?
    ];
    let info = reader.next_frame(&mut buffer).map_err(|e| bad(file, e))?;
    let data = &buffer[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err(bad(file, "palette was not expanded")),
    };
    Ok(Picture {
        width: w,
        height: h,
        rgba,
        delay_cs: 0,
    })
}

/// GIF frames composed onto the logical screen, honouring each frame's disposal.
fn decode_gif(file: &str, bytes: &[u8]) -> Result<Vec<Picture>, ImportError> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options
        .read_info(std::io::Cursor::new(bytes))
        .map_err(|e| bad(file, e))?;
    let (w, h) = (u32::from(decoder.width()), u32::from(decoder.height()));
    check_size(file, w, h)?;
    let frame_bytes = u64::from(w) * u64::from(h) * 4;
    let mut canvas = vec![0_u8; frame_bytes as usize];
    let mut out = Vec::new();
    let mut total = 0_u64;
    while let Some(frame) = decoder.read_next_frame().map_err(|e| bad(file, e))? {
        if out.len() >= MAX_IMAGE_FRAMES {
            return Err(bad(file, format!("more than {MAX_IMAGE_FRAMES} frames")));
        }
        total += frame_bytes;
        if total > MAX_DECODED_BYTES {
            return Err(bad(file, "the frames hold too many pixels"));
        }
        let before = (frame.dispose == gif::DisposalMethod::Previous).then(|| canvas.clone());
        let (fx, fy) = (usize::from(frame.left), usize::from(frame.top));
        let (fw, fh) = (usize::from(frame.width), usize::from(frame.height));
        for row in 0..fh {
            let y = fy + row;
            if y >= h as usize {
                break;
            }
            for col in 0..fw {
                let x = fx + col;
                if x >= w as usize {
                    break;
                }
                let src = (row * fw + col) * 4;
                if frame.buffer[src + 3] != 0 {
                    let dst = (y * w as usize + x) * 4;
                    canvas[dst..dst + 4].copy_from_slice(&frame.buffer[src..src + 4]);
                }
            }
        }
        out.push(Picture {
            width: w,
            height: h,
            rgba: canvas.clone(),
            delay_cs: u32::from(frame.delay),
        });
        match frame.dispose {
            gif::DisposalMethod::Background => {
                for row in 0..fh.min((h as usize).saturating_sub(fy)) {
                    let start = ((fy + row) * w as usize + fx) * 4;
                    let end = (start + fw * 4).min((fy + row + 1) * w as usize * 4);
                    canvas[start.min(end)..end].fill(0);
                }
            }
            gif::DisposalMethod::Previous => {
                if let Some(saved) = before {
                    canvas = saved;
                }
            }
            _ => {}
        }
    }
    if out.is_empty() {
        return Err(bad(file, "the GIF has no frames"));
    }
    Ok(out)
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Builds the project: one layer, one keyframe per picture held for `holds[i]` frames. Identical
/// pictures share one bitmap.
fn build(
    name: &str,
    pictures: Vec<Picture>,
    holds: &[u32],
    rate: FrameRate,
) -> Result<Imported, ImportError> {
    let (w, h) = pictures
        .iter()
        .fold((1, 1), |(w, h), p| (w.max(p.width), h.max(p.height)));
    let mut library: Vec<Bitmap> = Vec::new();
    let mut seen: HashMap<[u8; 32], BitmapId> = HashMap::new();
    let mut spans = Vec::new();
    let mut start = 0_u32;
    for (i, (picture, hold)) in pictures.into_iter().zip(holds).enumerate() {
        let bitmap = Bitmap::new(
            BitmapId::new(),
            format!("{name} {}", i + 1),
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
        let hold = NonZeroU32::new(*hold).unwrap_or(NonZeroU32::MIN);
        let content = Content::Bitmap(BitmapInstance::new(
            InstanceId::new(),
            id,
            Transform2D::IDENTITY,
        ));
        spans.push(FrameSpan::new(start, hold, content));
        start = start.saturating_add(hold.get());
    }
    let layer = Layer::new(LayerId::new(), name, spans).map_err(|e| bad(name, e))?;
    let scene = Scene::new(SceneId::new(), name, Timeline::new(vec![layer]));
    let stage = Stage::new(w.min(MAX_STAGE_SIDE), h.min(MAX_STAGE_SIDE), Rgba8::WHITE)
        .map_err(|e| bad(name, e))?;
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .map_err(ImportError::Invalid)?
        .with_stage(stage)
        .with_bitmaps(library)
        .map_err(ImportError::Invalid)?;
    Ok(Imported {
        document,
        diagnostics: vec![Diagnostic {
            severity: Severity::Info,
            location: name.to_owned(),
            message: "imported as bitmaps".into(),
        }],
    })
}

fn default_rate() -> Result<FrameRate, ImportError> {
    FrameRate::new(24, 1).map_err(|e| bad("frame rate", e))
}

/// A PNG as a one-frame project.
pub(crate) fn import_png(name: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let picture = decode_png(name, bytes)?;
    build(name, vec![picture], &[1], default_rate()?)
}

/// An animated (or still) GIF: the frame rate is the finest the delays need, up to 100 a second.
pub(crate) fn import_gif(name: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let pictures = decode_gif(name, bytes)?;
    // A delay of 0 or 1 hundredth is shown by browsers as 10: do the same.
    let delays: Vec<u32> = pictures
        .iter()
        .map(|p| if p.delay_cs < 2 { 10 } else { p.delay_cs })
        .collect();
    let step = delays.iter().fold(0, |g, &d| gcd(g, d)).max(1);
    let rate = FrameRate::new((100 / step).max(1), 1).map_err(|e| bad(name, e))?;
    let holds: Vec<u32> = delays.iter().map(|d| d / step).collect();
    build(name, pictures, &holds, rate)
}

/// Numbered picture files as one layer, in natural order (`2.png` before `10.png`): one keyframe
/// per file, 24 frames a second. Only PNG is read.
pub fn import_image_sequence(files: Vec<(String, Vec<u8>)>) -> Result<Imported, ImportError> {
    if files.is_empty() {
        return Err(bad("sequence", "no files"));
    }
    if files.len() > MAX_IMAGE_FRAMES {
        return Err(bad(
            "sequence",
            format!("more than {MAX_IMAGE_FRAMES} files"),
        ));
    }
    let mut files = files;
    files.sort_by(|a, b| natural_cmp(&a.0, &b.0));
    let mut total = 0_u64;
    let mut pictures = Vec::with_capacity(files.len());
    for (name, bytes) in &files {
        let picture = decode_png(name, bytes)?;
        total += picture.rgba.len() as u64;
        if total > MAX_DECODED_BYTES {
            return Err(bad(name, "the sequence holds too many pixels"));
        }
        pictures.push(picture);
    }
    let holds = vec![1; pictures.len()];
    build("Sequence", pictures, &holds, default_rate()?)
}

/// Compares names with runs of digits as numbers.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let run = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut digits = String::new();
                    while let Some(c) = it.next_if(char::is_ascii_digit) {
                        digits.push(c);
                    }
                    digits.trim_start_matches('0').to_owned()
                };
                let (x, y) = (run(&mut a), run(&mut b));
                let order = x.len().cmp(&y.len()).then_with(|| x.cmp(&y));
                if order != std::cmp::Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                a.next();
                b.next();
            }
        }
    }
}
