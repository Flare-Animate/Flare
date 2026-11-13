//! The reference mannequin in the workspace: the figure being posed, what the Mannequin panel is
//! doing with it, the copy shown over the stage (never part of the document, never exported),
//! and placing it on a layer. The figure itself, its posing and its picture are
//! `flare-mannequin`'s; the layer it is placed on is `flare_tools::mannequin`'s.

use std::cell::RefCell;

use egui::{Color32, Pos2, Rect, Vec2};
use flare_mannequin::{
    Body, Grab, Joint, Look, MAX_PICTURE_SIDE, Mannequin, Picture, Placement, Pose, Proportions,
    RingGrab,
};

use super::*;

/// The smallest and largest zoom of the panel's view.
pub(crate) const ZOOM_RANGE: (f32, f32) = (0.4, 4.0);
/// The shortest and tallest the panel's view is made, in points.
pub(crate) const VIEW_HEIGHT_RANGE: (f32, f32) = (200.0, 2400.0);
/// The longest side of the picture over the stage, in pixels; a larger figure is stretched, so
/// a close zoom never makes the stage slow.
const STAGE_PICTURE_SIDE: f32 = 1400.0;

/// What a drag in the panel's view is doing.
#[derive(Clone, Debug)]
pub(crate) enum MannequinDrag {
    /// Turning the view round the figure.
    Orbit,
    /// Moving the view across.
    Pan,
    /// A part taken hold of and pulled, the body following.
    Pose(Box<Grab>),
    /// Turning a part about the line of sight (with Alt); the pointer's last place.
    Turn { joint: Joint, last: [f64; 2] },
    /// Turning a joint about one axis on its ring; the pointer's last place.
    Ring { grab: RingGrab, last: [f64; 2] },
}

/// Where Place puts the figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlaceInto {
    /// A layer of its own at the top of the scene.
    #[default]
    NewLayer,
    /// The active layer's keyframe at the playhead, beside what it holds.
    ThisKeyframe,
    /// A keyframe of its own on the active layer from the playhead.
    NewKeyframe,
}

impl PlaceInto {
    pub const ALL: [PlaceInto; 3] = [Self::NewLayer, Self::ThisKeyframe, Self::NewKeyframe];
}

/// A picture of the figure made for the screen, and what it was made of, so it is made again
/// only when that changes.
pub(crate) struct Shown {
    key: (Mannequin, Placement, [u32; 2], Look),
    pub(crate) texture: egui::TextureHandle,
    pub(crate) picture: Picture,
}

/// The figure, and what the panel and the stage show of it.
pub(crate) struct MannequinState {
    pub(crate) figure: Mannequin,
    pub(crate) selected: Option<Joint>,
    pub(crate) zoom: f32,
    /// How far the view's middle is moved from the figure's, in points.
    pub(crate) pan: Vec2,
    pub(crate) drag: Option<MannequinDrag>,
    /// The hands and feet that stay where they are while the body is posed.
    pub(crate) held: Vec<Joint>,
    pub(crate) guides: bool,
    /// The view's height as the person made it; none follows the panel's width.
    pub(crate) view_height: Option<f32>,
    /// The panel made large, beside the strip as wide as the window allows.
    pub(crate) expanded: bool,
    /// Where Place puts the figure.
    pub(crate) into: PlaceInto,
    /// Shown over the stage, see-through, as a guide to draw over.
    pub(crate) on_stage: bool,
    pub(crate) opacity: f32,
    /// Where on the stage its middle is and how tall it is, in stage units; none is the middle
    /// of the stage, four fifths of its height.
    pub(crate) stage_at: Option<([f64; 2], f64)>,
    /// Where the panel last drew its view of the figure.
    pub(crate) view_rect: Option<Rect>,
    pub(crate) shown: Option<Shown>,
    pub(crate) stage_shown: RefCell<Option<Shown>>,
}

impl Default for MannequinState {
    fn default() -> Self {
        let figure = Mannequin {
            pose: flare_mannequin::preset("stand").unwrap_or_default(),
            ..Mannequin::default()
        };
        Self {
            figure,
            selected: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            drag: None,
            held: vec![Joint::LeftAnkle, Joint::RightAnkle],
            guides: true,
            view_height: None,
            expanded: false,
            into: PlaceInto::default(),
            on_stage: false,
            opacity: 0.5,
            stage_at: None,
            view_rect: None,
            shown: None,
            stage_shown: RefCell::new(None),
        }
    }
}

/// The picture in `slot`, made again (and its texture with it) when what it shows changed.
/// None when no picture can be made of that size.
pub(crate) fn show<'a>(
    ctx: &egui::Context,
    slot: &'a mut Option<Shown>,
    name: &str,
    figure: &Mannequin,
    placement: Placement,
    size: [u32; 2],
    look: Look,
) -> Option<&'a Shown> {
    let key = (*figure, placement, size, look);
    if slot.as_ref().is_none_or(|s| s.key != key) {
        let picture = figure.render(placement, size[0], size[1], look)?;
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [size[0] as usize, size[1] as usize],
            picture.rgba(),
        );
        let options = egui::TextureOptions::LINEAR;
        match slot {
            Some(shown) => {
                shown.texture.set(image, options);
                shown.key = key;
                shown.picture = picture;
            }
            None => {
                let texture = ctx.load_texture(name, image, options);
                *slot = Some(Shown {
                    key,
                    texture,
                    picture,
                });
            }
        }
    }
    slot.as_ref()
}

/// Pixels for `points` at the screen's density, held to what a picture may be.
pub(crate) fn pixels(points: f32, per_point: f32) -> u32 {
    (points * per_point)
        .round()
        .clamp(1.0, MAX_PICTURE_SIDE as f32) as u32
}

impl Workspace {
    /// Where the figure stands on the stage: the place chosen, else the middle of the stage.
    pub(crate) fn mannequin_on_stage(&self) -> Option<Placement> {
        let stage = self.document().stage();
        let (center, height) = self.mannequin.stage_at.unwrap_or((
            [
                f64::from(stage.width()) / 2.0,
                f64::from(stage.height()) / 2.0,
            ],
            f64::from(stage.height()) * 0.8,
        ));
        Placement::new(center, height)
    }

    /// Gives the figure a built-in pose by its id.
    pub fn set_mannequin_pose(&mut self, id: &str) -> bool {
        match flare_mannequin::preset(id) {
            Some(pose) => {
                self.mannequin.figure.pose = pose;
                true
            }
            None => false,
        }
    }

    pub fn set_mannequin_body(&mut self, body: Body) {
        self.mannequin.figure.body = body;
    }

    pub fn mirror_mannequin(&mut self) {
        self.mannequin.figure.pose = self.mannequin.figure.pose.mirrored();
    }

    pub fn reset_mannequin_pose(&mut self) {
        self.mannequin.figure.pose = Pose::default();
    }

    pub fn reset_mannequin_proportions(&mut self) {
        self.mannequin.figure.proportions = Proportions::default();
    }

    /// The view as it first was: from the front and a little to the side, the whole figure.
    pub fn reset_mannequin_view(&mut self) {
        self.mannequin.figure.view = flare_mannequin::View::default();
        self.mannequin.zoom = 1.0;
        self.mannequin.pan = Vec2::ZERO;
    }

    /// Pins a hand or a foot so it stays put while the body is posed, or lets it go. Whether
    /// `joint` is one that can be pinned.
    pub fn toggle_mannequin_pin(&mut self, joint: Joint) -> bool {
        if joint.reach_chain().is_none() {
            return false;
        }
        let held = &mut self.mannequin.held;
        match held.iter().position(|j| *j == joint) {
            Some(i) => {
                held.remove(i);
            }
            None => held.push(joint),
        }
        true
    }

    /// Makes the panel large, or as it was.
    pub fn set_mannequin_expanded(&mut self, expanded: bool) {
        self.mannequin.expanded = expanded;
    }

    pub fn mannequin(&self) -> &Mannequin {
        &self.mannequin.figure
    }

    /// Sets where Place puts the figure.
    pub fn set_mannequin_place_into(&mut self, into: PlaceInto) {
        self.mannequin.into = into;
    }

    /// Places the figure, as the panel shows it, where the panel says (a new layer at the top
    /// of the scene, or the active layer's keyframe at the playhead, or a new keyframe there),
    /// as one undo step, and selects it with Free Transform, so it can be dragged into place and
    /// scaled or turned by its handles straight away. Whether it was placed; a refusal is
    /// reported.
    pub fn place_mannequin(&mut self) -> bool {
        let (Some(scene), Some(placement)) = (self.scene_id(), self.mannequin_on_stage()) else {
            let message = tr!(self, "tools-no-scene");
            self.report(message);
            return false;
        };
        let name = tr!(self, "mannequin-layer-name");
        let figure = self.mannequin.figure;
        let placed = match (self.mannequin.into, self.active_layer_id()) {
            (PlaceInto::NewLayer, _) | (_, None) => flare_tools::mannequin::place(
                self.document(),
                scene,
                self.frame,
                &figure,
                placement,
                &name,
            ),
            (into, Some(layer)) => {
                let keyframe = if into == PlaceInto::NewKeyframe {
                    flare_tools::mannequin::Keyframe::New
                } else {
                    flare_tools::mannequin::Keyframe::Join
                };
                flare_tools::mannequin::place_onto(
                    self.document(),
                    scene,
                    self.frame,
                    &figure,
                    placement,
                    layer,
                    keyframe,
                    &name,
                )
            }
        };
        let label = tr!(self, "mannequin-place");
        let outcome = placed.map_err(|e| e.to_string()).and_then(|placed| {
            let at = placed.at;
            self.active_mut()
                .apply_labeled(placed.commands, &label)
                .map(|()| at)
                .map_err(|e| e.to_string())
        });
        match outcome {
            Ok(at) => {
                self.current_layer = Some(at.layer);
                self.set_tool(Tool::FreeTransform);
                self.also_selected.clear();
                self.selected = Some(at);
                let layer = self
                    .active_layer()
                    .map_or_else(|| name.clone(), |l| l.name().to_owned());
                let message = tr!(self, "mannequin-placed", &[("name", &layer)]);
                self.report(message);
                true
            }
            Err(error) => {
                let message = tr!(self, "mannequin-place-error", &[("error", &error)]);
                self.report(message);
                false
            }
        }
    }

    /// The figure over the stage, see-through, when it is shown there: its picture made at the
    /// size it has on the screen. It is drawn for the view only: it is not in the document and
    /// no export has it.
    pub(super) fn stage_mannequin(
        &self,
        painter: &egui::Painter,
        to_screen: &impl Fn(Pos2) -> Pos2,
    ) {
        if !self.mannequin.on_stage {
            return;
        }
        let Some(placement) = self.mannequin_on_stage() else {
            return;
        };
        let figure = &self.mannequin.figure;
        let look = Look {
            guides: self.mannequin.guides,
            ..Look::default()
        };
        let (lo, hi) = figure.bounds(placement, look.shadow);
        let screen = |p: [f64; 2]| to_screen(Pos2::new(p[0] as f32, p[1] as f32));
        let (top_left, bottom_right) = (screen(lo), screen(hi));
        let area = Rect::from_two_pos(top_left, bottom_right).expand(2.0);
        if !area.is_finite() || !painter.clip_rect().intersects(area) {
            return;
        }
        let per_point = painter.ctx().pixels_per_point();
        let longest = area.width().max(area.height()) * per_point;
        let k = (STAGE_PICTURE_SIDE / longest.max(1.0)).min(1.0) * per_point;
        let center = screen(placement.center);
        let tall = (screen([placement.center[0], placement.center[1] + placement.height]) - center)
            .length();
        let Some(inside) = Placement::new(
            [
                f64::from((center.x - area.min.x) * k),
                f64::from((center.y - area.min.y) * k),
            ],
            f64::from(tall * k),
        ) else {
            return;
        };
        let size = [pixels(area.width(), k), pixels(area.height(), k)];
        let mut slot = self.mannequin.stage_shown.borrow_mut();
        let ctx = painter.ctx().clone();
        let Some(shown) = show(
            &ctx,
            &mut slot,
            "mannequin-stage",
            figure,
            inside,
            size,
            look,
        ) else {
            return;
        };
        let alpha = self.mannequin.opacity.clamp(0.05, 1.0);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(
            shown.texture.id(),
            area,
            uv,
            Color32::WHITE.gamma_multiply(alpha),
        );
    }

    /// The figure as the preferences keep it.
    pub(super) fn mannequin_preferences(&self) -> String {
        self.mannequin.figure.to_json()
    }

    /// The figure from the preferences; one that does not read leaves the figure as it is.
    pub(super) fn apply_mannequin_preferences(&mut self, json: &str) {
        if let Ok(figure) = Mannequin::from_json(json) {
            self.mannequin.figure = figure;
        }
    }
}
