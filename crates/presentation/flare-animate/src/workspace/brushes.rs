//! Custom brushes in the workspace: the shelf of named nibs lives in `flare_tools::brush`; this is
//! what a click on its buttons does.

use flare_tools::brush::Nib;

use super::*;

impl Workspace {
    /// The custom brushes, in the order they were made.
    pub fn custom_brushes(&self) -> &[BrushPreset] {
        self.brush_shelf.presets()
    }

    /// The custom brush in hand, if the nib is one of them rather than the plain brush.
    pub fn selected_brush(&self) -> Option<usize> {
        self.brush_shelf.selected()
    }

    /// Keeps the nib in hand as a new custom brush and chooses it. Refuses past the limit.
    pub fn new_custom_brush(&mut self) {
        if self.brush_shelf.keep_nib().is_err() {
            self.report(format!("Only {MAX_BRUSHES} custom brushes can be kept"));
        }
    }

    /// Chooses a custom brush, or the plain round brush for `None` or a place that is not there.
    pub fn choose_brush(&mut self, index: Option<usize>) {
        self.brush_shelf.choose(index);
    }

    /// Removes the chosen custom brush; the plain brush is not one and cannot be removed.
    pub fn delete_custom_brush(&mut self) {
        self.brush_shelf.delete_selected();
    }

    /// Sets the nib in hand; if a custom brush is chosen, it is edited to match.
    pub(super) fn set_brush_nib(&mut self, nib: Nib) {
        self.brush_shelf.set_nib(nib);
    }
}
