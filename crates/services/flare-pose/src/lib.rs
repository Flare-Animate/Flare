//! Finding a person's pose in a picture, for the reference mannequin to copy: where their head,
//! neck, shoulders, elbows, hands, hips, knees and feet are, as marks on the picture
//! (`flare_mannequin::Marks`), which `Mannequin::copy_pose` turns into a pose.
//!
//! The joints are found by MoveNet SinglePose Lightning, Google's pose estimation network
//! (Apache-2.0; `data/LICENSE-MOVENET.txt`), built into Flare and run on the CPU by RTen, a
//! pure-Rust runtime, so it works offline, the same in the desktop app, the browser, the command
//! line and the MCP server. It looks at a square of 192 by 192 pixels: first the whole picture,
//! then closer, round where it found the person, while that makes it surer, which finds a small
//! figure in a large picture far better. A part it is not sure of is left unmarked (and keeps the figure's pose);
//! a picture with no one in it finds nothing. One look takes about 10 ms; a picture is two or three.

mod file;
mod look;
mod marks;

use std::sync::OnceLock;

use flare_mannequin::Marks;
use rten::Model;
use thiserror::Error;

pub use file::{LOOK_SIDE, find_pose_in_file};
pub use marks::MIN_SCORE;

/// Most closer looks after the first.
const CLOSER_LOOKS: usize = 2;
/// The network, built in.
const MODEL: &[u8] = include_bytes!("../data/movenet-singlepose-lightning.onnx");

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PoseError {
    #[error("the picture's pixels do not match its size")]
    Pixels,
    #[error("the pose detector could not run: {0}")]
    Model(String),
}

/// A pose found in a picture: the marks, in the picture's own units, and how sure the
/// detector was of the body's joints on the whole (0 to 1).
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub marks: Marks,
    pub confidence: f64,
}

/// A picture to look in: straight RGBA pixels, `size` across and down. A see-through pixel is
/// seen against black.
#[derive(Debug, Clone, Copy)]
pub struct Pixels<'a> {
    pub rgba: &'a [u8],
    pub size: [u32; 2],
}

fn model() -> Result<&'static Model, PoseError> {
    static LOADED: OnceLock<Result<Model, String>> = OnceLock::new();
    LOADED
        .get_or_init(|| Model::load_static_slice(MODEL).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| PoseError::Model(e.clone()))
}

/// Finds the pose of the person in `pixels`, its marks given in the units of a picture
/// `picture` across and down (the size the pixels were made smaller from, or the same). None
/// when no one is found.
pub fn find_pose(pixels: Pixels<'_>, picture: [f64; 2]) -> Result<Option<Found>, PoseError> {
    let [w, h] = pixels.size;
    let expected = u64::from(w) * u64::from(h) * 4;
    if w == 0 || h == 0 || pixels.rgba.len() as u64 != expected {
        return Err(PoseError::Pixels);
    }
    if !picture.iter().all(|v| v.is_finite() && *v > 0.0) {
        return Err(PoseError::Pixels);
    }
    let model = model()?;
    let whole = look::Window::whole(pixels.size);
    let first = look::run(model, pixels, whole)?;
    let mut best = first;
    // Closer while it gets surer: the first close look may still be off when the person was
    // small, and the second puts it right.
    for _ in 0..CLOSER_LOOKS {
        let Some(closer) = marks::round_person(&best, pixels.size) else {
            break;
        };
        let next = look::run(model, pixels, closer)?;
        if marks::confidence(&next) <= marks::confidence(&best) {
            break;
        }
        best = next;
    }
    let scale = [picture[0] / f64::from(w), picture[1] / f64::from(h)];
    Ok(marks::to_marks(&best, scale))
}

#[cfg(test)]
mod tests;
