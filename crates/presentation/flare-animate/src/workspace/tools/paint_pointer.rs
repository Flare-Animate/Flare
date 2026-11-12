//! The Paint Bucket, Ink Bottle and Eyedropper on the stage: they act the moment the button goes
//! down, and while one is in hand the pointer over the stage is its own picture, with its point
//! where the tool touches.

use egui::{Color32, CursorIcon, Pos2, Rect, Response, Ui, Vec2};

use crate::workspace::*;

/// How big the tool's picture is drawn as the pointer, in screen points.
const POINTER_SIZE: f32 = 22.0;

/// The rim drawn around the picture, so a white tool shows on a white stage too.
const RIM: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 220);

/// The icon each of these tools is drawn with as the pointer, and where in its 24-unit square
/// the tool touches the stage: the bucket's drip, the bottle's base, the dropper's tip.
pub(in crate::workspace) fn pointer_picture(tool: Tool) -> Option<(&'static str, Vec2)> {
    match tool {
        Tool::Bucket => Some(("tool-bucket", Vec2::new(19.25, 20.5))),
        Tool::InkBottle => Some(("tool-ink-bottle", Vec2::new(12.0, 20.8))),
        Tool::Eyedropper => Some(("tool-eyedropper", Vec2::new(4.3, 19.95))),
        _ => None,
    }
}

impl Workspace {
    /// Acts where the primary button went down on the stage.
    pub(in crate::workspace) fn paint_tool_press(
        &mut self,
        ui: &Ui,
        response: &Response,
        to_doc: &impl Fn(Pos2) -> Pos2,
    ) {
        let (pressed, shift) = ui.input(|i| {
            let pressed = i.pointer.primary_pressed();
            (
                pressed.then(|| i.pointer.press_origin()).flatten(),
                i.modifiers.shift,
            )
        });
        if response.hovered()
            && let Some(p) = pressed
        {
            self.click_with_tool(to_doc(p), shift);
        }
    }

    /// Draws the tool in hand as the pointer while it is over the stage, in place of the
    /// system's arrow.
    pub(in crate::workspace) fn paint_tool_pointer(&self, ui: &Ui, response: &Response) {
        let Some((icon, tip)) = pointer_picture(self.tool) else {
            return;
        };
        let Some(at) = response.hover_pos() else {
            return;
        };
        ui.ctx().set_cursor_icon(CursorIcon::None);
        let rect = Rect::from_min_size(at - tip * (POINTER_SIZE / 24.0), Vec2::splat(POINTER_SIZE));
        for step in [
            (-1.0, 0.0),
            (1.0, 0.0),
            (0.0, -1.0),
            (0.0, 1.0),
            (-1.0, -1.0),
            (1.0, 1.0),
            (-1.0, 1.0),
            (1.0, -1.0),
        ] {
            flare_ui::icons::image(icon, POINTER_SIZE, RIM)
                .paint_at(ui, rect.translate(Vec2::from(step)));
        }
        flare_ui::icons::image(icon, POINTER_SIZE, Color32::WHITE).paint_at(ui, rect);
    }
}
