//! The application: owns the workspace and the window, and routes events between them.

use anyhow::Result;
use flare_animate::Workspace;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::window::WindowId;

use super::UserEvent;
use super::window::Gpu;
use flare_platform::dialogs;

pub(super) struct App {
    /// Test movies still playing; dropping one stops its player and removes its files.
    pub(super) movies: Vec<flare_platform::jobs::TestMovie>,
    /// The sound card, if there is one. Sounds are silent without it; nothing else changes.
    player: Option<flare_platform::audio::Player>,
    pub(super) proxy: EventLoopProxy<UserEvent>,
    #[cfg(target_os = "macos")]
    menu: Option<super::menu::NativeMenu>,
    ctx: egui::Context,
    pub(super) workspace: Workspace,
    gpu: Option<Gpu>,
    fatal: Option<anyhow::Error>,
}

impl App {
    pub(super) fn new(workspace: Workspace, proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            movies: Vec::new(),
            player: None,
            proxy,
            #[cfg(target_os = "macos")]
            menu: None,
            ctx: egui::Context::default(),
            workspace,
            gpu: None,
            fatal: None,
        }
    }

    pub(super) fn take_fatal(&mut self) -> Option<anyhow::Error> {
        self.fatal.take()
    }

    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        // Drawing frames and reading and writing files happen on worker threads, never here.
        self.workspace.start_background_threads(&self.ctx);
        self.player = match flare_platform::audio::Player::new() {
            Ok(player) => Some(player),
            Err(error) => {
                eprintln!("sound is off: {error}");
                None
            }
        };
        self.gpu = Some(Gpu::open(event_loop, &self.ctx, &mut self.workspace)?);
        Ok(())
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(gpu) = &mut self.gpu else { return };
        gpu.frame(&self.ctx, &mut self.workspace, event_loop);
        if let Some(player) = &self.player {
            player.apply(self.workspace.take_audio_events());
        } else {
            // Without a sound card the events are dropped, not kept for later.
            drop(self.workspace.take_audio_events());
        }
        #[cfg(target_os = "macos")]
        if let Some(menu) = &self.menu {
            menu.sync(&self.workspace);
        }
        self.after_actions(event_loop);
    }

    /// What a menu click or a frame may have asked of the shell.
    fn after_actions(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_dialog();
        if let (Some(request), Some(gpu)) = (self.workspace.take_window_request(), &self.gpu) {
            gpu.apply(request);
        }
        self.handle_test_movie();
        if self.workspace.take_quit_request() && self.confirm_close() {
            self.persist_layout();
            self.persist_preferences();
            event_loop.exit();
        }
    }

    /// The native menu bar exists only on macOS; elsewhere the window draws its own menu.
    #[cfg(not(target_os = "macos"))]
    fn ensure_native_menu(&mut self) {}

    #[cfg(target_os = "macos")]
    fn ensure_native_menu(&mut self) {
        if self.menu.is_none() {
            self.install_native_menu();
        }
    }

    /// Installs the native menu bar on macOS and routes its clicks into the event loop.
    #[cfg(target_os = "macos")]
    fn install_native_menu(&mut self) {
        use super::menu::NativeMenu;
        match NativeMenu::build(&self.workspace) {
            Ok(native) => {
                native.install();
                let proxy = self.proxy.clone();
                NativeMenu::on_action(move |action| {
                    // A closed event loop means the app is exiting; nothing to do.
                    let _ = proxy.send_event(UserEvent::Menu(action));
                });
                self.workspace.set_native_menu(true);
                native.sync(&self.workspace);
                self.menu = Some(native);
            }
            // Keep the in-window menu if the native one cannot be built.
            Err(error) => eprintln!("native menu unavailable: {error:#}"),
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_none()
            && let Err(error) = self.start(event_loop)
        {
            self.fatal = Some(error);
            event_loop.exit();
            return;
        }
        self.ensure_native_menu();
    }

    // `event_loop` is only needed to apply native menu clicks, which exist on macOS.
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            #[cfg(target_os = "macos")]
            UserEvent::Menu(action) => {
                self.workspace.perform(action);
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
                self.after_actions(event_loop);
            }
            UserEvent::MovieStarted(Ok(movie)) => self.movies.push(movie),
            UserEvent::Exported(Ok(())) => {}
            UserEvent::Exported(Err(message)) => dialogs::message("Export failed", &message),
            UserEvent::MovieStarted(Err(error)) => {
                dialogs::message("Cannot start Test Movie", &error.to_string());
            }
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            event_loop.set_control_flow(ControlFlow::Wait);
            if let Some(gpu) = &self.gpu {
                gpu.window.request_redraw();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gpu) = &mut self.gpu else { return };
        let response = gpu.state.on_window_event(&gpu.window, &event);
        if response.repaint {
            gpu.window.request_redraw();
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.confirm_close() {
                    self.persist_layout();
                    self.persist_preferences();
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (
                    std::num::NonZeroU32::new(size.width),
                    std::num::NonZeroU32::new(size.height),
                ) {
                    gpu.painter.on_window_resized(egui::ViewportId::ROOT, w, h);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}
