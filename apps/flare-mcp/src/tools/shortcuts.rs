//! Keyboard shortcut sets: the same behaviour as `flare shortcuts`, from `flare_menubar`. The
//! server keeps no shortcut file; a file is passed in as text and the checked form comes back.

use flare_menubar::{ShortcutEdit, ShortcutMap};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};

use crate::params::{AnimateShortcutsParams, ShortcutsParams};
use crate::params_shortcuts::EditShortcutsParams;
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
        description = "Change one command's keyboard shortcuts, starting from a set or from the shortcut file text passed as `file`, and return the new `file` with the command's `keys` (first one first; the first is the one its menu item shows). A command can have several shortcuts, up to 4: give exactly one of `keys` (replace them all), `add` (one more), `remove` (take one off, the others stay), `clear` or `reset` (the set's own keys). Keys another command has are taken from it and `taken_from` names it. A bad key, an unknown command, too many keys or keys it does not have are refused."
    )]
    fn edit_shortcuts(&self, Parameters(a): Parameters<EditShortcutsParams>) -> Outcome {
        let asked = [
            a.keys.clone().map(ShortcutEdit::Set),
            a.add.clone().map(ShortcutEdit::Add),
            a.remove.clone().map(ShortcutEdit::Remove),
            a.clear.unwrap_or(false).then_some(ShortcutEdit::Clear),
            a.reset.unwrap_or(false).then_some(ShortcutEdit::Reset),
        ];
        let mut asked = asked.into_iter().flatten();
        let (Some(edit), None) = (asked.next(), asked.next()) else {
            return Err("give exactly one of keys, add, remove, clear or reset".to_owned());
        };
        let mut map = match a.file.as_deref() {
            Some(text) => ShortcutMap::from_json(text),
            None => ShortcutMap::from_set(a.set.as_deref().unwrap_or("flare")),
        }
        .map_err(|e| e.to_string())?;
        let taken = map
            .edit(&a.action, &edit)
            .map_err(|e| format!("{}: {e}", a.action))?;
        let keys: Vec<String> = flare_menubar::MenuAction::from_id(&a.action)
            .map(|action| map.keys(action).iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        Ok(serde_json::json!({
            "action": a.action,
            "keys": keys,
            "taken_from": taken.map(|t| t.id()),
            "file": map.file_value(),
        })
        .to_string())
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
