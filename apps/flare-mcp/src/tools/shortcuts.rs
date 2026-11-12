//! Keyboard shortcut sets: the same behaviour as `flare shortcuts`, from `flare_menubar`. The
//! server keeps no shortcut file; a file is passed in as text and the checked form comes back.

use flare_menubar::ShortcutMap;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};

use crate::params::{AnimateShortcutsParams, ShortcutsParams};
use crate::server::{FlareServer, Outcome};

#[tool_router(router = shortcut_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "List the keyboard shortcuts of a set (flare, opentoonz, animate or tahoma), or of the shortcut file text passed as `file`; a file that has an unknown command, a bad key or a conflict is refused. With `search` only the commands found by English name, id or keys (\"save as\", \"cmd+s\", \"option\") are listed, each with its name. With `export` true the checked file is returned as well."
    )]
    fn list_shortcuts(&self, Parameters(a): Parameters<ShortcutsParams>) -> Outcome {
        let map = match a.file.as_deref() {
            Some(text) => ShortcutMap::from_json(text),
            None => ShortcutMap::from_set(a.set.as_deref().unwrap_or("flare")),
        }
        .map_err(|e| e.to_string())?;
        let mut out = match a.search.as_deref() {
            Some(query) => map.search_json(query),
            None => map.list_json(),
        };
        if a.export.unwrap_or(false) {
            out["file"] = map.file_value();
        }
        Ok(out.to_string())
    }

    #[tool(
        description = "Read Animate's own keyboard shortcut file (the XML text of `Shortcuts/<set>.xml`) onto the animate set. Returns `applied` (each command and its keys), `skipped` (each binding not taken over and why: a command Flare does not have, a key code not known yet, a modifier not understood) and `file`, the Flare shortcut file to keep. A file that is not XML, too large or too deeply nested is refused."
    )]
    fn import_animate_shortcuts(
        &self,
        Parameters(a): Parameters<AnimateShortcutsParams>,
    ) -> Outcome {
        let import = ShortcutMap::from_animate_xml(&a.xml).map_err(|e| e.to_string())?;
        Ok(import.to_json().to_string())
    }
}
