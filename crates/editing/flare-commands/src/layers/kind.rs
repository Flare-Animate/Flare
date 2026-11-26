//! What kind of art a layer takes (vector shapes or raster pixels) and how its picture is mixed
//! with the layers under it.

use flare_core::{LayerId, SceneId};
use flare_document::{Document, Layer, LayerBlend};

use super::{LayerRefusal, layer_in, update};
use crate::{Command, LayerChange};

/// A layer that draws nothing of its own (a folder, a mask, a motion guide or the camera) has no
/// picture to blend or paint.
fn draws_art(layer: &Layer) -> bool {
    !(layer.is_folder() || layer.is_mask() || layer.is_guide() || layer.is_camera())
}

/// A new empty layer named `name`; a raster layer when `raster` is set. New layers are vector
/// layers unless asked, and do not hold their last drawing: a keyframe shows for its own frames
/// only, as in Animate, so a drawing on a new layer is not stretched to the length of the others.
pub fn new_layer(id: LayerId, name: &str, raster: bool) -> Result<Layer, LayerRefusal> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LayerRefusal::NeedsAName);
    }
    Layer::new(id, name, vec![])
        .map(|layer| layer.with_raster(raster))
        .map_err(|_| LayerRefusal::Invalid("the layer could not be made"))
}

/// Sets how the layer's picture is mixed with the layers under it. Nothing to do when it already
/// is; refused on a layer that draws nothing.
pub fn set_blend(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    blend: LayerBlend,
) -> Result<Vec<Command>, LayerRefusal> {
    let found = layer_in(document, scene, layer)?;
    if found.blend() == blend {
        return Ok(Vec::new());
    }
    if !draws_art(found) && !blend.is_normal() {
        return Err(LayerRefusal::Invalid(
            "a folder, mask, guide or camera layer draws nothing to blend",
        ));
    }
    Ok(update(scene, layer, LayerChange::Blend(blend)))
}

/// Makes the layer a raster layer or a vector layer. What it holds stays as it is. Nothing to do
/// when it already is that kind; a layer that draws nothing cannot be made raster.
pub fn set_raster(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    raster: bool,
) -> Result<Vec<Command>, LayerRefusal> {
    let found = layer_in(document, scene, layer)?;
    if found.is_raster() == raster {
        return Ok(Vec::new());
    }
    if raster && !draws_art(found) {
        return Err(LayerRefusal::Invalid(
            "a folder, mask, guide or camera layer cannot be a raster layer",
        ));
    }
    Ok(update(scene, layer, LayerChange::Raster(raster)))
}
