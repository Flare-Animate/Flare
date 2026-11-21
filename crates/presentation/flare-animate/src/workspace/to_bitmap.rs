//! Modify > Convert to Bitmap: the selection, checked here so a refusal is said at once, is drawn
//! into one bitmap on the pixels worker (`flare_runtime::convert_to_bitmap`, which every surface
//! calls), and the edit that comes back is applied as one undo step and the bitmap selected.

use std::sync::Arc;

use flare_runtime::PixelJob;
use flare_tools::to_bitmap::{self, ToBitmapEdit};

use super::*;

/// Pixels to a stage unit: one, as the Raster Brush paints, so the two line up.
const SCALE: u32 = 1;

impl Workspace {
    /// Whether Convert to Bitmap can start: something is selected and no conversion is under way.
    pub(super) fn can_convert_to_bitmap(&self) -> bool {
        self.has_selection() && self.to_bitmap.is_none()
    }

    /// Hands the selection to the pixels worker to be drawn into one bitmap.
    pub(super) fn convert_selection_to_bitmap(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        let selection = self.selection_all();
        if let Err(error) = to_bitmap::plan(self.document(), scene, &selection, SCALE) {
            let message = tr!(self, "msg-to-bitmap", &[("reason", &error.to_string())]);
            self.report(message);
            return;
        }
        let id = self.background.next_id();
        self.to_bitmap = Some(id);
        self.background.submit_pixels(PixelJob::ToBitmap {
            id,
            document: self.active().snapshot(),
            scene,
            frame: self.frame,
            selection,
            scale: SCALE,
            name: tr!(self, "name-converted-bitmap"),
        });
        self.settle_inline();
    }

    /// The conversion came back: applied if the document is still the one it was made for.
    pub(super) fn bitmap_conversion_arrived(
        &mut self,
        id: u64,
        document: Arc<flare_document::Document>,
        result: Result<ToBitmapEdit, String>,
    ) {
        if self.to_bitmap.take_if(|pending| *pending == id).is_none() {
            return;
        }
        if !Arc::ptr_eq(&document, &self.active().snapshot()) {
            self.report(tr!(self, "msg-to-bitmap-changed"));
            return;
        }
        match result {
            Ok(edit) => {
                self.run_batch(edit.commands, "convert to bitmap");
                if self.status.is_none() {
                    self.selected = Some(edit.at);
                    self.also_selected.clear();
                }
            }
            Err(reason) => self.report(tr!(self, "msg-to-bitmap", &[("reason", &reason)])),
        }
    }
}
