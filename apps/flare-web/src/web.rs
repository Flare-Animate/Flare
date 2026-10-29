//! Starting the workspace on a canvas.

use eframe::wasm_bindgen::JsCast;
use flare_animate::Workspace;
use wasm_bindgen::prelude::wasm_bindgen;

/// The page element the workspace is drawn on.
const CANVAS_ID: &str = "flare-canvas";

struct WebApp {
    workspace: Workspace,
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.workspace.show(ui);
    }
}

/// Runs when the module loads: finds the canvas and starts the workspace on it.
#[wasm_bindgen(start)]
pub fn start() {
    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(CANVAS_ID))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
        let (Some(canvas), Ok(workspace)) = (canvas, Workspace::untitled()) else {
            return;
        };
        let runner = eframe::WebRunner::new();
        let started = runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|_cc| Ok(Box::new(WebApp { workspace }))),
            )
            .await;
        if let Err(error) = started {
            web_sys::console::error_1(&format!("could not start Flare: {error:?}").into());
        }
    });
}
