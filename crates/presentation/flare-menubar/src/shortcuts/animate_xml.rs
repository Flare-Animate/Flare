//! Reading the keyboard shortcuts Animate keeps (`Shortcuts/<set>.xml`), so a person who comes
//! from Animate keeps their keys. Each binding is one element:
//!
//! ```xml
//! <keycmd><cmd>File.New</cmd><uchar>N</uchar><mod><cmdkey/></mod></keycmd>
//! ```
//!
//! A typed key is `uchar`; a function or arrow key is a number in an element of its own. Only
//! the numbers that have been matched against Animate's menus are known (F1 to F12 are 7 to 18,
//! Up is 44); a binding with another number, a modifier that is not understood, or a command
//! Flare does not have is skipped and said so, never guessed. Animate's set is the starting point,
//! and a key the file gives a command is taken from any command that held it there.

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};

use super::{ShortcutError, ShortcutMap};
use crate::MenuAction;
use crate::MenuAction as A;

/// The largest Animate shortcut file read. Animate's own holds about 260 bindings in 40 KiB.
pub const MAX_ANIMATE_SHORTCUT_BYTES: usize = 512 * 1024;
/// The most bindings read from one file.
const MAX_BINDINGS: usize = 4096;
/// How deep the elements may nest.
const MAX_DEPTH: usize = 16;

/// Animate's command ids, as its shortcut file names them, and the command each is in Flare.
const COMMANDS: &[(&str, MenuAction)] = &[
    ("File.New", A::New),
    ("File.Open", A::Open),
    ("File.Close", A::Close),
    ("File.Save", A::Save),
    ("File.SaveAs", A::SaveAs),
    ("File.PublishSettings", A::PublishSettings),
    ("File.Publish", A::Publish),
    ("File.Quit", A::Quit),
    ("Export.ExportMovie", A::ExportMovie),
    ("Edit.Undo", A::Undo),
    ("Edit.Redo", A::Redo),
    ("Edit.Cut", A::Cut),
    ("Edit.Copy", A::Copy),
    ("Edit.Paste", A::Paste),
    ("Edit.PasteInPlace", A::PasteInPlace),
    ("Edit.Duplicate", A::Duplicate),
    ("Edit.SelectAll", A::SelectAll),
    ("Edit.DeselectAll", A::DeselectAll),
    ("Edit.EditSymbols", A::EditSymbol),
    ("Edit.Clear", A::Delete),
    ("Preferences.SetPreferences", A::Preferences),
    ("Timeline.RemoveFrames", A::RemoveFrame),
    ("Timeline.CutFrames", A::CutFrames),
    ("Timeline.CopyFrames", A::CopyFrames),
    ("Timeline.PasteFrames", A::PasteFrames),
    ("Timeline.SelectAllFrames", A::SelectAllFrames),
    ("Timeline.InsertFrame", A::InsertFrame),
    ("Timeline.ConvertToKeyframes", A::InsertKeyframe),
    ("Timeline.ConvertToBlankKeyframes", A::InsertBlankKeyframe),
    ("Timeline.ClearKeyframe", A::ClearKeyframe),
    ("Timeline.OnionSkin", A::ToggleOnionSkin),
    ("View.ZoomIn", A::ZoomIn),
    ("View.ZoomOut", A::ZoomOut),
    ("View.Rulers", A::ToggleRulers),
    ("Magnification.Percent100", A::ActualSize),
    ("Magnification.ShowFrame", A::ZoomFit),
    ("PreviewMode.Outlines", A::ToggleOutlineMode),
    ("Grid.ShowGrid", A::ToggleGrid),
    ("Guides.ShowGuides", A::ToggleGuides),
    ("Insert.NewSymbol", A::NewSymbol),
    ("Modify.Document", A::DocumentSettings),
    ("Modify.ConvertToSymbol", A::ConvertToSymbol),
    ("Modify.BreakApart", A::BreakApart),
    ("Modify.Group", A::Group),
    ("Modify.Ungroup", A::Ungroup),
    ("Arrange.BringToFront", A::BringToFront),
    ("Arrange.BringForward", A::BringForward),
    ("Arrange.SendBackward", A::SendBackward),
    ("Arrange.SendToBack", A::SendToBack),
    ("Transform.Rotate90CW", A::RotateClockwise),
    ("Transform.Rotate90CCW", A::RotateCounterclockwise),
    ("TextStyle.Bold", A::TextBold),
    ("TextStyle.Italic", A::TextItalic),
    ("Control.Play", A::PlayPause),
    ("Control.Rewind", A::Rewind),
    ("Control.GoToEnd", A::GoToEnd),
    ("Control.StepForwardOneFrame", A::StepForward),
    ("Control.StepBackwardOneFrame", A::StepBackward),
    ("Control.MuteSounds", A::ToggleMute),
    ("TestMovie.Test", A::TestMovie),
    ("Window.Timeline", A::ShowTimeline),
    ("Window.Properties", A::ShowProperties),
    ("Window.Library", A::ShowLibrary),
    ("Window.Output", A::ShowOutput),
    ("Window.Align", A::ShowAlign),
    ("Window.Color", A::ShowColor),
    ("Window.Info", A::ShowInfo),
    ("Window.Transform", A::ShowTransform),
    ("Window.History", A::ShowHistory),
    ("Tool.Selection", A::ToolSelect),
    ("Tool.SubSelection", A::ToolSubselection),
    ("Tool.FreeTransform", A::ToolFreeTransform),
    ("Tool.Lasso", A::ToolLasso),
    ("Tool.Pen", A::ToolPen),
    ("Tool.Text", A::ToolText),
    ("Tool.Line", A::ToolLine),
    ("Tool.Rectangle", A::ToolRectangle),
    ("Tool.Oval", A::ToolEllipse),
    ("Tool.Pencil", A::ToolPencil),
    ("Tool.Brush", A::ToolBrush),
    ("Tool.PaintBucket", A::ToolBucket),
    ("Tool.InkBottle", A::ToolInkBottle),
    ("Tool.EyeDropper", A::ToolEyedropper),
    ("Tool.Eraser", A::ToolEraser),
    ("Tool.Hand", A::ToolHand),
    ("Tool.Rotate", A::ToolRotateView),
    ("Tool.Zoom", A::ToolZoom),
];

/// A binding from the file that was not used, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub command: String,
    pub keys: String,
    pub reason: String,
}

/// What reading an Animate shortcut file made: the map, what was taken over, and what was not.
#[derive(Clone, Debug)]
pub struct AnimateImport {
    pub map: ShortcutMap,
    /// Each command given keys from the file, with those keys.
    pub applied: Vec<(MenuAction, String)>,
    pub skipped: Vec<Skipped>,
}

impl AnimateImport {
    /// The outcome as the command line and the MCP server print it.
    pub fn to_json(&self) -> Value {
        json!({
            "set": self.map.set(),
            "applied": self.applied.iter().map(|(a, k)| json!({"command": a.id(), "keys": k})).collect::<Vec<_>>(),
            "skipped": self.skipped.iter().map(|s| json!({"command": s.command, "keys": s.keys, "reason": s.reason})).collect::<Vec<_>>(),
            "file": self.map.file_value(),
        })
    }
}

/// One `<keycmd>` as read.
#[derive(Default)]
struct Raw {
    command: String,
    typed: Option<String>,
    code: Option<String>,
    modifiers: Vec<String>,
}

impl Raw {
    /// The binding as Animate shows it, for messages.
    fn keys(&self) -> String {
        let mut parts: Vec<String> = self.modifiers.clone();
        parts.push(
            self.typed
                .clone()
                .or_else(|| self.code.as_ref().map(|c| format!("key code {c}")))
                .unwrap_or_default(),
        );
        parts.join("+")
    }

    /// The binding in Flare's accelerator spelling, or why it cannot be one.
    fn accelerator(&self) -> Result<String, String> {
        let mut text = String::new();
        for name in &self.modifiers {
            // `cmdkey` is Ctrl on Windows and Cmd on a Mac, as Flare's CmdOrCtrl is.
            let part = match name.as_str() {
                "cmdkey" => "CmdOrCtrl+",
                "shiftkey" => "Shift+",
                "altkey" | "optkey" => "Alt+",
                other => return Err(format!("the modifier `{other}` is not known")),
            };
            if !text.contains(part) {
                text.push_str(part);
            }
        }
        let key = match (&self.typed, &self.code) {
            (Some(typed), _) => match typed.as_str() {
                "+" => "Plus".to_owned(),
                " " => "Space".to_owned(),
                other => other.to_owned(),
            },
            (None, Some(code)) => key_for_code(code)?,
            (None, None) => return Err("it names no key".to_owned()),
        };
        text.push_str(&key);
        Ok(text)
    }
}

/// The keys Animate stores as a number, as far as they have been matched against its menus.
fn key_for_code(code: &str) -> Result<String, String> {
    let n: u32 = code
        .trim()
        .parse()
        .map_err(|_| format!("`{code}` is not a key"))?;
    match n {
        7..=18 => Ok(format!("F{}", n - 6)),
        44 => Ok("Up".to_owned()),
        _ => Err(format!("key code {n} is not known yet")),
    }
}

impl ShortcutMap {
    /// Animate's set with the keys of an Animate shortcut file on it. The file is untrusted: too
    /// large, not XML, nested too deep or holding too many bindings is an error and builds
    /// nothing; a binding Flare cannot use is skipped with the reason.
    pub fn from_animate_xml(text: &str) -> Result<AnimateImport, ShortcutError> {
        if text.len() > MAX_ANIMATE_SHORTCUT_BYTES {
            return Err(ShortcutError::TooLarge(MAX_ANIMATE_SHORTCUT_BYTES));
        }
        let raws = read(text)?;
        let mut map = Self::from_set("animate")?;
        let mut applied: Vec<(MenuAction, String)> = Vec::new();
        let mut skipped = Vec::new();
        let mut skip = |raw: &Raw, reason: String| {
            skipped.push(Skipped {
                command: raw.command.clone(),
                keys: raw.keys(),
                reason,
            });
        };
        for raw in &raws {
            let Some(action) = COMMANDS
                .iter()
                .find(|(id, _)| *id == raw.command)
                .map(|(_, action)| *action)
            else {
                skip(raw, "Flare has no such command yet".to_owned());
                continue;
            };
            if applied.iter().any(|(a, _)| *a == action) {
                skip(raw, "Flare keeps one shortcut a command".to_owned());
                continue;
            }
            let keys = match raw.accelerator() {
                Ok(keys) => keys,
                Err(reason) => {
                    skip(raw, reason);
                    continue;
                }
            };
            match map.bind_taking(action, &keys) {
                Ok(taken) => {
                    if let Some(taken) = taken {
                        // A key the file gave twice: the later binding wins, as it would in Animate's editor.
                        applied.retain(|(a, _)| *a != taken);
                    }
                    let shown = map.get(action).map(ToString::to_string).unwrap_or(keys);
                    applied.push((action, shown));
                }
                Err(error) => skip(raw, error.to_string()),
            }
        }
        Ok(AnimateImport {
            map,
            applied,
            skipped,
        })
    }
}

fn read(text: &str) -> Result<Vec<Raw>, ShortcutError> {
    let bad = |message: String| ShortcutError::BadFile(message);
    let mut reader = Reader::from_str(text);
    let mut path: Vec<String> = Vec::new();
    let mut raws = Vec::new();
    let mut current: Option<Raw> = None;
    let mut text_of = String::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|e| bad(format!("it is not XML: {e}")))?;
        match event {
            Event::Start(start) => {
                let name = start.local_name().as_ref().to_ascii_lowercase();
                if path.len() >= MAX_DEPTH {
                    return Err(bad("its elements nest too deeply".to_owned()));
                }
                if name == "keycmd" {
                    if raws.len() >= MAX_BINDINGS {
                        return Err(bad(format!("it has more than {MAX_BINDINGS} shortcuts")));
                    }
                    current = Some(Raw::default());
                }
                path.push(name);
                text_of.clear();
            }
            Event::Empty(empty) => {
                let name = empty.local_name().as_ref().to_ascii_lowercase();
                if path.last().is_some_and(|p| p == "mod")
                    && let Some(raw) = current.as_mut()
                {
                    raw.modifiers.push(name);
                }
            }
            Event::Text(t) => {
                text_of.push_str(&t.xml10_content());
            }
            Event::GeneralRef(r) => {
                let name: &str = r.as_ref();
                text_of.push_str(
                    &entity(name).ok_or_else(|| bad(format!("`&{name};` is not a character")))?,
                );
            }
            Event::End(_) => {
                let Some(name) = path.pop() else {
                    return Err(bad("an element closes that never opened".to_owned()));
                };
                if let Some(raw) = current.as_mut() {
                    match name.as_str() {
                        "keycmd" => {
                            if let Some(raw) = current.take() {
                                raws.push(raw);
                            }
                        }
                        "cmd" => raw.command = text_of.trim().to_owned(),
                        "uchar" => raw.typed = Some(text_of.clone()),
                        "mod" => {}
                        // A number in any other element of the binding is a key code.
                        _ if path.last().is_some_and(|p| p == "keycmd")
                            && !text_of.trim().is_empty() =>
                        {
                            raw.code = Some(text_of.trim().to_owned());
                        }
                        _ => {}
                    }
                }
                text_of.clear();
            }
            Event::DocType(_) => return Err(bad("it declares a document type".to_owned())),
            Event::Eof => break,
            _ => {}
        }
    }
    if !path.is_empty() {
        return Err(bad("it ends inside an element".to_owned()));
    }
    Ok(raws)
}

/// The character an entity names: the five XML ones and numeric references.
fn entity(name: &str) -> Option<String> {
    let c = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)?
        }
    };
    Some(c.to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[path = "animate_xml_tests.rs"]
mod tests;
