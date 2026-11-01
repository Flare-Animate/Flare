//! The Test Movie's own operating-system window: its surface on the editor's GPU, its input, and
//! the frames it draws. What it shows and how it plays is `movie.rs`.
//!
//! It shares the editor's egui context and renderer, as a second viewport, so the movie's texture
//! (registered with that renderer) can be drawn here and the fonts are not made twice.

use std::sync::Arc;

use anyhow::{Context, Result};
use egui::ViewportId;
use egui_wgpu::winit::Painter;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use super::movie::{MIN_WIDTH, MovieWindow, open_size};

/// How much of the screen the window may take when it opens: room left for the title bar, the
/// menu bar and the dock.
const SCREEN_SHARE: (f64, f64) = (0.94, 0.86);

/// The room to open in when there is no monitor to ask.
const FALLBACK_ROOM: egui::Vec2 = egui::vec2(1400.0, 900.0);

/// The Test Movie window's viewport in egui.
pub(crate) fn viewport_id() -> ViewportId {
    ViewportId::from_hash_of("flare-test-movie")
}

/// Says, in the editor's own pass, that the Test Movie viewport is still in use.
///
/// At the end of each pass egui removes the child viewports that pass did not declare, with
/// everything it remembered of them: the rectangles of their widgets from the pass before, which
/// is what a click is tested against. Without this, every frame of the editor between two frames
/// of the Test Movie wipes them, and none of its controls can be clicked. The callback is never
/// run: the window is drawn by `MovieViewport`, not by egui's own viewport machinery.
pub(crate) fn keep_viewport(ctx: &egui::Context) {
    ctx.show_viewport_deferred(viewport_id(), egui::ViewportBuilder::default(), |_, _| {});
}

/// What an event asked of the window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Wish {
    Nothing,
    Close,
}

pub(crate) struct MovieViewport {
    pub(crate) window: Arc<Window>,
    state: egui_winit::State,
    id: ViewportId,
    player: MovieWindow,
}

impl MovieViewport {
    /// Opens the window, sized to the movie's picture and its controls, and starts the movie.
    pub(crate) fn open(
        event_loop: &ActiveEventLoop,
        ctx: &egui::Context,
        painter: &mut Painter,
        swf: Vec<u8>,
        scale: f32,
    ) -> Result<Self> {
        let render_state = painter.render_state().context("the editor has no GPU")?;
        let player = MovieWindow::open(swf, &render_state, scale).context("start the movie")?;
        let picture = player.picture_points();
        // The movie's own size, as long as the screen has room for it.
        let room = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .map_or(FALLBACK_ROOM, |monitor| {
                let screen = monitor.size().to_logical::<f64>(monitor.scale_factor());
                egui::vec2(
                    (screen.width * SCREEN_SHARE.0) as f32,
                    (screen.height * SCREEN_SHARE.1) as f32,
                )
            });
        let open = open_size(picture, room);
        let size = LogicalSize::new(f64::from(open.x), f64::from(open.y));
        let attributes = Window::default_attributes()
            .with_title("Test Movie")
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(f64::from(MIN_WIDTH), 160.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context("create the Test Movie window")?,
        );
        let id = viewport_id();
        let state = egui_winit::State::new(
            ctx.clone(),
            id,
            &*window,
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        pollster::block_on(painter.set_window(id, Some(window.clone())))
            .context("initialize the GPU surface")?;
        window.request_redraw();
        Ok(Self {
            window,
            state,
            id,
            player,
        })
    }

    /// Takes one event for this window. A close request, or a click on the player's Close
    /// button, asks the caller to drop it.
    pub(crate) fn handle(
        &mut self,
        ctx: &egui::Context,
        painter: &mut Painter,
        event: &WindowEvent,
    ) -> Wish {
        let response = self.state.on_window_event(&self.window, event);
        if response.repaint {
            self.window.request_redraw();
        }
        match event {
            WindowEvent::CloseRequested => return Wish::Close,
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (
                    std::num::NonZeroU32::new(size.width),
                    std::num::NonZeroU32::new(size.height),
                ) {
                    painter.on_window_resized(self.id, w, h);
                }
            }
            WindowEvent::RedrawRequested => return self.redraw(ctx, painter),
            _ => {}
        }
        Wish::Nothing
    }

    /// One frame: the movie advances, the window is drawn, and the next frame is asked for (a
    /// movie moves with the clock, so the window keeps drawing while it is open).
    fn redraw(&mut self, ctx: &egui::Context, painter: &mut Painter) -> Wish {
        let raw = self.state.take_egui_input(&self.window);
        let mut output = ctx.run_ui(raw, |ui| self.player.show(ui));
        self.state
            .handle_platform_output(&self.window, output.platform_output.clone());
        let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        painter.paint_and_update_textures(
            self.id,
            output.pixels_per_point,
            [0.07, 0.07, 0.07, 1.0],
            &primitives,
            &mut output.textures_delta,
            Vec::new(),
            &self.window,
        );
        if let Some(state) = painter.render_state() {
            super::window::free_left_over(&mut output.textures_delta, |id| {
                state.renderer.write().free_texture(&id);
            });
        }
        if self.player.is_closed() {
            return Wish::Close;
        }
        self.window.request_redraw();
        Wish::Nothing
    }

    /// Gives the window's surface back. The player's texture goes when `self` is dropped, which
    /// the caller does after this.
    ///
    /// Not `set_window(id, None)`: that clears every surface, the editor's own included, and the
    /// editor's next frame would have nothing to draw on (and its texture updates would be
    /// dropped, which egui treats as a fault). Only the editor's surface is kept instead.
    pub(crate) fn release(&self, painter: &mut Painter) {
        let mut keep = egui::ViewportIdSet::default();
        keep.insert(ViewportId::ROOT);
        painter.gc_viewports(&keep);
    }
}
