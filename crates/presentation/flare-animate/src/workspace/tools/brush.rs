//! The Brush in the workspace: the nib in hand and the custom brushes are `flare_tools::brush`'s
//! `BrushShelf`; what is left here is what only the workspace can do, which is to say so when the
//! shelf is full.

use flare_tools::brush::MAX_BRUSHES;

use crate::workspace::*;

impl Workspace {
    /// Keeps the nib in hand as a new custom brush and chooses it. Refuses past the limit, and says so.
    pub(in crate::workspace) fn new_custom_brush(&mut self) {
        if self.brush_shelf.keep_nib().is_err() {
            let message = tr!(
                self,
                "tools-brush-limit",
                &[("max", &MAX_BRUSHES.to_string())]
            );
            self.report(message);
        }
    }
}
