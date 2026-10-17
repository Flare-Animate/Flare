//! Original vector icons in the visual language of Animate's UI: light glyphs
//! on an 18-unit grid, rasterized with tiny-skia and tinted when drawn.
//! Toolbar glyphs fill the 16-unit box at 1..17, one unit per pixel at 100%:
//! about half solid shapes, half 1 px outlines, diagonal tools working at the
//! lower left.

use std::collections::HashMap;

use eframe::egui::{self, Color32, ColorImage, Painter, Rect, TextureHandle, TextureOptions, pos2, vec2};
use minnamate_core::tiny_skia::{self as sk, FillRule, LineCap, LineJoin, PathBuilder, Pixmap, Transform};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    // Tools.
    Selection,
    Subselection,
    FreeTransform,
    GradientTransform,
    AssetWarp,
    Lasso,
    Polygon,
    MagicWand,
    Pen,
    AddAnchor,
    DeleteAnchor,
    ConvertAnchor,
    Text,
    Line,
    Rectangle,
    RectanglePrimitive,
    Oval,
    OvalPrimitive,
    PolyStar,
    Pencil,
    /// The Classic Brush.
    Brush,
    FluidBrush,
    PaintBrush,
    PaintBucket,
    InkBottle,
    Eyedropper,
    Eraser,
    Width,
    Bone,
    Bind,
    Camera,
    Hand,
    Rotation,
    TimeScrub,
    Zoom,
    Rotation3D,
    Translation3D,
    /// The Edit Toolbar drawer's separator tile.
    Spacer,
    // Tool options.
    ObjectDrawing,
    Smooth,
    Straighten,
    LockFill,
    PaintNormal,
    EraseNormal,
    EraseFills,
    EraseLines,
    EraseSelectedFills,
    EraseInside,
    GapNone,
    GapSmall,
    GapMedium,
    GapLarge,
    // Timeline and chrome.
    Play,
    Pause,
    ToStart,
    StepBack,
    StepForward,
    ToEnd,
    Loop,
    OnionSkin,
    OnionOutline,
    Eye,
    Lock,
    Outline,
    NewLayer,
    NewFolder,
    Delete,
    Layer,
    Folder,
    Scene,
    Back,
    CenterStage,
    ClipContent,
    EditSymbols,
    InsertKeyframe,
    InsertBlankKeyframe,
    InsertFrame,
    ClassicTween,
    TestMovie,
    Publish,
    More,
    Swap,
    MovieClip,
    Graphic,
    Button,
    // App bar, edit bar and docks.
    Home,
    Workspace,
    Reset,
    StageRotation,
    Club,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChevronsRight,
    ChevronsLeft,
    WinMinimize,
    WinMaximize,
    WinRestore,
    WinClose,
    // Collapsed panels.
    ColorPanel,
    Swatches,
    Align,
    Info,
    TransformPanel,
    MotionPresets,
    // Timeline (2024).
    TlNewLayer,
    TlParenting,
    TlLayerDepth,
    TlKeyframe,
    TlBlankKeyframe,
    TlFrame,
    TlOnion,
    TlEditMultiple,
    TlClassicTween,
    TlCenterFrame,
    TlLoop,
    TlStepBack,
    TlStepForward,
    TlResetZoom,
    TlMountains,
    TlLayerView,
    TlEyeSlash,
    TlLock,
    TlOutlineBar,
    /// Layer type icons: a mask, a guide, a motion guide (a guide with layers under it).
    LayerMask,
    LayerGuide,
    LayerMotionGuide,
    // Home screen quick-start tiles (drawn about 72 px across).
    HomeFullHd,
    HomeGamepad,
    HomeBanner,
    HomeTablet,
    HomeCanvas,
    HomeMore,
    // New Document: tab icons (about 30 px), then tile glyphs (about 72 px).
    NdCharacter,
    NdSocial,
    NdGame,
    NdEducation,
    NdAds,
    NdWeb,
    NdAdvanced,
    NdPreset,
    NdCanvas,
    NdActionScript,
    // Properties: quick actions, segmented controls, headers.
    Magnet,
    SnapAlign,
    Rulers,
    CapNone,
    CapRound,
    CapSquare,
    JoinMiter,
    JoinRound,
    JoinBevel,
    PlayOnce,
    SingleFrame,
    ReversePlayOnce,
    ReverseLoop,
    Unlink,
    LinesToFills,
    Group,
    BreakApart,
    ConvertToSymbol,
    CreateBrush,
    Shape,
    Frame,
    Actions,
    MotionTween,
    ShapeTween,
    Sliders,
    AlignLeft,
    AlignCenter,
    AlignRight,
    AlignJustify,
    // Library and the Swatches panel.
    Pin,
    NewLibrary,
    FolderOpen,
    Tag,
    /// A sound: a speaker sending out two arcs.
    Sound,
    /// Edit Envelope: a speaker and a pencil.
    EditEnvelope,
    /// A library bitmap: a frame with a hill and a sun.
    Bitmap,
    // Document Settings.
    Link,
    // The Filters section's header.
    Plus,
    Gear,
    // The Actions panel's Code Snippets.
    Code,
    // The Scene panel.
    Duplicate,
}

/// A gamepad's outline, for Home's Android tile and New Document's Game tab.
const GAMEPAD: &str = "M5.2 4.5 H12.8 C15 4.5 16.3 5.8 16.8 8 L17.4 11.4 C17.7 13 16.9 14.4 15.6 14.4 C14.7 14.4 14.1 13.9 13.6 13.2 \
    L12.6 11.8 H5.4 L4.4 13.2 C3.9 13.9 3.3 14.4 2.4 14.4 C1.1 14.4 0.3 13 0.6 11.4 L1.2 8 C1.7 5.8 3 4.5 5.2 4.5 Z";

/// One drawing step, in 18-unit grid coordinates.
#[derive(Clone)]
enum Part {
    Fill(&'static str),
    Line(&'static str, f32),
    Dot(f32, f32, f32),
    Ring(f32, f32, f32, f32),
    /// An ellipse: outlined `width` wide, or filled when `width` is 0.
    Oval(f32, f32, f32, f32, f32),
    /// A rectangle (x, y, w, h) shading from the tint at its left to dark grey.
    Ramp(f32, f32, f32, f32),
    /// Parts drawn in grey, `level` times the tint: the mid-grey inside shape glyphs.
    Grey(f32, Vec<Part>),
    /// Parts that erase what's drawn under them: the gap that keeps overlapping parts apart.
    Cut(Vec<Part>),
    /// Parts drawn through a transform: turned onto the diagonal, or scaled down.
    T(Transform, Vec<Part>),
}
use Part::*;

/// Turned 45° clockwise about the middle, so a glyph drawn upright, working
/// end down, ends up working at the lower left as Animate's diagonal tools do.
fn diag(parts: Vec<Part>) -> Part {
    T(Transform::from_rotate_at(45.0, 9.0, 9.0), parts)
}

/// Scaled by `s` about the middle, then moved by (dx, dy).
fn scaled(s: f32, dx: f32, dy: f32, parts: Vec<Part>) -> Part {
    T(Transform::from_row(s, 0.0, 0.0, s, 9.0 - 9.0 * s + dx, 9.0 - 9.0 * s + dy), parts)
}

/// The Selection arrowhead: tip at the top left, no tail.
const ARROW: &str = "M4.5 1.5 V16.5 L8.5 12.5 H15.5 Z";
/// The Rectangle glyph's outline; the Eraser and Brush mode glyphs use it too.
const SQUARE: &str = "M2.5 2.5 H15.5 V15.5 H2.5 Z";
const HEXAGON: &str = "M1.5 9 L5.25 2.5 H12.75 L16.5 9 L12.75 15.5 H5.25 Z";

/// Classic Brush: a round-ended handle, a gap, then a teardrop of bristles.
fn brush() -> Vec<Part> {
    vec![diag(vec![
        Line("M9 -1.4 V6.8", 3.0),
        Dot(9.0, 11.6, 2.25),
        Fill("M6.75 11.6 C6.75 14.6 8.3 17.2 9 19.6 C9.7 17.2 11.25 14.6 11.25 11.6 Z"),
    ])]
}

/// A fountain-pen nib with its slit and hole, a gap, then the collar.
fn nib() -> Vec<Part> {
    vec![diag(vec![
        Fill("M9 19.4 C8.2 16.6 5.4 13.4 5.4 10.6 V5.4 H12.6 V10.6 C12.6 13.4 9.8 16.6 9 19.4 Z"),
        Cut(vec![Line("M9 19.8 V12.4", 0.9), Dot(9.0, 11.6, 1.1)]),
        Fill("M6.2 1.2 H11.8 V4.2 H6.2 Z"),
    ])]
}

/// An open hand: four fingers, the middle one tallest, and the thumb out to the left.
fn hand() -> Vec<Part> {
    vec![
        Line("M6.4 9 L5.6 4.6 M9.1 8.6 V2.6 M11.8 8.6 L12.4 3.4 M14.3 9.6 L15.4 6", 2.3),
        Line("M5.6 13.6 L2.4 10.4", 2.3),
        Cut(vec![Line("M7.75 8.4 L7.3 2.6 M10.45 8.4 L10.8 1.8 M13.1 8.8 L14 4", 0.9)]),
        Fill("M5.2 8.4 H15.4 V12.8 C15.4 15.4 13.4 17 10.8 17 H9.6 C7.8 17 6.4 16.2 5.6 14.6 L5.2 13.8 Z"),
    ]
}

/// The Lasso's loop where the rope crosses, and its tail.
fn lasso_tail() -> Vec<Part> {
    vec![Ring(4.2, 13.3, 1.7, 1.0), Line("M4.9 15 C5.5 15.9 5.2 16.8 3.8 17.1", 1.0)]
}

fn bone() -> Vec<Part> {
    vec![T(
        Transform::from_rotate_at(-45.0, 9.0, 9.0),
        vec![Fill("M7.6 4 H10.4 V14 H7.6 Z"), Dot(7.5, 3.6, 2.0), Dot(10.5, 3.6, 2.0), Dot(7.5, 14.4, 2.0), Dot(10.5, 14.4, 2.0)],
    )]
}

/// The mode glyphs' shape: mid-grey inside a light outline (`SQUARE`).
fn grey_square() -> Part {
    Grey(0.69, vec![Fill("M3 3 H15 V15 H3 Z")])
}

fn parts(icon: Icon) -> Vec<Part> {
    match icon {
        Icon::Selection => vec![Line(ARROW, 1.0)],
        Icon::Subselection => vec![Fill(ARROW), Line(ARROW, 1.0)],
        Icon::FreeTransform => vec![
            Line("M5.5 2.5 H16.5 V15.5 H5.5 Z", 1.0),
            Fill("M5 2h2v2h-2z M10 2h2v2h-2z M15 2h2v2h-2z M15 8h2v2h-2z M15 14h2v2h-2z M10 14h2v2h-2z M5 14h2v2h-2z M5 8h2v2h-2z"),
            Cut(vec![Fill("M0.5 0.5 V14 L10 9.5 Z"), Line("M0.5 0.5 V14 L10 9.5 Z", 2.0)]),
            Fill("M0.5 0.5 V14 L10 9.5 Z"),
        ],
        Icon::GradientTransform => vec![Ramp(4.0, 4.0, 10.0, 10.0), Line("M3.5 3.5 H14.5 V14.5 H3.5 Z", 1.0)],
        Icon::AssetWarp => vec![diag(vec![
            Fill("M6 1.5 C6 0.7 6.7 0 7.5 0 H10.5 C11.3 0 12 0.7 12 1.5 V3.2 H6 Z"),
            Fill("M7.4 3.2 H10.6 V6.6 H7.4 Z"),
            Fill("M5.5 6.6 H12.5 V8.2 H5.5 Z"),
            Line("M9 8.2 V18.6", 1.5),
        ])],
        Icon::Lasso => [vec![Oval(9.0, 7.5, 7.5, 5.0, 1.0)], lasso_tail()].concat(),
        Icon::Polygon => [vec![Line("M3.6 11 L4.6 3.2 L11.4 1.5 L16.5 6.8 L10.6 12.4 Z", 1.0)], lasso_tail()].concat(),
        Icon::MagicWand => vec![
            Line("M2.5 15.5 L10.5 7.5", 1.5),
            Fill("M13.5 1.5 L14.1 3.9 L16.5 4.5 L14.1 5.1 L13.5 7.5 L12.9 5.1 L10.5 4.5 L12.9 3.9 Z"),
            Fill("M8 0.5 L8.5 2 L10 2.5 L8.5 3 L8 4.5 L7.5 3 L6 2.5 L7.5 2 Z"),
            Fill("M15.5 8.5 L16 10 L17.5 10.5 L16 11 L15.5 12.5 L15 11 L13.5 10.5 L15 10 Z"),
        ],
        Icon::Pen => nib(),
        Icon::AddAnchor => vec![scaled(0.8, 2.0, 2.0, nib()), Line("M4.5 2 V7 M2 4.5 H7", 1.0)],
        Icon::DeleteAnchor => vec![scaled(0.8, 2.0, 2.0, nib()), Line("M2 4.5 H7", 1.0)],
        Icon::ConvertAnchor => vec![Line("M2.5 15 L9 3 L15.5 15", 1.25)],
        Icon::Text => vec![Fill("M2 2 H16 V6 H14.5 V4 H10 V14.5 H12 V16 H6 V14.5 H8 V4 H3.5 V6 H2 Z")],
        Icon::Line => vec![Line("M2.5 15.5 L15.5 2.5", 1.5)],
        Icon::Rectangle => vec![Grey(0.69, vec![Fill("M2 3 H16 V15 H2 Z")]), Line("M1.5 2.5 H16.5 V15.5 H1.5 Z", 1.0)],
        Icon::RectanglePrimitive => vec![
            Grey(0.69, vec![Fill("M3 4 H15 V14 H3 Z")]),
            Line("M2.5 3.5 H15.5 V14.5 H2.5 Z", 1.0),
            Fill("M1 2h3v3h-3z M14 2h3v3h-3z M1 13h3v3h-3z M14 13h3v3h-3z"),
        ],
        Icon::Oval => vec![Grey(0.69, vec![Oval(9.0, 9.0, 7.0, 5.0, 0.0)]), Oval(9.0, 9.0, 6.5, 4.5, 1.0)],
        Icon::OvalPrimitive => vec![
            Grey(0.69, vec![Oval(9.0, 9.5, 7.0, 5.0, 0.0)]),
            Oval(9.0, 9.5, 6.5, 4.5, 1.0),
            Line("M9 9.5 H15", 1.0),
            Dot(9.0, 9.5, 1.2),
            Fill("M14.5 8 h3 v3 h-3 z"),
        ],
        Icon::PolyStar => vec![Grey(0.69, vec![Fill(HEXAGON)]), Line(HEXAGON, 1.0)],
        Icon::Pencil => vec![diag(vec![
            Fill("M6.5 0.5 H11.5 V13 L9 18.6 L6.5 13 Z"),
            Cut(vec![Line("M6 3.4 H12", 0.9), Line("M6 13.2 H12", 0.8)]),
        ])],
        Icon::Brush => brush(),
        Icon::FluidBrush => [brush(), vec![Grey(0.86, vec![Dot(14.0, 13.0, 3.5)]), Ring(14.0, 13.0, 3.0, 1.0)]].concat(),
        Icon::PaintBrush => vec![scaled(0.75, 2.5, -2.5, brush()), Line("M5.6 12.6 C3.2 12.2 1.6 13.8 3.2 15 C4.6 16 3.6 17.2 1.2 16.9", 1.25)],
        Icon::PaintBucket => vec![
            diag(vec![Fill("M4 6 H12 V14.5 C12 15.6 11.1 16.5 10 16.5 H6 C4.9 16.5 4 15.6 4 14.5 Z")]),
            Line("M6.1 4.8 C3.8 2.5 0.1 6.2 2.4 8.5", 1.25),
            Line("M13.6 9.6 V13", 1.5),
            Fill("M13.6 12.4 C14.5 13.6 15 14.4 15 15.1 C15 15.9 14.4 16.5 13.6 16.5 C12.8 16.5 12.2 15.9 12.2 15.1 C12.2 14.4 12.7 13.6 13.6 12.4 Z"),
        ],
        Icon::InkBottle => vec![
            T(
                Transform::from_rotate_at(135.0, 6.0, 7.0),
                vec![Fill("M3 5.5 C3 4.7 3.7 4 4.5 4 H7.5 C8.3 4 9 4.7 9 5.5 V10 C9 10.6 8.6 11 8 11 H4 C3.4 11 3 10.6 3 10 Z M4.6 2 H7.4 V4 H4.6 Z")],
            ),
            Dot(11.4, 12.0, 0.9),
            Dot(13.2, 13.6, 0.8),
            Line("M10.5 16.5 H16.5 V10.5", 1.25),
        ],
        Icon::Eyedropper => vec![diag(vec![
            Fill("M6.9 4.2 V1.2 C6.9 -1.6 11.1 -1.6 11.1 1.2 V4.2 Z"),
            Fill("M5.6 4.8 H12.4 V6.4 H5.6 Z"),
            Line("M7.5 7 V17.2 C7.5 19.2 10.5 19.2 10.5 17.2 V7", 1.0),
        ])],
        Icon::Eraser => vec![diag(vec![Fill("M4.5 2.5 H13.5 V10.7 H4.5 Z"), Line("M5 12.2 H13 V15 H5 Z", 1.0)])],
        Icon::Width => vec![
            Fill("M2 11.5 C5 7.2 8 6.6 9.6 8.6 C11.1 10.5 13 10.8 16 6.6 C13.6 12.4 10.6 13.2 8.7 10.8 C7.4 9.2 5 9.2 2 11.5 Z"),
            Cut(vec![Dot(10.5, 3.5, 2.4), Dot(10.5, 14.5, 2.4)]),
            Line("M10.5 5 V13", 1.0),
            Ring(10.5, 3.5, 1.5, 1.0),
            Ring(10.5, 14.5, 1.5, 1.0),
        ],
        Icon::Bone => bone(),
        Icon::Bind => vec![
            Ring(5.0, 5.0, 4.0, 1.0),
            Cut(vec![Line("M0 5 H10 M5 0 V10 M1.5 1.5 L8.5 8.5 M8.5 1.5 L1.5 8.5", 1.0)]),
            scaled(0.8, 1.2, 1.2, bone()),
        ],
        Icon::Camera => vec![
            Fill("M1.5 6.5 C1.5 5.7 2.2 5 3 5 H10 C10.8 5 11.5 5.7 11.5 6.5 V11.5 C11.5 12.3 10.8 13 10 13 H3 C2.2 13 1.5 12.3 1.5 11.5 Z"),
            Fill("M12.5 7.6 L16.5 5.2 V12.8 L12.5 10.4 Z"),
        ],
        Icon::Hand => hand(),
        Icon::Rotation => vec![
            T(Transform::from_rotate_at(20.0, 10.5, 10.5), vec![Line("M5 5 H16 V16 H5 Z", 1.0)]),
            scaled(0.75, 2.2, 2.2, hand()),
            Line("M1.5 8.5 C1.5 4.6 4.6 1.5 8.5 1.5 M6.6 -0.4 L8.5 1.5 L6.6 3.4", 1.0),
        ],
        Icon::TimeScrub => vec![
            Line("M2.5 3.5 H15.5 M4.5 1.5 L2.5 3.5 L4.5 5.5 M13.5 1.5 L15.5 3.5 L13.5 5.5", 1.0),
            scaled(0.7, 0.4, 2.8, hand()),
        ],
        Icon::Zoom => vec![Ring(6.7, 7.0, 5.5, 1.25), Line("M11.2 11.4 L16.2 16.4", 2.0)],
        Icon::Rotation3D => vec![
            Ring(9.0, 9.0, 6.0, 1.0),
            T(Transform::from_rotate_at(-20.0, 9.0, 9.0), vec![Oval(9.0, 9.0, 6.0, 2.2, 1.0)]),
            Oval(9.0, 9.0, 2.4, 6.0, 1.0),
        ],
        Icon::Translation3D => vec![Line(
            "M8 10 V2.5 M6 4.5 L8 2.5 L10 4.5 M8 10 H15.5 M13.5 8 L15.5 10 L13.5 12 M8 10 L2.5 15.5 M2.5 12.7 V15.5 H5.3",
            1.25,
        )],
        Icon::Spacer => vec![Fill("M1 8 H17 V10 H1 Z")],
        Icon::ObjectDrawing => vec![Line("M2.5 2.5 H14.5 V14.5 H2.5 Z", 1.0), Grey(0.8, vec![Dot(8.5, 8.5, 4.5)])],
        Icon::Smooth => vec![Line("M12.5 1.5 C3 1.5 15 16.5 5.5 16.5", 1.0)],
        Icon::Straighten => vec![Line("M5.5 1.5 V9.5 H12.5 V16.5", 1.0)],
        Icon::LockFill => vec![
            Ramp(2.0, 2.0, 11.0, 11.0),
            Line("M1.5 1.5 H13.5 V13.5 H1.5 Z", 1.0),
            Cut(vec![Fill("M9.5 8 H17.5 V17.5 H9.5 Z")]),
            Fill("M10.5 12.5 H16.5 V16.5 H10.5 Z"),
            Line("M11.8 12.5 V11 C11.8 8.9 15.2 8.9 15.2 11 V12.5", 1.0),
        ],
        // Erasing and painting modes: a shape whose fill and outline the band
        // across it covers, or doesn't.
        Icon::PaintNormal => vec![grey_square(), Line(SQUARE, 1.0), Fill("M0.5 7.5 H17.5 V10.5 H0.5 Z")],
        Icon::EraseNormal => vec![grey_square(), Line(SQUARE, 1.0), Cut(vec![Fill("M0 7.5 H18 V10.5 H0 Z")])],
        Icon::EraseFills => vec![grey_square(), Cut(vec![Fill("M0 7.5 H18 V10.5 H0 Z")]), Line(SQUARE, 1.0)],
        Icon::EraseLines => vec![Line(SQUARE, 1.0), Cut(vec![Fill("M0 7.5 H18 V10.5 H0 Z")]), grey_square()],
        Icon::EraseSelectedFills => vec![grey_square(), Cut(vec![Fill("M0 7.5 H9 V10.5 H0 Z")]), Line(SQUARE, 1.0)],
        Icon::EraseInside => vec![grey_square(), Cut(vec![Fill("M5.5 7.5 H12.5 V10.5 H5.5 Z")]), Line(SQUARE, 1.0)],
        Icon::GapNone => vec![Ring(9.0, 9.0, 5.5, 1.25)],
        Icon::GapSmall => vec![Ring(9.0, 9.0, 5.5, 1.25), Cut(vec![Fill("M8 1 H10 V6 H8 Z")])],
        Icon::GapMedium => vec![Ring(9.0, 9.0, 5.5, 1.25), Cut(vec![Fill("M6.5 1 H11.5 V6 H6.5 Z")])],
        Icon::GapLarge => vec![Ring(9.0, 9.0, 5.5, 1.25), Cut(vec![Fill("M4.5 1 H13.5 V7 H4.5 Z")])],
        Icon::Play => vec![Fill("M5 3 L15 9 L5 15 Z")],
        Icon::Pause => vec![Fill("M5 3.5 H8 V14.5 H5 Z M10 3.5 H13 V14.5 H10 Z")],
        Icon::ToStart => vec![Fill("M3.5 4 H5 V14 H3.5 Z M10 4 L5.5 9 L10 14 Z M15 4 L10.5 9 L15 14 Z")],
        Icon::StepBack => vec![Fill("M4.5 4 H6 V14 H4.5 Z M13.5 4 L7 9 L13.5 14 Z")],
        Icon::StepForward => vec![Fill("M12 4 H13.5 V14 H12 Z M4.5 4 L11 9 L4.5 14 Z")],
        Icon::ToEnd => vec![Fill("M13 4 H14.5 V14 H13 Z M3 4 L7.5 9 L3 14 Z M8 4 L12.5 9 L8 14 Z")],
        Icon::Loop => vec![Line(
            "M3.5 8 V7 C3.5 5.3 4.8 4 6.5 4 H13.5 M11.3 1.9 L13.6 4 L11.3 6.1 M14.5 10 V11 C14.5 12.7 13.2 14 11.5 14 H4.5 M6.7 16.1 L4.4 14 L6.7 11.9",
            1.3,
        )],
        Icon::OnionSkin => vec![Line("M2.5 2.5 H11.5 V11.5 H2.5 Z", 1.1), Fill("M6.5 6.5 H15.5 V15.5 H6.5 Z")],
        Icon::OnionOutline => vec![Line("M2.5 2.5 H11.5 V11.5 H2.5 Z M6.5 6.5 H15.5 V15.5 H6.5 Z", 1.1)],
        Icon::Eye => vec![Line("M1.5 9 C4 5 7 4 9 4 C11 4 14 5 16.5 9 C14 13 11 14 9 14 C7 14 4 13 1.5 9 Z", 1.2), Dot(9.0, 9.0, 2.4)],
        Icon::Lock => vec![Fill("M4.5 8 H13.5 V15.5 H4.5 Z"), Line("M6.6 8 V6 C6.6 3 11.4 3 11.4 6 V8", 1.4)],
        Icon::Outline => vec![Line("M4 4 H14 V14 H4 Z", 1.4)],
        Icon::NewLayer => vec![Line("M4 2.5 H10.8 L14 5.7 V15.5 H4 Z", 1.1), Line("M9 7.8 V13.2 M6.3 10.5 H11.7", 1.3)],
        Icon::NewFolder => vec![Line("M2 4.5 H7 L8.5 6 H16 V14.5 H2 Z", 1.1), Line("M9 7.8 V12.8 M6.5 10.3 H11.5", 1.2)],
        Icon::Delete => vec![Line("M3.5 5 H14.5 M7 5 V3.2 H11 V5 M5 5 L5.8 15.5 H12.2 L13 5 M7.8 7.5 V13 M10.2 7.5 V13", 1.1)],
        Icon::Layer => vec![Line("M4.5 2.5 H10.5 L13.5 5.5 V15.5 H4.5 Z M10.5 2.5 V5.5 H13.5", 1.0)],
        Icon::Folder => vec![Fill("M2 4 H7 L8.5 5.5 H16 V14.5 H2 Z")],
        Icon::Scene => vec![Fill("M2.5 7.2 H15.5 V15 H2.5 Z"), Fill("M2.4 3.9 L14.9 2.1 L15.4 4.8 L2.9 6.6 Z")],
        Icon::Back => vec![Line("M15 9 H3.5 M8 4.5 L3.5 9 L8 13.5", 1.4)],
        Icon::CenterStage => vec![Ring(9.0, 9.0, 4.5, 1.2), Line("M9 1.5 V5 M9 13 V16.5 M1.5 9 H5 M13 9 H16.5", 1.2)],
        Icon::ClipContent => vec![Line("M5 1.5 V13 H16.5 M1.5 5 H13 V16.5", 1.3)],
        Icon::EditSymbols => vec![Fill("M2.5 15.5 L7 7.5 L11.5 15.5 Z"), Dot(12.5, 6.0, 3.3)],
        Icon::InsertKeyframe => vec![Line("M4.5 2 H13.5 V16 H4.5 Z", 1.1), Dot(9.0, 11.5, 2.3)],
        Icon::InsertBlankKeyframe => vec![Line("M4.5 2 H13.5 V16 H4.5 Z", 1.1), Ring(9.0, 11.5, 2.1, 1.1)],
        Icon::InsertFrame => vec![Line("M4.5 2 H13.5 V16 H4.5 Z M7.3 9.3 H10.7 V14 H7.3 Z", 1.1)],
        Icon::ClassicTween => vec![Dot(3.0, 9.0, 1.7), Line("M4.5 9 H14.5 M11.2 5.8 L14.6 9 L11.2 12.2", 1.3)],
        Icon::TestMovie => vec![Ring(9.0, 9.0, 7.0, 1.2), Fill("M7.2 5.6 L12.6 9 L7.2 12.4 Z")],
        Icon::Publish => vec![Line("M9 2.5 V11 M5.5 6 L9 2.5 L12.5 6 M3.5 10 V15.5 H14.5 V10", 1.3)],
        Icon::More => vec![Dot(4.0, 9.0, 1.4), Dot(9.0, 9.0, 1.4), Dot(14.0, 9.0, 1.4)],
        // A quarter arc bulging up and right, an arrowhead at each end; it fills the middle 12 units.
        Icon::Swap => vec![Line("M4.5 4.5 C9.47 4.5 13.5 8.53 13.5 13.5 M6.7 2.3 L4.5 4.5 L6.7 6.7 M11.3 11.3 L13.5 13.5 L15.7 11.3", 1.0)],
        // A square with a jagged bite out of its top right corner.
        Icon::MovieClip => vec![Fill("M2.5 2.5 H9.5 L10.8 4.2 L12 3.2 L12.6 5.3 L14.5 5 L13.9 6.8 L15.5 8 V15.5 H2.5 Z")],
        Icon::Graphic => vec![Fill("M2 15.5 L6.2 8.2 L10.4 15.5 Z"), Dot(12.6, 5.6, 3.1), Fill("M10.8 10 H15.8 V15 H10.8 Z")],
        Icon::Button => vec![Line("M2 4.8 H16 V13.2 H2 Z", 1.3), Fill("M7.5 7.6 L12.2 9.6 L10.2 10.4 L11.8 12.6 L10.9 13.2 L9.3 11 L7.8 12.4 Z")],
        // A solid house with a door notch.
        Icon::Home => vec![Fill("M9 2.4 L16.2 9 H14.2 V15.6 H10.4 V11.6 H7.6 V15.6 H3.8 V9 H1.8 Z")],
        Icon::Workspace => vec![Line("M2 3.5 H16 V14.5 H2 Z M11 3.5 V14.5", 1.2)],
        Icon::Reset => vec![Line("M14.5 9 C14.5 12 12 14.5 9 14.5 C6 14.5 3.5 12 3.5 9 C3.5 6 6 3.5 9 3.5 H12 M10 1.5 L12 3.5 L10 5.5", 1.3)],
        Icon::StageRotation => vec![
            Line("M15 9 C15 12.3 12.3 15 9 15 C5.7 15 3 12.3 3 9 C3 5.7 5.7 3 9 3 M9 3 L11.4 1.4 M9 3 L11.4 4.8", 1.2),
            Line("M7 7.3 L10.8 6.3 L11.7 9.8 L7.9 10.8 Z", 1.0),
        ],
        Icon::Club => vec![Dot(9.0, 5.4, 3.0), Dot(5.5, 10.4, 3.0), Dot(12.5, 10.4, 3.0), Fill("M8.2 10 H9.8 L10.9 16 H7.1 Z")],
        Icon::ChevronLeft => vec![Line("M11 4.5 L6.5 9 L11 13.5", 1.4)],
        Icon::ChevronRight => vec![Line("M7 4.5 L11.5 9 L7 13.5", 1.4)],
        Icon::ChevronDown => vec![Line("M5 7 L9 11 L13 7", 1.4)],
        Icon::ChevronsRight => vec![Line("M4 5 L8 9 L4 13 M9.5 5 L13.5 9 L9.5 13", 1.3)],
        Icon::ChevronsLeft => vec![Line("M14 5 L10 9 L14 13 M8.5 5 L4.5 9 L8.5 13", 1.3)],
        Icon::WinMinimize => vec![Line("M4.5 9.5 H13.5", 1.0)],
        Icon::WinMaximize => vec![Line("M4.5 4.5 H13.5 V13.5 H4.5 Z", 1.0)],
        Icon::WinRestore => vec![Line("M4.5 6.5 H11.5 V13.5 H4.5 Z M6.5 6.5 V4.5 H13.5 V11.5 H11.5", 1.0)],
        Icon::WinClose => vec![Line("M4.5 4.5 L13.5 13.5 M13.5 4.5 L4.5 13.5", 1.0)],
        Icon::ColorPanel => vec![Ring(9.0, 9.0, 6.2, 1.3), Fill("M9 2.8 C12.4 2.8 15.2 5.6 15.2 9 L9 9 Z")],
        Icon::Swatches => vec![Fill("M2.5 2.5 H8 V8 H2.5 Z M10 2.5 H15.5 V8 H10 Z M2.5 10 H8 V15.5 H2.5 Z M10 10 H15.5 V15.5 H10 Z")],
        Icon::Align => vec![Line("M3 2.5 V15.5", 1.2), Fill("M5 4.5 H14.5 V7.5 H5 Z M5 10.5 H11 V13.5 H5 Z")],
        Icon::Info => vec![Ring(9.0, 9.0, 6.5, 1.2), Fill("M8.2 8 H9.8 V13 H8.2 Z"), Dot(9.0, 5.6, 1.0)],
        Icon::TransformPanel => vec![Line("M2.5 6.5 H9.5 V13.5 H2.5 Z M6 4.5 L8.5 2.5 H15.5 L13 4.5 M15.5 2.5 V9.5 L13.5 11.5", 1.1)],
        Icon::MotionPresets => vec![Line("M2.5 14 C6.5 14 7 6 12.5 6", 1.3), Dot(14.2, 6.0, 1.8)],
        // Timeline glyphs. Where Animate shows a dark mark on a light shape, the
        // mark is cut out of the shape (even-odd), so the panel shows through.
        Icon::TlNewLayer => vec![Line("M3 3 H15 V15 H3 Z M9 5.6 V12.4 M5.6 9 H12.4", 1.5)],
        Icon::TlParenting => vec![
            Fill("M5.5 1.5 H12.5 V5.5 H5.5 Z M1 11 H7.5 V16 H1 Z M10.5 11 H17 V16 H10.5 Z"),
            Line("M9 5.5 V8.2 M4.2 8.2 H13.8 M4.2 8.2 V11 M13.8 8.2 V11", 1.2),
        ],
        Icon::TlLayerDepth => vec![Line("M3 2.5 V15 H15.5 M3 15 L13 5", 1.3), Fill("M14.5 3.5 L14.2 8.2 L9.8 4 Z")],
        Icon::TlKeyframe => vec![Fill("M3.5 2.5 H14.5 Q15.5 2.5 15.5 3.5 V14.5 Q15.5 15.5 14.5 15.5 H3.5 Q2.5 15.5 2.5 14.5 V3.5 Q2.5 2.5 3.5 2.5 Z M9 6.8 C10.2 6.8 11.2 7.8 11.2 9 C11.2 10.2 10.2 11.2 9 11.2 C7.8 11.2 6.8 10.2 6.8 9 C6.8 7.8 7.8 6.8 9 6.8 Z")],
        Icon::TlBlankKeyframe => vec![
            Fill("M3.5 2.5 H14.5 Q15.5 2.5 15.5 3.5 V14.5 Q15.5 15.5 14.5 15.5 H3.5 Q2.5 15.5 2.5 14.5 V3.5 Q2.5 2.5 3.5 2.5 Z M9 6.3 C10.5 6.3 11.7 7.5 11.7 9 C11.7 10.5 10.5 11.7 9 11.7 C7.5 11.7 6.3 10.5 6.3 9 C6.3 7.5 7.5 6.3 9 6.3 Z M9 7.7 C8.3 7.7 7.7 8.3 7.7 9 C7.7 9.7 8.3 10.3 9 10.3 C9.7 10.3 10.3 9.7 10.3 9 C10.3 8.3 9.7 7.7 9 7.7 Z"),
        ],
        Icon::TlFrame => vec![Fill("M3.5 2.5 H14.5 Q15.5 2.5 15.5 3.5 V14.5 Q15.5 15.5 14.5 15.5 H3.5 Q2.5 15.5 2.5 14.5 V3.5 Q2.5 2.5 3.5 2.5 Z M8 6 H10 V12 H8 Z")],
        Icon::TlOnion => vec![Ring(6.5, 9.0, 4.6, 1.3), Dot(11.5, 9.0, 5.0)],
        Icon::TlEditMultiple => vec![Fill("M2.5 3 H5.5 V15 H2.5 Z M3.3 12.2 H4.7 V13.6 H3.3 Z M7.5 3 H10.5 V15 H7.5 Z M8.3 12.2 H9.7 V13.6 H8.3 Z M12.5 3 H15.5 V15 H12.5 Z M13.3 12.2 H14.7 V13.6 H13.3 Z")],
        Icon::TlClassicTween => vec![
            Fill("M2 7.5 H10.5 V16 H2 Z"),
            Fill("M7.5 2 H16 V10.5 H7.5 Z M9.2 5.4 H12.2 V3.8 L14.8 6.25 L12.2 8.7 V7.1 H9.2 Z"),
        ],
        Icon::TlCenterFrame => vec![Fill("M3 3.5 H15 Q16.5 3.5 16.5 5 V13 Q16.5 14.5 15 14.5 H3 Q1.5 14.5 1.5 13 V5 Q1.5 3.5 3 3.5 Z M4 7.5 H14 V9.5 H10 V14.5 H8 V9.5 H4 Z")],
        Icon::TlLoop => vec![Fill("M1.5 4.2 H11.8 V2 L16.5 5.2 L11.8 8.4 V6.2 H3.5 V13 H13.5 V10 H15.5 V15 H1.5 Z")],
        Icon::TlStepBack => vec![Fill("M11 4 L4.5 9 L11 14 Z M12.5 4 H14.5 V14 H12.5 Z")],
        Icon::TlStepForward => vec![Fill("M3.5 4 H5.5 V14 H3.5 Z M7 4 L13.5 9 L7 14 Z")],
        Icon::TlResetZoom => vec![Line("M13.5 8.5 C13.5 5.8 11.3 3.8 8.8 3.8 C6.5 3.8 4.6 5.4 4.2 7.6 M2.5 5.5 L4.2 7.8 L6.5 6.2 M2.5 14 H15.5", 1.3)],
        Icon::TlMountains => vec![Fill("M1 15 L5.5 9 L10 15 Z M6.5 15 L12 6 L17.5 15 Z")],
        Icon::TlLayerView => vec![Fill("M9 2.5 L15.5 6 L9 9.5 L2.5 6 Z"), Line("M2.5 9.2 L9 12.7 L15.5 9.2 M2.5 12.4 L9 15.9 L15.5 12.4", 1.2)],
        Icon::TlEyeSlash => vec![Line("M1.5 9 C4 5 7 4 9 4 C11 4 14 5 16.5 9 C14 13 11 14 9 14 C7 14 4 13 1.5 9 Z M2.5 2.5 L15.5 15.5", 1.2), Dot(9.0, 9.0, 2.2)],
        Icon::TlLock => vec![Fill("M4.5 8 H13.5 V16 H4.5 Z M8.3 10.5 H9.7 V13.5 H8.3 Z"), Line("M6.4 8 V5.8 C6.4 2.6 11.6 2.6 11.6 5.8 V8", 1.5)],
        Icon::TlOutlineBar => vec![Line("M6 3 H12 V15 H6 Z", 2.0)],
        // A sheet with a round hole; a T-square with its marks; a curved path between two dots.
        Icon::LayerMask => vec![Fill("M4 2.5 H14 Q15.5 2.5 15.5 4 V14 Q15.5 15.5 14 15.5 H4 Q2.5 15.5 2.5 14 V4 Q2.5 2.5 4 2.5 Z M9 5.4 C11 5.4 12.6 7 12.6 9 C12.6 11 11 12.6 9 12.6 C7 12.6 5.4 11 5.4 9 C5.4 7 7 5.4 9 5.4 Z")],
        Icon::LayerGuide => vec![Line("M2.5 3.5 H15.5 M9 3.5 V15.5", 1.4), Line("M9 7.5 H11.5 M9 10.5 H11.5 M9 13.5 H11.5", 1.0)],
        Icon::LayerMotionGuide => vec![Line("M3.5 14.5 C4.5 7 12 11.5 14.5 3.5", 1.3), Dot(3.5, 14.5, 1.8), Dot(14.5, 3.5, 1.8)],
        // Large glyphs keep Animate's 1.5 px lines: 0.38 units at 72 px, 0.8 at 30 px.
        Icon::HomeFullHd => vec![Line("M1.6 4 H16.4 Q17 4 17 4.6 V13.4 Q17 14 16.4 14 H1.6 Q1 14 1 13.4 V4.6 Q1 4 1.6 4 Z M7.6 6.6 L11.4 9 L7.6 11.4 Z", 0.38)],
        Icon::HomeGamepad => vec![
            Line(GAMEPAD, 0.38),
            Line("M5 6.9 V9.9 M3.5 8.4 H6.5", 0.38),
            Ring(13.0, 7.1, 0.5, 0.38),
            Ring(13.0, 9.7, 0.5, 0.38),
            Ring(11.7, 8.4, 0.5, 0.38),
            Ring(14.3, 8.4, 0.5, 0.38),
        ],
        Icon::HomeBanner => vec![Line(
            "M1.6 3.5 H16.4 Q17 3.5 17 4.1 V13.9 Q17 14.5 16.4 14.5 H1.6 Q1 14.5 1 13.9 V4.1 Q1 3.5 1.6 3.5 Z M1 5.6 H17 M13.2 7.4 H15.2 V12.9 H13.2 Z",
            0.38,
        )],
        Icon::HomeTablet => vec![Line(
            "M2.6 3 H15.4 Q16 3 16 3.6 V14.4 Q16 15 15.4 15 H2.6 Q2 15 2 14.4 V3.6 Q2 3 2.6 3 Z \
             M9 5.6 L13.2 7.5 L9 9.4 L4.8 7.5 Z M6.6 8.5 V10.7 C7.6 11.9 10.4 11.9 11.4 10.7 V8.5 M13.2 7.5 V10.2",
            0.38,
        )],
        Icon::HomeCanvas => vec![Line("M4.4 2.8 H11.2 L13.8 5.4 V15.2 H4.4 Z M11.2 2.8 V5.4 H13.8", 0.38)],
        Icon::HomeMore => vec![Ring(9.0, 9.0, 6.0, 0.38), Dot(6.6, 9.0, 0.55), Dot(9.0, 9.0, 0.55), Dot(11.4, 9.0, 0.55)],
        Icon::NdCharacter => vec![Ring(6.4, 6.4, 4.0, 0.8), Ring(11.0, 11.0, 5.6, 0.8)],
        Icon::NdSocial => vec![Line("M1.6 8.4 L16.4 2 L12.6 15.4 L8.6 11.2 Z M8.6 11.2 L16.4 2 M8.6 11.2 V15.2 L10.6 13.2", 0.8)],
        Icon::NdGame => vec![Line(GAMEPAD, 0.8), Line("M5 6.8 V10 M3.4 8.4 H6.6", 0.8), Dot(12.3, 9.4, 0.85), Dot(14.2, 7.6, 0.85)],
        Icon::NdEducation => vec![Line("M9 3.4 L17 7.2 L9 11 L1 7.2 Z M4.4 9.1 V12.9 C6.4 15 11.6 15 13.6 12.9 V9.1 M17 7.2 V11.8", 0.8)],
        Icon::NdAds => vec![Line(
            "M2 7 H5.4 L12.4 3.2 V14.8 L5.4 11 H2 Z M5.4 11 L6.8 15.2 H8.6 L7.6 11.6 M14.6 6.4 L16.4 5.4 M14.8 9 H16.8 M14.6 11.6 L16.4 12.6",
            0.8,
        )],
        Icon::NdWeb => vec![Ring(9.0, 9.0, 7.0, 0.8), Oval(9.0, 9.0, 3.0, 7.0, 0.8), Line("M2 9 H16 M3 5.4 H15 M3 12.6 H15", 0.8)],
        Icon::NdAdvanced => vec![Line("M4 1.8 H11 L14 4.8 V16.2 H4 Z M11 1.8 V4.8 H14", 0.8)],
        Icon::NdPreset => vec![
            Line(
                "M1.4 2.8 H16.6 Q17.6 2.8 17.6 3.8 V14.2 Q17.6 15.2 16.6 15.2 H1.4 Q0.4 15.2 0.4 14.2 V3.8 Q0.4 2.8 1.4 2.8 Z \
                 M7.7 5.6 L11.2 7.8 L7.7 10 Z M2.4 12.6 H15.6",
                0.38,
            ),
            Dot(5.6, 12.6, 0.6),
        ],
        Icon::NdCanvas => vec![Line(
            "M1.5 3 H16.5 V12.8 H1.5 Z M7.2 12.8 L6.6 15.2 M10.8 12.8 L11.4 15.2 M5 15.2 H13 M7.2 6.2 L5.6 7.9 L7.2 9.6 M10.8 6.2 L12.4 7.9 L10.8 9.6",
            0.38,
        )],
        Icon::NdActionScript => vec![Line(
            "M4.6 2.4 H11 L13.8 5.2 V15.6 H4.6 Z M11 2.4 V5.2 H13.8 M7.6 8.6 L6.2 10.2 L7.6 11.8 M10.8 8.6 L12.2 10.2 L10.8 11.8 M9.8 8 L8.6 12.4",
            0.38,
        )],
        Icon::Link => vec![Line(
            "M7.6 10.4 L10.4 7.6 M8.6 5.2 L10 3.8 C11.3 2.5 13.4 2.5 14.6 3.8 C15.9 5 15.9 7.1 14.6 8.4 L13.2 9.8 \
             M9.4 12.8 L8 14.2 C6.7 15.5 4.6 15.5 3.4 14.2 C2.1 13 2.1 10.9 3.4 9.6 L4.8 8.2",
            1.3,
        )],
        // An arch magnet; its pole tips in outline.
        Icon::Magnet => vec![
            Line("M4.5 12 V8 C4.5 5 6.5 3 9 3 C11.5 3 13.5 5 13.5 8 V12", 2.4),
            Line("M3.4 13.4 H5.6 V15.5 H3.4 Z M12.4 13.4 H14.6 V15.5 H12.4 Z", 1.0),
        ],
        Icon::SnapAlign => vec![Line("M5 2.5 V15.5 M15 7 H8 M10.4 4.6 L8 7 L10.4 9.4 M8.5 11.5 C8.5 14.5 12 15.3 14.5 13", 1.2)],
        // An L-shaped ruler with ticks along both arms.
        Icon::Rulers => vec![
            Line("M2.5 2.5 H7 V11 H15.5 V15.5 H2.5 Z", 1.1),
            Dot(4.7, 5.0, 0.7),
            Dot(4.7, 7.5, 0.7),
            Dot(4.7, 10.0, 0.7),
            Dot(7.5, 13.3, 0.7),
            Dot(10.0, 13.3, 0.7),
            Dot(12.5, 13.3, 0.7),
        ],
        // Caps: a thick line ending at a thin guide; Round and Square run past it.
        Icon::CapNone => vec![Fill("M2 6.5 H11 V11.5 H2 Z"), Line("M11.6 3 V15", 0.9)],
        Icon::CapRound => vec![
            Fill("M2 6.5 H10.4 V11.5 H2 Z M11.6 6.6 C13 6.8 13.9 7.8 13.9 9 C13.9 10.2 13 11.2 11.6 11.4 Z"),
            Line("M11 3 V15", 0.9),
        ],
        Icon::CapSquare => vec![Fill("M2 6.5 H10.4 V11.5 H2 Z M11.6 6.5 H13.9 V11.5 H11.6 Z"), Line("M11 3 V15", 0.9)],
        // Joins: a thick corner, sharp, rounded or cut off.
        Icon::JoinMiter => vec![Fill("M3 15.5 V3 H15.5 V8 H8 V15.5 Z")],
        Icon::JoinRound => vec![Fill("M3 15.5 V7.5 C3 5 5 3 7.5 3 H15.5 V8 H8 V15.5 Z")],
        Icon::JoinBevel => vec![Fill("M3 15.5 V6.6 L6.6 3 H15.5 V8 H8 V15.5 Z")],
        Icon::PlayOnce => vec![Fill("M2.5 4 L10 9 L2.5 14 Z"), Line("M12.3 6.3 L14.3 4.3 V14", 1.4)],
        Icon::SingleFrame => vec![Fill("M2.5 3 H4.5 V15 H2.5 Z M13.5 3 H15.5 V15 H13.5 Z"), Line("M6.8 5.5 H11.2 V12.5 H6.8 Z", 1.2)],
        Icon::ReversePlayOnce => vec![Fill("M15.5 4 L8 9 L15.5 14 Z"), Line("M3.7 6.3 L5.7 4.3 V14", 1.4)],
        Icon::ReverseLoop => vec![Line(
            "M14.5 8 V7 C14.5 5.3 13.2 4 11.5 4 H4.5 M6.7 1.9 L4.4 4 L6.7 6.1 M3.5 10 V11 C3.5 12.7 4.8 14 6.5 14 H13.5 M11.3 16.1 L13.6 14 L11.3 11.9",
            1.3,
        )],
        Icon::Unlink => vec![Line(
            "M8.6 5.2 L10 3.8 C11.3 2.5 13.4 2.5 14.6 3.8 C15.9 5 15.9 7.1 14.6 8.4 L13.2 9.8 \
             M9.4 12.8 L8 14.2 C6.7 15.5 4.6 15.5 3.4 14.2 C2.1 13 2.1 10.9 3.4 9.6 L4.8 8.2 M2 6 L4.5 6.5 M6 2 L6.5 4.5 M16 12 L13.5 11.5 M12 16 L11.5 13.5",
            1.3,
        )],
        Icon::LinesToFills => vec![Fill("M2.5 4 H3.7 V14 H2.5 Z M12.5 4 H15.5 V14 H12.5 Z"), Line("M5.5 9 H10.4 M8.4 7 L10.4 9 L8.4 11", 1.2)],
        Icon::Group => vec![Line("M2 2 H16 V16 H2 Z", 1.1), Fill("M4.5 4.5 H8.5 V8.5 H4.5 Z M9.5 9.5 H13.5 V13.5 H9.5 Z M4.5 10 H8 V13.5 H4.5 Z")],
        Icon::BreakApart => vec![
            Line("M2 2.5 H8.5 V9 H2 Z M9.5 9 H16 V15.5 H9.5 Z", 1.1),
            Fill("M3.8 4.3 H6.7 V7.2 H3.8 Z M11.3 10.8 H14.2 V13.7 H11.3 Z"),
        ],
        Icon::ConvertToSymbol => vec![
            Line("M1.5 9.5 H6.2 M4.2 7.5 L6.2 9.5 L4.2 11.5", 1.2),
            Dot(12.5, 5.4, 2.3),
            Dot(9.9, 9.8, 2.3),
            Dot(15.1, 9.8, 2.3),
            Fill("M11.9 9.5 H13.1 L13.9 15.5 H11.1 Z"),
        ],
        Icon::CreateBrush => vec![Fill("M3.5 9 H11.5 L10.4 16 H4.6 Z"), Line("M5.5 9 L4.6 3.5 M7.5 9 V2.5 M9.5 9 L10.4 3.5 M14.5 2 V7.5 M11.8 4.8 H17", 1.2)],
        // A square with a circle over its top right corner.
        Icon::Shape => vec![Line("M2.5 6.5 H11.5 V15.5 H2.5 Z", 1.2), Dot(11.5, 6.5, 4.0)],
        Icon::Frame => vec![Line("M3.5 2.5 H14.5 V15.5 H3.5 Z", 1.1), Fill("M5.5 2.5 H6.8 V15.5 H5.5 Z M11.2 2.5 H12.5 V15.5 H11.2 Z"), Dot(8.0, 12.8, 0.8), Dot(10.0, 12.8, 0.8)],
        Icon::Actions => vec![Ring(9.0, 9.0, 6.6, 1.2), Line("M6.4 11.6 L11.4 6.6 M7.6 6.6 H11.4 V10.4", 1.2)],
        Icon::MotionTween => vec![Fill("M4.5 1.5 L7.5 4.5 L4.5 7.5 L1.5 4.5 Z M13.5 10.5 L16.5 13.5 L13.5 16.5 L10.5 13.5 Z"), Line("M6.5 11.5 L11 7 M8 7 H11 V10", 1.2)],
        Icon::ShapeTween => vec![Ring(5.0, 13.0, 3.0, 1.2), Fill("M10.5 2.5 H15.5 V7.5 H10.5 Z"), Line("M6.8 11.2 L10 8 M7.8 8 H10 V10.2", 1.1)],
        Icon::Sliders => vec![Line("M2.5 5 H15.5 M2.5 9 H15.5 M2.5 13 H15.5", 1.1), Dot(6.0, 5.0, 1.8), Dot(12.0, 9.0, 1.8), Dot(8.0, 13.0, 1.8)],
        Icon::AlignLeft => vec![Line("M3 4 H15 M3 7.3 H11 M3 10.6 H15 M3 13.9 H11", 1.2)],
        Icon::AlignCenter => vec![Line("M3 4 H15 M5 7.3 H13 M3 10.6 H15 M5 13.9 H13", 1.2)],
        Icon::AlignRight => vec![Line("M3 4 H15 M7 7.3 H15 M3 10.6 H15 M7 13.9 H15", 1.2)],
        Icon::AlignJustify => vec![Line("M3 4 H15 M3 7.3 H15 M3 10.6 H15 M3 13.9 H11", 1.2)],
        // A pushpin lying down, its point to the left.
        Icon::Pin => vec![Line("M1.5 9 H6", 1.2), Fill("M6 5 H7.5 V7.2 H12 L13.5 5.8 H15.5 V12.2 H13.5 L12 10.8 H7.5 V13 H6 Z")],
        Icon::NewLibrary => vec![Line("M2.5 6 H11.5 V15.5 H2.5 Z M6.5 6 V2.5 H15.5 V11.5 H11.5", 1.2)],
        Icon::FolderOpen => vec![Line("M2 13.5 V4 H7 L8.5 5.5 H14.5 V7.5", 1.1), Fill("M4.6 8.5 H17 L14 14.5 H1.6 Z")],
        Icon::Tag => vec![Line("M2.5 2.5 H9 L15.5 9 L9 15.5 L2.5 9 Z", 1.2), Dot(6.0, 6.0, 1.2)],
        Icon::Sound => vec![Fill("M2 6.6 H5 L9.4 3 V15 L5 11.4 H2 Z"), Line("M11.6 6.6 C12.7 7.8 12.7 10.2 11.6 11.4 M13.8 4.4 C16 6.8 16 11.2 13.8 13.6", 1.2)],
        Icon::EditEnvelope => vec![Fill("M1.5 5.6 H3.8 L7.4 2.6 V12.4 L3.8 9.4 H1.5 Z"), Fill("M8.2 16.4 L8.8 13.6 L14.6 7.8 L16.8 10 L11 15.8 Z")],
        Icon::Bitmap => vec![Line("M2.5 3.5 H15.5 V14.5 H2.5 Z", 1.0), Fill("M4 13 L7.8 7.6 L10.4 10.8 L12 9.2 L14 13 Z"), Dot(12.2, 6.4, 1.3)],
        Icon::Plus => vec![Line("M9 3.5 V14.5 M3.5 9 H14.5", 1.3)],
        // A ring with eight rounded teeth.
        Icon::Gear => vec![
            Ring(9.0, 9.0, 4.4, 2.2),
            Line("M9 3.2 V1.8 M9 14.8 V16.2 M3.2 9 H1.8 M14.8 9 H16.2 M4.9 4.9 L3.9 3.9 M13.1 13.1 L14.1 14.1 M4.9 13.1 L3.9 14.1 M13.1 4.9 L14.1 3.9", 2.4),
        ],
        Icon::Code => vec![Line("M6 5 L2.5 9 L6 13 M12 5 L15.5 9 L12 13 M10.2 3.5 L7.8 14.5", 1.3)],
        // Two pages, one over the other.
        Icon::Duplicate => vec![Line("M6.5 2.5 H15.5 V11.5 H6.5 Z", 1.1), Line("M3.5 6.5 V15.5 H12.5", 1.1)],
    }
}

/// Draws `icon` centered in `rect`, as large as fits, tinted `tint`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, tint: Color32) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let px = (rect.width().min(rect.height()) * ppp).round().max(1.0) as u32;
    let id = egui::Id::new("minnamate-icons");
    let cached = ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(id).0.get(&(icon, px)).cloned());
    let tex = cached.unwrap_or_else(|| {
        let tex = ctx.load_texture(format!("icon-{icon:?}-{px}"), rasterize(icon, px), TextureOptions::LINEAR);
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(id).0.insert((icon, px), tex.clone()));
        tex
    });
    // Snap to whole device pixels so the glyph stays crisp.
    let side = px as f32 / ppp;
    let min = ((rect.center() - vec2(side, side) / 2.0) * ppp).round() / ppp;
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(tex.id(), Rect::from_min_size(min, vec2(side, side)), uv, tint);
}

#[derive(Clone, Default)]
struct Cache(HashMap<(Icon, u32), TextureHandle>);

/// White glyph on transparent, `px` square.
fn rasterize(icon: Icon, px: u32) -> ColorImage {
    let mut pm = Pixmap::new(px, px).expect("icon size is non-zero");
    let mut paint = sk::Paint::default();
    paint.set_color(sk::Color::WHITE);
    draw(&mut pm, parts(icon), Transform::from_scale(px as f32 / 18.0, px as f32 / 18.0), &paint);
    ColorImage::from_rgba_premultiplied([px as usize, px as usize], pm.data())
}

fn draw(pm: &mut Pixmap, parts: Vec<Part>, ts: Transform, paint: &sk::Paint) {
    // No line thinner than a device pixel, or a small glyph fades away.
    let (sx, sy) = ts.get_scale();
    let thinnest = 1.0 / sx.min(sy);
    let stroke = |width: f32| sk::Stroke { width: width.max(thinnest), line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
    for part in parts {
        match part {
            Fill(d) => {
                if let Some(path) = svg_path(d) {
                    pm.fill_path(&path, paint, FillRule::EvenOdd, ts, None);
                }
            }
            Line(d, w) => {
                if let Some(path) = svg_path(d) {
                    pm.stroke_path(&path, paint, &stroke(w), ts, None);
                }
            }
            Dot(x, y, r) => {
                if let Some(path) = PathBuilder::from_circle(x, y, r) {
                    pm.fill_path(&path, paint, FillRule::Winding, ts, None);
                }
            }
            Ring(x, y, r, w) => {
                if let Some(path) = PathBuilder::from_circle(x, y, r) {
                    pm.stroke_path(&path, paint, &stroke(w), ts, None);
                }
            }
            Oval(x, y, rx, ry, w) => {
                if let Some(path) = sk::Rect::from_xywh(x - rx, y - ry, 2.0 * rx, 2.0 * ry).and_then(PathBuilder::from_oval) {
                    if w > 0.0 {
                        pm.stroke_path(&path, paint, &stroke(w), ts, None);
                    } else {
                        pm.fill_path(&path, paint, FillRule::Winding, ts, None);
                    }
                }
            }
            Ramp(x, y, w, h) => {
                let light = paint.shader.clone();
                let sk::Shader::SolidColor(c) = light else { continue };
                let dark = sk::Color::from_rgba(c.red() * 0.3, c.green() * 0.3, c.blue() * 0.3, c.alpha()).unwrap_or(c);
                let stops = vec![sk::GradientStop::new(0.0, c), sk::GradientStop::new(1.0, dark)];
                let shader = sk::LinearGradient::new(sk::Point::from_xy(x, y), sk::Point::from_xy(x + w, y), stops, sk::SpreadMode::Pad, Transform::identity());
                if let (Some(shader), Some(rect)) = (shader, sk::Rect::from_xywh(x, y, w, h)) {
                    pm.fill_rect(rect, &sk::Paint { shader, ..paint.clone() }, ts, None);
                }
            }
            Grey(level, parts) => {
                let mut grey = paint.clone();
                grey.set_color(sk::Color::from_rgba(level, level, level, 1.0).unwrap_or(sk::Color::WHITE));
                draw(pm, parts, ts, &grey);
            }
            Cut(parts) => {
                let clear = sk::Paint { blend_mode: sk::BlendMode::Clear, ..paint.clone() };
                draw(pm, parts, ts, &clear);
            }
            T(t, parts) => draw(pm, parts, ts.pre_concat(t), paint),
        }
    }
}

/// Parses SVG path data with M, L, H, V, C, Q and Z (absolute or relative).
pub(crate) fn svg_path(d: &str) -> Option<sk::Path> {
    enum Tok {
        Cmd(char),
        Num(f32),
    }
    let mut toks = Vec::new();
    let mut num = String::new();
    let flush = |num: &mut String, toks: &mut Vec<Tok>| {
        if !num.is_empty() {
            toks.push(Tok::Num(num.parse().unwrap_or(0.0)));
            num.clear();
        }
    };
    for c in d.chars() {
        match c {
            c if "MmLlHhVvCcQqZz".contains(c) => {
                flush(&mut num, &mut toks);
                toks.push(Tok::Cmd(c));
            }
            '-' => {
                flush(&mut num, &mut toks);
                num.push(c);
            }
            '0'..='9' | '.' => {
                if c == '.' && num.contains('.') {
                    flush(&mut num, &mut toks);
                }
                num.push(c);
            }
            _ => flush(&mut num, &mut toks),
        }
    }
    flush(&mut num, &mut toks);

    let mut pb = PathBuilder::new();
    let (mut cmd, mut cur, mut start, mut i) = ('M', (0.0f32, 0.0f32), (0.0f32, 0.0f32), 0);
    while i < toks.len() {
        if let Tok::Cmd(c) = toks[i] {
            cmd = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                // tiny-skia strokes the segment `close` adds a fifth of a pixel off,
                // which blurs the last edge of outlines; drawn as a line it's crisp.
                if cur != start {
                    pb.line_to(start.0, start.1);
                }
                pb.close();
                cur = start;
            }
            continue;
        }
        let n = match cmd.to_ascii_uppercase() {
            'M' | 'L' => 2,
            'H' | 'V' => 1,
            'C' => 6,
            'Q' => 4,
            _ => return None,
        };
        let args: Vec<f32> = toks.get(i..i + n)?.iter().map(|t| if let Tok::Num(v) = t { Some(*v) } else { None }).collect::<Option<_>>()?;
        i += n;
        let rel = cmd.is_ascii_lowercase();
        let pt = |k: usize| if rel { (cur.0 + args[k], cur.1 + args[k + 1]) } else { (args[k], args[k + 1]) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                cur = pt(0);
                start = cur;
                pb.move_to(cur.0, cur.1);
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => {
                cur = pt(0);
                pb.line_to(cur.0, cur.1);
            }
            'H' => {
                cur.0 = if rel { cur.0 + args[0] } else { args[0] };
                pb.line_to(cur.0, cur.1);
            }
            'V' => {
                cur.1 = if rel { cur.1 + args[0] } else { args[0] };
                pb.line_to(cur.0, cur.1);
            }
            'C' => {
                let (a, b, p) = (pt(0), pt(2), pt(4));
                pb.cubic_to(a.0, a.1, b.0, b.1, p.0, p.1);
                cur = p;
            }
            _ => {
                let (a, p) = (pt(0), pt(2));
                pb.quad_to(a.0, a.1, p.0, p.1);
                cur = p;
            }
        }
    }
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[Icon] = &[
        Icon::Selection, Icon::Subselection, Icon::FreeTransform, Icon::GradientTransform, Icon::AssetWarp, Icon::Lasso,
        Icon::Polygon, Icon::MagicWand, Icon::Pen, Icon::AddAnchor, Icon::DeleteAnchor, Icon::ConvertAnchor, Icon::Text,
        Icon::Line, Icon::Rectangle, Icon::RectanglePrimitive, Icon::Oval, Icon::OvalPrimitive, Icon::PolyStar, Icon::Pencil,
        Icon::Brush, Icon::FluidBrush, Icon::PaintBrush, Icon::PaintBucket, Icon::InkBottle, Icon::Eyedropper, Icon::Eraser,
        Icon::Width, Icon::Bone, Icon::Bind, Icon::Camera, Icon::Hand, Icon::Rotation, Icon::TimeScrub, Icon::Zoom,
        Icon::Rotation3D, Icon::Translation3D, Icon::Spacer, Icon::ObjectDrawing, Icon::Smooth, Icon::Straighten,
        Icon::LockFill, Icon::PaintNormal, Icon::EraseNormal, Icon::EraseFills, Icon::EraseLines, Icon::EraseSelectedFills,
        Icon::EraseInside, Icon::GapNone, Icon::GapSmall, Icon::GapMedium, Icon::GapLarge, Icon::Play, Icon::Pause, Icon::ToStart, Icon::StepBack,
        Icon::StepForward, Icon::ToEnd, Icon::Loop, Icon::OnionSkin, Icon::OnionOutline, Icon::Eye, Icon::Lock,
        Icon::Outline, Icon::NewLayer, Icon::NewFolder, Icon::Delete, Icon::Layer, Icon::Folder, Icon::Scene,
        Icon::Back, Icon::CenterStage, Icon::ClipContent, Icon::EditSymbols, Icon::InsertKeyframe,
        Icon::InsertBlankKeyframe, Icon::InsertFrame, Icon::ClassicTween, Icon::TestMovie, Icon::Publish,
        Icon::More, Icon::Swap, Icon::MovieClip, Icon::Graphic, Icon::Button, Icon::Home, Icon::Workspace, Icon::Reset,
        Icon::StageRotation, Icon::Club, Icon::ChevronLeft, Icon::ChevronRight, Icon::ChevronDown, Icon::ChevronsRight,
        Icon::ChevronsLeft, Icon::WinMinimize, Icon::WinMaximize, Icon::WinRestore, Icon::WinClose, Icon::ColorPanel,
        Icon::Swatches, Icon::Align, Icon::Info, Icon::TransformPanel, Icon::MotionPresets, Icon::TlNewLayer,
        Icon::TlParenting, Icon::TlLayerDepth, Icon::TlKeyframe, Icon::TlBlankKeyframe, Icon::TlFrame, Icon::TlOnion,
        Icon::TlEditMultiple, Icon::TlClassicTween, Icon::TlCenterFrame, Icon::TlLoop, Icon::TlStepBack, Icon::TlStepForward,
        Icon::TlResetZoom, Icon::TlMountains, Icon::TlLayerView, Icon::TlEyeSlash, Icon::TlLock, Icon::TlOutlineBar,
        Icon::LayerMask, Icon::LayerGuide, Icon::LayerMotionGuide,
        Icon::HomeFullHd, Icon::HomeGamepad, Icon::HomeBanner, Icon::HomeTablet, Icon::HomeCanvas, Icon::HomeMore,
        Icon::NdCharacter, Icon::NdSocial, Icon::NdGame, Icon::NdEducation, Icon::NdAds, Icon::NdWeb, Icon::NdAdvanced,
        Icon::NdPreset, Icon::NdCanvas, Icon::NdActionScript, Icon::Link, Icon::Magnet, Icon::SnapAlign, Icon::Rulers,
        Icon::CapNone, Icon::CapRound, Icon::CapSquare, Icon::JoinMiter, Icon::JoinRound, Icon::JoinBevel,
        Icon::PlayOnce, Icon::SingleFrame, Icon::ReversePlayOnce, Icon::ReverseLoop, Icon::Unlink, Icon::LinesToFills, Icon::Group,
        Icon::BreakApart, Icon::ConvertToSymbol, Icon::CreateBrush, Icon::Shape, Icon::Frame,
        Icon::Actions, Icon::MotionTween, Icon::ShapeTween, Icon::Sliders, Icon::AlignLeft, Icon::AlignCenter, Icon::AlignRight, Icon::AlignJustify, Icon::Pin, Icon::NewLibrary, Icon::FolderOpen, Icon::Tag, Icon::Sound, Icon::EditEnvelope, Icon::Bitmap,
        Icon::Plus, Icon::Gear, Icon::Code, Icon::Duplicate,
    ];

    #[test]
    fn every_icon_draws_something() {
        fn check(icon: Icon, parts: Vec<Part>) {
            for part in parts {
                match part {
                    Fill(d) | Line(d, _) => assert!(svg_path(d).is_some(), "{icon:?}: bad path {d:?}"),
                    Grey(_, inner) | Cut(inner) | T(_, inner) => check(icon, inner),
                    _ => {}
                }
            }
        }
        for &icon in ALL {
            check(icon, parts(icon));
            let img = rasterize(icon, 18);
            assert!(img.pixels.iter().any(|p| p.a() > 0), "{icon:?} is blank");
        }
    }

    /// Writes target/screenshots/icons.png: every icon at 100% on the panel color, 16 to a row.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn icons_screenshot() {
        let rows = ALL.len().div_ceil(16) as f32;
        let mut h = egui_kittest::Harness::builder().with_size(vec2(16.0 * 30.0, rows * 30.0)).wgpu().build_ui(|ui| {
            let at = ui.ctx().content_rect().min;
            ui.painter().rect_filled(ui.ctx().content_rect(), 0.0, crate::theme::PANEL_BG);
            for (k, &icon) in ALL.iter().enumerate() {
                let c = at + vec2(15.0 + 30.0 * (k % 16) as f32, 15.0 + 30.0 * (k / 16) as f32);
                paint(ui.painter(), Rect::from_center_size(c, vec2(18.0, 18.0)), icon, crate::theme::TEXT);
            }
        });
        h.run();
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
        std::fs::create_dir_all(&dir).unwrap();
        h.render().expect("render").save(dir.join("icons.png")).unwrap();
    }

    #[test]
    fn small_glyphs_keep_whole_pixel_strokes() {
        // As small as the chrome draws them (the zoom field's caret, dock and
        // floating-panel chevrons, a floating panel's close box, the Share
        // popover's chevrons), and smaller. A 1 px line's most-covered pixel is
        // over 60% inked even on a diagonal; thinner lines faded to half or less.
        let small = [
            (Icon::ChevronDown, 6),
            (Icon::ChevronDown, 10),
            (Icon::ChevronsLeft, 8),
            (Icon::ChevronsRight, 8),
            (Icon::ChevronLeft, 10),
            (Icon::ChevronRight, 10),
            (Icon::WinClose, 10),
            (Icon::ChevronLeft, 12),
            (Icon::ChevronRight, 12),
        ];
        let peaks = small.map(|(icon, px)| (icon, px, rasterize(icon, px).pixels.iter().map(|p| p.a()).max().unwrap()));
        assert!(peaks.iter().all(|p| p.2 >= 150), "{peaks:?}");
        // The window's Minimize, at its 18 px: one crisp row.
        let img = rasterize(Icon::WinMinimize, 18);
        let rows: Vec<usize> = (0..18).filter(|&y| (0..18).any(|x| img.pixels[y * 18 + x].a() > 0)).collect();
        assert_eq!(rows.len(), 1, "{rows:?}");
    }

    #[test]
    fn closed_outlines_stay_crisp() {
        // The Rectangle's left edge, the one Z closes, fills its pixel column and spills into none.
        let img = rasterize(Icon::Rectangle, 18);
        let alpha = |x: usize| img.pixels[9 * 18 + x].a();
        assert_eq!((alpha(0), alpha(1)), (0, 255));
    }

    #[test]
    fn relative_and_implicit_commands() {
        let p = svg_path("M2 2h3v3h-3z m10 0 l1 1 1 1").unwrap();
        let b = p.bounds();
        assert_eq!((b.left(), b.top(), b.right(), b.bottom()), (2.0, 2.0, 14.0, 5.0));
    }
}
