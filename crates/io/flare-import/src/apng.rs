//! Animated PNG (APNG): each frame is drawn onto the canvas the way the APNG specification says,
//! by its blend and dispose operations, and each whole canvas becomes one picture of the animation.

use std::io::Cursor;

use png::{BlendOp, DisposeOp, FrameControl};

use crate::bitmap::{MAX_DECODED_BYTES, MAX_IMAGE_FRAMES, Picture, bad, check_size, to_rgba};
use crate::xfl::ImportError;

/// Whether `bytes` are a PNG with an animation control chunk. A damaged header is not an
/// animation here; the still reader then reports the damage.
pub(crate) fn is_animated(bytes: &[u8]) -> bool {
    png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .is_ok_and(|reader| reader.info().animation_control().is_some())
}

/// Every frame of an APNG, composited, as whole-canvas pictures in order.
pub(crate) fn decode(file: &str, bytes: &[u8]) -> Result<Vec<Picture>, ImportError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| bad(file, e))?;
    let (width, height) = (reader.info().width, reader.info().height);
    check_size(file, width, height)?;
    let frames = reader
        .info()
        .animation_control()
        .map_or(0, |control| control.num_frames as usize);
    if frames == 0 || frames > MAX_IMAGE_FRAMES {
        return Err(bad(
            file,
            format!("{frames} frames; an animation holds 1 to {MAX_IMAGE_FRAMES}"),
        ));
    }
    // Every frame is a whole canvas, so the total is checked before the first is made.
    let canvas_bytes = u64::from(width) * u64::from(height) * 4;
    if canvas_bytes * frames as u64 > MAX_DECODED_BYTES {
        return Err(bad(file, "too many pixels across the frames"));
    }
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| bad(file, "too large"))?
    ];
    let mut canvas = vec![0u8; canvas_bytes as usize];
    let mut pictures = Vec::with_capacity(frames);
    for index in 0..frames {
        // The reader already stands on the first frame's data when it is opened, unless that data
        // is the default image outside the animation (no control chunk before it): then the first
        // frame comes after it, and `next_frame_info` skips the default image.
        let control = match reader.info().frame_control().copied() {
            Some(first) if index == 0 => first,
            _ => *reader.next_frame_info().map_err(|e| bad(file, e))?,
        };
        let info = reader.next_frame(&mut buffer).map_err(|e| bad(file, e))?;
        let subframe = to_rgba(file, info.color_type, &buffer[..info.buffer_size()])?;
        let rect = Rect::of(&control);
        if rect.x + rect.width > width || rect.y + rect.height > height {
            return Err(bad(file, "a frame lies outside the picture"));
        }
        if subframe.len() as u64 != u64::from(rect.width) * u64::from(rect.height) * 4 {
            return Err(bad(file, "a frame's pixels do not match its size"));
        }
        let before = (control.dispose_op == DisposeOp::Previous).then(|| canvas.clone());
        place(&mut canvas, width, &subframe, rect, control.blend_op);
        pictures.push(Picture {
            width,
            height,
            rgba: canvas.clone(),
            delay_cs: delay_cs(&control),
        });
        match control.dispose_op {
            DisposeOp::None => {}
            DisposeOp::Background => clear(&mut canvas, width, rect),
            DisposeOp::Previous => {
                if let Some(saved) = before {
                    canvas = saved;
                }
            }
        }
    }
    Ok(pictures)
}

/// Where a frame sits on the canvas.
#[derive(Clone, Copy)]
struct Rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Rect {
    fn of(control: &FrameControl) -> Self {
        Rect {
            x: control.x_offset,
            y: control.y_offset,
            width: control.width,
            height: control.height,
        }
    }
}

/// Draws a frame onto the canvas: replaced, or blended over what is there.
fn place(canvas: &mut [u8], canvas_width: u32, subframe: &[u8], rect: Rect, blend: BlendOp) {
    for row in 0..rect.height as usize {
        for col in 0..rect.width as usize {
            let src = (row * rect.width as usize + col) * 4;
            let dst = ((rect.y as usize + row) * canvas_width as usize + rect.x as usize + col) * 4;
            match blend {
                BlendOp::Source => canvas[dst..dst + 4].copy_from_slice(&subframe[src..src + 4]),
                BlendOp::Over => over(&mut canvas[dst..dst + 4], &subframe[src..src + 4]),
            }
        }
    }
}

/// Source-over on straight-alpha pixels.
fn over(dst: &mut [u8], src: &[u8]) {
    let sa = f32::from(src[3]) / 255.0;
    let da = f32::from(dst[3]) / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        dst.fill(0);
        return;
    }
    for c in 0..3 {
        let v = (f32::from(src[c]) * sa + f32::from(dst[c]) * da * (1.0 - sa)) / out_a;
        dst[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Clears a frame's rectangle to transparent, the APNG background disposal.
fn clear(canvas: &mut [u8], canvas_width: u32, rect: Rect) {
    for row in 0..rect.height as usize {
        let start = ((rect.y as usize + row) * canvas_width as usize + rect.x as usize) * 4;
        canvas[start..start + rect.width as usize * 4].fill(0);
    }
}

/// The frame's time in hundredths of a second; a denominator of 0 means 100.
fn delay_cs(control: &FrameControl) -> u32 {
    let den = if control.delay_den == 0 {
        100
    } else {
        u32::from(control.delay_den)
    };
    (u32::from(control.delay_num) * 100 + den / 2) / den
}
