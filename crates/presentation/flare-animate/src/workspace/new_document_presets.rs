//! The kinds of project the New Document dialog offers, each with sizes common for it, as
//! Animate's dialog groups them. The sizes are the usual ones of each medium (HD video, square
//! posts, the standard ad units), Flare's own choice, not a copy of Animate's list.

/// A size a project can start at. `name` is the message that names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Preset {
    pub(super) name: &'static str,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) fps: u32,
}

/// A kind of project and its sizes. `name` is the message that names it, `icon` its tab's icon.
#[derive(Clone, Copy, Debug)]
pub(super) struct Category {
    pub(super) name: &'static str,
    pub(super) icon: &'static str,
    pub(super) presets: &'static [Preset],
}

const fn p(name: &'static str, width: u32, height: u32, fps: u32) -> Preset {
    Preset {
        name,
        width,
        height,
        fps,
    }
}

const CHARACTER: &[Preset] = &[
    p("start-size-full-hd", 1920, 1080, 24),
    p("start-size-hd", 1280, 720, 24),
    p("start-size-4k", 3840, 2160, 24),
    p("start-size-standard", 640, 480, 24),
];
const SOCIAL: &[Preset] = &[
    p("start-size-square", 1080, 1080, 30),
    p("start-preset-portrait", 1080, 1920, 30),
    p("start-size-landscape", 1920, 1080, 30),
];
const GAME: &[Preset] = &[
    p("start-size-hd", 1280, 720, 60),
    p("start-size-full-hd", 1920, 1080, 60),
    p("start-size-phone", 720, 1280, 60),
];
const EDUCATION: &[Preset] = &[
    p("start-size-hd", 1280, 720, 24),
    p("start-size-standard", 1024, 768, 24),
];
const ADS: &[Preset] = &[
    p("start-size-medium-rectangle", 300, 250, 24),
    p("start-size-leaderboard", 728, 90, 24),
    p("start-size-skyscraper", 160, 600, 24),
];
const WEB: &[Preset] = &[
    p("start-size-hd", 1280, 720, 24),
    p("start-size-full-hd", 1920, 1080, 24),
    p("start-preset-classic", 550, 400, 24),
];
const ADVANCED: &[Preset] = &[p("start-preset-classic", 550, 400, 24)];

/// Every kind, in the order of the dialog's tabs.
pub(super) const CATEGORIES: [Category; 7] = [
    Category {
        name: "start-preset-character",
        icon: "new-character",
        presets: CHARACTER,
    },
    Category {
        name: "start-preset-social",
        icon: "new-social",
        presets: SOCIAL,
    },
    Category {
        name: "start-cat-game",
        icon: "new-game",
        presets: GAME,
    },
    Category {
        name: "start-cat-education",
        icon: "new-education",
        presets: EDUCATION,
    },
    Category {
        name: "start-cat-ads",
        icon: "new-ads",
        presets: ADS,
    },
    Category {
        name: "start-preset-web",
        icon: "new-web",
        presets: WEB,
    },
    Category {
        name: "start-cat-advanced",
        icon: "new-advanced",
        presets: ADVANCED,
    },
];

/// The kind at `index`, the first for one out of range.
pub(super) fn category(index: usize) -> &'static Category {
    CATEGORIES.get(index).unwrap_or(&CATEGORIES[0])
}

/// Which of `category`'s presets has exactly these numbers, if one has.
pub(super) fn matching(category: &Category, width: u32, height: u32, fps: u32) -> Option<usize> {
    category
        .presets
        .iter()
        .position(|p| (p.width, p.height, p.fps) == (width, height, fps))
}

#[cfg(test)]
#[path = "new_document_presets_tests.rs"]
mod tests;
