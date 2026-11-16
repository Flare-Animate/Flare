//! The capture flow: pixels from a [`CameraSource`] land in a bounded [`FrameStore`],
//! keyed by the frame ids a session hands out, so a session's holds and a store's pixels
//! stay two halves of one state the caller keeps together.
//!
//! The store holds raw RGBA8 captures (4 bytes per pixel, straight alpha, row-major), the
//! one pixel format this crate's overlays work on; a backend that captures encoded bytes
//! decodes them before handing them to [`capture_next`], and its error comes back as
//! [`StopMotionError::Camera`].

use crate::camera::{CameraSource, MAX_CAPTURE_BYTES};
use crate::error::StopMotionError;
use crate::session::{FrameId, StopMotionSession};

/// The most pixels one capture may carry, at 4 bytes each — the same ceiling the encoded
/// captures have, so no backend can hand the store more than it may keep.
pub const MAX_CAPTURE_PIXELS: usize = MAX_CAPTURE_BYTES / 4;
/// The most bytes of captures one store keeps. A quarter of a gigabyte is a few thousand
/// phone-resolution frames; past it the capture is refused and the session is untouched.
pub const MAX_STORE_BYTES: usize = 256 * 1024 * 1024;

/// Captured pixels by frame id. Ids are never reused within a session, so a store entry
/// always belongs to the one frame that carries its id.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FrameStore {
    frames: Vec<(FrameId, Vec<u8>)>,
    bytes: usize,
}

impl FrameStore {
    /// Puts pixels in for `id`, replacing whatever was there. The capture must be a whole
    /// number of RGBA8 pixels and the store must stay under its budget.
    pub fn insert(&mut self, id: FrameId, pixels: Vec<u8>) -> Result<(), StopMotionError> {
        if !pixels.len().is_multiple_of(4) {
            return Err(StopMotionError::BadCapturePixels {
                bytes: pixels.len(),
            });
        }
        if pixels.len() > MAX_CAPTURE_BYTES {
            return Err(StopMotionError::CaptureTooLarge {
                bytes: pixels.len(),
                max: MAX_CAPTURE_BYTES,
            });
        }
        let replaced = self
            .frames
            .iter()
            .position(|(stored, _)| *stored == id)
            .map(|at| (at, self.frames[at].1.len()));
        let grown = pixels.len() + self.bytes - replaced.map_or(0, |(_, len)| len);
        if grown > MAX_STORE_BYTES {
            return Err(StopMotionError::StoreFull {
                bytes: grown,
                max: MAX_STORE_BYTES,
            });
        }
        self.bytes = grown;
        match replaced {
            Some((at, _)) => self.frames[at].1 = pixels,
            None => self.frames.push((id, pixels)),
        }
        Ok(())
    }

    /// The pixels of the frame at `id`.
    pub fn get(&self, id: FrameId) -> Result<&[u8], StopMotionError> {
        self.frames
            .iter()
            .find(|(stored, _)| *stored == id)
            .map(|(_, pixels)| pixels.as_slice())
            .ok_or(StopMotionError::UnknownFrame(id))
    }

    /// Drops the pixels of the frame at `id`; a capture may then claim the room again.
    pub fn remove(&mut self, id: FrameId) -> Result<Vec<u8>, StopMotionError> {
        let at = self
            .frames
            .iter()
            .position(|(stored, _)| *stored == id)
            .ok_or(StopMotionError::UnknownFrame(id))?;
        let (_, pixels) = self.frames.remove(at);
        self.bytes -= pixels.len();
        Ok(pixels)
    }

    /// How many bytes of captures the store currently keeps.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Drops every capture, leaving the store empty and its budget unspent.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.bytes = 0;
    }
}

/// Captures from `source`, records the frame at the end of `session`, and keeps the pixels
/// in `store`. A camera failure or a store refusal takes the just-added frame back out, so
/// one failed capture changes neither half of the state.
pub fn capture_next(
    source: &mut dyn CameraSource,
    session: &mut StopMotionSession,
    store: &mut FrameStore,
) -> Result<FrameId, StopMotionError> {
    let id = session.add_frame()?;
    let pixels = match source.capture() {
        Ok(pixels) => pixels,
        Err(error) => {
            let _ = session.remove_frame(id);
            return Err(StopMotionError::Camera(error));
        }
    };
    if let Err(error) = store.insert(id, pixels) {
        let _ = session.remove_frame(id);
        return Err(error);
    }
    Ok(id)
}

/// Captures over the frame at `id`: the pixels are replaced, the session's hold and
/// position stay as they were. An unknown id is refused by the store first, so a
/// re-shoot of a removed frame cannot invent one.
pub fn recapture(
    source: &mut dyn CameraSource,
    session: &StopMotionSession,
    store: &mut FrameStore,
    id: FrameId,
) -> Result<(), StopMotionError> {
    // The session is asked first so a bogus id never spends a camera capture.
    session_check(session, id)?;
    let pixels = source.capture().map_err(StopMotionError::Camera)?;
    store.insert(id, pixels)?;
    Ok(())
}

fn session_check(session: &StopMotionSession, id: FrameId) -> Result<(), StopMotionError> {
    if session.frames().iter().any(|frame| frame.id() == id) {
        Ok(())
    } else {
        Err(StopMotionError::UnknownFrame(id))
    }
}

/// Appends a frame carrying the pixels of the session's last frame: a new frame id with
/// the same pixels, the default hold, and no shutter — this is how a set-up that moved
/// between shots records the still moment.
pub fn duplicate_last(
    session: &mut StopMotionSession,
    store: &mut FrameStore,
) -> Result<FrameId, StopMotionError> {
    let last = session
        .frames()
        .last()
        .map(|frame| frame.id())
        .ok_or(StopMotionError::NoFrames)?;
    let pixels = store.get(last)?.to_vec();
    let id = session.add_frame()?;
    if let Err(error) = store.insert(id, pixels) {
        let _ = session.remove_frame(id);
        return Err(error);
    }
    Ok(id)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::camera::CameraError;
    use crate::session::StopMotionSession;

    fn pixel(r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        vec![r, g, b, a]
    }

    struct Fixed(Vec<u8>);
    impl CameraSource for Fixed {
        fn name(&self) -> &str {
            "fixed"
        }
        fn capture(&mut self) -> Result<Vec<u8>, CameraError> {
            Ok(self.0.clone())
        }
        fn start_live_view(&mut self) -> Result<(), CameraError> {
            Ok(())
        }
    }

    /// Fresh frame ids for the store tests, handed out the way a session does.
    fn ids() -> impl Iterator<Item = FrameId> {
        let mut session = StopMotionSession::new(12, 1).unwrap();
        std::iter::from_fn(move || session.add_frame().ok())
    }

    #[test]
    fn a_capture_records_session_and_store_together() {
        let mut session = StopMotionSession::new(12, 2).unwrap();
        let mut store = FrameStore::default();
        let mut source = Fixed(pixel(1, 2, 3, 255));
        let id = capture_next(&mut source, &mut session, &mut store).unwrap();
        assert_eq!(session.frames().len(), 1);
        assert_eq!(session.frames()[0].hold(), 2);
        assert_eq!(store.get(id).unwrap(), &pixel(1, 2, 3, 255)[..]);
    }

    #[test]
    fn a_failed_capture_leaves_both_halves_untouched() {
        let mut session = StopMotionSession::new(12, 1).unwrap();
        let mut store = FrameStore::default();
        let mut source = crate::camera::NoCamera;
        assert!(capture_next(&mut source, &mut session, &mut store).is_err());
        assert!(session.frames().is_empty());
        assert_eq!(store.bytes(), 0);
    }

    #[test]
    fn a_recapture_replaces_the_pixels_but_keeps_the_hold() {
        let mut session = StopMotionSession::new(12, 3).unwrap();
        let mut store = FrameStore::default();
        let mut source = Fixed(pixel(1, 1, 1, 255));
        let id = capture_next(&mut source, &mut session, &mut store).unwrap();
        let mut second = Fixed(pixel(9, 9, 9, 255));
        recapture(&mut second, &session, &mut store, id).unwrap();
        assert_eq!(store.get(id).unwrap(), &pixel(9, 9, 9, 255)[..]);
        assert_eq!(session.frames()[0].hold(), 3);
        // A removed frame cannot be re-shot.
        session.remove_frame(id).unwrap();
        store.remove(id).unwrap();
        assert!(recapture(&mut second, &session, &mut store, id).is_err());
    }

    #[test]
    fn a_duplicate_of_the_last_frame_carries_the_same_pixels() {
        let mut session = StopMotionSession::new(12, 1).unwrap();
        let mut store = FrameStore::default();
        let mut source = Fixed(pixel(5, 5, 5, 255));
        let first = capture_next(&mut source, &mut session, &mut store).unwrap();
        let copy = duplicate_last(&mut session, &mut store).unwrap();
        assert_ne!(copy, first);
        assert_eq!(store.get(copy).unwrap(), store.get(first).unwrap());
        // An empty session has nothing to duplicate.
        let mut empty = StopMotionSession::new(12, 1).unwrap();
        let mut empty_store = FrameStore::default();
        assert!(duplicate_last(&mut empty, &mut empty_store).is_err());
    }

    #[test]
    fn the_store_refuses_partial_pixels_and_keeps_its_budget() {
        let mut store = FrameStore::default();
        let mut id = ids();
        assert_eq!(
            store.insert(id.next().unwrap(), vec![1, 2, 3]),
            Err(StopMotionError::BadCapturePixels { bytes: 3 })
        );
        // Eight captures at the per-capture ceiling fill the store's budget exactly.
        for _ in 0..8 {
            store
                .insert(id.next().unwrap(), vec![0u8; MAX_CAPTURE_BYTES])
                .unwrap();
        }
        assert_eq!(store.bytes(), MAX_STORE_BYTES);
        // A ninth capture does not fit, however small.
        assert_eq!(
            store.insert(id.next().unwrap(), vec![0u8; 4]),
            Err(StopMotionError::StoreFull {
                bytes: MAX_STORE_BYTES + 4,
                max: MAX_STORE_BYTES,
            })
        );
    }

    #[test]
    fn a_removal_frees_the_budget_and_a_replace_grows_it_by_the_difference() {
        let mut store = FrameStore::default();
        let mut id = ids();
        let first = id.next().unwrap();
        store.insert(first, vec![7u8; 8]).unwrap();
        store.insert(first, vec![7u8; 4]).unwrap();
        assert_eq!(store.bytes(), 4);
        store.insert(first, vec![7u8; MAX_CAPTURE_BYTES]).unwrap();
        assert_eq!(store.bytes(), MAX_CAPTURE_BYTES);
        // Dropping the capture gives the whole budget back.
        store.remove(first).unwrap();
        assert_eq!(store.bytes(), 0);
        store
            .insert(id.next().unwrap(), vec![0u8; MAX_CAPTURE_BYTES])
            .unwrap();
        assert_eq!(store.bytes(), MAX_CAPTURE_BYTES);
    }
}
