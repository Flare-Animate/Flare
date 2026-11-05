//! Image layers: the picture of a Moho image layer, added to the document's bitmap library once
//! per path and placed as a symbol. The pictures come from the caller's [`ImageSource`].

use flare_commands::Command;
use flare_core::{BitmapId, InstanceId, LayerId, Transform2D};
use flare_document::{Bitmap, BitmapInstance, Content};
use glamx::{DAffine2, DVec2};

use crate::convert::{Builder, Severity, rig_for};
use crate::convert_anim::place;
use crate::model::{Layer as MohoLayer, Project};

/// An image layer: its picture centred on the layer's origin, placed like a vector layer.
pub(crate) fn image_out(
    project: &Project,
    layer: &MohoLayer,
    path: &str,
    place_name: &str,
    parent: Option<LayerId>,
    b: &mut Builder<'_>,
) {
    let Some(file) = layer.image_path.as_deref() else {
        b.note(
            Severity::Unsupported,
            place_name,
            "image layer has no picture",
        );
        return;
    };
    let (bitmap, width, height) = match b.bitmap(file) {
        Ok(found) => found,
        Err(why) => {
            b.note(
                Severity::Unsupported,
                place_name,
                format!("image layer ({file}): {why}"),
            );
            return;
        }
    };
    // Pixels are taken as document pixels; Moho's own picture scale is not read.
    let centre = DAffine2::from_translation(DVec2::new(
        -f64::from(width) / 2.0,
        -f64::from(height) / 2.0,
    ));
    let Ok(local) = Transform2D::new(centre) else {
        return;
    };
    let content = Content::Bitmap(BitmapInstance::new(InstanceId::new(), bitmap, local));
    let symbol = b.symbol_content(&layer.name, Some(content));
    let placement = place(
        rig_for(project, path),
        b.end_frame,
        b.origin,
        b.units_to_px,
        false,
    );
    if placement.approximated {
        b.note(
            Severity::Warning,
            place_name,
            "smooth or eased interpolation became linear",
        );
    }
    b.posed_layer(&layer.name, symbol, placement, parent, layer.visible);
}

impl Builder<'_> {
    /// The bitmap for a picture path, added to the library the first time it is loaded.
    fn bitmap(&mut self, file: &str) -> Result<(BitmapId, u32, u32), String> {
        if let Some(known) = self.bitmaps.get(file) {
            return known.clone();
        }
        let loaded = self.images.load(file).and_then(|picture| {
            let id = BitmapId::new();
            let bitmap = Bitmap::new(id, file, picture.width, picture.height, picture.rgba)
                .map_err(|e| e.to_string())?;
            self.commands.push(Command::AddBitmap(bitmap));
            Ok((id, picture.width, picture.height))
        });
        self.bitmaps.insert(file.to_owned(), loaded.clone());
        loaded
    }
}
