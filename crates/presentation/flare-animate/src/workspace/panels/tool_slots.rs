//! The tool strip's slots. As in Animate, a slot holds related tools and shows the one in use;
//! pressing and holding it, or right-clicking, opens a list of the rest. Tools Flare has not
//! built are in the lists dimmed, where Animate has them.

use crate::workspace::{Tool, Workspace};
use egui::Ui;
use flare_ui::{chrome, icons, theme};

use super::TOOL_ICON;

/// One tool in a slot, built or not.
pub(crate) struct Variant {
    pub(crate) icon: &'static str,
    pub(crate) name: &'static str,
    /// The key that picks it; empty for a tool that is not built.
    pub(crate) key: &'static str,
    /// The tool itself, or `None` for one that is only a place for it in the list.
    pub(crate) tool: Option<Tool>,
}

const fn real(icon: &'static str, name: &'static str, key: &'static str, tool: Tool) -> Variant {
    Variant {
        icon,
        name,
        key,
        tool: Some(tool),
    }
}

const fn mock(icon: &'static str, name: &'static str) -> Variant {
    Variant {
        icon,
        name,
        key: "",
        tool: None,
    }
}

/// The tools one cell of the strip stands for.
pub(crate) type Slot = &'static [Variant];

/// How long a press has to last to open a slot's list, in seconds.
const HOLD: f64 = 0.35;

/// The strip's categories, divided by a line: selecting, drawing, painting, helping and viewing.
/// Every tool Flare has is in exactly one slot.
pub(crate) const SLOT_GROUPS: &[&[Slot]] = &[
    &[
        &[real("tool-selection", "Selection", "V", Tool::Select)],
        &[real(
            "tool-subselection",
            "Subselection",
            "A",
            Tool::Subselection,
        )],
        &[
            real("tool-transform", "Free Transform", "Q", Tool::FreeTransform),
            mock("tool-gradient-transform", "Gradient Transform"),
        ],
        &[
            mock("tool-3d-rotation", "3D Rotation"),
            mock("tool-3d-translation", "3D Translation"),
        ],
        &[
            real("tool-lasso", "Lasso", "L", Tool::Lasso),
            mock("tool-lasso-polygon", "Polygon"),
            mock("tool-magic-wand", "Magic Wand"),
        ],
    ],
    &[
        &[
            real("tool-pen", "Pen", "P", Tool::Pen),
            mock("tool-anchor-add", "Add Anchor Point"),
            mock("tool-anchor-delete", "Delete Anchor Point"),
            mock("tool-anchor-convert", "Convert Anchor Point"),
        ],
        &[real("tool-text", "Text", "T", Tool::Text)],
        &[real("tool-pencil", "Pencil", "Y", Tool::Pencil)],
        &[real("tool-brush", "Brush", "B", Tool::Brush)],
        &[real("tool-line", "Line", "N", Tool::Line)],
        &[
            real("tool-rectangle", "Rectangle", "R", Tool::Rectangle),
            real("tool-oval", "Oval", "O", Tool::Ellipse),
            mock("tool-rectangle-primitive", "Rectangle Primitive"),
            mock("tool-oval-primitive", "Oval Primitive"),
            real("tool-polystar", "PolyStar", "G", Tool::PolyStar),
        ],
        &[mock("tool-deco", "Deco")],
        &[mock("tool-bone", "Bone"), mock("tool-bind", "Bind")],
    ],
    &[
        &[
            real("tool-bucket", "Paint Bucket", "K", Tool::Bucket),
            real("tool-ink-bottle", "Ink Bottle", "S", Tool::InkBottle),
        ],
        &[real("tool-eyedropper", "Eyedropper", "I", Tool::Eyedropper)],
        &[real("tool-eraser", "Eraser", "E", Tool::Eraser)],
    ],
    &[
        &[real("tool-mirror", "Mirror", "X", Tool::Mirror)],
        &[real("tool-ruler", "Ruler", "M", Tool::Ruler)],
    ],
    &[
        &[real("tool-hand", "Hand", "H", Tool::Hand)],
        &[real("tool-zoom", "Zoom", "Z", Tool::Zoom)],
        &[mock("tool-camera", "Camera")],
    ],
];

/// The member of `slot` the cell shows: the tool in hand if the slot has it, else the one last
/// picked from its list, else the first that is built, else the first.
pub(crate) fn front(slot: Slot, current: Tool, picked: Option<usize>) -> &'static Variant {
    let variant = slot
        .iter()
        .find(|v| v.tool == Some(current))
        .or_else(|| {
            picked
                .and_then(|i| slot.get(i))
                .filter(|v| v.tool.is_some())
        })
        .or_else(|| slot.iter().find(|v| v.tool.is_some()))
        .or(slot.first());
    // Slots are written out above and none is empty.
    variant.unwrap_or(&SLOT_GROUPS[0][0][0])
}

impl Workspace {
    /// One cell of the strip. Clicking picks the tool shown; the list opens on a right-click or
    /// a long press. Returns the tool picked, if any.
    pub(crate) fn slot_cell(&mut self, ui: &mut Ui, slot: Slot) -> Option<Tool> {
        let picked = self.slot_choice.get(slot[0].name).copied();
        let shown = front(slot, self.tool, picked);
        let active = shown.tool.is_some_and(|t| t == self.tool);
        let hint = if shown.key.is_empty() {
            format!("{} ({})", shown.name, chrome::NOT_YET)
        } else {
            format!("{} ({})", shown.name, shown.key)
        };
        let response = chrome::icon_button(
            ui,
            shown.icon,
            TOOL_ICON,
            active,
            shown.tool.is_some(),
            &hint,
        );
        let mut chosen = shown.tool.filter(|_| response.clicked());
        if slot.len() < 2 {
            return chosen;
        }
        corner_mark(ui, response.rect);
        let ctx = ui.ctx().clone();
        let held = response.is_pointer_button_down_on()
            && ctx
                .input(|i| i.pointer.press_start_time().map(|t| i.time - t))
                .is_some_and(|held| held >= HOLD);
        if response.is_pointer_button_down_on() {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD));
        }
        if response.secondary_clicked() || held {
            self.slot_open = Some(slot[0].name);
        }
        if self.slot_open == Some(slot[0].name) {
            let list = egui::Area::new(ui.make_persistent_id(("slot-list", slot[0].name)))
                .order(egui::Order::Foreground)
                .fixed_pos(response.rect.right_top() + egui::vec2(4.0, 0.0))
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        for (index, variant) in slot.iter().enumerate() {
                            if let Some(tool) = variant_row(ui, variant) {
                                self.slot_choice.insert(slot[0].name, index);
                                chosen = Some(tool);
                            }
                        }
                    });
                });
            // The list goes once something is picked, on Escape, or on a click anywhere else.
            let clicked_elsewhere = ctx.input(|i| i.pointer.any_pressed())
                && ctx
                    .pointer_interact_pos()
                    .is_some_and(|at| !list.response.rect.contains(at));
            let opened_now = response.secondary_clicked() || held;
            if chosen.is_some()
                || ctx.input(|i| i.key_pressed(egui::Key::Escape))
                || (clicked_elsewhere && !opened_now)
            {
                self.slot_open = None;
            }
        }
        chosen
    }
}

/// A row of a slot's list: the tool's picture, name and key. A tool not built is dimmed.
fn variant_row(ui: &mut Ui, variant: &Variant) -> Option<Tool> {
    let built = variant.tool.is_some();
    let ink = if built {
        theme::TEXT
    } else {
        theme::TEXT_DISABLED
    };
    let mut button =
        egui::Button::image_and_text(icons::image(variant.icon, 16.0, ink), variant.name);
    if !variant.key.is_empty() {
        button = button.shortcut_text(variant.key);
    }
    let response = ui.add_enabled(built, button);
    if built {
        response.clicked().then_some(variant.tool).flatten()
    } else {
        response.on_disabled_hover_text(chrome::NOT_YET);
        None
    }
}

/// The small triangle in a cell's corner that says there is a list.
fn corner_mark(ui: &Ui, cell: egui::Rect) {
    let corner = cell.right_bottom() - egui::vec2(3.0, 3.0);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            corner,
            corner - egui::vec2(4.0, 0.0),
            corner - egui::vec2(0.0, 4.0),
        ],
        theme::TEXT_DIM,
        egui::Stroke::NONE,
    ));
}
