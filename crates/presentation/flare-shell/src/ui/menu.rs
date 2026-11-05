//! Native menu bar. The structure comes from `flare_menubar::MENU_LAYOUT` and the behaviour
//! from `Workspace::{is_enabled, label, perform}`, so this file only adds what is specific
//! to a native menu: accelerators, the app and Window menus, and syncing item state.

#[cfg(any(target_os = "macos", test))]
use flare_menubar::MenuAction;

/// The accelerator for an action, in muda's syntax. `None` means no shortcut.
///
/// Single-letter shortcuts are avoided on purpose: a native menu would intercept them
/// before egui, breaking typing into text fields. Delete and Backspace are handled by the
/// workspace for the same reason.
#[cfg(any(target_os = "macos", test))]
pub fn accelerator(action: MenuAction) -> Option<&'static str> {
    action.shortcut()
}

/// The accelerator in force for an action, from the workspace's shortcut map. Tools are left out:
/// their bare letters are handled by the workspace, never by the native menu.
#[cfg(any(target_os = "macos", test))]
pub fn bound_accelerator(
    workspace: &flare_animate::Workspace,
    action: MenuAction,
) -> Option<String> {
    if action.tool_key().is_some() {
        return None;
    }
    workspace
        .shortcuts()
        .get(action)
        .map(flare_menubar::Accelerator::muda_text)
}

#[cfg(target_os = "macos")]
pub use native::NativeMenu;

#[cfg(target_os = "macos")]
mod native {
    use std::str::FromStr;

    use anyhow::{Context, Result};
    use flare_animate::Workspace;
    use flare_menubar::{MENU_LAYOUT, MenuAction, MenuEntry};
    use muda::accelerator::Accelerator;
    use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

    use super::{accelerator, bound_accelerator};

    /// The macOS menu bar and the items whose state follows the workspace.
    pub struct NativeMenu {
        menu: Menu,
        window_menu: Submenu,
        items: Vec<(MenuAction, MenuItem)>,
        /// The accelerator each item shows now, in step with `items`, so a rebinding is applied
        /// once and an idle pass sets nothing.
        accelerators: std::cell::RefCell<Vec<Option<String>>>,
        /// What the other items say, kept with the English name the layout knows them by so they
        /// can be said again when the language changes.
        names: Names,
    }

    /// Menus, menus inside menus, planned commands and the system's own items: everything whose
    /// words are not an action's, with the key or English name to translate.
    #[derive(Default)]
    struct Names {
        titles: Vec<(String, Submenu)>,
        submenus: Vec<(String, Submenu)>,
        mocks: Vec<(String, MenuItem)>,
        system: Vec<(&'static str, PredefinedMenuItem)>,
    }

    impl Names {
        fn apply(&self, workspace: &Workspace) {
            for (english, submenu) in &self.titles {
                set_submenu_text(submenu, workspace.menu_title(english));
            }
            for (english, submenu) in &self.submenus {
                set_submenu_text(submenu, workspace.submenu_title(english));
            }
            for (english, item) in &self.mocks {
                let text = workspace.mock_label(english);
                if item.text() != text {
                    item.set_text(text);
                }
            }
            for (name, item) in &self.system {
                let text = workspace.system_menu_text(name);
                if item.text() != text {
                    item.set_text(text);
                }
            }
        }
    }

    fn set_submenu_text(submenu: &Submenu, text: String) {
        if submenu.text() != text {
            submenu.set_text(text);
        }
    }

    fn item(action: MenuAction, label: &str) -> Result<MenuItem> {
        let accelerator = accelerator(action)
            .map(Accelerator::from_str)
            .transpose()
            .with_context(|| format!("bad accelerator for {action:?}"))?;
        Ok(MenuItem::with_id(action.id(), label, true, accelerator))
    }

    /// Adds `entries` to `submenu`, opening each menu inside it as a submenu of its own.
    fn append_entries(
        submenu: &Submenu,
        entries: &[MenuEntry],
        workspace: &Workspace,
        items: &mut Vec<(MenuAction, MenuItem)>,
        names: &mut Names,
    ) -> Result<()> {
        for entry in entries {
            match entry {
                // These live in the application menu on macOS.
                MenuEntry::Action(
                    MenuAction::Quit | MenuAction::Preferences | MenuAction::About,
                ) => {}
                MenuEntry::Action(action) => {
                    let it = item(*action, &workspace.label(*action))?;
                    submenu.append(&it)?;
                    items.push((*action, it));
                }
                MenuEntry::Separator => submenu.append(&PredefinedMenuItem::separator())?,
                // A planned command: present, dimmed, and inert.
                MenuEntry::Mock(label) => {
                    let it = MenuItem::new(workspace.mock_label(label), false, None);
                    submenu.append(&it)?;
                    names.mocks.push(((*label).to_owned(), it));
                }
                MenuEntry::Submenu(title, inner) => {
                    let nested = Submenu::new(workspace.submenu_title(title), true);
                    append_entries(&nested, inner, workspace, items, names)?;
                    submenu.append(&nested)?;
                    names.submenus.push(((*title).to_owned(), nested));
                }
            }
        }
        Ok(())
    }

    impl NativeMenu {
        pub fn build(workspace: &Workspace) -> Result<Self> {
            let menu = Menu::new();
            let mut items: Vec<(MenuAction, MenuItem)> = Vec::new();
            let mut names = Names::default();
            let system = |name: &str| workspace.system_menu_text(name);

            // The application menu (named after the app) holds About, Preferences, Hide and Quit.
            // About is Flare's own window, which has the links; the system's panel has none.
            let app = Submenu::new("Flare", true);
            let about = item(MenuAction::About, &workspace.label(MenuAction::About))?;
            let preferences = item(
                MenuAction::Preferences,
                &workspace.label(MenuAction::Preferences),
            )?;
            let quit = item(MenuAction::Quit, &workspace.label(MenuAction::Quit))?;
            let services = PredefinedMenuItem::services(Some(&system("services")));
            let hide = PredefinedMenuItem::hide(Some(&system("hide")));
            let hide_others = PredefinedMenuItem::hide_others(Some(&system("hide-others")));
            let show_all = PredefinedMenuItem::show_all(Some(&system("show-all")));
            app.append_items(&[
                &about,
                &PredefinedMenuItem::separator(),
                &preferences,
                &PredefinedMenuItem::separator(),
                &services,
                &PredefinedMenuItem::separator(),
                &hide,
                &hide_others,
                &show_all,
                &PredefinedMenuItem::separator(),
                &quit,
            ])?;
            names.system.extend([
                ("services", services),
                ("hide", hide),
                ("hide-others", hide_others),
                ("show-all", show_all),
            ]);
            items.push((MenuAction::About, about));
            items.push((MenuAction::Preferences, preferences));
            items.push((MenuAction::Quit, quit));
            menu.append(&app)?;

            // The system's Window menu (minimize, full screen) also carries Flare's own panel
            // entries, so there is one Window menu, not two.
            let window_menu = Submenu::new(workspace.menu_title("Window"), true);
            for (title, entries) in MENU_LAYOUT {
                let is_window = *title == "Window";
                let submenu = if is_window {
                    window_menu.clone()
                } else {
                    Submenu::new(workspace.menu_title(title), true)
                };
                append_entries(&submenu, entries, workspace, &mut items, &mut names)?;
                if is_window {
                    let minimize = PredefinedMenuItem::minimize(Some(&system("minimize")));
                    let fullscreen = PredefinedMenuItem::fullscreen(Some(&system("fullscreen")));
                    let close = PredefinedMenuItem::close_window(Some(&system("close-window")));
                    window_menu.append_items(&[
                        &PredefinedMenuItem::separator(),
                        &minimize,
                        &fullscreen,
                        &PredefinedMenuItem::separator(),
                        &close,
                    ])?;
                    names.system.extend([
                        ("minimize", minimize),
                        ("fullscreen", fullscreen),
                        ("close-window", close),
                    ]);
                }
                names.titles.push(((*title).to_owned(), submenu.clone()));
                // Window is placed where the layout puts it, before Help, as in Animate.
                menu.append(&submenu)?;
            }
            Ok(Self {
                menu,
                window_menu,
                accelerators: std::cell::RefCell::new(
                    items
                        .iter()
                        .map(|(a, _)| accelerator(*a).map(str::to_owned))
                        .collect(),
                ),
                items,
                names,
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
            let mut shown = self.accelerators.borrow_mut();
            for (index, (action, it)) in self.items.iter().enumerate() {
                // A rebinding in Help > Keyboard Shortcuts moves the chord the menu owns.
                let wanted = bound_accelerator(workspace, *action);
                if shown.get(index) != Some(&wanted) {
                    let parsed = wanted.as_deref().map(Accelerator::from_str).transpose();
                    if let Ok(parsed) = parsed
                        && it.set_accelerator(parsed).is_ok()
                        && let Some(slot) = shown.get_mut(index)
                    {
                        *slot = wanted;
                    }
                }
                let enabled = workspace.is_enabled(*action);
                if it.is_enabled() != enabled {
                    it.set_enabled(enabled);
                }
                let label = workspace.label(*action);
                if it.text() != label {
                    it.set_text(label);
                }
            }
            // The titles and the system's items are said again only when they differ, which is
            // only after the language changed, so the menu is relabelled in place, not rebuilt.
            self.names.apply(workspace);
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

    #[test]
    #[allow(clippy::expect_used)]
    fn the_native_menu_registers_what_the_shortcut_map_says() {
        let mut workspace = flare_animate::Workspace::untitled().expect("a new workspace");
        // The built-in set is the table the menu was always built from.
        for action in MenuAction::ALL {
            assert_eq!(
                bound_accelerator(&workspace, action).as_deref(),
                accelerator(action),
                "{action:?}"
            );
        }
        workspace
            .set_shortcut(MenuAction::Undo, "CmdOrCtrl+U")
            .expect("a free chord");
        assert_eq!(
            bound_accelerator(&workspace, MenuAction::Undo).as_deref(),
            Some("CmdOrCtrl+U")
        );
        workspace.clear_shortcut(MenuAction::Save);
        assert_eq!(bound_accelerator(&workspace, MenuAction::Save), None);
        // A tool key is never given to the native menu.
        assert_eq!(bound_accelerator(&workspace, MenuAction::ToolSelect), None);
    }
}
