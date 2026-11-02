//! P2 group 3 (drawing and rigging) editing commands: stroke scaling and
//! hairlines, Auto Keyframe, 3D space mode, Asset Warp density.
//! Every command is one undo step and does nothing (no undo step) when it has
//! nothing to act on.

use crate::editor::Editor;
use crate::model::*;

impl Editor {
    /// True when the selection holds at least one stroked drawing object.
    pub fn selection_has_stroke(&self) -> bool {
        self.selection.iter().any(|s| matches!(self.element_at(*s), Some(Element { kind: ElementKind::Shape(sh), .. }) if sh.stroke.is_some()))
    }

    fn edit_stroke_style(&mut self, label: &str, f: impl Fn(&mut StrokeStyle)) -> bool {
        if !self.selection_has_stroke() {
            return false;
        }
        self.modify_selected(label, |e| {
            if let ElementKind::Shape(sh) = &mut e.kind {
                if sh.stroke.is_some() {
                    f(&mut sh.stroke_style);
                }
            }
        });
        true
    }

    /// Properties ▸ Scale: normal / horizontal / vertical / none, on the selected strokes.
    pub fn set_stroke_scale(&mut self, mode: StrokeScale) -> bool {
        self.edit_stroke_style("Stroke Scale", move |st| st.scale = mode)
    }
    /// Make the selected strokes hairlines (1 device pixel at any zoom), or solid again.
    pub fn set_stroke_hairline(&mut self, on: bool) -> bool {
        self.edit_stroke_style("Hairline", move |st| st.hairline = on)
    }
    /// Stroke hinting (`pixelHinting` in XFL).
    pub fn set_stroke_hinting(&mut self, on: bool) -> bool {
        self.edit_stroke_style("Stroke Hinting", move |st| st.pixel_hinting = on)
    }

    /// 3D Rotation / Translation tools: switch the space to local (the object's own
    /// axes) or global (the stage's). Sets the tool's mode and, when objects with a 3D
    /// transform are selected, converts them in one undo step. Returns true if any
    /// object changed.
    pub fn set_3d_local(&mut self, local: bool) -> bool {
        self.three_d_local = local;
        let any = self.selection.iter().any(|s| matches!(self.element_at(*s), Some(Element { three_d: Some(t), .. }) if t.local != local));
        if !any {
            return false;
        }
        self.modify_selected(if local { "3D Local Space" } else { "3D Global Space" }, |e| {
            if let Some(t) = e.three_d.as_mut() {
                t.local = local;
            }
        });
        true
    }

    /// Asset Warp options: mesh density (cells along the longer side) of the
    /// selected element's warp. Clamped to 4..=64; 0 restores the default.
    /// False (no undo step) when nothing selected has a warp or the value is unchanged.
    pub fn set_mesh_density(&mut self, density: u32) -> bool {
        use crate::deform::MeshWarp;
        let d = if density == 0 { 0 } else { density.clamp(MeshWarp::MIN_DENSITY, MeshWarp::MAX_DENSITY) };
        let any = self.selection.iter().any(|s| matches!(self.element_at(*s), Some(Element { mesh: Some(w), .. }) if w.density != d));
        if !any {
            return false;
        }
        self.modify_selected("Warp Mesh Density", move |e| {
            if let Some(w) = e.mesh.as_mut() {
                w.density = d;
            }
        });
        true
    }

    /// Bone springiness and damping for the bone that attaches `child` on `layer`.
    /// Values are validated: non-finite is rejected, springiness is clamped to
    /// 0..=`Bone::MAX_SPRING`, damping to 0.05..=4. False (no undo step) when there
    /// is no such bone, a value is not finite, or nothing changes.
    pub fn set_bone_spring(&mut self, layer: usize, child: usize, spring: f32, damping: f32) -> bool {
        if !spring.is_finite() || !damping.is_finite() {
            return false;
        }
        let (sp, dm) = (spring.clamp(0.0, Bone::MAX_SPRING), damping.clamp(0.05, 4.0));
        let Some(b) = self.bones(layer).iter().find(|b| b.child == child) else { return false };
        if b.spring == sp && b.damping == dm {
            return false;
        }
        self.checkpoint("Bone Springiness");
        if let Some(b) = self.timeline_mut().layers[layer].bones.iter_mut().find(|b| b.child == child) {
            b.spring = sp;
            b.damping = dm;
        }
        true
    }
}
