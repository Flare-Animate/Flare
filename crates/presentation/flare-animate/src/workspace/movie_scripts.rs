//! Debug > Movie Scripts: a Flash movie's ActionScript 3 read in a window, with the classes
//! `flare scripts` and the MCP server list, one class written out, and the strings the code
//! carries. All of it is `flare_as3`; this holds what the window shows and asks it again when
//! the search changes. Nothing is run and nothing is written.

use flare_as3::{
    ClassSource, Movie, Outline, OutlineQuery, ScriptError, SourceOptions, StringQuery, Strings,
};

use super::*;

/// Classes listed at most, so an engine's thousands do not fill the window.
pub(super) const MAX_CLASSES: usize = 200;
/// Strings listed at most.
pub(super) const MAX_STRINGS: usize = 500;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum ScriptsTab {
    #[default]
    Classes,
    Strings,
}

/// What the window holds: the movie, what was asked of it and the answers.
pub(super) struct MovieScripts {
    pub(super) name: String,
    pub(super) movie: Movie,
    pub(super) tab: ScriptsTab,
    /// The search box's text, applied by Search or Enter.
    pub(super) find: String,
    /// List method bodies as instructions when a class is written out.
    pub(super) code: bool,
    /// Only strings the code uses as text.
    pub(super) text_only: bool,
    pub(super) outline: Outline,
    pub(super) strings: Strings,
    /// The class chosen, by full name, and what reading it gave.
    pub(super) chosen: Option<(String, Result<ClassSource, String>)>,
}

impl MovieScripts {
    pub(super) fn new(name: String, movie: Movie) -> Self {
        let mut window = Self {
            name,
            movie,
            tab: ScriptsTab::default(),
            find: String::new(),
            code: false,
            text_only: false,
            outline: Outline {
                blocks: Vec::new(),
                unreadable: Vec::new(),
                matched: 0,
                classes: Vec::new(),
            },
            strings: Strings {
                matched: 0,
                strings: Vec::new(),
                unreadable: Vec::new(),
            },
            chosen: None,
        };
        window.search();
        window
    }

    /// Lists the classes and strings that match the search box again.
    pub(super) fn search(&mut self) {
        let find = Some(self.find.trim().to_owned()).filter(|f| !f.is_empty());
        self.outline = flare_as3::outline(
            &self.movie,
            &OutlineQuery {
                find: find.clone(),
                limit: Some(MAX_CLASSES),
            },
        );
        self.strings = flare_as3::strings(
            &self.movie,
            &StringQuery {
                find,
                text_only: self.text_only,
                limit: Some(MAX_STRINGS),
            },
        );
    }

    /// Writes the class called `full_name` out (again, when `code` changed).
    pub(super) fn choose(&mut self, full_name: &str) {
        let options = SourceOptions {
            code: self.code,
            member: None,
        };
        let source = flare_as3::class_source(&self.movie, full_name, &options)
            .map_err(|error| error.to_string());
        self.chosen = Some((full_name.to_owned(), source));
    }
}

impl Workspace {
    /// Opens the window on a movie that was read, or says why it could not be.
    pub fn show_movie_scripts(&mut self, name: String, movie: Result<Movie, ScriptError>) {
        match movie {
            Ok(movie) => self.movie_scripts = Some(MovieScripts::new(name, movie)),
            Err(error) => self.report(tr!(
                self,
                "scripts-cannot-read",
                &[("name", &name), ("error", &error.to_string())]
            )),
        }
    }

    /// Whether the Movie Scripts window is open.
    pub fn movie_scripts_open(&self) -> bool {
        self.movie_scripts.is_some()
    }
}
