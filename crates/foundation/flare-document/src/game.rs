//! The game description's references into the document: a game entity draws a symbol of the
//! library, and an audio component plays a sound of it. The description checks its own values
//! (`flare_game`); this checks what it names.

use flare_game::{Component, GameError};

use crate::Document;
use crate::validate::Diagnostic;

pub(crate) fn validate_game(document: &Document) -> Vec<Diagnostic> {
    let Some(game) = &document.game else {
        return Vec::new();
    };
    let mut out: Vec<Diagnostic> = game.problems().into_iter().map(Diagnostic::Game).collect();
    for entity in game.entities() {
        if let Some(symbol) = entity.symbol()
            && document.symbol(symbol).is_none()
        {
            out.push(Diagnostic::Game(GameError::NoSuchSymbol(symbol)));
        }
        if let Some(Component::Audio(audio)) = entity.component(flare_game::ComponentKind::Audio)
            && let Some(sound) = audio.sound
            && document.sound(sound).is_none()
        {
            out.push(Diagnostic::Game(GameError::NoSuchSound(sound)));
        }
    }
    out
}
