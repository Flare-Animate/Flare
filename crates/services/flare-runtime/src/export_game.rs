//! The game as files an engine can load: the description as JSON, and the scene's frames as a
//! sprite sheet. A sprite's frame `n` is sheet frame `sheet.first_frame + n`, so the sheet and
//! the description always name the same pictures.

use flare_document::Document;
use serde_json::json;

use crate::{SpriteSheet, sprite_sheet};

/// The format name and version of the game description.
pub const GAME_FORMAT: &str = "flare-game";
pub const GAME_FORMAT_VERSION: u32 = 1;

/// The exported game: its description and the sheet of the frames its sprites show.
#[derive(Debug, Clone, PartialEq)]
pub struct GameExport {
    /// The description, to be written as `<name>.json`.
    pub json: String,
    /// The sheet, to be written as `<name>.png` with its atlas `<name>.sheet.json`.
    pub sheet: SpriteSheet,
}

/// Exports the document's game with the frames `start..=end` of its first scene (the whole scene
/// when `end` is `None`) in a grid of `columns`. `name` is the base name of the files.
pub fn export_game(
    document: &Document,
    start: u32,
    end: Option<u32>,
    columns: Option<u32>,
    name: &str,
) -> Result<GameExport, String> {
    let game = document
        .game()
        .ok_or("The project has no game to export.")?;
    let game = serde_json::to_value(game).map_err(|e| e.to_string())?;
    let image = format!("{name}.png");
    let sheet = sprite_sheet(document, start, end, columns, &image)?;
    let rate = document.frame_rate();
    let description = json!({
        "format": GAME_FORMAT,
        "version": GAME_FORMAT_VERSION,
        "frames_per_second": f64::from(rate.numerator()) / f64::from(rate.denominator()),
        "sheet": {
            "image": image,
            "atlas": format!("{name}.sheet.json"),
            "first_frame": start,
            "frames": sheet.frames,
        },
        "game": game,
    });
    let json = serde_json::to_string_pretty(&description).map_err(|e| e.to_string())?;
    Ok(GameExport { json, sheet })
}

#[cfg(test)]
#[path = "export_game_tests.rs"]
mod tests;
