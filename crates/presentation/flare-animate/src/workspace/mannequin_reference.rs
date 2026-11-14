//! The picture the mannequin is posed from, and moving round the figure in the panel's view.
//! The picture is read off the UI thread (`flare_import`'s `read_still`, kept no larger than the
//! view needs), and the person in it is found there too (`flare_pose::find_pose`): their joints
//! become marks on the picture and the figure takes their pose at once
//! (`flare_mannequin::Mannequin::copy_pose`). The picture stands behind the figure, seen square
//! from the front; a mark the detector put wrong is dragged onto the person and Copy Pose poses
//! the figure again. With no one found, the marks start on the figure's joints. The picture is
//! only looked at: it is not in the document, the preferences or any export.

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use flare_mannequin::{Board, Depth, Facing, Joint, Marks};

use super::*;

/// The longest side a reference picture is kept at, in pixels.
pub(crate) const REFERENCE_SIDE: u32 = 2048;
/// A view not yet drawn is framed as if it were this large, in points.
const UNDRAWN_VIEW: [f64; 2] = [300.0, 400.0];

/// The picture, where it stands, and the marks on it.
pub(crate) struct Reference {
    pub(crate) name: String,
    pub(crate) board: Board,
    pub(crate) marks: Marks,
    /// The pixels, until the view makes a texture of them.
    pub(crate) image: Option<egui::ColorImage>,
    pub(crate) texture: Option<egui::TextureHandle>,
    pub(crate) show_marks: bool,
    pub(crate) opacity: f32,
}

/// A picture read, and the pose found in it (none when no one is in it).
pub(crate) struct Picked {
    pub(crate) still: flare_import::Still,
    pub(crate) found: Result<Option<flare_pose::Found>, String>,
}

/// A picture being read on another thread.
pub(crate) struct Loading {
    pub(crate) name: String,
    pub(crate) answer: Receiver<Result<Picked, String>>,
}

/// Reads the picture in `bytes` and looks for a person in it.
pub(crate) fn read(name: &str, bytes: &[u8]) -> Result<Picked, String> {
    let still = flare_import::read_still(name, bytes, REFERENCE_SIDE).map_err(|e| e.to_string())?;
    let pixels = flare_pose::Pixels {
        rgba: &still.rgba,
        size: still.kept,
    };
    let picture = [f64::from(still.width), f64::from(still.height)];
    let found = flare_pose::find_pose(pixels, picture).map_err(|e| e.to_string());
    Ok(Picked { still, found })
}

impl Workspace {
    /// Asks the shell for a picture to pose the mannequin from.
    pub fn ask_mannequin_reference(&mut self) {
        self.dialog = Some(DialogRequest::ImportMannequinReference);
    }

    /// Reads the picture at `path` to pose the mannequin from.
    pub fn import_mannequin_reference(&mut self, path: &Path) {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let too_large =
            std::fs::metadata(path).is_ok_and(|m| m.len() > flare_import::MAX_STILL_BYTES as u64);
        let bytes = if too_large {
            Err(tr!(self, "mannequin-reference-too-large"))
        } else {
            std::fs::read(path).map_err(|e| e.to_string())
        };
        match bytes {
            Ok(bytes) => self.import_mannequin_reference_bytes(&name, bytes),
            Err(error) => self.reference_failed(&name, &error),
        }
    }

    /// Reads a picture already held in memory, as a browser's file picker gives it.
    pub fn import_mannequin_reference_bytes(&mut self, name: &str, bytes: Vec<u8>) {
        let threads =
            !cfg!(target_arch = "wasm32") && self.background.mode() == flare_runtime::Mode::Threads;
        if !threads {
            let result = read(name, &bytes);
            return self.take_reference(name, result);
        }
        let (send, answer) = channel();
        let file = name.to_owned();
        let started = std::thread::Builder::new()
            .name("mannequin-reference".into())
            .spawn(move || {
                // The panel has gone if nobody is waiting; nothing is lost.
                let _ = send.send(read(&file, &bytes));
            });
        match started {
            Ok(_) => {
                self.mannequin.loading = Some(Loading {
                    name: name.to_owned(),
                    answer,
                });
            }
            Err(error) => self.reference_failed(name, &error.to_string()),
        }
    }

    /// Whether a picture is still being read; takes it when it is ready.
    pub(crate) fn poll_mannequin_reference(&mut self) -> bool {
        let Some(loading) = &self.mannequin.loading else {
            return false;
        };
        let result = match loading.answer.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => Err(tr!(self, "mannequin-reference-stopped")),
        };
        let name = loading.name.clone();
        self.mannequin.loading = None;
        self.take_reference(&name, result);
        false
    }

    fn reference_failed(&mut self, name: &str, error: &str) {
        let message = tr!(
            self,
            "mannequin-reference-error",
            &[("name", name), ("error", error)]
        );
        self.report(message);
    }

    /// The picture read: it stands behind the figure, which is turned to face it. With a person
    /// found the marks are on their joints and the figure takes their pose; else the marks are
    /// where the figure's joints are, to move onto the person by hand.
    fn take_reference(&mut self, name: &str, result: Result<Picked, String>) {
        let Picked { still, found } = match result {
            Ok(picked) => picked,
            Err(error) => return self.reference_failed(name, &error),
        };
        let size = [f64::from(still.width), f64::from(still.height)];
        let Some(board) = Board::new(size) else {
            return self.reference_failed(name, "no pixels");
        };
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [still.kept[0] as usize, still.kept[1] as usize],
            &still.rgba,
        );
        let figure = &mut self.mannequin.figure;
        figure.view = figure.view.facing(Facing::Front);
        let (marks, found) = match found {
            Ok(Some(found)) => (found.marks, Ok(true)),
            Ok(None) => (Marks::from_figure(figure, &board), Ok(false)),
            Err(error) => (Marks::from_figure(figure, &board), Err(error)),
        };
        self.mannequin.reference = Some(Reference {
            name: name.to_owned(),
            board,
            marks,
            image: Some(image),
            texture: None,
            show_marks: true,
            opacity: 0.6,
        });
        self.mannequin.frame_next = true;
        let message = match found {
            Ok(true) => match self.try_copy_mannequin_pose() {
                Ok(()) => tr!(self, "mannequin-found", &[("name", name)]),
                Err(error) => tr!(self, "mannequin-copy-error", &[("error", &error)]),
            },
            Ok(false) => tr!(self, "mannequin-not-found", &[("name", name)]),
            Err(error) => tr!(self, "mannequin-find-error", &[("error", &error)]),
        };
        self.report(message);
    }

    pub fn remove_mannequin_reference(&mut self) {
        self.mannequin.reference = None;
        self.mannequin.loading = None;
        if matches!(
            self.mannequin.drag,
            Some(super::mannequin::MannequinDrag::Mark(_))
        ) {
            self.mannequin.drag = None;
        }
    }

    /// The marks put back where the figure's joints are now, seen from the front.
    pub fn reset_mannequin_marks(&mut self) {
        let figure = self.mannequin.figure;
        if let Some(reference) = &mut self.mannequin.reference {
            reference.marks = Marks::from_figure(&figure, &reference.board);
        }
    }

    /// Puts `joint`'s mark at `at` on the picture, in its pixels. Whether it was taken.
    pub fn set_mannequin_mark(&mut self, joint: Joint, at: Option<[f64; 2]>) -> bool {
        self.mannequin
            .reference
            .as_mut()
            .is_some_and(|r| r.marks.set(joint, at))
    }

    /// Steps `joint`'s mark through working its depth out, nearer the viewer than the part it
    /// hangs from, and farther.
    pub fn cycle_mannequin_mark_depth(&mut self, joint: Joint) {
        if let Some(reference) = &mut self.mannequin.reference {
            let next = reference.marks.depth(joint).next();
            reference.marks.set_depth(joint, next);
        }
    }

    pub fn mannequin_mark_depth(&self, joint: Joint) -> Depth {
        self.mannequin
            .reference
            .as_ref()
            .map_or(Depth::Auto, |r| r.marks.depth(joint))
    }

    /// Poses the figure from the marks, turns it to face the picture and moves the picture so
    /// the figure stands over the person, as one change of the figure. Whether it was posed; a
    /// refusal is reported and changes nothing.
    pub fn copy_mannequin_pose(&mut self) -> bool {
        if self.mannequin.reference.is_none() {
            return false;
        }
        match self.try_copy_mannequin_pose() {
            Ok(()) => {
                self.report(tr!(self, "mannequin-copied"));
                true
            }
            Err(error) => {
                let message = tr!(self, "mannequin-copy-error", &[("error", &error)]);
                self.report(message);
                false
            }
        }
    }

    /// [`copy_mannequin_pose`](Self::copy_mannequin_pose), saying nothing.
    fn try_copy_mannequin_pose(&mut self) -> Result<(), String> {
        let reference = self.mannequin.reference.as_ref().ok_or("no picture")?;
        let (marks, size) = (reference.marks, reference.board.size());
        let mut figure = self.mannequin.figure;
        let fit = figure.copy_pose(&marks).map_err(|e| e.to_string())?;
        let board = Board::fitted(size, &fit).ok_or("no fit")?;
        figure.view = figure.view.facing(Facing::Front);
        self.mannequin.figure = figure;
        if let Some(reference) = &mut self.mannequin.reference {
            reference.board = board;
        }
        self.frame_mannequin();
        Ok(())
    }

    /// The view's middle and the figure's height in it at a zoom of one, and its size.
    pub(crate) fn mannequin_frame_box(&self) -> ([f64; 2], f64, [f64; 2]) {
        match self.mannequin.view_rect {
            Some(rect) => {
                let c = rect.center();
                (
                    [f64::from(c.x), f64::from(c.y)],
                    f64::from(rect.height() * 0.8),
                    [f64::from(rect.width()), f64::from(rect.height())],
                )
            }
            None => {
                let [w, h] = UNDRAWN_VIEW;
                ([w / 2.0, h / 2.0], h * 0.8, UNDRAWN_VIEW)
            }
        }
    }

    /// Brings the whole figure into the panel's view.
    pub fn frame_mannequin(&mut self) {
        let (center, height, size) = self.mannequin_frame_box();
        let figure = self.mannequin.figure;
        self.mannequin.camera.frame(&figure, center, height, size);
    }

    /// The view turns round `joint` from now on, which comes to its middle.
    pub fn look_at_mannequin_part(&mut self, joint: Joint) {
        let at = self.mannequin.figure.posed().at(joint);
        self.mannequin.camera.look_at(at);
    }

    /// Sees the figure square from one side.
    pub fn face_mannequin(&mut self, facing: Facing) {
        let figure = &mut self.mannequin.figure;
        figure.view = figure.view.facing(facing);
    }

    /// Zooms the view by `factor` about its middle.
    pub fn zoom_mannequin(&mut self, factor: f64) {
        let (center, height, _) = self.mannequin_frame_box();
        let view = self.mannequin.figure.view;
        self.mannequin
            .camera
            .zoom_about(&view, center, height, center, factor);
    }
}
