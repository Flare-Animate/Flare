//! Desktop shell: one winit event loop, egui-winit input, egui-wgpu painting.
//! All editing behaviour lives in `flare-ui`; this file only owns the window, the GPU
//! surface and native dialogs.
#![forbid(unsafe_code)]

mod menu;
mod paths;
mod recovery;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use egui::ViewportId;
use egui_wgpu::{RendererOptions, WgpuConfiguration, winit::Painter};
use flare_ui::{DialogRequest, MenuAction, Workspace};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

/// Events sent to the event loop from outside it (native menu clicks arrive on AppKit's side).
enum UserEvent {
    Menu(MenuAction),
}

struct Gpu {
    window: Arc<Window>,
    state: egui_winit::State,
    painter: Painter,
}

struct App {
    proxy: EventLoopProxy<UserEvent>,
    #[cfg(target_os = "macos")]
    menu: Option<menu::NativeMenu>,
    ctx: egui::Context,
    workspace: Workspace,
    gpu: Option<Gpu>,
    fatal: Option<anyhow::Error>,
}

impl App {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let attributes = Window::default_attributes()
            .with_title(self.workspace.title())
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context("create window")?,
        );
        let state = egui_winit::State::new(
            self.ctx.clone(),
            ViewportId::ROOT,
            &*window,
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        let mut painter = pollster::block_on(Painter::new(
            self.ctx.clone(),
            WgpuConfiguration::default(),
            false,
            RendererOptions::default(),
        ));
        pollster::block_on(painter.set_window(ViewportId::ROOT, Some(window.clone())))
            .context("initialize the GPU surface")?;
        window.request_redraw();
        self.gpu = Some(Gpu {
            window,
            state,
            painter,
        });
        Ok(())
    }

    fn handle_dialog(&mut self) {
        match self.workspace.take_dialog() {
            Some(DialogRequest::Open) => {
                if let Some(path) = project_dialog().pick_file() {
                    self.workspace.open(&path);
                }
            }
            Some(DialogRequest::SaveAs) => {
                if let Some(path) = project_dialog().save_file() {
                    self.workspace.save_as(&path);
                }
            }
            None => {}
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(gpu) = &mut self.gpu else { return };
        let raw = gpu.state.take_egui_input(&gpu.window);
        let workspace = &mut self.workspace;
        let mut output = self.ctx.run_ui(raw, |ui| workspace.show(ui));
        gpu.state
            .handle_platform_output(&gpu.window, output.platform_output.clone());
        gpu.window.set_title(&self.workspace.title());

        let primitives = self
            .ctx
            .tessellate(output.shapes.clone(), output.pixels_per_point);
        gpu.painter.paint_and_update_textures(
            ViewportId::ROOT,
            output.pixels_per_point,
            [0.1, 0.1, 0.1, 1.0],
            &primitives,
            &mut output.textures_delta,
            Vec::new(),
            &gpu.window,
        );

        let delay = output
            .viewport_output
            .get(&ViewportId::ROOT)
            .map_or(Duration::MAX, |v| v.repaint_delay);
        if delay.is_zero() {
            gpu.window.request_redraw();
        } else if let Some(deadline) = Instant::now().checked_add(delay) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
        #[cfg(target_os = "macos")]
        if let Some(menu) = &self.menu {
            menu.sync(&self.workspace);
        }
        self.handle_dialog();
        self.handle_quit(event_loop);
    }

    /// Quits when the workspace asks to, after confirming if there is unsaved work.
    fn handle_quit(&mut self, event_loop: &ActiveEventLoop) {
        if self.workspace.take_quit_request() && self.confirm_close() {
            self.persist_layout();
            event_loop.exit();
        }
    }

    /// Installs the native menu bar on macOS and routes its clicks into the event loop.
    #[cfg(target_os = "macos")]
    fn install_native_menu(&mut self) {
        match menu::NativeMenu::build(&self.workspace) {
            Ok(native) => {
                native.install();
                let proxy = self.proxy.clone();
                menu::NativeMenu::on_action(move |action| {
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

    /// Remembers the panel arrangement for the next session. Failure is not worth blocking
    /// an exit over, so it is only reported.
    fn persist_layout(&self) {
        let (Some(path), Ok(json)) = (paths::layout_file(), self.workspace.layout_json()) else {
            return;
        };
        if let Err(error) = paths::write_layout(&path, &json) {
            eprintln!("could not save the layout: {error}");
        }
    }

    fn confirm_close(&mut self) -> bool {
        if !self.workspace.is_dirty() {
            return true;
        }
        let answer = rfd::MessageDialog::new()
            .set_title("Unsaved changes")
            .set_description("Close without saving your changes?")
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        let discard = answer == rfd::MessageDialogResult::Yes;
        if discard {
            // Closing without saving really discards the work, including its recovery file.
            self.workspace.discard_recovery();
        }
        discard
    }
}

fn project_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Flare project", &["flare"])
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
        #[cfg(target_os = "macos")]
        if self.menu.is_none() {
            self.install_native_menu();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(action) => {
                self.workspace.perform(action);
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
                self.handle_dialog();
                self.handle_quit(event_loop);
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
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (
                    std::num::NonZeroU32::new(size.width),
                    std::num::NonZeroU32::new(size.height),
                ) {
                    gpu.painter.on_window_resized(ViewportId::ROOT, w, h);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let mut workspace = Workspace::untitled().map_err(anyhow::Error::msg)?;
    if let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) {
        let document = flare_storage::load(&path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        workspace = Workspace::new(document, Some(path));
    }
    if let Some(path) = paths::layout_file()
        && let Some(json) = paths::read_layout(&path)
        && let Err(error) = workspace.restore_layout(&json)
    {
        // A damaged or outdated layout just means starting from the default.
        eprintln!("ignoring the saved layout: {error}");
    }
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    // The native menu replaces winit's built-in default one.
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_default_menu(false);
    }
    if let Some(dir) = recovery::recovery_dir() {
        workspace.enable_recovery(dir.clone());
        // Only offer recovery on a plain launch, not when a project was named explicitly.
        if std::env::args_os().nth(1).is_none()
            && let Some(newest) = flare_storage::list_recoveries(&dir).first()
        {
            recovery::offer_recovery(&mut workspace, newest);
        }
    }
    let event_loop = builder.build().context("create event loop")?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        proxy: event_loop.create_proxy(),
        #[cfg(target_os = "macos")]
        menu: None,
        ctx: egui::Context::default(),
        workspace,
        gpu: None,
        fatal: None,
    };
    event_loop.run_app(&mut app).context("event loop")?;
    match app.fatal {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
