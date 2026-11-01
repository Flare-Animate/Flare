//! A scripted run of the real window, for `scripts/gui_tests.sh`: what only a window can show
//! (two windows, closing one of them, the editor drawing on afterwards) is exercised on timers,
//! with nobody at the mouse. The run fails if the app panics, hangs or exits with an error.

use std::time::Duration;

use flare_animate::{MenuAction, Workspace};
use winit::event_loop::EventLoopProxy;

use super::UserEvent;

/// One step of the run, sent to the event loop on its timer.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Step {
    /// Do what the menu item does.
    Menu(MenuAction),
    /// Close the Test Movie window as its Close button does.
    CloseMovie,
}

/// Opens a full-HD project and starts the timers: Test Movie, then its window closes, the editor is
/// asked to draw twice more, and the app quits.
pub(super) fn start(workspace: &mut Workspace, proxy: EventLoopProxy<UserEvent>) {
    // Full HD, so that the Test Movie window is big enough to cover the editor's own: a covered
    // window gets no surface texture, and the editor must go on drawing when it is uncovered.
    workspace.new_project(1920, 1080, 24);
    std::thread::spawn(move || {
        let script = [
            (2, Step::Menu(MenuAction::TestMovie)),
            (6, Step::CloseMovie),
            (1, Step::Menu(MenuAction::ZoomIn)),
            (1, Step::Menu(MenuAction::ZoomIn)),
            (1, Step::Menu(MenuAction::Quit)),
        ];
        for (wait, step) in script {
            std::thread::sleep(Duration::from_secs(wait));
            // A closed event loop means the app has already gone.
            if proxy.send_event(UserEvent::Autotest(step)).is_err() {
                return;
            }
        }
    });
}
