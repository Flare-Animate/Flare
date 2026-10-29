//! The page's application: the workspace, plus what a page does when the workspace asks for a
//! file.

use std::cell::RefCell;
use std::rc::Rc;

use eframe::egui;
use flare_animate::{DialogRequest, Workspace};

use crate::{files, preferences};

/// A file the person picked, waiting to be handed to the workspace.
struct Picked {
    name: String,
    bytes: Vec<u8>,
}

pub struct WebApp {
    workspace: Workspace,
    picked: Rc<RefCell<Vec<Picked>>>,
    /// The preferences as last written, so storage is touched only when they change.
    saved_preferences: String,
}

impl WebApp {
    pub fn new(workspace: Workspace) -> Self {
        let saved_preferences = workspace.preferences_json().unwrap_or_default();
        Self {
            workspace,
            picked: Rc::default(),
            saved_preferences,
        }
    }

    /// Does what the workspace asked of the outside world.
    fn handle_dialog(&mut self, ctx: &egui::Context) {
        let Some(request) = self.workspace.take_dialog() else {
            return;
        };
        match request {
            DialogRequest::Open | DialogRequest::ImportProject => {
                let (inbox, ctx) = (Rc::clone(&self.picked), ctx.clone());
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some((name, bytes)) = files::pick("Flash document", &["fla"]).await {
                        inbox.borrow_mut().push(Picked { name, bytes });
                        ctx.request_repaint();
                    }
                });
            }
            DialogRequest::SaveAs => self.save_fla(),
            DialogRequest::ExportImage => {
                let (document, frame) = self.workspace.image_export();
                self.deliver(
                    "frame.png",
                    "image/png",
                    flare_runtime::png_bytes(&document, frame),
                );
            }
            DialogRequest::ExportMovie => {
                let (document, _) = self.workspace.image_export();
                self.deliver(
                    "movie.gif",
                    "image/gif",
                    flare_runtime::gif_bytes(&document),
                );
            }
            DialogRequest::PublishSwf => {
                let (document, _) = self.workspace.image_export();
                let options = flare_publish::PublishOptions {
                    compress: self.workspace.publish_compress(),
                };
                let result = flare_publish::publish_swf(&document, &options)
                    .map(|published| published.bytes)
                    .map_err(|e| e.to_string());
                self.deliver("movie.swf", "application/x-shockwave-flash", result);
            }
            DialogRequest::ImportSound => {
                self.workspace
                    .report_message("Importing sounds is not available in the browser yet");
            }
        }
    }

    /// Saving offers the project as a `.fla` download: the `.flare` container needs SQLite,
    /// which a page does not have, and XFL keeps everything an Animate user can open elsewhere.
    fn save_fla(&mut self) {
        let (document, _) = self.workspace.image_export();
        let bytes = flare_export::export_xfl(&document)
            .and_then(|output| flare_export::fla_bytes(&output))
            .map_err(|e| e.to_string());
        self.deliver("project.fla", "application/zip", bytes);
    }

    fn deliver(&mut self, name: &str, mime: &str, bytes: Result<Vec<u8>, String>) {
        match bytes.and_then(|b| files::download(name, mime, &b)) {
            Ok(()) => {}
            Err(error) => self.workspace.report_message(&error),
        }
    }
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let picked: Vec<Picked> = self.picked.borrow_mut().drain(..).collect();
        for file in picked {
            self.workspace.import_project_bytes(&file.name, file.bytes);
        }
        self.workspace.show(ui);
        self.handle_dialog(&ctx);
        if let Ok(json) = self.workspace.preferences_json()
            && json != self.saved_preferences
        {
            preferences::save(&json);
            self.saved_preferences = json;
        }
    }
}
