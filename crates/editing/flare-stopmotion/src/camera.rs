/// Largest encoded capture a camera may hand over, so a source cannot exhaust memory.
pub const MAX_CAPTURE_BYTES: usize = 32 * 1024 * 1024;
/// The feature names the named errors carry. The command line and MCP server print them.
pub const NOT_YET_CAPTURE: &str = "camera capture";
pub const NOT_YET_LIVE_VIEW: &str = "camera live view";

/// Why a camera could not give a frame. Every variant names what happened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CameraError {
    #[error("{feature} is not yet implemented in Flare")]
    NotYetImplemented { feature: &'static str },
    #[error("a capture of {bytes} bytes is over the {max}-byte limit")]
    CaptureTooLarge { bytes: usize, max: usize },
}

/// A device that gives frames. A backend (a webcam, a DSLR over its SDK, a file replay for
/// tests) implements this; the session never talks to a device itself. Captures come back
/// encoded, so the bytes are the caller's to store.
pub trait CameraSource {
    fn name(&self) -> &str;

    fn capture(&mut self) -> Result<Vec<u8>, CameraError>;

    fn start_live_view(&mut self) -> Result<(), CameraError>;
}

/// Checks a capture a backend produced against [`MAX_CAPTURE_BYTES`]. Backends call it before
/// returning, so an oversized frame is refused in one place.
pub fn accept_capture(bytes: Vec<u8>) -> Result<Vec<u8>, CameraError> {
    if bytes.len() > MAX_CAPTURE_BYTES {
        return Err(CameraError::CaptureTooLarge {
            bytes: bytes.len(),
            max: MAX_CAPTURE_BYTES,
        });
    }
    Ok(bytes)
}

/// The source used when no camera backend is built: every request is refused by name.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NoCamera;

impl CameraSource for NoCamera {
    fn name(&self) -> &str {
        "no camera"
    }

    fn capture(&mut self) -> Result<Vec<u8>, CameraError> {
        Err(CameraError::NotYetImplemented {
            feature: NOT_YET_CAPTURE,
        })
    }

    fn start_live_view(&mut self) -> Result<(), CameraError> {
        Err(CameraError::NotYetImplemented {
            feature: NOT_YET_LIVE_VIEW,
        })
    }
}
