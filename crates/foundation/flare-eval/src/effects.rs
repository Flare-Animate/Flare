//! Marking which shapes and bitmaps a scene layer with effects or a blend mode drew, so the
//! renderer can run them through the layer's graph and mix them with what is under them.

use flare_document::{Document, Layer};

use crate::{EffectRun, Frame};

/// Most layers whose effects run at one frame. Each costs a picture the size of the view and a
/// run of its graph, so past this the rest are drawn as they are, with a warning.
pub const MAX_EFFECT_RUNS: usize = 64;

/// Records `layer`'s effects and blend mode over what it drew since `(shapes, bitmaps)`. A layer
/// that blends normally with no effects, or a graph that draws it as it is, records nothing. A
/// layer that drew nothing still records its run: a graph can make a picture from nothing (snow).
/// Only effects count towards [`MAX_EFFECT_RUNS`]: a blend mode costs a layer in the rasterizer,
/// not a picture of its own.
pub(crate) fn record<'a>(
    document: &Document,
    layer: &'a Layer,
    frame: u32,
    (shapes, bitmaps): (usize, usize),
    out: &mut Frame<'a>,
) {
    let mut graph = layer.effects().filter(|g| !g.is_identity());
    let blend = layer.blend();
    if graph.is_some()
        && out.effects.iter().filter(|r| r.graph.is_some()).count() >= MAX_EFFECT_RUNS
    {
        out.warnings.push(format!(
            "more than {MAX_EFFECT_RUNS} layers have effects at this frame; layer \"{}\" is drawn without its effects",
            layer.name()
        ));
        graph = None;
    }
    if graph.is_none() && blend.is_normal() {
        return;
    }
    let stage = document.stage();
    out.effects.push(EffectRun {
        layer: layer.id(),
        graph,
        blend,
        shapes: shapes..out.shapes.len(),
        bitmaps: bitmaps..out.bitmaps.len(),
        frame,
        stage: [f64::from(stage.width()), f64::from(stage.height())],
    });
}
