//! The Polygon tool through the MCP server: the corners given become one closed shape
//! (`flare_tools::polygon`), drawn into a layer of the first scene as `draw_rect` draws.

use flare_core::Rgba8;
use flare_document::{Stroke, StrokeStyle};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::color::parse_color;
use crate::ops::{apply, color_or, duration, first_scene, target_layer};
use crate::params::DrawPolygon;
use crate::server::{FlareServer, Outcome};

#[tool_router(router = polygon_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Draw a filled shape with straight sides through `points` ([x, y] corners, at least three, in order around the outline), as the Polygon tool does: it joins the layer's keyframe at `start` as one more element, or starts a keyframe of `duration` frames there, in the layer given or the topmost that can take it. With `merge` it is Merge Drawing, as for `draw_rect`; `elements` lists where its pieces went. Corners on one line, or fewer than three, are refused. One undo step (needs write access)."
    )]
    fn draw_polygon(&self, Parameters(args): Parameters<DrawPolygon>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let corners: Vec<(f64, f64)> = args.points.iter().map(|[x, y]| (*x, *y)).collect();
            let fill = color_or(args.color.as_deref(), Rgba8::opaque(0, 0, 0))?;
            let stroke = match (args.stroke.as_deref(), args.stroke_style.as_deref()) {
                (None, Some(_)) => return Err("stroke_style needs a stroke color".to_owned()),
                (None, None) => None,
                (Some(color), style) => {
                    let style = match style {
                        None => StrokeStyle::Solid,
                        Some(name) => StrokeStyle::parse(name).ok_or_else(|| {
                            format!(
                                "unknown stroke style {name:?}, expected one of {}",
                                StrokeStyle::name_list()
                            )
                        })?,
                    };
                    let width = args.stroke_width.unwrap_or(2.0);
                    let stroke =
                        Stroke::new(width, parse_color(color)?).map_err(|e| e.to_string())?;
                    Some(stroke.with_style(style))
                }
            };
            let shape = flare_tools::polygon::polygon_shape(&corners, fill, stroke)
                .map_err(|e| e.to_string())?;
            let scene = first_scene(state)?;
            let target = match args.layer.as_deref() {
                Some(text) => flare_tools::ops::Target::Layer(target_layer(state, Some(text))?),
                None => flare_tools::ops::Target::Default,
            };
            let plan = flare_tools::ops::draw_shapes(
                state.session.document(),
                scene,
                target,
                args.start.unwrap_or(state.playhead),
                Some(duration(args.duration)?),
                vec![shape.merged(args.merge.unwrap_or(false))],
                "Layer 1",
            )
            .map_err(|e| format!("cannot draw: {e}"))?;
            let elements: Vec<usize> = plan.placed.iter().map(|at| at.element).collect();
            let mut out = apply(state, plan.commands)?;
            out["layer"] = json!(plan.layer.as_uuid());
            out["element"] = json!(elements.first().copied().unwrap_or(0));
            out["elements"] = json!(elements);
            out["corners"] = json!(corners.len());
            Ok(out)
        })
    }
}
