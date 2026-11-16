//! A camera for tests and for trying the flow without hardware: a deterministic pattern
//! whose captures differ, so a session of captures is not a session of identical frames.
//! It hands raw RGBA8, the format [`crate::capture`] keeps, and says so in its name.

use crate::NOT_SIMULATED_LIVE_VIEW;
use crate::camera::{CameraError, CameraSource, accept_capture};

/// The pattern sheet's size, in pixels. Small on purpose: a store of these fits its
/// budget thousands of times over.
pub const PATTERN_WIDTH: u32 = 64;
pub const PATTERN_HEIGHT: u32 = 48;

/// Counts captures. Every capture draws the count into the sheet's first pixel, so
/// consecutive captures are distinguishable and a re-shoot is visibly different.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SimulatedCamera {
    captured: u32,
}

impl SimulatedCamera {
    /// How many captures this camera has taken so far.
    pub fn captures(&self) -> u32 {
        self.captured
    }
}

impl CameraSource for SimulatedCamera {
    fn name(&self) -> &str {
        "simulated camera"
    }

    fn capture(&mut self) -> Result<Vec<u8>, CameraError> {
        self.captured = self.captured.saturating_add(1);
        let count = self.captured;
        let mut pixels = vec![0u8; (PATTERN_WIDTH * PATTERN_HEIGHT) as usize * 4];
        // A grey ramp across the sheet, and the capture count's low byte in the red of the
        // first pixel: enough for a test to tell one capture from the next.
        for (at, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x = (at % PATTERN_WIDTH as usize) as u8;
            let y = (at / PATTERN_WIDTH as usize) as u8;
            *pixel = [x.wrapping_mul(3), y.wrapping_mul(5), 128, 255];
        }
        pixels[0] = (count & 0xFF) as u8;
        accept_capture(pixels)
    }

    fn start_live_view(&mut self) -> Result<(), CameraError> {
        Err(CameraError::NotYetImplemented {
            feature: NOT_SIMULATED_LIVE_VIEW,
        })
    }
}

/// Where two captures differ, as the live-difference overlay shows it: for every pixel,
/// the live pixel's colour with an alpha of 255 where any channel (the alpha included)
/// differs from the previous capture, and 0 where the two captures agree — so the overlay
/// paints only what moved. Sizes must match and both must be whole RGBA8 pixels.
pub fn live_difference(previous: &[u8], live: &[u8]) -> Result<Vec<u8>, StopMissingPixels> {
    if previous.len() != live.len() || !previous.len().is_multiple_of(4) {
        return Err(StopMissingPixels {
            previous: previous.len(),
            live: live.len(),
        });
    }
    let mut out = live.to_vec();
    for (at, pixel) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let before = &previous[at * 4..at * 4 + 4];
        let moved = before.iter().zip(pixel.iter()).any(|(was, now)| was != now);
        pixel[3] = if moved { 255 } else { 0 };
    }
    Ok(out)
}

/// Why a live difference could not be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "the previous capture is {previous} bytes and the live one {live} bytes; they must be the \
     same length and a multiple of 4"
)]
pub struct StopMissingPixels {
    previous: usize,
    live: usize,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_captures_differ_in_their_first_pixel() {
        let mut camera = SimulatedCamera::default();
        let first = camera.capture().unwrap();
        let second = camera.capture().unwrap();
        assert_ne!(first[0], second[0]);
        assert_eq!(camera.captures(), 2);
        // The rest of the sheet is the same ramp every time.
        assert_eq!(first[4..], second[4..]);
    }

    #[test]
    fn the_difference_paints_what_moved_and_nothing_else() {
        let mut previous = vec![10u8, 20, 30, 255];
        previous.extend_from_slice(&[10u8, 20, 30, 255]);
        let mut live = previous.clone();
        live[0] = 40;
        let diff = live_difference(&previous, &live).unwrap();
        assert_eq!(&diff[0..4], &[40, 20, 30, 255]);
        assert_eq!(&diff[4..8], &[10, 20, 30, 0]);
        // The alpha difference alone counts as movement.
        let mut faded = previous.clone();
        faded[3] = 128;
        let diff = live_difference(&previous, &faded).unwrap();
        assert_eq!(diff[3], 255);
        // Mismatched sizes are refused.
        assert!(live_difference(&previous, &live[..4]).is_err());
        assert!(live_difference(&[1, 2, 3], &previous).is_err());
    }
}
