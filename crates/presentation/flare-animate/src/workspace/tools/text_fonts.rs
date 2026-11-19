//! The Google Fonts families text names and this machine lacks: the font worker asks for them,
//! the shell downloads them its own way (a thread on the desktop, `fetch` in a browser) and hands
//! them back, and the text waiting for them is set.

use flare_runtime::TextJob;

use crate::workspace::Workspace;

/// Most families waiting to be downloaded at once; text names few, and more is a runaway.
const MAX_DOWNLOADS: usize = 16;

impl Workspace {
    /// The families there are arrived: those here, and so the Google Fonts ones that are not.
    pub(in crate::workspace) fn families_arrived(&mut self, families: Vec<String>) {
        let here: std::collections::HashSet<String> =
            families.iter().map(|name| name.to_lowercase()).collect();
        self.text.google = flare_text::google::families()
            .iter()
            .map(flare_text::GoogleFamily::name)
            .filter(|name| !here.contains(&name.to_lowercase()))
            .collect();
        self.text.families = families;
    }

    /// Text names families to download. Each is asked of the shell once while it downloads.
    pub(in crate::workspace) fn fonts_wanted(&mut self, families: Vec<String>) {
        for family in families {
            let key = family.to_lowercase();
            if self.text.downloading.contains(&key) {
                continue;
            }
            if self.text.downloading.len() >= MAX_DOWNLOADS {
                // Answered at once, so text waiting for it falls back instead of waiting forever.
                self.font_downloaded(family, Err("too many fonts are downloading".into()));
                continue;
            }
            self.text.downloading.insert(key);
            let message = tr!(self, "msg-downloading-font", &[("family", &family)]);
            self.report(message);
            self.text.downloads.push(family);
        }
    }

    /// A family was added to the font worker, or could not be.
    pub(in crate::workspace) fn fonts_added(&mut self, family: String, result: Result<(), String>) {
        self.text.downloading.remove(&family.to_lowercase());
        // The text being typed is asked for again on the next frame, now in the family (or, if
        // it could not be had, in the one after it, for good).
        self.text.preview_asked = None;
        match result {
            Ok(()) => {
                let message = tr!(self, "msg-font-downloaded", &[("family", &family)]);
                self.report(message);
                // It is here now: list it with the others.
                self.text.families_asked = false;
                self.ask_font_families();
            }
            Err(reason) => {
                let message = tr!(
                    self,
                    "msg-cannot-get-font",
                    &[("family", &family), ("reason", &reason)]
                );
                self.report(message);
            }
        }
    }

    /// Returns (and clears) the Google Fonts families the shell should download, by name;
    /// `flare_text::google::family_named` says where their files are. Each is answered with
    /// [`Workspace::font_downloaded`].
    pub fn take_font_downloads(&mut self) -> Vec<String> {
        std::mem::take(&mut self.text.downloads)
    }

    /// The files of `family` as the shell downloaded them, or why it could not. The text that
    /// waited for them is then set, in them or in the next family of its list.
    pub fn font_downloaded(&mut self, family: String, fonts: Result<Vec<Vec<u8>>, String>) {
        self.background
            .submit_text(TextJob::AddFonts { family, fonts });
        self.settle_inline();
    }
}
