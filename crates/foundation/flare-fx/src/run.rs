//! Running a graph over a layer's picture.

use std::collections::HashMap;

use flare_core::fx_params::{Params, Scalable};

use crate::ops::blur::blur;
use crate::ops::color::{Adjustment, adjust, tint};
use crate::ops::composite::{fade_in_place, over_in_place};
use crate::ops::shadow::shadow;
use crate::ops::snow::{Snowfall, snow};
use crate::{EffectGraph, EffectKind, FxError, Node, NodeId, Pixels, Space, Value};

/// The most a picture of part of the stage takes in beyond each edge for effects that spread
/// (see [`EffectGraph::reach`]), in pixels. Past this a shadow cast from far outside the view
/// is cut; a picture of the whole stage is never cut.
pub const MAX_REACH_PIXELS: f64 = 1024.0;

/// A node's settings with every length in pixels: the parameter set `fx_params` scales.
struct Settings {
    values: Vec<Value>,
    lengths: Vec<bool>,
}

impl Scalable for Settings {
    fn scale(&mut self, factor: f64) {
        for (value, is_length) in self.values.iter_mut().zip(&self.lengths) {
            if let (Value::Number(n), true) = (value, is_length) {
                *n = (f64::from(*n) * factor) as f32;
            }
        }
    }
}

/// `node`'s settings in pixels of a picture where one stage unit is `scale` pixels.
fn in_pixels(node: &Node, scale: f64) -> Result<Node, FxError> {
    let lengths = node
        .kind()
        .params()
        .iter()
        .map(|spec| match spec.kind {
            crate::ParamKind::Number { unit, .. } => unit.scales(),
            crate::ParamKind::Color { .. } => false,
        })
        .collect();
    let mut set = Params::default();
    set.push(Settings {
        values: node.values().to_vec(),
        lengths,
    })
    .map_err(|_| FxError::TooManyNodes)?;
    set.scale_by(scale).map_err(|_| FxError::BadScale)?;
    let values = set
        .params()
        .first()
        .map(|s| s.values.clone())
        .unwrap_or_default();
    Ok(Node::from_parts(
        node.id(),
        node.kind(),
        node.inputs().to_vec(),
        values,
        node.at(),
    ))
}

/// Runs `graph` over `source`, the layer's picture, at `frame`, where `space` says where the
/// stage is in the picture. Every picture made is the size of `source`; each is dropped as soon
/// as the last node that reads it has run, so at most a few are kept at once.
pub fn run(
    graph: &EffectGraph,
    source: Pixels,
    space: &Space,
    frame: u32,
) -> Result<Pixels, FxError> {
    if !(space.scale.is_finite() && space.scale > 0.0) {
        return Err(FxError::BadScale);
    }
    // Every picture is the source's size, which is already held, so a blank is a copy of this.
    let blank = source.cleared();
    let order = graph.order();
    // How many inputs still to run read each node's picture.
    let mut readers: HashMap<NodeId, usize> = HashMap::with_capacity(order.len());
    for id in &order {
        for from in graph
            .node(*id)
            .map(Node::inputs)
            .unwrap_or_default()
            .iter()
            .flatten()
        {
            *readers.entry(*from).or_default() += 1;
        }
    }
    let mut made: HashMap<NodeId, Pixels> = HashMap::with_capacity(order.len());
    let mut source = Some(source);
    for id in order {
        let Some(node) = graph.node(id) else { continue };
        let mut inputs = Vec::with_capacity(node.inputs().len());
        for from in node.inputs() {
            let picture = match from {
                None => blank.clone(),
                Some(from) => take(&mut made, &mut readers, *from, &blank),
            };
            inputs.push(picture);
        }
        let picture = match node.kind() {
            EffectKind::Layer => source.take().unwrap_or_else(|| blank.clone()),
            EffectKind::Output => {
                return Ok(inputs.pop().unwrap_or_else(|| blank.clone()));
            }
            _ => apply(&in_pixels(node, space.scale)?, inputs, space, frame),
        };
        made.insert(id, picture);
    }
    Ok(blank)
}

/// The picture `from` made, for one reader: moved out to its last reader, copied for the others.
fn take(
    made: &mut HashMap<NodeId, Pixels>,
    readers: &mut HashMap<NodeId, usize>,
    from: NodeId,
    blank: &Pixels,
) -> Pixels {
    let left = readers.get_mut(&from).map(|n| {
        *n = n.saturating_sub(1);
        *n
    });
    match left {
        Some(0) => made.remove(&from),
        _ => made.get(&from).cloned(),
    }
    .unwrap_or_else(|| blank.clone())
}

/// One node's picture from its inputs. `node`'s lengths are already in pixels.
fn apply(node: &Node, mut inputs: Vec<Pixels>, space: &Space, frame: u32) -> Pixels {
    let n = |i: usize| f64::from(node.number(i));
    let first = |inputs: &mut Vec<Pixels>| {
        if inputs.is_empty() {
            None
        } else {
            Some(inputs.swap_remove(0))
        }
    };
    let Some(mut picture) = first(&mut inputs) else {
        return Pixels::default();
    };
    let (w, h) = (picture.width() as usize, picture.height() as usize);
    match node.kind() {
        EffectKind::Blur => blur(picture.data_mut(), w, h, 4, n(0)),
        EffectKind::DropShadow => {
            let (distance, angle) = (n(0), n(1).to_radians());
            let offset = [distance * angle.cos(), distance * angle.sin()];
            picture = shadow(&picture, offset, n(2), node.color(3), n(4) / 100.0);
        }
        EffectKind::Glow => {
            picture = shadow(&picture, [0.0, 0.0], n(0), node.color(2), n(1) / 100.0);
        }
        EffectKind::Adjust => adjust(
            &mut picture,
            Adjustment {
                brightness: node.number(0) / 100.0,
                contrast: node.number(1) / 100.0,
                saturation: node.number(2) / 100.0,
                hue: node.number(3),
            },
        ),
        EffectKind::Tint => tint(&mut picture, node.color(0), node.number(1) / 100.0),
        EffectKind::Opacity => fade_in_place(&mut picture, n(0) / 100.0),
        EffectKind::Snow => {
            let [left, top] = space.origin;
            let stage = [
                left,
                top,
                left + space.stage[0] * space.scale,
                top + space.stage[1] * space.scale,
            ];
            let fall = Snowfall {
                flakes: n(0) as u32,
                size: n(1),
                speed: n(2),
                wind: n(3),
                sway: n(4),
                seed: n(5) as u32,
                color: node.color(6),
                opacity: n(7) / 100.0,
            };
            snow(&mut picture, &fall, stage, frame);
        }
        EffectKind::Over => {
            // The first input is the one laid over; the second, now first, is what goes under.
            if let Some(mut under) = first(&mut inputs) {
                over_in_place(&mut under, &picture);
                picture = under;
            }
        }
        EffectKind::Layer | EffectKind::Output => {}
    }
    picture
}
