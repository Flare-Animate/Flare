//! Marking which shapes and bitmaps a scene layer with effects drew, so the renderer can run them
//! through the layer's graph.

use flare_document::{Document, Layer};

use crate::{EffectRun, Frame};

/// Most layers whose effects run at one frame. Each costs a picture the size of the view and a
/// run of its graph, so past this the rest are drawn as they are, with a warning.
pub const MAX_EFFECT_RUNS: usize = 64;

/// Records `layer`'s effects over what it drew since `(shapes, bitmaps)`. A layer with no
/// effects, or a graph that draws it as it is, records nothing. A layer that drew nothing still
/// records its run: a graph can make a picture from nothing (snow).
pub(crate) fn record<'a>(
    document: &Document,
    layer: &'a Layer,
    frame: u32,
    (shapes, bitmaps): (usize, usize),
    out: &mut Frame<'a>,
) {
    let Some(graph) = layer.effects().filter(|g| !g.is_identity()) else {
        return;
    };
    if out.effects.len() >= MAX_EFFECT_RUNS {
        out.warnings.push(format!(
            "more than {MAX_EFFECT_RUNS} layers have effects at this frame; layer \"{}\" is drawn without its effects",
            layer.name()
        ));
        return;
    }
    let stage = document.stage();
    out.effects.push(EffectRun {
        layer: layer.id(),
        graph,
        shapes: shapes..out.shapes.len(),
        bitmaps: bitmaps..out.bitmaps.len(),
        frame,
        stage: [f64::from(stage.width()), f64::from(stage.height())],
    });
}
