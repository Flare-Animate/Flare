//! Placing the reference mannequin on the stage: the posed figure, shaded as the panel shows
//! it, with its guide lines and its shadow, becomes a bitmap on a layer of its own above the
//! others, drawn see-through while editing, so it can be drawn over, moved, scaled and turned
//! like any picture, and hidden or deleted like any layer. One batch, one undo step. The desktop app, `flare mannequin place`
//! and the MCP server's `place_mannequin` all call [`place`].

use std::fmt;
use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{BitmapId, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer};
use flare_mannequin::{Look, MAX_PICTURE_SIDE, Mannequin, Picture, Placement};
use glamx::{DAffine2, DVec2};

use crate::common::{SpanRef, scene_timeline};
use crate::drawing::{DrawRefusal, Target, place_contents};

/// How opaque the stage draws the placed figure while editing, in percent; exports ignore it.
pub const REFERENCE_OPACITY: u8 = 50;
/// Pixels of the placed picture to a unit of the stage, so it stays sharp when zoomed in; a
/// figure too large for that is made at the most a picture may be.
pub const RESOLUTION: f64 = 2.0;
/// Stage units left clear round the figure, so its outline is not cut.
const MARGIN: f64 = 2.0;

/// Why the figure was not placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceRefusal {
    NoSuchScene,
    /// The figure is too far off or too small to be pictured there.
    BadPlacement,
    /// The layer, the picture or its keyframe could not be built.
    TooMany,
    /// The layer asked for cannot take it, and why.
    Layer(DrawRefusal),
}

impl fmt::Display for PlaceRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoSuchScene => "no such scene",
            Self::BadPlacement => "the mannequin is too far off or too small to be drawn there",
            Self::TooMany => "the mannequin's layer cannot be made",
            Self::Layer(why) => return write!(f, "the mannequin cannot go on that layer: {why}"),
        })
    }
}

impl std::error::Error for PlaceRefusal {}

/// The commands that place the figure, and what they make.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub commands: Vec<Command>,
    pub layer: LayerId,
    /// The placed picture, as an element of its keyframe.
    pub at: SpanRef,
    pub bitmap: BitmapId,
    /// The picture's size in pixels.
    pub size: (u32, u32),
}

/// The figure pictured for `placement` on the stage, and where the picture's top left corner
/// goes there and how many of its pixels make a stage unit.
pub fn picture(
    figure: &Mannequin,
    placement: Placement,
) -> Result<(Picture, [f64; 2], f64), PlaceRefusal> {
    let (lo, hi) = figure.bounds(placement, true);
    let corner = [lo[0] - MARGIN, lo[1] - MARGIN];
    let size = [hi[0] - lo[0] + 2.0 * MARGIN, hi[1] - lo[1] + 2.0 * MARGIN];
    let most = f64::from(MAX_PICTURE_SIDE);
    let scale = RESOLUTION.min(most / size[0].max(size[1]));
    if !(scale.is_finite() && scale > 0.0) {
        return Err(PlaceRefusal::BadPlacement);
    }
    let pixels = |v: f64| (v * scale).ceil().clamp(1.0, most) as u32;
    let center = [
        (placement.center[0] - corner[0]) * scale,
        (placement.center[1] - corner[1]) * scale,
    ];
    let inside =
        Placement::new(center, placement.height * scale).ok_or(PlaceRefusal::BadPlacement)?;
    let picture = figure
        .render(inside, pixels(size[0]), pixels(size[1]), Look::default())
        .ok_or(PlaceRefusal::BadPlacement)?;
    Ok((picture, corner, scale))
}

/// Places `figure` at `placement` on a new layer called `name`, at the top of `scene`, in one
/// keyframe from `frame` to the end of the scene (one frame in an empty one).
pub fn place(
    document: &Document,
    scene: SceneId,
    frame: u32,
    figure: &Mannequin,
    placement: Placement,
    name: &str,
) -> Result<Placed, PlaceRefusal> {
    let timeline = scene_timeline(document, scene).ok_or(PlaceRefusal::NoSuchScene)?;
    let (bitmap, shown, size) = pictured(document, figure, placement, name)?;
    let bitmap_id = shown.bitmap();
    let to_end = u32::try_from(timeline.frame_count())
        .unwrap_or(u32::MAX)
        .saturating_sub(frame);
    let span = FrameSpan::new(
        frame,
        NonZeroU32::new(to_end).unwrap_or(NonZeroU32::MIN),
        Content::Bitmap(shown),
    );
    let id = LayerId::new();
    let layer = Layer::new(id, name, vec![span])
        .map_err(|_| PlaceRefusal::TooMany)?
        .with_view_opacity(REFERENCE_OPACITY);
    let mut commands: Vec<Command> = bitmap.into_iter().map(Command::AddBitmap).collect();
    commands.extend([
        Command::AddLayer { scene, layer },
        Command::MoveLayer {
            scene,
            layer: id,
            index: 0,
        },
    ]);
    Ok(Placed {
        commands,
        layer: id,
        at: SpanRef {
            layer: id,
            start: frame,
            element: 0,
        },
        bitmap: bitmap_id,
        size,
    })
}

/// Which keyframe of an existing layer the figure goes into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyframe {
    /// The keyframe that shows at the frame, beside what it holds; with none there, a keyframe
    /// of its own from the frame up to the layer's next one (or the end of the scene).
    Join,
    /// A keyframe of its own from the frame: one that shows there from an earlier frame is cut
    /// short, and the figure shows to where it ended. A keyframe that starts at the frame is
    /// joined.
    New,
}

/// Places `figure` at `placement` on the existing `layer` of `scene` at `frame`, into the
/// keyframe `keyframe` says, as drawing on that layer would go: a locked, hidden, folder, mask,
/// guide or camera layer is refused. The picture is named `name` in the library.
#[allow(clippy::too_many_arguments)]
pub fn place_onto(
    document: &Document,
    scene: SceneId,
    frame: u32,
    figure: &Mannequin,
    placement: Placement,
    layer: LayerId,
    keyframe: Keyframe,
    name: &str,
) -> Result<Placed, PlaceRefusal> {
    let timeline = scene_timeline(document, scene).ok_or(PlaceRefusal::NoSuchScene)?;
    let (bitmap, shown, size) = pictured(document, figure, placement, name)?;
    let bitmap_id = shown.bitmap();
    let content = Content::Bitmap(shown);
    let joined = place_contents(
        document,
        scene,
        Target::Layer(layer),
        frame,
        None,
        vec![content.clone()],
        name,
    );
    let held = timeline
        .layers()
        .iter()
        .find(|l| l.id() == layer)
        .and_then(|l| l.span_at(frame))
        .filter(|span| span.start() < frame);
    let mut commands: Vec<Command> = bitmap.into_iter().map(Command::AddBitmap).collect();
    let at = match (keyframe, held) {
        (Keyframe::New, Some(span)) => {
            // The checks a drawing gets, except that a keyframe that tweens can be cut.
            match joined {
                Ok(_) | Err(DrawRefusal::Held) => {}
                Err(why) => return Err(PlaceRefusal::Layer(why)),
            }
            let before = NonZeroU32::new(frame - span.start()).ok_or(PlaceRefusal::TooMany)?;
            let rest = u32::try_from(span.end() - u64::from(frame))
                .ok()
                .and_then(NonZeroU32::new)
                .ok_or(PlaceRefusal::TooMany)?;
            commands.extend([
                Command::RemoveSpan {
                    scene,
                    layer,
                    start: span.start(),
                },
                Command::InsertSpan {
                    scene,
                    layer,
                    span: span.held().retimed(span.start(), before),
                },
                Command::InsertSpan {
                    scene,
                    layer,
                    span: FrameSpan::new(frame, rest, content),
                },
            ]);
            SpanRef {
                layer,
                start: frame,
                element: 0,
            }
        }
        _ => {
            let plan = joined.map_err(PlaceRefusal::Layer)?;
            commands.extend(plan.commands);
            *plan.placed.first().ok_or(PlaceRefusal::TooMany)?
        }
    };
    Ok(Placed {
        commands,
        layer,
        at,
        bitmap: bitmap_id,
        size,
    })
}

/// A picture for the library (none when it has the same one), an instance showing the figure
/// where it was placed, and the picture's size.
type Pictured = (Option<Bitmap>, BitmapInstance, (u32, u32));

/// The figure as a picture, and an instance showing it where it was placed. The picture is
/// for the library unless the library has the same one already (the same pose placed again),
/// which is shown instead.
fn pictured(
    document: &Document,
    figure: &Mannequin,
    placement: Placement,
    name: &str,
) -> Result<Pictured, PlaceRefusal> {
    let (picture, corner, scale) = picture(figure, placement)?;
    let size = (picture.width(), picture.height());
    let bitmap = Bitmap::new(BitmapId::new(), name, size.0, size.1, picture.into_rgba())
        .map_err(|_| PlaceRefusal::TooMany)?;
    let known = document
        .bitmaps()
        .iter()
        .find(|b| b.content_hash() == bitmap.content_hash())
        .map(Bitmap::id);
    let matrix = DAffine2::from_scale_angle_translation(
        DVec2::splat(1.0 / scale),
        0.0,
        DVec2::new(corner[0], corner[1]),
    );
    let transform = Transform2D::new(matrix).map_err(|_| PlaceRefusal::BadPlacement)?;
    let shown = BitmapInstance::new(InstanceId::new(), known.unwrap_or(bitmap.id()), transform);
    Ok((known.is_none().then_some(bitmap), shown, size))
}

#[cfg(test)]
mod tests;
