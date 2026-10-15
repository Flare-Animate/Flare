//! Movable docks. Each docked module (Tools, Properties, Library, Timeline)
//! has a grip; dragging it shows the places it can go, and releasing over one
//! moves it there. Escape, or releasing anywhere else, leaves the layout as it
//! was. The layout is saved in the settings; Window › Reset Layout restores
//! the standard one.
//!
//! The behaviour follows Flare Studio's workspace docking
//! (`flare/rust/studio/workspace.rs`, Flare-Animate/Flare 8a75f53); this is our
//! own egui code and our own look. Floating panels and arbitrary tab groups
//! are later work (they need a newer egui).

use crate::App;
use eframe::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum LibraryDock {
    /// Under Properties, in the same dock.
    #[default]
    WithProperties,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct Layout {
    #[serde(default)]
    pub tools_right: bool,
    #[serde(default)]
    pub props_left: bool,
    #[serde(default)]
    pub timeline_top: bool,
    #[serde(default)]
    pub library: LibraryDock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Module {
    Tools,
    Properties,
    Library,
    Timeline,
}

impl Module {
    fn name(self) -> &'static str {
        match self {
            Module::Tools => "Tools",
            Module::Properties => "Properties",
            Module::Library => "Library",
            Module::Timeline => "Timeline",
        }
    }
}

/// Where a module can be dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    Left,
    Right,
    Top,
    Bottom,
    WithProperties,
}

impl Place {
    fn label(self) -> &'static str {
        match self {
            Place::Left => "Dock left",
            Place::Right => "Dock right",
            Place::Top => "Dock above the stage",
            Place::Bottom => "Dock below the stage",
            Place::WithProperties => "Under Properties",
        }
    }
}

impl Layout {
    /// Apply a drop. Returns false when the place doesn't fit the module.
    pub(crate) fn apply(&mut self, m: Module, p: Place) -> bool {
        match (m, p) {
            (Module::Tools, Place::Left | Place::Right) => self.tools_right = p == Place::Right,
            (Module::Properties, Place::Left | Place::Right) => self.props_left = p == Place::Left,
            (Module::Timeline, Place::Top | Place::Bottom) => self.timeline_top = p == Place::Top,
            (Module::Library, Place::Left) => self.library = LibraryDock::Left,
            (Module::Library, Place::Right) => self.library = LibraryDock::Right,
            (Module::Library, Place::WithProperties) => self.library = LibraryDock::WithProperties,
            _ => return false,
        }
        true
    }

    pub(crate) fn tools_side(&self) -> egui::panel::Side {
        if self.tools_right { egui::panel::Side::Right } else { egui::panel::Side::Left }
    }
    pub(crate) fn props_side(&self) -> egui::panel::Side {
        if self.props_left { egui::panel::Side::Left } else { egui::panel::Side::Right }
    }
    pub(crate) fn timeline_side(&self) -> egui::panel::TopBottomSide {
        if self.timeline_top { egui::panel::TopBottomSide::Top } else { egui::panel::TopBottomSide::Bottom }
    }
}

/// The drop places offered for a module, each with its target box in `area`.
pub(crate) fn targets(m: Module, area: Rect) -> Vec<(Place, Rect)> {
    let c = area.center();
    let side = vec2(130.0, 64.0);
    let left = Rect::from_center_size(pos2(area.left() + 90.0, c.y), side);
    let right = Rect::from_center_size(pos2(area.right() - 90.0, c.y), side);
    match m {
        Module::Tools | Module::Properties => vec![(Place::Left, left), (Place::Right, right)],
        Module::Library => vec![(Place::Left, left), (Place::WithProperties, Rect::from_center_size(c, side)), (Place::Right, right)],
        Module::Timeline => vec![
            (Place::Top, Rect::from_center_size(pos2(c.x, area.top() + 60.0), vec2(220.0, 44.0))),
            (Place::Bottom, Rect::from_center_size(pos2(c.x, area.bottom() - 60.0), vec2(220.0, 44.0))),
        ],
    }
}

/// A drag handle: a dotted grip, optionally followed by the panel's title.
/// Starting a drag on it picks the module up.
pub(crate) fn grip(app: &mut App, ui: &mut egui::Ui, m: Module, title: Option<&str>) -> egui::Response {
    let galley = title.map(|t| ui.painter().layout_no_wrap(t.to_string(), egui::FontId::proportional(13.0), Color32::from_gray(225)));
    let w = 12.0 + galley.as_ref().map_or(0.0, |g| g.size().x + 6.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 18.0), Sense::click_and_drag());
    let hot = resp.hovered() || app.dock_drag == Some(m);
    let dot = if hot { Color32::from_rgb(0x6C, 0xB4, 0xFF) } else { Color32::from_gray(120) };
    for row in 0..3 {
        for col in 0..2 {
            ui.painter().circle_filled(pos2(rect.left() + 3.5 + col as f32 * 4.0, rect.center().y - 4.0 + row as f32 * 4.0), 1.1, dot);
        }
    }
    if let Some(g) = galley {
        ui.painter().galley(pos2(rect.left() + 14.0, rect.center().y - g.size().y / 2.0), g, Color32::from_gray(225));
    }
    if resp.drag_started() && app.dock_drag.is_none() && !app.dock_cancelled {
        app.dock_drag = Some(m);
    }
    resp.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text(format!("Drag to move {}", m.name()))
}

/// Escape while a module is picked up cancels the move. Runs before the
/// app's shortcuts so they don't take the key (Escape also deselects).
pub(crate) fn escape(app: &mut App, ctx: &egui::Context) {
    let Some(m) = app.dock_drag else { return };
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        app.dock_drag = None;
        app.dock_cancelled = true;
        app.status = format!("Moving {} cancelled", m.name());
    }
}

/// While a module is picked up: draw its drop places, and drop or cancel.
pub(crate) fn drop_targets(app: &mut App, ctx: &egui::Context) {
    // a cancelled drag stays cancelled until the button is let go
    if app.dock_cancelled {
        app.dock_cancelled = ctx.input(|i| i.pointer.any_down());
        return;
    }
    let Some(m) = app.dock_drag else { return };
    let area = ctx.screen_rect().shrink(8.0);
    let pointer = ctx.input(|i| i.pointer.latest_pos());
    let places = targets(m, area);
    let hit = places.iter().find(|(_, r)| pointer.is_some_and(|p| r.contains(p))).map(|(p, _)| *p);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("dock.targets")));
    painter.rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(60));
    let accent = Color32::from_rgb(0x3D, 0x8E, 0xF0);
    for (p, r) in &places {
        let on = hit == Some(*p);
        painter.rect_filled(*r, 6.0, if on { Color32::from_rgb(0x1E, 0x4F, 0x8A) } else { Color32::from_rgba_unmultiplied(0x26, 0x29, 0x2E, 235) });
        painter.rect_stroke(*r, 6.0, Stroke::new(if on { 2.5_f32 } else { 1.5 }, accent));
        painter.text(r.center(), egui::Align2::CENTER_CENTER, p.label(), egui::FontId::proportional(13.0), Color32::from_gray(235));
    }
    if let Some(pos) = pointer {
        painter.text(pos + vec2(14.0, 14.0), egui::Align2::LEFT_TOP, m.name(), egui::FontId::proportional(12.0), Color32::from_gray(220));
    }
    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    if ctx.input(|i| i.pointer.any_released() || !i.pointer.any_down()) {
        app.dock_drag = None;
        match hit {
            Some(p) if app.layout.apply(m, p) => {
                app.lib.open |= m == Module::Library;
                app.save_layout();
                app.status = format!("{} moved: {}", m.name(), p.label().to_lowercase());
            }
            _ => app.status = format!("Moving {} cancelled", m.name()),
        }
    }
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_only_where_the_module_fits() {
        let mut l = Layout::default();
        assert!(!l.apply(Module::Tools, Place::Top));
        assert!(!l.apply(Module::Timeline, Place::Left));
        assert!(!l.apply(Module::Properties, Place::WithProperties));
        assert_eq!(l, Layout::default());
        assert!(l.apply(Module::Tools, Place::Right));
        assert!(l.apply(Module::Properties, Place::Left));
        assert!(l.apply(Module::Timeline, Place::Top));
        assert!(l.apply(Module::Library, Place::Right));
        assert_eq!(l, Layout { tools_right: true, props_left: true, timeline_top: true, library: LibraryDock::Right });
        let json = serde_json::to_string(&l).unwrap();
        assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), l);
        // older settings without the field read as the standard layout
        assert_eq!(serde_json::from_str::<Layout>("{}").unwrap(), Layout::default());
    }

    #[test]
    fn every_module_has_distinct_non_overlapping_targets() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0));
        for m in [Module::Tools, Module::Properties, Module::Library, Module::Timeline] {
            let t = targets(m, area);
            assert!(t.len() >= 2);
            for (i, a) in t.iter().enumerate() {
                assert!(area.contains_rect(a.1));
                for b in &t[i + 1..] {
                    assert!(!a.1.intersects(b.1), "{m:?}: {:?} overlaps {:?}", a.0, b.0);
                }
            }
        }
    }
}
