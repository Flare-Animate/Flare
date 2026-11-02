//! The window's frame where the program draws its own title bar (Linux and Windows): opening the
//! window without the system's, and resizing it from its edges, which the system's frame did.

#[cfg(any(target_os = "linux", target_os = "windows"))]
use flare_animate::Workspace;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
#[cfg(any(target_os = "linux", target_os = "windows"))]
use winit::window::WindowAttributes;
use winit::window::{CursorIcon, ResizeDirection, Window};

/// How far in from the window's edge a press starts a resize, in logical pixels.
const EDGE: f64 = 6.0;

/// Whether the person asked for the system's own title bar: `FLARE_SYSTEM_TITLEBAR=1`, for a
/// window manager that does not get on with a window that draws its own.
#[cfg(any(target_os = "linux", target_os = "windows", test))]
pub(super) fn system_titlebar_wanted(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

/// The window's attributes without the system's title bar and the top bar's own window buttons,
/// unless the system's was asked for. Returns whether the program draws the frame.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) fn own_title_bar(
    attributes: WindowAttributes,
    workspace: &mut Workspace,
) -> (WindowAttributes, bool) {
    if system_titlebar_wanted(std::env::var("FLARE_SYSTEM_TITLEBAR").ok().as_deref()) {
        return (attributes, false);
    }
    workspace.set_window_controls(true);
    let attributes = attributes.with_decorations(false);
    // An undecorated window has no shadow of its own on Windows; ask for the system's.
    #[cfg(target_os = "windows")]
    let attributes = {
        use winit::platform::windows::WindowAttributesExtWindows;
        attributes.with_undecorated_shadow(true)
    };
    (attributes, true)
}

/// Which edge or corner of a window of `size` the pointer is on, if it is within `border` of one.
/// A corner is twice as long as the border along both of its edges, so it is easy to hit.
pub(super) fn resize_edge(
    at: PhysicalPosition<f64>,
    size: PhysicalSize<u32>,
    border: f64,
) -> Option<ResizeDirection> {
    let (width, height) = (f64::from(size.width), f64::from(size.height));
    if width <= 0.0 || height <= 0.0 || at.x < 0.0 || at.y < 0.0 || at.x >= width || at.y >= height
    {
        return None;
    }
    let corner = border * 2.0;
    let (left, right) = (at.x < border, at.x >= width - border);
    let (top, bottom) = (at.y < border, at.y >= height - border);
    let (near_left, near_right) = (at.x < corner, at.x >= width - corner);
    let (near_top, near_bottom) = (at.y < corner, at.y >= height - corner);
    use ResizeDirection::{East, North, NorthEast, NorthWest, South, SouthEast, SouthWest, West};
    match (left, right, top, bottom) {
        _ if top && near_left || left && near_top => Some(NorthWest),
        _ if top && near_right || right && near_top => Some(NorthEast),
        _ if bottom && near_left || left && near_bottom => Some(SouthWest),
        _ if bottom && near_right || right && near_bottom => Some(SouthEast),
        (true, ..) => Some(West),
        (_, true, ..) => Some(East),
        (_, _, true, _) => Some(North),
        (.., true) => Some(South),
        _ => None,
    }
}

/// Where the pointer is on the window's edges, and what a press there does: resizes the window
/// the way the system's frame would.
#[derive(Debug, Default)]
pub(super) struct EdgeResize {
    edge: Option<ResizeDirection>,
}

impl EdgeResize {
    /// Looks at one window event. Returns whether it was the frame's, which the rest of the
    /// program must not see: a pointer on an edge is not over the top bar or the stage.
    pub(super) fn handle(&mut self, window: &Window, event: &WindowEvent) -> bool {
        if window.is_maximized() {
            // The edges of a maximized window are the screen's.
            if self.edge.take().is_some() {
                window.set_cursor(CursorIcon::Default);
            }
            return false;
        }
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let border = EDGE * window.scale_factor();
                let edge = resize_edge(*position, window.inner_size(), border);
                if edge != self.edge {
                    window.set_cursor(edge.map_or(CursorIcon::Default, cursor_for));
                    self.edge = edge;
                }
                edge.is_some()
            }
            WindowEvent::CursorLeft { .. } => {
                self.edge = None;
                false
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => match self.edge {
                Some(direction) => {
                    // A system that cannot start the resize leaves the window as it was.
                    let _ = window.drag_resize_window(direction);
                    true
                }
                None => false,
            },
            _ => false,
        }
    }
}

/// The pointer that shows a resize in `direction`.
pub(super) fn cursor_for(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::NsResize,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::EwResize,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::NwseResize,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::NeswResize,
    }
}
