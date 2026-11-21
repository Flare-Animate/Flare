//! The phone arrangement: a window narrower than [`PHONE_MAX_WIDTH`] that is driven with fingers
//! puts every side panel into the icon strip, so the stage keeps the width and each panel opens
//! as a sheet over it (the strip's own 300 point flyout). Widen the window and the arrangement
//! the person had comes back as it was; the one saved between sessions is always that one.

use super::*;

/// The widest window that gets the phone arrangement, in points. Phones are 320 to 430 points
/// wide in portrait; the smallest tablet in portrait is 744.
pub(super) const PHONE_MAX_WIDTH: f32 = 600.0;

/// Whether a window `width` points wide, driven by `touch`, gets the phone arrangement.
pub(super) fn is_phone(touch: bool, width: f32) -> bool {
    touch && width < PHONE_MAX_WIDTH
}

impl Workspace {
    /// Puts the side panels away for a phone, or gives back the arrangement put away when the
    /// window is wide again. Called before the rail and dock are drawn each frame; the stage and
    /// the panels that cannot be kept in the strip (the tool strip, the timeline, Output) stay.
    pub(super) fn apply_phone_arrangement(&mut self, width: f32) {
        if is_phone(self.touch, width) {
            if self.phone_stash.is_none() {
                let mut phone = self.panels.layout.clone();
                for pane in flare_dock::Pane::ALL {
                    if pane.can_collapse() && !phone.in_strip(pane) {
                        // A pane that cannot be kept in the strip is skipped by the check above.
                        let _ = phone.collapse(pane);
                    }
                }
                self.phone_stash = Some(std::mem::replace(&mut self.panels.layout, phone));
                self.panels.flyout = None;
            }
        } else if let Some(saved) = self.phone_stash.take() {
            self.panels.layout = saved;
            self.panels.flyout = None;
        }
    }
}

#[cfg(test)]
#[path = "phone_tests.rs"]
mod tests;
