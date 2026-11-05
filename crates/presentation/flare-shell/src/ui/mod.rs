//! The desktop window: one winit event loop, egui-winit input, egui-wgpu painting, and the
//! native menu bar. What the workspace does is `flare-animate`'s business and what the operating
//! system provides is `flare-platform`'s; this folder only puts the workspace in a window.

mod app;
#[cfg(feature = "autotest")]
mod autotest;
mod frame;
#[cfg(test)]
mod frame_tests;
mod menu;
#[cfg(feature = "ruffle")]
mod movie;
#[cfg(feature = "ruffle")]
mod movie_window;
mod shell;
#[cfg(all(unix, not(target_os = "ios")))]
mod tablet;
mod window;

/// The Test Movie window; only builds with the player inside have one.
#[cfg(feature = "ruffle")]
pub(crate) type Movie = Option<movie_window::MovieViewport>;

use anyhow::{Context, Result};
use flare_animate::Workspace;
use winit::event_loop::{ControlFlow, EventLoop};

/// Events sent to the event loop from outside it (native menu clicks arrive on AppKit's side,
/// background work finishes on its own thread).
pub(crate) enum UserEvent {
    #[cfg(target_os = "macos")]
    Menu(flare_menubar::MenuAction),
    /// A test movie finished starting (or failed to) on its own thread, in a Ruffle program of
    /// its own; the in-window player has `MovieBytes` instead.
    #[cfg(not(feature = "ruffle"))]
    MovieStarted(Result<flare_platform::jobs::TestMovie, flare_platform::jobs::TestMovieError>),
    /// The movie for the in-window player is published (or could not be).
    #[cfg(feature = "ruffle")]
    MovieBytes(Result<Vec<u8>, String>),
    /// A step of the scripted run (`autotest` builds only).
    #[cfg(feature = "autotest")]
    Autotest(autotest::Step),
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
    #[cfg(target_os = "linux")]
    tablet::prefer_x11(&mut builder);
    let event_loop = builder.build().context("create event loop")?;
    drive(event_loop, workspace)
}

/// The same shell on Android: the event loop is the activity's own (winit's android backend),
/// and everything after that - the input, the painting, the workspace in its window - is the
/// desktop's, unchanged. It is here only on the android target, so a desktop build compiles
/// no phone code and a phone build never opens a desktop event loop.
#[cfg(target_os = "android")]
pub fn run_on_activity(
    workspace: Workspace,
    android_app: winit::platform::android::activity::AndroidApp,
) -> Result<()> {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .with_android_app(android_app)
        .build()
        .context("create event loop")?;
    drive(event_loop, workspace)
}

/// Puts the workspace in the window and runs until it is closed or fails.
fn drive(event_loop: EventLoop<UserEvent>, workspace: Workspace) -> Result<()> {
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = app::App::new(workspace, event_loop.create_proxy());
    event_loop.run_app(&mut app).context("event loop")?;
    match app.take_fatal() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
