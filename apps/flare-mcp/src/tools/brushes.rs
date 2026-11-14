//! Another art tool's pen profiles read as Flare's brush presets: the same reading the desktop
//! app and the command line use, from a file in the configured export directory. The server
//! keeps no brush list of its own; the presets come back so the caller can keep or use them.
//! The file is passed by name (`source`) because profile files are binary.

use std::path::PathBuf;

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use super::export::export_dir;
use crate::color::safe_name;
use crate::params::ImportBrushes;
use crate::server::{FlareServer, Outcome, State};

/// Longest pen-profile file read, the bound the library itself applies.
const MAX_PROFILE_BYTES: u64 = flare_import::brushes::MAX_OWNED_BYTES;

impl State {
    /// The pen-profile file called `source` in the export directory, which it must stay inside.
    fn profile_path(&self, source: &str) -> Result<PathBuf, String> {
        let dir = export_dir(self)?
            .canonicalize()
            .map_err(|error| format!("cannot resolve the export directory: {error}"))?;
        let path = dir.join(safe_name(source)?);
        if !path.is_file() {
            return Err(format!("{source} is not a file in the export directory"));
        }
        let path = path
            .canonicalize()
            .map_err(|error| format!("cannot resolve {source}: {error}"))?;
        if !path.starts_with(&dir) {
            return Err("the profile file must stay inside the export directory".to_owned());
        }
        Ok(path)
    }
}

#[tool_router(router = brush_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Read another art tool's pen profiles from a file in the export directory and return them as Flare's brush presets: each with its name, nib (shape, angle, flatness), pressure response, velocity, flow, stabilizer and taper, the parts that map over. `source` names a Photoshop `.abr` (versions 1 and 2), a Krita `.kpp` or a Krita `.bundle`; the ending decides the reader. Returns `presets` and `notes`, one per thing the file held that does not map (a tip that is a picture, a preset too large). A file that is not a pen profile at all is refused by name."
    )]
    fn import_brush_profiles(&self, Parameters(args): Parameters<ImportBrushes>) -> Outcome {
        self.read(|state| {
            let path = state.profile_path(&args.source)?;
            let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if meta.len() > MAX_PROFILE_BYTES {
                return Err(format!(
                    "{source} is larger than {MAX_PROFILE_BYTES} bytes, more than a pen-profile file holds",
                    source = args.source
                ));
            }
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let set = flare_import::brushes::import_profile_bytes(&path, &bytes)
                .map_err(|e| e.to_string())?;
            let presets: Vec<Value> = set
                .presets
                .iter()
                .map(serde_json::to_value)
                .collect::<std::result::Result<_, _>>()
                .map_err(|e| e.to_string())?;
            Ok(json!({ "presets": presets, "notes": set.notes }))
        })
    }
}
