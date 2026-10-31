//! Tools the person has dragged out of the "…" list into the strip, as Animate's Edit Toolbar
//! lets them. A pinned tool is a cell of its own just above the dots; right-click it to take it
//! back out. They are kept with the preferences.

use egui::{Rect, Ui};
use flare_ui::icons;

use super::tool_slots::{EXTRA, Slot};
use crate::workspace::Workspace;

impl Workspace {
    /// The pinned tools as cells, in the order they were pinned.
    pub(crate) fn pinned_slots(&self) -> Vec<Slot> {
        self.pinned_tools
            .iter()
            .filter_map(|name| EXTRA.iter().find(|v| v.name == *name))
            .map(std::slice::from_ref)
            .collect()
    }

    pub(crate) fn is_pinned(&self, name: &str) -> bool {
        self.pinned_tools.contains(&name)
    }

    /// Puts the "…" list's tool called `name` in the strip. False if there is no such tool or
    /// it is there already.
    pub fn pin_tool(&mut self, name: &str) -> bool {
        let Some(variant) = EXTRA.iter().find(|v| v.name == name) else {
            return false;
        };
        if self.is_pinned(name) {
            return false;
        }
        self.pinned_tools.push(variant.name);
        true
    }

    /// Takes a pinned tool out of the strip; it is in the "…" list again.
    pub fn unpin_tool(&mut self, name: &str) {
        self.pinned_tools.retain(|n| *n != name);
    }

    /// While a tool from the list is being dragged: its picture follows the pointer, and letting
    /// go over `strip` pins it. Escape, or letting go anywhere else, drops it.
    pub(crate) fn tool_drag(&mut self, ui: &Ui, strip: Rect) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        let Some(name) = self.slot_drag else {
            return;
        };
        let at = ctx.input(|i| i.pointer.latest_pos());
        if let (Some(at), Some(variant)) = (at, EXTRA.iter().find(|v| v.name == name)) {
            egui::Area::new(ui.make_persistent_id("dragged-tool"))
                .order(egui::Order::Tooltip)
                .fixed_pos(at + egui::vec2(8.0, 8.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.add(icons::image(variant.icon, 24.0, flare_ui::theme::TEXT));
                });
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        if ctx.input(|i| i.pointer.any_released()) {
            if at.is_some_and(|at| strip.contains(at)) {
                self.pin_tool(name);
            }
            self.slot_drag = None;
        } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.slot_drag = None;
        }
    }
}
