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

/// The strip's categories, divided by a line, in the order Animate's toolbar has them. Every tool
/// Flare has is in exactly one slot or in [`EXTRA`].
pub(crate) static SLOT_GROUPS: &[&[Slot]] = &[
    &[
        &[
            real("tool-selection", "Selection", "V", Tool::Select),
            real("tool-subselection", "Subselection", "A", Tool::Subselection),
        ],
        &[
            real("tool-transform", "Free Transform", "Q", Tool::FreeTransform),
            mock("tool-gradient-transform", "Gradient Transform"),
        ],
        &[
            real("tool-lasso", "Lasso", "L", Tool::Lasso),
            real("tool-lasso-polygon", "Polygon", "", Tool::PolygonLasso),
            mock("tool-magic-wand", "Magic Wand"),
        ],
    ],
    &[
        &[
            real("tool-pencil", "Pencil", "Y", Tool::Pencil),
            mock("tool-paint-brush", "Paint Brush"),
        ],
        &[real("tool-brush", "Brush", "B", Tool::Brush)],
        &[real("tool-eraser", "Eraser", "E", Tool::Eraser)],
        &[
            real("tool-rectangle", "Rectangle", "R", Tool::Rectangle),
            real("tool-oval", "Oval", "O", Tool::Ellipse),
            mock("tool-rectangle-primitive", "Rectangle Primitive"),
            mock("tool-oval-primitive", "Oval Primitive"),
            real("tool-polystar", "PolyStar", "G", Tool::PolyStar),
        ],
        &[real("tool-line", "Line", "N", Tool::Line)],
    ],
    &[
        &[
            real("tool-pen", "Pen", "P", Tool::Pen),
            mock("tool-anchor-add", "Add Anchor Point"),
            mock("tool-anchor-delete", "Delete Anchor Point"),
            mock("tool-anchor-convert", "Convert Anchor Point"),
        ],
        &[real("tool-text", "Text", "T", Tool::Text)],
        &[
            real("tool-bucket", "Paint Bucket", "K", Tool::Bucket),
            real("tool-ink-bottle", "Ink Bottle", "S", Tool::InkBottle),
        ],
        &[real("tool-eyedropper", "Eyedropper", "I", Tool::Eyedropper)],
        &[mock("tool-asset-warp", "Asset Warp")],
    ],
    &[
        &[
            real("tool-hand", "Hand", "H", Tool::Hand),
            mock("tool-rotate-view", "Rotation"),
            mock("tool-time-scrub", "Time scrub"),
        ],
        &[real("tool-zoom", "Zoom", "Z", Tool::Zoom)],
        EXTRA,
    ],
];

/// The "…" cell at the foot of the strip: the tools that are not in
/// the strip itself. Its list holds Flare's own Mirror and Ruler, and the tools Animate has that
/// Flare has not built.
pub(crate) static EXTRA: Slot = &[
    real("tool-mirror", "Mirror", "X", Tool::Mirror),
    real("tool-ruler", "Ruler", "M", Tool::Ruler),
    mock("tool-3d-rotation", "3D Rotation"),
    mock("tool-3d-translation", "3D Translation"),
    mock("tool-deco", "Deco"),
    mock("tool-bone", "Bone"),
    mock("tool-bind", "Bind"),
    mock("tool-width", "Width"),
    mock("tool-camera", "Camera"),
];

/// Whether `slot` is the "…" cell, which shows its dots whatever tool is in hand.
pub(crate) fn is_extra(slot: Slot) -> bool {
    slot.len() == EXTRA.len() && std::ptr::eq(slot.as_ptr(), EXTRA.as_ptr())
}

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
        let extra = is_extra(slot);
        let active = if extra {
            slot.iter().any(|v| v.tool == Some(self.tool))
        } else {
            shown.tool.is_some_and(|t| t == self.tool)
        };
        // A tool not built gets "(Not available yet)" added by the button itself.
        let hint = if extra {
            "More tools".to_owned()
        } else if shown.key.is_empty() {
            shown.name.to_owned()
        } else {
            format!("{} ({})", shown.name, shown.key)
        };
        let response = chrome::icon_button_for(
            ui,
            if extra { "toolbar-more" } else { shown.icon },
            TOOL_ICON,
            active,
            extra || shown.tool.is_some(),
            &hint,
            !extra && shown.tool.is_none(),
        );
        let mut chosen = shown.tool.filter(|_| response.clicked() && !extra);
        if self.is_pinned(shown.name) && slot.len() == 1 {
            // A tool put in the strip from the list goes back with a right click, built or not.
            let menu = ui.interact(
                response.rect,
                ui.make_persistent_id(("pinned", shown.name)),
                egui::Sense::click(),
            );
            menu.context_menu(|ui| {
                if ui.button("Remove from toolbar").clicked() {
                    self.unpin_tool(shown.name);
                    ui.close();
                }
            });
        }
        if slot.len() < 2 {
            return chosen;
        }
        if !extra {
            corner_mark(ui, response.rect);
        }
        let ctx = ui.ctx().clone();
        let held = response.is_pointer_button_down_on()
            && ctx
                .input(|i| i.pointer.press_start_time().map(|t| i.time - t))
                .is_some_and(|held| held >= HOLD);
        if response.is_pointer_button_down_on() {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(HOLD));
        }
        if response.secondary_clicked() || held || (extra && response.clicked()) {
            self.slot_open = Some(slot[0].name);
        }
        if self.slot_open == Some(slot[0].name) {
            let list = egui::Area::new(ui.make_persistent_id(("slot-list", slot[0].name)))
                .order(egui::Order::Foreground)
                .fixed_pos(response.rect.right_top() + egui::vec2(4.0, 0.0))
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        for (index, variant) in slot.iter().enumerate() {
                            // A tool put in the strip is not in the list while it is there.
                            if extra && self.is_pinned(variant.name) {
                                continue;
                            }
                            let (tool, dragged) = variant_row(ui, variant);
                            if let Some(tool) = tool {
                                self.slot_choice.insert(slot[0].name, index);
                                chosen = Some(tool);
                            }
                            if dragged && extra {
                                self.slot_drag = Some(variant.name);
                            }
                        }
                    });
                });
            // The list goes once something is picked, on Escape, or on a click anywhere else.
            let clicked_elsewhere = ctx.input(|i| i.pointer.any_pressed())
                && ctx
                    .pointer_interact_pos()
                    .is_some_and(|at| !list.response.rect.contains(at));
            let opened_now = response.secondary_clicked() || held || (extra && response.clicked());
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

/// A row of a slot's list: the tool's picture, name and key. A tool not built is dimmed and
/// does nothing when clicked. Returns the tool picked, and whether the row has been dragged.
fn variant_row(ui: &mut Ui, variant: &Variant) -> (Option<Tool>, bool) {
    let built = variant.tool.is_some();
    let ink = if built {
        theme::text()
    } else {
        theme::text_disabled()
    };
    let mut label = egui::RichText::new(variant.name);
    if !built {
        label = label.color(theme::text_disabled());
    }
    let mut button = egui::Button::image_and_text(icons::image(variant.icon, 16.0, ink), label)
        .sense(egui::Sense::click_and_drag());
    if !variant.key.is_empty() {
        button = button.shortcut_text(variant.key);
    }
    let response = ui.add(button);
    if !built {
        response.clone().on_hover_text(chrome::NOT_YET);
    }
    (
        response.clicked().then_some(variant.tool).flatten(),
        response.drag_started(),
    )
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
        theme::text_dim(),
        egui::Stroke::NONE,
    ));
}
