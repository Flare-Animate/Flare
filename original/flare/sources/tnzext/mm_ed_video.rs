//! File > Import > Import Video (docs/animate-ui.md §7.4): ffmpeg turns the video into
//! an image for each frame of the document, each a bitmap on a keyframe of its own.

use std::path::Path;

use minnamate_core::doc::{BitmapInstance, BitmapItem, Element, Frame, Matrix, Media, Tween};
use minnamate_core::video;

use crate::app::Editor;

/// The video files Import Video offers.
const VIDEO: [&str; 10] = ["mp4", "m4v", "mov", "avi", "webm", "mkv", "flv", "mpg", "mpeg", "wmv"];

pub fn import_dialog(ed: &mut Editor) {
    if !video::ffmpeg() {
        ed.error = Some("Import Video needs ffmpeg, to read the video. Install it (ffmpeg.org), make sure it's on the PATH, and try again.".into());
        return;
    }
    if let Some(path) = rfd::FileDialog::new().add_filter("Video", &VIDEO).pick_file() {
        import(ed, &path);
    }
}

/// The video at `path`, as images at the document's frame rate (`place`).
pub fn import(ed: &mut Editor, path: &Path) {
    let fps = ed.doc.frame_rate;
    match video::frames(path, fps) {
        Ok(frames) if !frames.is_empty() => {
            let name = path.file_name().map_or_else(|| "Video".into(), |n| n.to_string_lossy().into_owned());
            ed.log(format!("Imported {} frames of {} at {fps} fps", frames.len(), path.display()));
            place(ed, &name, frames);
        }
        Ok(_) => ed.error = Some(format!("{} has no frames to import.", path.display())),
        Err(e) => ed.error = Some(format!("Couldn't import {}.\n\n{e}", path.display())),
    }
}

/// Video `name`'s frames (image files): each a library bitmap "<name>/0001.png"..., in a
/// folder named after it, on a keyframe of its own on a new layer of that name, from the
/// current frame on, at the stage's top left. One undo step.
// ponytail: Animate embeds a video as one video item its timeline plays; as bitmaps the frames
// play wherever bitmaps do (the SWF, HTML5 Canvas), at the cost of the file's size.
fn place(ed: &mut Editor, name: &str, frames: Vec<Vec<u8>>) {
    let folder = (1..).map(|n| if n == 1 { name.to_string() } else { format!("{name} {n}") }).find(|f| !minnamate_core::library::in_use(&ed.doc, f)).unwrap_or_default();
    ed.new_layer();
    let (l, start) = (ed.layer, ed.frame);
    let (tx, ty) = ed.to_local((0.0, 0.0));
    let mut keys = Vec::new();
    if start > 0 {
        keys.push(Frame { index: 0, duration: start, tween: Tween::None, elements: Vec::new(), extra: Default::default() });
    }
    for (i, png) in frames.into_iter().enumerate() {
        let item = format!("{folder}/{:04}.png", i + 1);
        ed.doc.media.push(Media::Bitmap(BitmapItem::new(&item, png)));
        let bitmap = BitmapInstance { name: item, matrix: Matrix { tx, ty, ..Matrix::IDENTITY }, extra: Default::default() };
        keys.push(Frame { index: start + i as u32, duration: 1, tween: Tween::None, elements: vec![Element::Bitmap(bitmap)], extra: Default::default() });
    }
    let layer = &mut ed.timeline_mut().layers[l];
    layer.name = folder.clone();
    layer.frames = keys;
    ed.library_selection = Some(folder);
}

#[cfg(test)]
mod tests {
    use minnamate_core::tiny_skia::Pixmap;

    use super::*;
    use crate::app::Action;
    use crate::app::testkit::*;

    #[test]
    fn a_videos_frames_go_on_keyframes_of_a_layer_of_its_own() {
        let mut h = blank();
        let png = Pixmap::new(4, 3).unwrap().encode_png().unwrap();
        let ed = h.state_mut();
        ed.frame = 2;
        place(ed, "clip.mp4", vec![png.clone(), png.clone(), png]);
        let layer = &ed.doc.scenes[0].layers[ed.layer];
        assert_eq!(layer.name, "clip.mp4");
        assert_eq!(layer.frames.iter().map(|k| (k.index, k.duration, k.elements.len())).collect::<Vec<_>>(), [(0, 2, 0), (2, 1, 1), (3, 1, 1), (4, 1, 1)]);
        assert!(matches!(&layer.frames[3].elements[0], Element::Bitmap(b) if b.name == "clip.mp4/0003.png"));
        assert_eq!(ed.doc.bitmaps().count(), 3);
        assert_eq!(ed.doc.scenes[0].layers.len(), 2, "above the layer there was");
        // Again: a folder of its own.
        ed.frame = 0;
        place(ed, "clip.mp4", vec![Pixmap::new(1, 1).unwrap().encode_png().unwrap()]);
        assert!(ed.doc.bitmap("clip.mp4 2/0001.png").is_some());
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        h.state_mut().run(Action::Undo, &ctx);
        assert!(h.state().doc.media.is_empty() && h.state().doc.scenes[0].layers.len() == 1, "one undo step each");
    }
}
