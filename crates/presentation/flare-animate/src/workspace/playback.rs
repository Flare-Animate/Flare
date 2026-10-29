//! Playing and onion skin.

use super::*;

impl Workspace {
    // ---- playback --------------------------------------------------------------------

    pub(super) fn advance(&mut self, now: f64) {
        let last = self.last_time.replace(now);
        if !self.playing {
            return;
        }
        let Some(last) = last else { return };
        let rate = self.document().frame_rate();
        let fps = f64::from(rate.numerator()) / f64::from(rate.denominator());
        self.play_remainder += (now - last).max(0.0) * fps;
        let whole = self.play_remainder.floor();
        self.play_remainder -= whole;
        let frames = self.scene_frames();
        // `whole` is small (one UI frame's worth), so the cast cannot overflow.
        let next = u64::from(self.frame) + whole as u64;
        if !self.loop_playback && next >= u64::from(frames) {
            self.frame = frames.saturating_sub(1);
            self.playing = false;
            return;
        }
        self.frame = next.rem_euclid(u64::from(frames)) as u32;
    }

    // ---- rendering -------------------------------------------------------------------

    /// Onion skinning is off while playing, where it would only cost time.
    pub(super) fn effective_onion(&self) -> OnionSkin {
        if self.playing {
            OnionSkin::OFF
        } else {
            self.onion
        }
    }

    pub fn onion_skin(&self) -> OnionSkin {
        self.onion
    }

    pub fn set_onion_skin(&mut self, enabled: bool) {
        self.onion.enabled = enabled;
    }

    /// Draws the ghosts as outlines (true) or as faded pictures (false).
    pub fn set_onion_outlines(&mut self, outlines: bool) {
        self.onion.outlines = outlines;
    }

    pub fn set_onion_range(&mut self, before: u32, after: u32) {
        self.onion.before = before.min(MAX_ONION_FRAMES);
        self.onion.after = after.min(MAX_ONION_FRAMES);
    }
}
