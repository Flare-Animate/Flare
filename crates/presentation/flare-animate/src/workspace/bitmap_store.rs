//! How the library's bitmaps are stored, chosen in the Doc tab as Animate's Bitmap Properties
//! choose it for one: as photos (JPEG) or lossless. A video brought in to draw over is hundreds
//! of bitmaps, most of a project file's size, and as photos they take about a ninth of the room.
//! Making the JPEGs takes seconds for a library of video frames, so it runs on the pixels worker
//! and the window applies the result as one undo step when it comes back.

use std::sync::Arc;

use egui::Ui;
use flare_document::{DEFAULT_PHOTO_QUALITY, Document};
use flare_runtime::PixelJob;
use flare_tools::library::{Compressed, Compression, stored_bytes};
use flare_tools::video::megabytes;
use flare_ui::chrome;

use super::*;

/// The Doc tab's choice, and the storing under way.
pub(crate) struct BitmapStore {
    /// The job storing the bitmaps, while it runs.
    pub(super) pending: Option<u64>,
    /// Photo (JPEG) or lossless, as chosen.
    photo: bool,
    quality: u8,
}

impl Default for BitmapStore {
    fn default() -> Self {
        Self {
            pending: None,
            photo: true,
            quality: DEFAULT_PHOTO_QUALITY,
        }
    }
}

impl Workspace {
    /// Stores every bitmap in the library as `compression` says, off the window's thread.
    pub fn store_bitmaps(&mut self, compression: Compression) {
        if self.document().bitmaps().is_empty() || self.bitmap_store.pending.is_some() {
            return;
        }
        let id = self.background.next_id();
        self.bitmap_store.pending = Some(id);
        self.background.submit_pixels(PixelJob::CompressBitmaps {
            id,
            document: self.active().snapshot(),
            compression,
        });
        self.settle_inline();
    }

    /// Whether bitmaps are being stored another way right now.
    pub fn is_storing_bitmaps(&self) -> bool {
        self.bitmap_store.pending.is_some()
    }

    /// The storing came back: applied if the document is still the one it was made for.
    pub(super) fn bitmaps_stored(
        &mut self,
        id: u64,
        document: Arc<Document>,
        result: Result<Compressed, String>,
    ) {
        if self.bitmap_store.pending.take_if(|p| *p == id).is_none() {
            return;
        }
        if !Arc::ptr_eq(&document, &self.active().snapshot()) {
            self.report(tr!(self, "msg-bitmaps-changed"));
            return;
        }
        match result {
            Ok(done) if done.commands.is_empty() => self.report(tr!(self, "msg-bitmaps-unchanged")),
            Ok(done) => {
                let (changed, before, after) = (
                    done.changed.to_string(),
                    megabytes(done.bytes_before).to_string(),
                    megabytes(done.bytes_after).to_string(),
                );
                self.run_batch(done.commands, "store bitmaps");
                if self.status.is_none() {
                    let text = tr!(
                        self,
                        "msg-bitmaps-stored",
                        &[
                            ("changed", &changed),
                            ("before", &before),
                            ("after", &after)
                        ],
                    );
                    self.report(text);
                }
            }
            Err(reason) => self.report(tr!(self, "msg-bitmaps", &[("reason", &reason)])),
        }
    }

    /// The Doc tab's Bitmaps rows: how many and the room they take, and how to store them.
    pub(super) fn bitmaps_section(&mut self, ui: &mut Ui) {
        let bitmaps = self.document().bitmaps();
        let count = bitmaps.len();
        let photos = bitmaps.iter().filter(|b| b.photo().is_some()).count();
        let bytes: u64 = bitmaps.iter().map(stored_bytes).sum();
        let summary = tr!(
            self,
            "doc-bitmaps-summary",
            &[
                ("count", &count.to_string()),
                ("photos", &photos.to_string()),
                ("mb", &megabytes(bytes).to_string()),
            ],
        );
        let label = tr!(self, "doc-bitmaps");
        chrome::row(ui, &label, |ui| {
            ui.label(summary);
        });
        if count == 0 {
            return;
        }
        let (store, photo, lossless, tip) = (
            tr!(self, "doc-bitmaps-store"),
            tr!(self, "doc-bitmaps-photo"),
            tr!(self, "doc-bitmaps-lossless"),
            tr!(self, "doc-bitmaps-tip"),
        );
        let form = &mut self.bitmap_store;
        chrome::row(ui, &store, |ui| {
            egui::ComboBox::from_id_salt("bitmap_store")
                .selected_text(if form.photo { &photo } else { &lossless })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.photo, true, &photo);
                    ui.selectable_value(&mut form.photo, false, &lossless);
                })
                .response
                .on_hover_text(&tip);
            if form.photo {
                ui.add(
                    egui::DragValue::new(&mut form.quality)
                        .range(1..=100)
                        .suffix("%"),
                )
                .on_hover_text(&tip);
            }
        });
        let working = self.bitmap_store.pending.is_some();
        let apply = if working {
            tr!(self, "doc-bitmaps-working")
        } else {
            tr!(self, "doc-bitmaps-apply")
        };
        let compression = if self.bitmap_store.photo {
            Compression::Photo {
                quality: self.bitmap_store.quality,
            }
        } else {
            Compression::Lossless
        };
        if ui.add_enabled(!working, egui::Button::new(apply)).clicked() {
            self.store_bitmaps(compression);
        }
    }
}
