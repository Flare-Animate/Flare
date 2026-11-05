//! The vocabulary of document edits.

use flare_core::{BitmapId, FrameRate, LayerId, SceneId, SoundId, SymbolId};
use flare_document::Timeline;
use flare_document::{
    Bitmap, FrameSpan, Layer, Playback, Scene, Sound, SoundCue, SpanInterp, Stage, Symbol,
    SymbolKind,
};

/// A property change on a layer.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerChange {
    Name(String),
    Visible(bool),
    Locked(bool),
    Mask(bool),
    /// Makes the layer a motion guide (or a plain layer again).
    Guide(bool),
    /// Sets the layer's outline color, or clears it.
    Outline(Option<flare_core::Rgba8>),
    /// Puts the layer in a folder, or takes it out with `None`.
    Parent(Option<LayerId>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddSymbol(Symbol),
    AddScene(Scene),
    /// Renames a scene. The name is trimmed and must be 1 to [`crate::MAX_SCENE_NAME_CHARS`] characters.
    RenameScene {
        scene: SceneId,
        name: String,
    },
    SetSymbolKind {
        symbol: SymbolId,
        kind: SymbolKind,
    },
    RenameSymbol {
        symbol: SymbolId,
        name: String,
    },
    /// Removes a symbol from the library. The edit is refused while an instance still uses it.
    RemoveSymbol(SymbolId),
    /// Replaces a symbol's whole timeline.
    ReplaceSymbolTimeline {
        symbol: SymbolId,
        timeline: Timeline,
    },
    /// Leaving in-place symbol editing: replaces `symbol`'s timeline and adds the symbols
    /// created while editing, as one step. They must go together because artwork may have
    /// moved from the edited timeline into a new symbol, which is only valid as a whole.
    MergeSymbolEdit {
        symbol: SymbolId,
        timeline: Timeline,
        new_symbols: Vec<Symbol>,
    },
    SetStage(Stage),
    SetFrameRate(FrameRate),
    /// Adds a bitmap to the library. Refused when an identical image (same content hash) is
    /// already there: place that one instead, so repeated frames are stored once.
    AddBitmap(Bitmap),
    /// Removes a bitmap from the library. Fails while a frame still shows it.
    RemoveBitmap(BitmapId),
    /// Adds a sound to the library.
    AddSound(Sound),
    /// Removes a sound from the library. Fails if a cue still plays it.
    RemoveSound(SoundId),
    /// Replaces the sound cues of a layer.
    SetCues {
        scene: SceneId,
        layer: LayerId,
        cues: Vec<SoundCue>,
    },
    UpdateLayer {
        scene: SceneId,
        layer: LayerId,
        change: LayerChange,
    },
    RemoveLayer {
        scene: SceneId,
        layer: LayerId,
    },
    /// Moves a layer to `index` in the top-to-bottom order (clamped to the end).
    MoveLayer {
        scene: SceneId,
        layer: LayerId,
        index: usize,
    },
    AddLayer {
        scene: SceneId,
        layer: Layer,
    },
    InsertSpan {
        scene: SceneId,
        layer: LayerId,
        span: FrameSpan,
    },
    RemoveSpan {
        scene: SceneId,
        layer: LayerId,
        start: u32,
    },
    /// Sets how the instance held by the span starting at `start` plays its symbol (Loop, Play
    /// Once or Single Frame and its first frame). The span's timing and tween are kept; a span
    /// that holds no instance is refused.
    SetInstancePlayback {
        scene: SceneId,
        layer: LayerId,
        start: u32,
        playback: Playback,
    },
    /// Sets the interpolation (family, easing, handle type) of the tween that starts at the
    /// keyframe at `start`; `None` goes back to its plain ease. Refused when no span starts there.
    SetKeyframeInterpolation {
        scene: SceneId,
        layer: LayerId,
        start: u32,
        interp: Option<SpanInterp>,
    },
    /// Adds an entity to the game description. The ID is the caller's (see
    /// `GameSpec::next_id`), so a batch names exactly the entity it creates.
    AddGameEntity(flare_game::EntitySpec),
    /// Removes an entity from the game. Refused while a camera follows it.
    RemoveGameEntity(flare_game::EntityId),
    /// Sets one component of an entity, replacing one of its kind.
    SetGameComponent {
        entity: flare_game::EntityId,
        component: flare_game::Component,
    },
    /// Removes an entity's component of `kind`. Refused while another component needs it.
    RemoveGameComponent {
        entity: flare_game::EntityId,
        kind: flare_game::ComponentKind,
    },
    /// Sets the world's gravity, in pixels per second squared.
    SetGameGravity {
        x: f64,
        y: f64,
    },
}
