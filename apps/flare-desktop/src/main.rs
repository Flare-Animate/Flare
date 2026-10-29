//! Desktop shell: one winit event loop, egui-winit input, egui-wgpu painting.
//! All editing behaviour lives in `flare-ui`; this file only owns the window, the GPU
//! surface and native dialogs.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use egui::ViewportId;
use egui_wgpu::{RendererOptions, WgpuConfiguration, winit::Painter};
use flare_ui::{DialogRequest, Workspace};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct Gpu {
    window: Arc<Window>,
    state: egui_winit::State,
    painter: Painter,
}

struct App {
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
        self.handle_dialog();
    }

    fn confirm_close(&self) -> bool {
        if !self.workspace.is_dirty() {
            return true;
        }
        let answer = rfd::MessageDialog::new()
            .set_title("Unsaved changes")
            .set_description("Close without saving your changes?")
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        answer == rfd::MessageDialogResult::Yes
    }
}

fn project_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Flare project", &["flare"])
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_none()
            && let Err(error) = self.start(event_loop)
        {
            self.fatal = Some(error);
            event_loop.exit();
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
    let event_loop = EventLoop::new().context("create event loop")?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
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
