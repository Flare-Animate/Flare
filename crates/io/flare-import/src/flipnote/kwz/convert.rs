//! A KWZ as a Flare project: three layers and the paper colour under them, one frame for each
//! frame of the file, at the file's speed. Layers are stacked 1 over 2 over 3 over paper: the
//! per-frame draw order and the layer depths (the stereo offsets) are not imported yet.

use flare_core::{BitmapId, FrameRate};

use super::{Kwz, KwzState, LAYER_HEIGHT, LAYER_WIDTH, Sound};
use crate::flipnote::convert::{
    LayerFrames, Library, assemble, foreign, kwz_paper, kwz_pen, kwz_picture, note,
};
use crate::xfl::{Diagnostic, ImportError, Imported, Severity};

pub(crate) fn convert(kwz: &Kwz<'_>, file: &str) -> Result<Imported, ImportError> {
    let (num, den) = kwz.frame_rate();
    let rate = FrameRate::new(num, den).map_err(|e| foreign(file, e))?;
    let pixels = LAYER_WIDTH * LAYER_HEIGHT;
    let mut library = Library::new(file, LAYER_WIDTH, LAYER_HEIGHT);
    let mut per_layer: [Vec<BitmapId>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    let mut state = KwzState::new();
    for index in 0..kwz.frame_count() {
        kwz.decode_frame(&mut state, index)
            .map_err(|e| foreign(file, e))?;
        let frame = kwz
            .frame(index)
            .ok_or_else(|| foreign(file, "a frame is missing from its table"))?;
        let palette = frame.palette;
        let paper = kwz_pen(palette[0]);
        for (k, layer) in state.layers().iter().enumerate() {
            let pens = [
                kwz_pen(palette.get(1 + 2 * k).copied().unwrap_or(6)),
                kwz_pen(palette.get(2 + 2 * k).copied().unwrap_or(6)),
            ];
            let picture = kwz_picture(layer, pens);
            let id = library.intern(picture)?;
            if let Some(slot) = per_layer.get_mut(k) {
                slot.push(id);
            }
        }
        let under = library.intern(kwz_paper(paper, pixels))?;
        if let Some(slot) = per_layer.get_mut(3) {
            slot.push(under);
        }
    }
    let visible = kwz.meta().layer_visible;
    let [a, b, c, paper] = per_layer;
    let layers = vec![
        LayerFrames {
            name: "Layer 1",
            visible: visible[0],
            bitmaps: a,
        },
        LayerFrames {
            name: "Layer 2",
            visible: visible[1],
            bitmaps: b,
        },
        LayerFrames {
            name: "Layer 3",
            visible: visible[2],
            bitmaps: c,
        },
        LayerFrames {
            name: "Paper",
            visible: true,
            bitmaps: paper,
        },
    ];
    let diagnostics = diagnostics(kwz, file);
    assemble(
        file,
        rate,
        (LAYER_WIDTH, LAYER_HEIGHT),
        library,
        layers,
        diagnostics,
    )
}

fn diagnostics(kwz: &Kwz<'_>, file: &str) -> Vec<Diagnostic> {
    let (num, den) = kwz.frame_rate();
    let mut out = vec![note(
        Severity::Info,
        file,
        format!(
            "{} frames at {} a second, as three layers over the paper colour",
            kwz.frame_count(),
            if den == 1 {
                num.to_string()
            } else {
                format!("{num}/{den}")
            }
        ),
    )];
    let meta = kwz.meta();
    out.push(note(
        Severity::Info,
        file,
        format!(
            "made by {} (FSID {}), first made by {} (FSID {}), modified at Unix time {}",
            meta.current.name,
            meta.current.fsid,
            meta.root.name,
            meta.root.fsid,
            meta.modified_unix
        ),
    ));
    if meta.locked {
        out.push(note(
            Severity::Info,
            file,
            "locked by its author in Flipnote Studio; Flare opens it for editing".into(),
        ));
    }
    match kwz.sound() {
        Some(sound) => out.push(sound_note(sound, file)),
        None => out.push(note(
            Severity::Info,
            file,
            "a comment: the file has no sound section".into(),
        )),
    }
    out.push(note(
        Severity::Info,
        file,
        "the layer depths and the per-frame draw order are not imported; the layers stack 1 over 2 over 3 over the paper; the signature is not checked".into(),
    ));
    out
}

fn sound_note(sound: &Sound, file: &str) -> Diagnostic {
    let tracks = sound.tracks.iter().filter(|&&len| len > 0).count();
    note(
        Severity::Unsupported,
        file,
        format!(
            "the sound is not imported: {tracks} audio tracks (BGM speed {}) have no Flare sound layer yet",
            sound.bgm_speed
        ),
    )
}
