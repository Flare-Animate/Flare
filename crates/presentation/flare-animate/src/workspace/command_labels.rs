//! What each menu item is called right now: the names are `flare-menubar`'s, and this says which
//! of the two names of a toggling item the workspace's state asks for.

use flare_document::{Layer, Tween};
use flare_menubar::{LabelState, MenuAction};

use super::*;

impl Workspace {
    /// The menu item's current label (some toggle between two meanings).
    pub fn label(&self, action: MenuAction) -> String {
        let layer = self.active_layer();
        let tween = self.selection_tween();
        let state = LabelState {
            motion_tween: tween == Some(Tween::Linear),
            shape_tween: tween == Some(Tween::Shape),
            playing: self.playing,
            layer_visible: layer.is_none_or(Layer::is_visible),
            layer_locked: layer.is_some_and(Layer::is_locked),
            layer_mask: layer.is_some_and(Layer::is_mask),
            layer_guide: layer.is_some_and(Layer::is_guide),
            text_bold: self.text.bold,
            text_italic: self.text.italic,
            guides_shown: self.display.show_guides,
            onion_on: self.onion.enabled,
            outline_mode: self.display.outline_mode,
            rulers_shown: self.display.show_rulers,
            grid_shown: self.display.show_grid,
            snapping: self.display.snap_to_grid,
            loop_playback: self.loop_playback,
            muted: self.muted,
        };
        action.label(&state).to_owned()
    }
}
