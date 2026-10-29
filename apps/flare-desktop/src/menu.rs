//! Native menu bar. The structure comes from `flare_animate::MENU_LAYOUT` and the behaviour
//! from `Workspace::{is_enabled, label, perform}`, so this file only adds what is specific
//! to a native menu: accelerators, the app and Window menus, and syncing item state.

use flare_animate::MenuAction;

/// The accelerator for an action, in muda's syntax. `None` means no shortcut.
///
/// Single-letter shortcuts are avoided on purpose: a native menu would intercept them
/// before egui, breaking typing into text fields. Delete and Backspace are handled by the
/// workspace for the same reason.
pub fn accelerator(action: MenuAction) -> Option<&'static str> {
    action.shortcut()
}

#[cfg(target_os = "macos")]
pub use native::NativeMenu;

#[cfg(target_os = "macos")]
mod native {
    use std::str::FromStr;

    use anyhow::{Context, Result};
    use flare_animate::{MENU_LAYOUT, MenuAction, MenuEntry, Workspace};
    use muda::accelerator::Accelerator;
    use muda::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

    use super::accelerator;

    /// The macOS menu bar and the items whose state follows the workspace.
    pub struct NativeMenu {
        menu: Menu,
        window_menu: Submenu,
        items: Vec<(MenuAction, MenuItem)>,
    }

    fn item(action: MenuAction, label: &str) -> Result<MenuItem> {
        let accelerator = accelerator(action)
            .map(Accelerator::from_str)
            .transpose()
            .with_context(|| format!("bad accelerator for {action:?}"))?;
        Ok(MenuItem::with_id(action.id(), label, true, accelerator))
    }

    impl NativeMenu {
        pub fn build(workspace: &Workspace) -> Result<Self> {
            let menu = Menu::new();
            let mut items: Vec<(MenuAction, MenuItem)> = Vec::new();

            // The application menu (named after the app) holds About, Hide and Quit.
            let app = Submenu::new("Flare", true);
            let about = AboutMetadata {
                name: Some("Flare".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                ..AboutMetadata::default()
            };
            let quit = item(MenuAction::Quit, &workspace.label(MenuAction::Quit))?;
            app.append_items(&[
                &PredefinedMenuItem::about(None, Some(about)),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &quit,
            ])?;
            items.push((MenuAction::Quit, quit));
            menu.append(&app)?;

            // The system's Window menu (minimize, full screen) also carries Flare's own panel
            // entries, so there is one Window menu, not two.
            let window_menu = Submenu::new("Window", true);
            for (title, entries) in MENU_LAYOUT {
                let submenu = if *title == "Window" {
                    window_menu.clone()
                } else {
                    Submenu::new(*title, true)
                };
                for entry in *entries {
                    match entry {
                        // Quit lives in the application menu on macOS.
                        MenuEntry::Action(MenuAction::Quit) => {}
                        MenuEntry::Action(action) => {
                            let it = item(*action, &workspace.label(*action))?;
                            submenu.append(&it)?;
                            items.push((*action, it));
                        }
                        MenuEntry::Separator => submenu.append(&PredefinedMenuItem::separator())?,
                        // A planned command: present, dimmed, and inert.
                        MenuEntry::Mock(label) => {
                            submenu.append(&MenuItem::new(*label, false, None))?;
                        }
                    }
                }
                if *title != "Window" {
                    menu.append(&submenu)?;
                }
            }
            window_menu.append_items(&[
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::minimize(None),
                &PredefinedMenuItem::fullscreen(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::close_window(None),
            ])?;
            menu.append(&window_menu)?;
            Ok(Self {
                menu,
                window_menu,
                items,
            })
        }

        /// Installs the menu on the running application. Call once the event loop is
        /// running (AppKit must be initialized).
        pub fn install(&self) {
            self.menu.init_for_nsapp();
            self.window_menu.set_as_windows_menu_for_nsapp();
        }

        /// Routes menu clicks to `send` (called on whichever thread AppKit uses).
        pub fn on_action(send: impl Fn(MenuAction) + Send + Sync + 'static) {
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                if let Some(action) = MenuAction::from_id(event.id().as_ref()) {
                    send(action);
                }
            }));
        }

        /// Makes enabled state and labels match the workspace. Cheap; call after each pass.
        pub fn sync(&self, workspace: &Workspace) {
            for (action, it) in &self.items {
                let enabled = workspace.is_enabled(*action);
                if it.is_enabled() != enabled {
                    it.set_enabled(enabled);
                }
                let label = workspace.label(*action);
                if it.text() != label {
                    it.set_text(label);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn accelerators_are_unique() {
        let mut seen = HashSet::new();
        for action in MenuAction::ALL {
            if let Some(accelerator) = accelerator(action) {
                assert!(seen.insert(accelerator), "{accelerator} is used twice");
            }
        }
    }

    #[test]
    fn no_action_takes_a_bare_letter_that_would_break_text_entry() {
        for action in MenuAction::ALL {
            if let Some(accelerator) = accelerator(action) {
                let key = accelerator.rsplit('+').next().unwrap_or_default();
                let bare = !accelerator.contains('+');
                let is_letter = key.len() == 1 && key.chars().all(|c| c.is_ascii_alphabetic());
                assert!(!(bare && is_letter), "{action:?}: {accelerator}");
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn every_accelerator_parses_as_a_muda_accelerator() {
        use std::str::FromStr;
        for action in MenuAction::ALL {
            if let Some(text) = accelerator(action) {
                assert!(
                    muda::accelerator::Accelerator::from_str(text).is_ok(),
                    "{action:?}: {text}"
                );
            }
        }
    }
}
