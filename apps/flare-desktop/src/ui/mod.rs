//! The desktop window: one winit event loop, egui-winit input, egui-wgpu painting, and the
//! native menu bar. What the workspace does is `flare-animate`'s business and what the operating
//! system provides is `flare-platform`'s; this folder only puts the workspace in a window.

mod app;
mod menu;
mod shell;
mod window;

use anyhow::{Context, Result};
use flare_animate::{MenuAction, Workspace};
use winit::event_loop::{ControlFlow, EventLoop};

/// Events sent to the event loop from outside it (native menu clicks arrive on AppKit's side,
/// background work finishes on its own thread).
pub(crate) enum UserEvent {
    Menu(MenuAction),
    /// A test movie finished starting (or failed to) on its own thread.
    MovieStarted(Result<flare_platform::jobs::TestMovie, flare_platform::jobs::TestMovieError>),
    /// An export or publish finished on its own thread; an error is worded for the person.
    Exported(Result<(), String>),
}

/// Runs the window until it is closed.
pub fn run(workspace: Workspace) -> Result<()> {
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    // The native menu replaces winit's built-in default one.
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_default_menu(false);
    }
    let event_loop = builder.build().context("create event loop")?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = app::App::new(workspace, event_loop.create_proxy());
    event_loop.run_app(&mut app).context("event loop")?;
    match app.take_fatal() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
