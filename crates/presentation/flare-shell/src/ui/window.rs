//! The window and its GPU surface, and drawing one frame into it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use egui::ViewportId;
use egui_wgpu::{RendererOptions, WgpuConfiguration, winit::Painter};
use flare_animate::{WindowRequest, Workspace};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::Window;

/// The egui context of the app. It says the app shows more than one operating-system window:
/// without that, egui draws a child viewport as an egui window inside its parent, and
/// `show_viewport_deferred` (what keeps the Test Movie's viewport alive) would draw an empty one.
pub(super) fn new_context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    ctx
}

/// Frees the textures a frame left to free.
///
/// Painting skips a frame it has no surface texture for (a window covered by another, minimized
/// or being resized) and returns before it frees what the frame asked to free. Left there, the
/// textures would never be freed and egui's debug check, which counts an unhandled delta as a
/// fault, would panic when the frame's output is dropped.
pub(super) fn free_left_over(
    textures: &mut egui::TexturesDelta,
    mut free: impl FnMut(egui::TextureId),
) {
    for id in textures.free.drain() {
        free(id);
    }
}

pub(super) struct Gpu {
    pub(super) window: Arc<Window>,
    pub(super) state: egui_winit::State,
    pub(super) painter: Painter,
    /// The program draws this window's title bar and resizes it from its edges.
    pub(super) own_frame: bool,
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
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
            .with_window_icon(super::window_icon::app_icon());
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
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        let (attributes, own_frame) = super::frame::own_title_bar(attributes, workspace);
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let own_frame = false;
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context("create window")?,
        );
        // The application exists now; outside an app bundle the Dock has no icon for it.
        flare_platform::app_icon::show_in_dock(flare_ui::icons::APP_ICON_PNG);
        #[cfg(all(unix, not(target_os = "ios")))]
        super::tablet::watch(&window, workspace.pen_feed());
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
            own_frame,
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
            WindowRequest::Minimize => self.window.set_minimized(true),
        }
    }

    /// Runs the workspace for one frame and paints it. Returns what the workspace's sound
    /// events were, for the caller to hand to the sound card.
    pub(super) fn frame(
        &mut self,
        ctx: &egui::Context,
        workspace: &mut Workspace,
        #[cfg_attr(not(feature = "ruffle"), allow(unused_variables))] movie_open: bool,
        event_loop: &ActiveEventLoop,
    ) {
        workspace.set_window_maximized(self.window.is_maximized());
        let raw = self.state.take_egui_input(&self.window);
        let mut output = ctx.run_ui(raw, |ui| {
            workspace.show(ui);
            #[cfg(feature = "ruffle")]
            if movie_open {
                super::movie_window::keep_viewport(ui.ctx());
            }
        });
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
        if let Some(state) = self.painter.render_state() {
            free_left_over(&mut output.textures_delta, |id| {
                state.renderer.write().free_texture(&id);
            });
        }
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

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
