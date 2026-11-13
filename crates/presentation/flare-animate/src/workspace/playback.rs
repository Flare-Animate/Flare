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
        let fps = rate.per_second();
        self.play_remainder += (now - last).max(0.0) * fps;
        let whole = self.play_remainder.floor();
        self.play_remainder -= whole;
        if whole < 1.0 {
            return;
        }
        // `whole` is small (one UI frame's worth), so the cast cannot overflow.
        let (frame, playing) = flare_editor::loop_range::advance(
            self.frame,
            whole as u64,
            self.scene_frames(),
            self.looping.on,
            self.looping.range,
        );
        self.frame = frame;
        self.playing = playing;
    }

    // ---- rendering -------------------------------------------------------------------

    /// Onion skinning is off while playing, where it would only cost time; while editing
    /// multiple frames their keyframes are the ghosts.
    pub(super) fn effective_onion(&self) -> OnionSkin {
        if self.playing {
            OnionSkin::OFF
        } else {
            OnionSkin {
                multi: self.multi_frame_range(),
                ..self.onion
            }
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

    /// Draws earlier frames in one color and later ones in another (true), as Animate's tinted
    /// onion skin does, or each in its own colors (false).
    pub fn set_onion_tinted(&mut self, tinted: bool) {
        self.onion.tinted = tinted;
    }

    /// The colors of the ghosts before and after the current frame when tinted. Opaque: a
    /// ghost's own fading is what makes it faint.
    pub fn set_onion_colors(&mut self, before: flare_core::Rgba8, after: flare_core::Rgba8) {
        self.onion.before_color = flare_core::Rgba8::opaque(before.r, before.g, before.b);
        self.onion.after_color = flare_core::Rgba8::opaque(after.r, after.g, after.b);
    }

    pub fn set_onion_range(&mut self, before: u32, after: u32) {
        self.onion.before = before.min(MAX_ONION_FRAMES);
        self.onion.after = after.min(MAX_ONION_FRAMES);
    }
}
