//! Setting text, on the thread that holds the font database.
//!
//! Text can name a Google Fonts family this machine lacks. Nothing here downloads: a text job
//! that needs one answers [`Done::FontsWanted`], the front end downloads the family its own way
//! and hands it back with [`TextJob::AddFonts`], and the job is then run. Setting text waits for
//! the fonts, so the shape that is committed is in the right font and is one undo step; a preview
//! does not wait, and the front end asks for it again when [`Done::FontsAdded`] comes (a preview
//! is answered under the number it was asked with, which the front end has already settled, so
//! the worker does not answer it twice). A family that could not be had is not asked
//! for again: text falls back to the next font in its list.

use std::collections::HashSet;

use crate::jobs::{Done, TextJob};

/// Fonts a front end wants known by name in addition to the system's (tests use this so they do
/// not depend on what a machine has installed).
pub type ExtraFonts = std::sync::Arc<Vec<Vec<u8>>>;

/// Everything the text thread keeps between jobs.
struct TextState {
    engine: flare_text::TextEngine,
    /// Families, lowercased, that could not be downloaded or were not their fonts.
    given_up: HashSet<String>,
    /// The newest text to set that waits for fonts.
    waiting_set: Option<TextJob>,
}

thread_local! {
    /// The font database of this thread, built the first time text is asked for.
    static TEXT: std::cell::RefCell<Option<TextState>> = const { std::cell::RefCell::new(None) };
}

/// Runs `work` with this thread's text state, built the first time it is needed.
fn with_state<T>(fonts: &ExtraFonts, work: impl FnOnce(&mut TextState) -> T) -> T {
    TEXT.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.get_or_insert_with(|| {
            let mut engine = flare_text::TextEngine::new();
            for font in fonts.iter() {
                engine.register_font(font.clone());
            }
            TextState {
                engine,
                given_up: HashSet::new(),
                waiting_set: None,
            }
        });
        work(state)
    })
}

impl TextState {
    /// The Google Fonts families `list` needs that are neither here nor given up on.
    fn wanted(&mut self, list: &str) -> Vec<String> {
        self.engine
            .missing_google_families(list)
            .into_iter()
            .map(|family| family.name().to_owned())
            .filter(|name| !self.given_up.contains(&name.to_lowercase()))
            .collect()
    }

    fn run(&mut self, job: TextJob) -> Vec<Done> {
        match job {
            TextJob::Set {
                id,
                source,
                shape,
                fill,
            } => {
                // A newer text replaces one still waiting, as the newest request always wins.
                self.waiting_set = None;
                let wanted = self.wanted(source.family());
                if !wanted.is_empty() {
                    self.waiting_set = Some(TextJob::Set {
                        id,
                        source,
                        shape,
                        fill,
                    });
                    return vec![Done::FontsWanted(wanted)];
                }
                let result = self
                    .engine
                    .shape(&source, shape, fill)
                    .map_err(|e| e.to_string());
                vec![Done::Text { id, result }]
            }
            TextJob::Families => vec![Done::Families(self.engine.families())],
            TextJob::Preview {
                id,
                text,
                style,
                origin,
                cursor,
                shape,
                fill,
            } => {
                let result = self
                    .engine
                    .preview(&text, &style, origin, cursor, shape, fill)
                    .map_err(|e| e.to_string());
                let wanted = self.wanted(&style.family);
                let mut done = vec![Done::TextPreview { id, result }];
                if !wanted.is_empty() {
                    done.push(Done::FontsWanted(wanted));
                }
                done
            }
            TextJob::AddFonts { family, fonts } => {
                let result = self.add(&family, fonts);
                if result.is_err() {
                    self.given_up.insert(family.to_lowercase());
                }
                let mut done = vec![Done::FontsAdded { family, result }];
                if let Some(waiting) = self.waiting_set.take() {
                    done.extend(self.run(waiting));
                }
                done
            }
        }
    }

    fn add(&mut self, family: &str, fonts: Result<Vec<Vec<u8>>, String>) -> Result<(), String> {
        let files = fonts?;
        let known = flare_text::google::family_named(family)
            .ok_or_else(|| format!("{family} is not a Google Fonts family"))?;
        self.engine
            .add_google_family(known, files)
            .map_err(|e| e.to_string())
    }
}

/// Runs one text job, with the answers it gives (a job waiting for fonts gives its answer when
/// they arrive, after the [`Done::FontsAdded`] of the last one).
pub fn run_text(job: TextJob, fonts: &ExtraFonts) -> Vec<Done> {
    with_state(fonts, |state| state.run(job))
}

/// The shape `source` sets, with identity `shape` and `fill`. The first call on a thread builds
/// the font database, which scans the system's fonts. A Google Fonts family the source names is
/// used if it has been added ([`add_google_fonts`]); otherwise the next family in the list is.
pub fn text_shape(
    source: &flare_document::TextSource,
    shape: flare_core::ShapeId,
    fill: flare_core::Rgba8,
    fonts: &ExtraFonts,
) -> Result<flare_document::Shape, String> {
    with_state(fonts, |state| {
        state
            .engine
            .shape(source, shape, fill)
            .map_err(|e| e.to_string())
    })
}

/// The font families there are to set text in, sorted: the system's, the ones added, and those
/// downloaded so far. The Google Fonts families that can be downloaded are
/// [`google_font_families`].
pub fn font_families(fonts: &ExtraFonts) -> Vec<String> {
    with_state(fonts, |state| state.engine.families())
}

/// The Google Fonts families that can be downloaded and are not here, by name.
pub fn google_font_families(fonts: &ExtraFonts) -> Vec<&'static str> {
    with_state(fonts, |state| {
        flare_text::google::families()
            .iter()
            .map(flare_text::GoogleFamily::name)
            .filter(|name| !state.engine.has_family(name))
            .collect()
    })
}

/// For a program with no window: downloads, with `fetch`, each Google Fonts family the family
/// list `list` needs and this thread lacks, and adds it, so [`text_shape`] sets text in it. A
/// family that cannot be had is an error naming it, rather than text in another font.
pub fn add_google_fonts(
    list: &str,
    fonts: &ExtraFonts,
    fetch: impl Fn(&str) -> Result<Vec<Vec<u8>>, String>,
) -> Result<(), String> {
    with_state(fonts, |state| {
        for name in state.wanted(list) {
            let files =
                fetch(&name).map_err(|e| format!("could not get {name} from Google Fonts: {e}"))?;
            state
                .add(&name, Ok(files))
                .map_err(|e| format!("could not use {name} from Google Fonts: {e}"))?;
        }
        Ok(())
    })
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
