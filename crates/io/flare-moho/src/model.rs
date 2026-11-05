//! The typed model of a Moho project, as read. Nothing here is a Flare document yet.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Project {
    /// The member of the container that held the document, or the file's own name for a bare one.
    pub entry: String,
    pub version: i64,
    pub major_version: i64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub start_frame: i64,
    pub end_frame: i64,
    /// Top level, in draw order.
    pub layers: Vec<Layer>,
    pub bones: Vec<Bone>,
    /// Every animated value, with the JSON path where it was found.
    pub tracks: Vec<Track>,
    /// Relative image paths the project refers to, sorted and deduplicated.
    pub images: Vec<String>,
    /// Relative audio paths the project refers to, sorted and deduplicated.
    pub audio: Vec<String>,
}

impl Project {
    /// Counts what the project contains, for reports.
    pub fn census(&self) -> Census {
        let mut census = Census {
            bones: self.bones.len(),
            tracks: self.tracks.len(),
            keyframes: self.tracks.iter().map(|t| t.channel.len()).sum(),
            images: self.images.len(),
            audio: self.audio.len(),
            ..Census::default()
        };
        count_layers(&self.layers, &mut census);
        census
    }
}

fn count_layers(layers: &[Layer], census: &mut Census) {
    for layer in layers {
        census.layers += 1;
        *census
            .layers_by_kind
            .entry(layer.kind.name().to_owned())
            .or_insert(0) += 1;
        if layer.switch.is_some() {
            census.switches += 1;
        }
        count_layers(&layer.children, census);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub name: String,
    pub kind: LayerKind,
    pub uuid: String,
    pub visible: bool,
    /// Nesting depth, 0 at the top.
    pub depth: usize,
    /// Which bone the layer follows.
    pub binding: Binding,
    /// Bone indices joined by `|`, when the layer is bound to a region of bones.
    pub flexible_bone_subset: String,
    pub image_path: Option<String>,
    pub texture_path: Option<String>,
    /// Present on switch layers: which child is shown over time.
    pub switch: Option<SwitchCells>,
    /// The drawing of a vector layer, when the document held one.
    pub mesh: Option<Mesh>,
    pub children: Vec<Layer>,
}

/// The art of a vector layer: points, and curves that join them. Bezier handles are not read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    /// Rest position of each point, in Moho units.
    pub points: Vec<[f64; 2]>,
    pub curves: Vec<MeshCurve>,
    /// Fill colour of the drawing (r, g, b, a in 0..=1): the first filled shape's. Moho fills
    /// regions, not whole drawings, so a drawing with several fills keeps only this one.
    pub fill: Option<[f64; 4]>,
    /// Distinct fill colours among the filled shapes.
    pub fills: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshCurve {
    pub closed: bool,
    /// Indices into [`Mesh::points`], in order along the curve.
    pub points: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerKind {
    /// `MeshLayer`: a vector drawing.
    Vector,
    Image,
    Bone,
    Switch,
    Group,
    /// Any other type name, kept as written.
    Other(String),
}

impl LayerKind {
    pub fn from_moho(raw: &str) -> Self {
        match raw {
            "MeshLayer" => Self::Vector,
            "ImageLayer" => Self::Image,
            "BoneLayer" => Self::Bone,
            "SwitchLayer" => Self::Switch,
            "GroupLayer" => Self::Group,
            other => Self::Other(other.to_owned()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Vector => "vector",
            Self::Image => "image",
            Self::Bone => "bone",
            Self::Switch => "switch",
            Self::Group => "group",
            Self::Other(raw) => raw.as_str(),
        }
    }
}

/// What a layer follows. Moho stores -2 for none and -1 for a region of bones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    None,
    Region,
    Bone(usize),
}

impl Binding {
    pub(crate) fn from_raw(raw: i64) -> Self {
        match raw {
            -1 => Self::Region,
            n if n >= 0 => usize::try_from(n).map_or(Self::None, Self::Bone),
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bone {
    pub name: String,
    /// Index of the parent in [`Project::bones`]; `None` for a root. A parent may come later in
    /// the list, and no chain of parents loops.
    pub parent: Option<usize>,
    pub length: f64,
    pub strength: f64,
    /// Whether the bone has spring and damping dynamics at playback.
    pub dynamic: bool,
    /// The bone's path in the document, the prefix of its keyframe tracks' paths (for example
    /// `layers[0].skeleton.bones[3]`).
    pub path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCells {
    /// Names of the child layers the switch can show.
    pub alternatives: Vec<String>,
    /// The string channel that names the child shown at each key.
    pub keys: Channel,
    /// The child named at the first key. Moho's frame 0 is the rest pose.
    pub at_rest: Option<String>,
    /// The child named at the last key.
    pub at_end: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// JSON path of the channel, such as `layers[0].transforms.translation`.
    pub path: String,
    pub channel: Channel,
}

/// One animated value: keys in frame order and how each segment is interpolated.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Channel {
    pub kind: ChannelKind,
    pub keys: Vec<Key>,
    /// One name per segment (for example `Smooth`, `Linear`), empty where none was given.
    pub interp: Vec<String>,
    pub muted: bool,
}

impl Channel {
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub frame: i64,
    pub value: KeyValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyValue {
    Number(f64),
    /// Two to four components, such as a point (x, y) or a colour.
    Vector(Vec<f64>),
    Bool(bool),
    Text(String),
    /// A value of a shape the reader does not know, kept out of the model.
    Unknown,
}

impl KeyValue {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ChannelKind {
    #[default]
    Val,
    Vec2,
    Vec3,
    Color,
    Bool,
    String,
    Other(String),
}

impl ChannelKind {
    pub(crate) fn from_moho(raw: &str) -> Self {
        match raw {
            "Val" => Self::Val,
            "Vec2" => Self::Vec2,
            "Vec3" => Self::Vec3,
            "Color" => Self::Color,
            "Bool" => Self::Bool,
            "String" => Self::String,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// Counts for reports: what the project contains.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Census {
    pub layers: usize,
    pub layers_by_kind: BTreeMap<String, usize>,
    pub bones: usize,
    pub switches: usize,
    pub tracks: usize,
    pub keyframes: usize,
    pub images: usize,
    pub audio: usize,
}
