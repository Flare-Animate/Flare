//! What the Test Movie window says: its title, the tips of its buttons and its clock, in the
//! language of the interface. The window has no translator of its own; whoever opens it looks the
//! words up (see `MovieWords::from_lookup`).

/// Placeholders in the clock text, replaced each frame by the time and the frame number.
const CLOCK: &str = "{clock}";
const FRAME: &str = "{frame}";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MovieWords {
    pub(crate) title: String,
    /// The clock line with `{clock}` and `{frame}` where the time and the frame number go.
    clock: String,
    pub(crate) restart: String,
    pub(crate) pause: String,
    pub(crate) play: String,
    pub(crate) step: String,
    pub(crate) speed: String,
    pub(crate) loop_on: String,
    pub(crate) loop_off: String,
    pub(crate) sound_off: String,
    pub(crate) sound_on: String,
    pub(crate) close: String,
}

impl MovieWords {
    /// The words in English, for a window opened without a translator.
    pub(crate) fn english() -> Self {
        let own = |text: &str| text.to_owned();
        Self {
            title: own("Test Movie"),
            clock: format!("{CLOCK} · frame {FRAME}"),
            restart: own("Restart"),
            pause: own("Pause"),
            play: own("Play"),
            step: own("Step one frame"),
            speed: own("Playback speed"),
            loop_on: own("Looping: the movie starts over at its end"),
            loop_off: own("Not looping: the movie stops at its last frame"),
            sound_off: own("Sound is off; click to turn it on"),
            sound_on: own("Sound is on; click to turn it off"),
            close: own("Close"),
        }
    }

    /// The words as `lookup` has them: it is given a message key and its arguments and returns
    /// the text in the interface's language.
    pub(crate) fn from_lookup(lookup: impl Fn(&str, &[(&str, &str)]) -> String) -> Self {
        let plain = |key: &str| lookup(key, &[]);
        Self {
            title: plain("movie-title"),
            clock: lookup("movie-clock", &[("clock", CLOCK), ("frame", FRAME)]),
            restart: plain("movie-restart"),
            pause: plain("movie-pause"),
            play: plain("movie-play"),
            step: plain("movie-step"),
            speed: plain("movie-speed"),
            loop_on: plain("movie-loop-on"),
            loop_off: plain("movie-loop-off"),
            sound_off: plain("movie-sound-off"),
            sound_on: plain("movie-sound-on"),
            close: plain("movie-close"),
        }
    }

    /// The clock line for movie time `clock` and frame number `frame`.
    pub(crate) fn clock_line(&self, clock: &str, frame: impl std::fmt::Display) -> String {
        self.clock
            .replace(CLOCK, clock)
            .replace(FRAME, &frame.to_string())
    }
}

#[cfg(test)]
#[path = "words_tests.rs"]
mod tests;
