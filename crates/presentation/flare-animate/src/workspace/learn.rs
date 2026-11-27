//! The Learn page: short, hands-on lessons on the Home page, and the first-run welcome that
//! points at them. A lesson names its steps and opens the tool or panel it teaches, so a person
//! can follow it in the workspace rather than read about it.

use egui::{Align, Layout, RichText, Ui};
use flare_menubar::MenuAction;
use flare_ui::theme;

use super::home::section_title;
use super::*;

/// Which page of the Home area is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HomeTab {
    #[default]
    Home,
    Learn,
}

/// One lesson: its name and summary, the steps in the message files, and what trying it opens.
pub(super) struct Lesson {
    pub(super) id: &'static str,
    pub(super) title: &'static str,
    pub(super) summary: &'static str,
    pub(super) steps: &'static [&'static str],
    pub(super) action: MenuAction,
}

/// The lessons, in the order a new person meets them.
pub(super) const LESSONS: [Lesson; 7] = [
    Lesson {
        id: "draw",
        title: "learn-draw-title",
        summary: "learn-draw-summary",
        steps: &[
            "learn-draw-step-1",
            "learn-draw-step-2",
            "learn-draw-step-3",
        ],
        action: MenuAction::ToolPencil,
    },
    Lesson {
        id: "frames",
        title: "learn-frames-title",
        summary: "learn-frames-summary",
        steps: &[
            "learn-frames-step-1",
            "learn-frames-step-2",
            "learn-frames-step-3",
        ],
        action: MenuAction::ShowTimeline,
    },
    Lesson {
        id: "symbols",
        title: "learn-symbols-title",
        summary: "learn-symbols-summary",
        steps: &[
            "learn-symbols-step-1",
            "learn-symbols-step-2",
            "learn-symbols-step-3",
        ],
        action: MenuAction::NewSymbol,
    },
    Lesson {
        id: "animate",
        title: "learn-animate-title",
        summary: "learn-animate-summary",
        steps: &[
            "learn-animate-step-1",
            "learn-animate-step-2",
            "learn-animate-step-3",
        ],
        action: MenuAction::ToggleMotionTween,
    },
    Lesson {
        id: "rig",
        title: "learn-rig-title",
        summary: "learn-rig-summary",
        steps: &["learn-rig-step-1", "learn-rig-step-2", "learn-rig-step-3"],
        action: MenuAction::ShowRigs,
    },
    Lesson {
        id: "effects",
        title: "learn-effects-title",
        summary: "learn-effects-summary",
        steps: &[
            "learn-effects-step-1",
            "learn-effects-step-2",
            "learn-effects-step-3",
        ],
        action: MenuAction::ShowEffects,
    },
    Lesson {
        id: "publish",
        title: "learn-publish-title",
        summary: "learn-publish-summary",
        steps: &[
            "learn-publish-step-1",
            "learn-publish-step-2",
            "learn-publish-step-3",
        ],
        action: MenuAction::Publish,
    },
];

impl Workspace {
    /// Help > Learn: the Home page on its lessons.
    pub fn show_learn(&mut self) {
        self.set_home(true);
        self.home.tab = HomeTab::Learn;
    }

    /// The Learn page, in the Home area's main column.
    pub(super) fn learn_page(&mut self, ui: &mut Ui) {
        section_title(ui, &self.tr("learn-title"));
        ui.label(
            RichText::new(self.tr("learn-intro"))
                .color(theme::text_dim())
                .size(13.0),
        );
        ui.add_space(12.0);
        if !self.home.welcome_seen {
            self.welcome_card(ui);
        }
        for lesson in &LESSONS {
            self.lesson_card(ui, lesson);
            ui.add_space(10.0);
        }
    }

    /// The first-run welcome, on the Home page and on the Learn page until it is dismissed.
    pub(super) fn welcome_card(&mut self, ui: &mut Ui) {
        let mut tour = false;
        let mut skip = false;
        egui::Frame::group(ui.style())
            .fill(theme::panel_raised())
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(self.tr("learn-welcome-title"))
                        .strong()
                        .size(18.0),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(self.tr("learn-welcome-body"))
                        .color(theme::text_dim())
                        .size(13.0),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(self.tr("learn-take-tour")).clicked() {
                        tour = true;
                    }
                    if ui.button(self.tr("learn-skip")).clicked() {
                        skip = true;
                    }
                });
            });
        if tour {
            self.home.tab = HomeTab::Learn;
        }
        if skip {
            self.dismiss_welcome();
        }
    }

    /// One lesson as a card: its name, its summary, the numbered steps and the button that
    /// closes Home and opens what it teaches.
    fn lesson_card(&mut self, ui: &mut Ui, lesson: &Lesson) {
        let title = self.tr(lesson.title);
        let summary = self.tr(lesson.summary);
        let mut try_it = false;
        egui::Frame::group(ui.style())
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&title).strong().size(15.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(self.tr("learn-try")).clicked() {
                            try_it = true;
                        }
                    });
                });
                ui.label(RichText::new(&summary).color(theme::text_dim()).size(13.0));
                ui.add_space(6.0);
                for (index, step) in lesson.steps.iter().enumerate() {
                    ui.label(format!("{}. {}", index + 1, self.tr(step)));
                }
            });
        if try_it {
            self.try_lesson(lesson);
        }
    }

    /// Follows a lesson: Home goes away, and the tool, panel or window it teaches opens.
    pub(super) fn try_lesson(&mut self, lesson: &Lesson) {
        self.home.welcome_seen = true;
        self.set_home(false);
        self.perform(lesson.action);
    }

    /// The welcome has been read; it is not shown again.
    pub(super) fn dismiss_welcome(&mut self) {
        self.home.welcome_seen = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_lesson_has_a_name_steps_and_an_action() {
        assert!(LESSONS.len() >= 5);
        let mut ids = std::collections::HashSet::new();
        for lesson in &LESSONS {
            assert!(ids.insert(lesson.id), "{} appears twice", lesson.id);
            assert!(lesson.title.starts_with("learn-"));
            assert!(lesson.summary.starts_with("learn-"));
            assert!(!lesson.steps.is_empty(), "{} has no steps", lesson.id);
            for step in lesson.steps {
                assert!(step.starts_with("learn-"));
            }
        }
    }

    #[test]
    fn the_lessons_cover_the_workspace() {
        let actions: Vec<MenuAction> = LESSONS.iter().map(|lesson| lesson.action).collect();
        for wanted in [
            MenuAction::ToolPencil,
            MenuAction::ShowTimeline,
            MenuAction::ShowRigs,
            MenuAction::ShowEffects,
            MenuAction::Publish,
        ] {
            assert!(actions.contains(&wanted), "{wanted:?} has no lesson");
        }
    }
}
