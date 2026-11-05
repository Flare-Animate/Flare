//! Interpolation tools: the same behaviour as `flare tween`, from `flare_tools::tween`.

use flare_tools::ops::{self, SpanRef};
use flare_tools::tween::spec;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::ops::{apply, first_scene, target_layer};
use crate::params::{EvalCurve, Inbetween, SetInterpolation};
use crate::server::{FlareServer, Outcome};

#[tool_router(router = tween_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "List the interpolation families, easings and handle types a keyframe can use."
    )]
    fn list_easings(&self) -> Outcome {
        Ok(json!({
            "families": spec::families().iter()
                .map(|f| json!({ "name": f.name, "eased": f.eased })).collect::<Vec<_>>(),
            "easings": spec::easing_names(),
            "handle_types": spec::handle_names(),
        })
        .to_string())
    }

    #[tool(
        description = "Evaluate an interpolation curve: the fraction of the change made at time t (0 to 1)."
    )]
    fn eval_curve(&self, Parameters(a): Parameters<EvalCurve>) -> Outcome {
        let value = spec::eval(
            &a.family,
            a.easing.as_deref(),
            a.handle.as_deref(),
            a.handles,
            a.t,
        )?;
        Ok(json!({ "t": a.t, "value": value }).to_string())
    }

    #[tool(
        description = "Set the interpolation (family, easing, handle type) of the tween that starts at a keyframe, or clear it by omitting family (needs write access)."
    )]
    fn set_interpolation(&self, Parameters(a): Parameters<SetInterpolation>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let interp = a
                .family
                .as_deref()
                .map(|f| spec::build_interp(f, a.easing.as_deref(), a.handle.as_deref(), a.handles))
                .transpose()?;
            let scene = first_scene(state)?;
            let layer = target_layer(state, a.layer.as_deref())?;
            let span = SpanRef {
                layer,
                start: a.at,
                element: 0,
            };
            let commands = ops::set_interpolation(state.session.document(), scene, span, interp)
                .ok_or_else(|| {
                    format!(
                        "no keyframe at frame {} or it already has that interpolation",
                        a.at
                    )
                })?;
            apply(state, commands)
        })
    }

    #[tool(
        description = "Bake in-between keyframes inside the keyframe at `at`, blended toward the next keyframe along its curve; strokes are matched point by point (needs write access)."
    )]
    fn create_inbetweens(&self, Parameters(a): Parameters<Inbetween>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = target_layer(state, a.layer.as_deref())?;
            let span = SpanRef {
                layer,
                start: a.at,
                element: 0,
            };
            let commands = ops::create_inbetweens(state.session.document(), scene, span, a.count)
                .map_err(|e| e.to_string())?;
            apply(state, commands)
        })
    }
}
