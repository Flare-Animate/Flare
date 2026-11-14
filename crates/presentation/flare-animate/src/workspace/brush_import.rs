//! File > Import > Import Brush Presets: another art tool's pen profiles brought onto the brush
//! list. What is read and how a `.abr`, `.kpp` or `.bundle` maps onto Flare's presets is
//! `flare_import::brushes`'s, the same the command line and the MCP server use; what is left
//! here is what only the workspace can do, which is to keep them and say what landed.

use super::*;

impl Workspace {
    /// Keeps the pen-profile file `name` (its bytes, as picked) on the brush list, after what is
    /// there. The reader is told what the file is by the name's ending, so a file renamed behind
    /// Flare's back is read by what it is called. Presets that do not map are said in Output,
    /// each with the reason; a file that cannot be read changes nothing and says why.
    pub fn import_brush_profiles(&mut self, name: &str, bytes: &[u8]) {
        let set =
            match flare_import::brushes::import_profile_bytes(std::path::Path::new(name), bytes) {
                Ok(set) => set,
                Err(error) => {
                    let reason = error.to_string();
                    let message = tr!(self, "brushes-import-refused", &[("reason", &reason)]);
                    self.report(message);
                    return;
                }
            };
        let kept = set.presets.len();
        if self.brush_shelf.keep_presets(set.presets).is_err() {
            let message = tr!(self, "brushes-shelf-full");
            self.report(message);
            return;
        }
        // The first preset that landed is the brush in hand now; its response is read back so
        // the panel's sliders show it rather than what was there before.
        self.brush_light = self.brush_shelf.pressure_min();
        self.brush_pressure_gamma = self.brush_shelf.pressure_gamma();
        let message = tr_n!(self, "brushes-imported", kept);
        self.report(message);
        for note in &set.notes {
            let line = note.clone();
            self.report(tr!(self, "brushes-import-note", &[("line", &line)]));
        }
    }
}
