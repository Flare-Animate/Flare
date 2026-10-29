//! The window and its GPU surface, and drawing one frame into it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use egui::ViewportId;
use egui_wgpu::{RendererOptions, WgpuConfiguration, winit::Painter};
use flare_animate::{WindowRequest, Workspace};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::Window;

pub(super) struct Gpu {
    pub(super) window: Arc<Window>,
    pub(super) state: egui_winit::State,
    pub(super) painter: Painter,
}

impl Gpu {
    /// Opens the window and the GPU surface for it.
    pub(super) fn open(
        event_loop: &ActiveEventLoop,
        ctx: &egui::Context,
        workspace: &mut Workspace,
    ) -> Result<Self> {
        let attributes = Window::default_attributes()
            .with_title(workspace.title())
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        // The top bar replaces the title bar on macOS; the traffic lights float over its left end.
        #[cfg(target_os = "macos")]
        let attributes = {
            use winit::platform::macos::WindowAttributesExtMacOS;
            workspace.set_titlebar_inset(78.0);
            attributes
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_fullsize_content_view(true)
        };
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context("create window")?,
        );
        let state = egui_winit::State::new(
            ctx.clone(),
            ViewportId::ROOT,
            &*window,
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        let mut painter = pollster::block_on(Painter::new(
            ctx.clone(),
            WgpuConfiguration::default(),
            false,
            RendererOptions::default(),
        ));
        pollster::block_on(painter.set_window(ViewportId::ROOT, Some(window.clone())))
            .context("initialize the GPU surface")?;
        window.request_redraw();
        Ok(Self {
            window,
            state,
            painter,
        })
    }

    /// Does what the top bar asked of the window.
    pub(super) fn apply(&self, request: WindowRequest) {
        match request {
            WindowRequest::Drag => {
                let _ = self.window.drag_window();
            }
            WindowRequest::ToggleMaximize => {
                self.window.set_maximized(!self.window.is_maximized());
            }
        }
    }

    /// Runs the workspace for one frame and paints it. Returns what the workspace's sound
    /// events were, for the caller to hand to the sound card.
    pub(super) fn frame(
        &mut self,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        event_loop: &ActiveEventLoop,
    ) {
        let raw = self.state.take_egui_input(&self.window);
        let mut output = ctx.run_ui(raw, |ui| workspace.show(ui));
        self.state
            .handle_platform_output(&self.window, output.platform_output.clone());
        self.window.set_title(&workspace.title());
        let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        self.painter.paint_and_update_textures(
            ViewportId::ROOT,
            output.pixels_per_point,
            [0.1, 0.1, 0.1, 1.0],
            &primitives,
            &mut output.textures_delta,
            Vec::new(),
            &self.window,
        );
        let delay = output
            .viewport_output
            .get(&ViewportId::ROOT)
            .map_or(Duration::MAX, |v| v.repaint_delay);
        if delay.is_zero() {
            self.window.request_redraw();
        } else if let Some(deadline) = Instant::now().checked_add(delay) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}
