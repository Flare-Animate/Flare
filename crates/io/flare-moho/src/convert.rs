//! Conversion of a Moho project into a Flare document. The result is built as one batch of
//! document commands and applied with [`flare_commands::apply_all`], so it either validates as a
//! whole or fails with nothing made.
//!
//! Mapping: a group is a folder layer; a vector layer is a Graphic symbol holding its mesh as one
//! shape, placed by an instance whose spans follow the layer's translation, rotation and scale
//! tracks; each bone is a folder with an art layer drawing the bone, posed by its tracks; a
//! switch layer is one symbol per alternative and an instance span per switch key. An image layer
//! is a symbol holding its picture from the document's bitmap library, centred on the layer's
//! origin at its pixel size; the picture comes from an [`ImageSource`]. Audio layers have no
//! document counterpart (sound needs the decoded file, which a Moho container is not read for)
//! and are reported by name.

use std::collections::HashMap;
use std::num::NonZeroU32;

use flare_commands::{Command, apply_all};
use flare_core::{
    BitmapId, DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
};
use flare_core::{DocumentPoint, Transform2D};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Stage, Symbol,
    Timeline,
};

use crate::convert_anim::{Placement, Rig, place};
use crate::convert_image::image_out;
use crate::convert_vector::shape;
use crate::error::MohoError;
use crate::images::{ImageSource, NoImages};
use crate::model::{Binding, Channel, Layer as MohoLayer, LayerKind, Project};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Imported faithfully but worth knowing about.
    Info,
    /// Imported with a change in meaning.
    Warning,
    /// Not imported.
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub severity: Severity,
    pub location: String,
    pub message: String,
}

#[derive(Debug)]
pub struct Converted {
    pub document: Document,
    pub notes: Vec<Note>,
}

/// Notes kept; a hostile project cannot make the report grow without bound.
pub(crate) const MAX_NOTES: usize = 1_000;
const DEFAULT_SIDE: (u32, u32) = (1920, 1080);

pub(crate) struct Builder<'a> {
    pub scene: SceneId,
    pub commands: Vec<Command>,
    pub notes: Vec<Note>,
    pub origin: [f64; 2],
    pub units_to_px: f64,
    pub end_frame: i64,
    pub images: &'a mut dyn ImageSource,
    /// Each picture path loaded so far: its bitmap and pixel size, or why it failed. A path is
    /// asked for once, however many layers name it.
    pub bitmaps: HashMap<String, Result<(BitmapId, u32, u32), String>>,
}

impl Builder<'_> {
    pub fn note(&mut self, severity: Severity, location: &str, message: impl Into<String>) {
        if self.notes.len() < MAX_NOTES {
            self.notes.push(Note {
                severity,
                location: location.to_owned(),
                message: message.into(),
            });
        }
    }

    /// Adds a Graphic symbol whose only art is `shape` (or nothing).
    pub fn symbol(&mut self, name: &str, shape: Option<Shape>) -> SymbolId {
        self.symbol_content(name, shape.map(Content::Shape))
    }

    /// Adds a Graphic symbol whose only art is `content` (or nothing).
    pub fn symbol_content(&mut self, name: &str, content: Option<Content>) -> SymbolId {
        let id = SymbolId::new();
        let spans = content
            .map(|c| vec![FrameSpan::new(0, NonZeroU32::MIN, c)])
            .unwrap_or_default();
        let layer = Layer::new(LayerId::new(), "Art", spans);
        let layers = layer.into_iter().collect();
        self.commands.push(Command::AddSymbol(Symbol::new(
            id,
            name,
            Timeline::new(layers),
        )));
        id
    }

    pub fn folder(&mut self, name: &str, parent: Option<LayerId>, visible: bool) -> LayerId {
        let id = LayerId::new();
        if let Ok(layer) = Layer::new(id, name, Vec::new()) {
            self.push_layer(
                layer
                    .with_folder(true)
                    .with_parent(parent)
                    .with_visible(visible),
            );
        }
        id
    }

    pub fn push_layer(&mut self, layer: Layer) {
        self.commands.push(Command::AddLayer {
            scene: self.scene,
            layer,
        });
    }

    /// A layer showing `symbol` at the poses of `placement`.
    pub fn posed_layer(
        &mut self,
        name: &str,
        symbol: SymbolId,
        placement: Placement,
        parent: Option<LayerId>,
        visible: bool,
    ) {
        let spans = placement
            .spans
            .into_iter()
            .map(|p| {
                let instance = Instance::new(InstanceId::new(), symbol, p.transform);
                FrameSpan::new(p.start, p.duration, Content::Instance(instance)).with_tween(p.tween)
            })
            .collect();
        if let Ok(layer) = Layer::new(LayerId::new(), name, spans) {
            self.push_layer(layer.with_parent(parent).with_visible(visible));
        }
    }
}

pub(crate) fn frame_rate(fps: f64) -> Option<FrameRate> {
    if !fps.is_finite() || fps <= 0.0 || fps > 1000.0 {
        return None;
    }
    FrameRate::new((fps * 1000.0).round() as u32, 1000).ok()
}

/// Builds the document from `commands`: the project's frame rate and stage first.
pub(crate) fn finish(
    b: Builder<'_>,
    rate: FrameRate,
    stage: Stage,
) -> Result<Converted, MohoError> {
    let scene = Scene::new(b.scene, "Scene 1", Timeline::new(Vec::new()));
    let base = Document::new(DocumentId::new(), rate, vec![scene], Vec::new())
        .map_err(|d| MohoError::Document(format!("{d:?}")))?
        .with_stage(stage);
    let mut commands = vec![Command::SetStage(stage), Command::SetFrameRate(rate)];
    commands.extend(b.commands);
    let document = apply_all(&base, commands)
        .map_err(|e| MohoError::Document(e.to_string()))?
        .unwrap_or(base);
    Ok(Converted {
        document,
        notes: b.notes,
    })
}

/// Turns a read project into a Flare document, with notes on everything that could not be carried
/// over.
pub fn to_flare_document(project: &Project) -> Result<Converted, MohoError> {
    to_flare_document_with(project, &mut NoImages)
}

/// As [`to_flare_document`], with image layers' pictures taken from `images`.
pub fn to_flare_document_with(
    project: &Project,
    images: &mut dyn ImageSource,
) -> Result<Converted, MohoError> {
    let (width, height) = if project.width >= 1
        && project.height >= 1
        && project.width <= flare_document::MAX_STAGE_SIDE
        && project.height <= flare_document::MAX_STAGE_SIDE
    {
        (project.width, project.height)
    } else {
        DEFAULT_SIDE
    };
    let stage =
        Stage::new(width, height, Rgba8::WHITE).map_err(|e| MohoError::Document(e.to_string()))?;
    let mut b = Builder {
        scene: SceneId::new(),
        commands: Vec::new(),
        notes: Vec::new(),
        origin: [f64::from(width) / 2.0, f64::from(height) / 2.0],
        // Assumption, not read from the file: Moho's default stage is two units tall.
        units_to_px: f64::from(height) / 2.0,
        end_frame: project.end_frame,
        images,
        bitmaps: HashMap::new(),
    };
    if (width, height) != (project.width, project.height) {
        b.note(
            Severity::Warning,
            "project",
            format!(
                "the size {}x{} is not usable; the stage is {width}x{height}",
                project.width, project.height
            ),
        );
    }
    let rate = match frame_rate(project.fps) {
        Some(rate) => rate,
        None => {
            b.note(
                Severity::Warning,
                "project",
                "the frame rate is missing or invalid; using 24",
            );
            FrameRate::new(24, 1).map_err(|e| MohoError::Document(e.to_string()))?
        }
    };
    skeleton(project, &mut b);
    // Moho lists layers back to front and a Flare timeline top to bottom.
    for (i, layer) in project.layers.iter().enumerate().rev() {
        layer_out(project, layer, &format!("layers[{i}]"), None, &mut b);
    }
    for path in &project.images {
        if !b.bitmaps.contains_key(path) {
            b.note(
                Severity::Unsupported,
                path,
                "image file: not an image layer's picture, so it is not placed",
            );
        }
    }
    for path in &project.audio {
        b.note(
            Severity::Unsupported,
            path,
            "audio file: not embedded; a Moho project stores only its path",
        );
    }
    finish(b, rate, stage)
}

/// The track at exactly `path`, such as `layers[0].transforms.translation`.
fn track<'a>(project: &'a Project, path: &str) -> Option<&'a Channel> {
    project
        .tracks
        .iter()
        .find(|t| t.path == path)
        .map(|t| &t.channel)
}

fn line(length: f64, px: f64) -> Option<Shape> {
    let end = DocumentPoint::new((length * px).max(1.0), 0.0).ok()?;
    let stroke = flare_document::Stroke::new(3.0, Rgba8::new(200, 60, 20, 255)).ok()?;
    Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(DocumentPoint::ORIGIN),
            PathSegment::LineTo(end),
        ],
        Rgba8::TRANSPARENT,
    )
    .ok()
    .map(|s| s.with_stroke(stroke))
}

/// Each bone is a folder (its children nest in it) with one art layer: the bone drawn as a line
/// of its length, posed by its `anim_pos`, `anim_angle` and `anim_scale` tracks.
fn skeleton(project: &Project, b: &mut Builder<'_>) {
    if project.bones.is_empty() {
        return;
    }
    let root = b.folder("Skeleton", None, true);
    let mut folders: Vec<Option<LayerId>> = vec![None; project.bones.len()];
    // A parent may follow its child in the file, so bones are made from the roots down.
    for i in bone_order(project) {
        let bone = &project.bones[i];
        let parent = bone.parent.and_then(|p| folders[p]).unwrap_or(root);
        let folder = b.folder(&bone.name, Some(parent), true);
        folders[i] = Some(folder);
        let rig = Rig {
            translation: track(project, &format!("{}.anim_pos", bone.path)),
            rotation: track(project, &format!("{}.anim_angle", bone.path)),
            scale: track(project, &format!("{}.anim_scale", bone.path)),
        };
        let placement = place(rig, b.end_frame, b.origin, b.units_to_px, true);
        let symbol = b.symbol(&bone.name, line(bone.length, b.units_to_px));
        if placement.approximated {
            b.note(
                Severity::Warning,
                &format!("bone {}", bone.name),
                "smooth or eased interpolation became linear",
            );
        }
        if bone.dynamic {
            b.note(
                Severity::Unsupported,
                &format!("bone {}", bone.name),
                "spring and damping dynamics",
            );
        }
        b.posed_layer(
            &format!("{} pose", bone.name),
            symbol,
            placement,
            Some(folder),
            true,
        );
    }
}

/// Bone indices with every parent before its children. Parents form no loops (the reader breaks
/// any), so every bone is reached from a root.
fn bone_order(project: &Project) -> Vec<usize> {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); project.bones.len()];
    let mut order = Vec::with_capacity(project.bones.len());
    for (i, bone) in project.bones.iter().enumerate() {
        match bone.parent {
            Some(p) => children[p].push(i),
            None => order.push(i),
        }
    }
    let mut next = 0;
    while next < order.len() {
        let parent = order[next];
        order.extend(children[parent].iter().copied());
        next += 1;
    }
    order
}

fn layer_out(
    project: &Project,
    layer: &MohoLayer,
    path: &str,
    parent: Option<LayerId>,
    b: &mut Builder<'_>,
) {
    let place_name = format!("{} ({path})", layer.name);
    if layer.binding != Binding::None {
        b.note(
            Severity::Info,
            &place_name,
            "bound to a bone: it keeps its own placement, not the bone's",
        );
    }
    match &layer.kind {
        LayerKind::Group | LayerKind::Bone => {
            let folder = b.folder(&layer.name, parent, layer.visible);
            for (i, child) in layer.children.iter().enumerate().rev() {
                layer_out(
                    project,
                    child,
                    &format!("{path}.layers[{i}]"),
                    Some(folder),
                    b,
                );
            }
        }
        LayerKind::Vector => {
            let art = layer
                .mesh
                .as_ref()
                .and_then(|m| shape(m, b.units_to_px, [0.0, 0.0]));
            if art.is_none() {
                b.note(
                    Severity::Warning,
                    &place_name,
                    "the vector layer has no drawable curve",
                );
            }
            let symbol = b.symbol(&layer.name, art);
            let rig = rig_for(project, path);
            let placement = place(rig, b.end_frame, b.origin, b.units_to_px, false);
            if placement.approximated {
                b.note(
                    Severity::Warning,
                    &place_name,
                    "smooth or eased interpolation became linear",
                );
            }
            let fill_note = match layer.mesh.as_ref() {
                Some(mesh) if mesh.fills > 1 => {
                    Some((Severity::Warning, "several fill colours; the first is used"))
                }
                Some(mesh) if mesh.fill.is_some() => None,
                _ => Some((
                    Severity::Info,
                    "fill colour not read; closed curves are grey",
                )),
            };
            if let Some((severity, message)) = fill_note {
                b.note(severity, &place_name, message);
            }
            b.note(
                Severity::Info,
                &place_name,
                "curve handles are not read; drawn as a polyline",
            );
            b.posed_layer(&layer.name, symbol, placement, parent, layer.visible);
        }
        LayerKind::Switch => switch_out(project, layer, &place_name, parent, b),
        LayerKind::Image => image_out(project, layer, path, &place_name, parent, b),
        LayerKind::Other(raw) => {
            b.note(
                Severity::Unsupported,
                &place_name,
                format!("{raw} layers are not imported"),
            );
        }
    }
}

pub(crate) fn rig_for<'a>(project: &'a Project, path: &str) -> Rig<'a> {
    Rig {
        translation: track(project, &format!("{path}.transforms.translation")),
        rotation: track(project, &format!("{path}.transforms.rotation_z")),
        scale: track(project, &format!("{path}.transforms.scale")),
    }
}

/// One symbol per alternative, and a span per key naming the one shown from that frame.
fn switch_out(
    project: &Project,
    layer: &MohoLayer,
    place_name: &str,
    parent: Option<LayerId>,
    b: &mut Builder<'_>,
) {
    let Some(cells) = &layer.switch else { return };
    let mut symbols = std::collections::HashMap::new();
    for child in &layer.children {
        let art = child
            .mesh
            .as_ref()
            .and_then(|m| shape(m, b.units_to_px, b.origin));
        if art.is_none() {
            b.note(
                Severity::Unsupported,
                &format!("{place_name} / {}", child.name),
                "this alternative has no vector art to show",
            );
        }
        symbols
            .entry(child.name.clone())
            .or_insert_with(|| b.symbol(&child.name, art));
    }
    let mut keys: Vec<_> = cells
        .keys
        .keys
        .iter()
        .filter(|k| (0..=crate::limits::MAX_FRAME).contains(&k.frame))
        .collect();
    keys.sort_by_key(|k| k.frame);
    let mut spans = Vec::new();
    for (i, key) in keys.iter().enumerate() {
        let Some(symbol) = key.value.as_text().and_then(|n| symbols.get(n)) else {
            b.note(
                Severity::Warning,
                place_name,
                format!(
                    "frame {} names a cell that is not an alternative",
                    key.frame
                ),
            );
            continue;
        };
        let next = keys
            .get(i + 1)
            .map_or(b.end_frame.max(key.frame) + 1, |k| k.frame);
        let (Ok(start), Some(duration)) = (
            u32::try_from(key.frame),
            u32::try_from(next - key.frame)
                .ok()
                .and_then(NonZeroU32::new),
        ) else {
            continue;
        };
        let instance = Instance::new(InstanceId::new(), *symbol, Transform2D::IDENTITY);
        spans.push(FrameSpan::new(start, duration, Content::Instance(instance)));
    }
    let _ = project;
    if let Ok(layer_out) = Layer::new(LayerId::new(), &layer.name, spans) {
        b.push_layer(layer_out.with_parent(parent).with_visible(layer.visible));
    }
}
