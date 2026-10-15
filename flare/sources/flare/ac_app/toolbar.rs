//! Tools panel. Same structure as PhotoCraft's toolbar: divider-separated
//! *sections* of *slots*; each slot is a flyout group whose button shows the
//! group's last-used tool (a small corner triangle marks groups). Right-click
//! or long-press a slot to pick another tool from its group. "…" opens Edit
//! Toolbar to hide/show tools. The tool set and shortcuts follow Animate.

use crate::{App, ACCENT};
use ac_core::text::{Align, FontFace};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Key, Modifiers, Rect, RichText, Sense, Stroke};
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Tool {
    Select,
    Subselect,
    FreeTransform,
    GradientTransform,
    Rotate3D,
    Translate3D,
    Lasso,
    PolyLasso,
    MagicWand,
    Pen,
    AddAnchor,
    DeleteAnchor,
    ConvertAnchor,
    Text,
    Line,
    Rect,
    RectPrimitive,
    Oval,
    OvalPrimitive,
    PolyStar,
    Pencil,
    Brush,
    FluidBrush,
    /// MyPaint raster brushes painting into bitmaps (OpenToonz).
    RasterBrush,
    Bone,
    Bind,
    Bucket,
    InkBottle,
    Eyedropper,
    Eraser,
    Width,
    AssetWarp,
    // OpenToonz deformation tools
    Magnet,
    Pinch,
    Pump,
    Bender,
    Iron,
    /// Drawing assistants and symmetry (OpenToonz).
    Assistant,
    /// Particle emitters.
    Particles,
    Camera,
    Hand,
    RotateView,
    Zoom,
}

pub struct ToolInfo {
    pub name: &'static str,
    /// Shortcut shown in tooltips; parsed from `key`/`shift`.
    pub key: Option<Key>,
    pub shift: bool,
    /// `None` = implemented. `Some(why)` = shown dimmed, not usable yet.
    pub planned: Option<&'static str>,
    pub tip: &'static str,
}

const fn t(name: &'static str, key: Option<Key>, shift: bool, tip: &'static str) -> ToolInfo {
    ToolInfo { name, key, shift, planned: None, tip }
}
#[allow(dead_code)] // kept for future planned tools
const fn planned(name: &'static str, key: Option<Key>, shift: bool, why: &'static str) -> ToolInfo {
    ToolInfo { name, key, shift, planned: Some(why), tip: "" }
}

impl Tool {
    pub fn info(self) -> ToolInfo {
        use Key::*;
        match self {
            Tool::Select => t("Selection", Some(V), false, "Click or box-select objects; drag to move, corners to scale"),
            Tool::Subselect => t("Subselection", Some(A), false, "Edit the anchor points and curve handles of a shape"),
            Tool::FreeTransform => t("Free Transform", Some(Q), false, "Scale (corners/sides), skew (drag an edge), rotate (just outside a corner) around the movable transformation point. Distort and Envelope modes in Tool properties"),
            Tool::GradientTransform => t("Gradient Transform", Some(F), false, "Move, rotate and stretch a shape's gradient fill"),
            Tool::Rotate3D => t("3D Rotation", Some(W), false, "Rotate in 3D: red = X, green = Y, blue ring = Z, orange ring = free. Drag the centre dot to move the 3D centre"),
            Tool::Translate3D => t("3D Translation", Some(G), false, "Move in 3D: red arrow = X, green arrow = Y, centre dot = depth (Z)"),
            Tool::Lasso => t("Lasso", Some(L), false, "Draw a freehand loop to select the objects inside it"),
            Tool::PolyLasso => t("Polygon Lasso", None, false, "Click corners of a selection polygon; double-click to finish"),
            Tool::MagicWand => t("Magic Wand", None, false, "Click a broken-apart bitmap to select the area of similar colour (Shift adds). Threshold in Tool properties"),
            Tool::Pen => t("Pen", Some(P), false, "Click for corners, drag for curves; click the first point to close; Enter finishes"),
            Tool::AddAnchor => t("Add Anchor Point", Some(Equals), false, "Click a shape's outline to add a point"),
            Tool::DeleteAnchor => t("Delete Anchor Point", Some(Minus), false, "Click a point to remove it"),
            Tool::ConvertAnchor => t("Convert Anchor Point", None, false, "Click a point to switch it between corner and smooth"),
            Tool::Text => t("Text", Some(T), false, "Click to type new text, or click text to edit it"),
            Tool::Line => t("Line", Some(N), false, "Drag a straight line (Shift: 45° steps)"),
            Tool::Rect => t("Rectangle", Some(R), false, "Drag a rectangle (Shift: square). Corner radius in Tool properties"),
            Tool::Oval => t("Oval", Some(O), false, "Drag an oval (Shift: circle). Start/end angle and inner radius in Tool properties"),
            Tool::RectPrimitive => t("Rectangle Primitive", None, false, "A rectangle whose corner radii stay editable in Properties (always a drawing object)"),
            Tool::OvalPrimitive => t("Oval Primitive", None, false, "An oval whose start/end angle and inner radius stay editable in Properties (always a drawing object)"),
            Tool::PolyStar => t("PolyStar", None, false, "Drag a polygon or star. Sides and style in Tool properties"),
            Tool::Pencil => t("Pencil", Some(Y), true, "Freehand line: Straighten, Smooth or Ink mode"),
            Tool::Brush => t("Paint Brush", Some(Y), false, "Freehand filled stroke using the fill colour"),
            Tool::FluidBrush => t("Fluid Brush", Some(B), false, "Brush whose width follows your speed: slow = thick, fast = thin"),
            Tool::Bone => t("Bone", Some(M), false, "Drag from one object to another to link them with a bone; drag inside one shape to give it shape bones. Then drag with the Selection tool to pose them (IK)"),
            Tool::Bind => t("Bind", None, false, "Click a shape bone, then click points of the shape to make them follow it (Alt+click: back to automatic)"),
            Tool::Bucket => t("Paint Bucket", Some(K), false, "Fill a shape (or tint an instance) with the fill colour"),
            Tool::InkBottle => t("Ink Bottle", Some(S), false, "Give a shape's outline the stroke colour and size"),
            Tool::Eyedropper => t("Eyedropper", Some(I), false, "Pick a colour from an object (fill → Paint Bucket, stroke → Ink Bottle)"),
            Tool::Eraser => t("Eraser", Some(E), false, "Erase whole objects you drag over (cutting fills needs Merge Drawing)"),
            Tool::Width => t("Width", Some(U), false, "Drag out from a stroke to make it thicker or thinner at that point"),
            Tool::AssetWarp => t("Asset Warp", Some(W), true, "Click a shape, bitmap or symbol to pin it, drag pins to bend it. Alt+click removes a pin"),
            Tool::RasterBrush => t("Raster Brush", None, false, "Paint pixels with MyPaint brushes (pencil, ink, charcoal, watercolour, airbrush…) into a bitmap on this frame. From OpenToonz"),
            Tool::Magnet => t("Magnet", None, false, "Drag near lines and shapes to pull them along (falloff over the Size radius). From OpenToonz"),
            Tool::Pinch => t("Pinch", None, false, "Drag a point of a line: the line follows over Size units each side; Corner makes it pointed. From OpenToonz"),
            Tool::Pump => t("Pump", None, false, "Press on a line and drag up to thicken it there, down to thin it (over Size units). From OpenToonz"),
            Tool::Bender => t("Bender", None, false, "Drag a segment across lines, then drag on one side of it to bend them around its start. From OpenToonz"),
            Tool::Iron => t("Iron", None, false, "Brush over wobbly lines to smooth them (Size, Strength). From OpenToonz"),
            Tool::Assistant => t("Assistants", None, false, "Place guides (line, vanishing point, ellipse, perspective, fisheye) that straighten freehand strokes, replicators (mirror, star, grid, affine) that repeat them, and jitter. From OpenToonz"),
            Tool::Particles => t("Particle Emitter", None, false, "Click to place an emitter that sprays dots, stars or a library symbol; drag to aim it (length = speed). Edit it in Properties"),
            Tool::Camera => t("Camera", Some(C), false, "Pan the camera by dragging, scroll to zoom, Shift+drag to rotate. Tweenable like any layer"),
            Tool::Hand => t("Hand", Some(H), false, "Drag to pan (or hold Space with any tool)"),
            Tool::RotateView => t("Rotation", Some(H), true, "Rotate the view of the stage (like turning paper). Shift snaps to 15°"),
            Tool::Zoom => t("Zoom", Some(Z), false, "Click to zoom in, Alt+click out, drag a box to zoom to it"),
        }
    }
    pub fn shortcut_label(self) -> String {
        let i = self.info();
        match i.key {
            Some(k) => format!("{}{}", if i.shift { "Shift+" } else { "" }, k.symbol_or_name()),
            None => String::new(),
        }
    }
    pub fn usable(self) -> bool {
        self.info().planned.is_none()
    }
    pub fn tooltip(self) -> String {
        let i = self.info();
        let sc = self.shortcut_label();
        let head = if sc.is_empty() { i.name.to_string() } else { format!("{} ({sc})", i.name) };
        match i.planned {
            Some(why) => format!("{head}\nPlanned: {why}"),
            None => format!("{head}\n{}", i.tip),
        }
    }
    pub fn is_drawing(self) -> bool {
        matches!(self, Tool::Rect | Tool::Oval | Tool::RectPrimitive | Tool::OvalPrimitive | Tool::PolyStar | Tool::Line | Tool::Pencil | Tool::Brush | Tool::Pen | Tool::Text)
    }
}

/// Toolbar layout: sections → slots (flyout groups) → tools. Animate order.
pub const SECTIONS: &[&[&[Tool]]] = &[
    &[&[Tool::Select], &[Tool::Subselect], &[Tool::FreeTransform, Tool::GradientTransform], &[Tool::Rotate3D, Tool::Translate3D], &[Tool::Lasso, Tool::PolyLasso, Tool::MagicWand]],
    &[&[Tool::Pen, Tool::AddAnchor, Tool::DeleteAnchor, Tool::ConvertAnchor], &[Tool::Text], &[Tool::Line], &[Tool::Rect, Tool::RectPrimitive, Tool::Oval, Tool::OvalPrimitive, Tool::PolyStar]],
    &[&[Tool::Brush, Tool::FluidBrush, Tool::Pencil, Tool::RasterBrush], &[Tool::Width], &[Tool::Magnet, Tool::Pinch, Tool::Pump, Tool::Bender, Tool::Iron], &[Tool::Assistant], &[Tool::Particles], &[Tool::Bone, Tool::Bind], &[Tool::AssetWarp], &[Tool::Bucket, Tool::InkBottle], &[Tool::Eyedropper], &[Tool::Eraser]],
    &[&[Tool::Camera], &[Tool::Hand, Tool::RotateView], &[Tool::Zoom]],
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FtMode {
    /// Scale, rotate and skew.
    Normal,
    /// Move corners independently (shapes).
    Distort,
    /// Bend with a 3×3 mesh of handles (shapes).
    Envelope,
}

pub fn all_tools() -> impl Iterator<Item = Tool> {
    SECTIONS.iter().flat_map(|s| s.iter()).flat_map(|slot| slot.iter().copied())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PencilMode {
    Straighten,
    Smooth,
    Ink,
}

/// Per-tool settings shown in the Tool tab of the Properties panel.
pub struct ToolOpts {
    pub rect_radius: f32,
    pub poly_sides: usize,
    pub poly_star: bool,
    pub poly_inner: f32,
    /// Oval options: start/end angle (degrees) and inner radius (%).
    pub oval_start: f32,
    pub oval_end: f32,
    pub oval_inner: f32,
    pub pencil_mode: PencilMode,
    pub brush_size: f32,
    pub brush_smooth: f32,
    pub brush_shape: ac_core::merge::BrushShape,
    /// Magic Wand colour tolerance (0–200, Animate's default 10).
    pub wand_threshold: f32,
    pub eraser_size: f32,
    pub ft_mode: FtMode,
    pub fluid_size: f32,
    pub fluid_sensitivity: f32,
    pub show_bones: bool,
    /// Fill type for new shapes: None = solid, Some(kind) = gradient from fill → fill2.
    pub fill_gradient: Option<ac_core::GradientKind>,
    pub fill2: Color32,
    /// All the stops of a gradient swatch (used while its ends still match fill → fill2).
    pub grad_stops: Option<Vec<(f32, ac_core::Color)>>,
    pub text_size: f32,
    /// Text tool: Static, Dynamic or Input text.
    pub text_kind: ac_core::text::TextKind,
    pub text_font: FontFace,
    pub text_bold: bool,
    pub text_italic: bool,
    pub text_align: Align,
    pub snap: bool,
    /// Object Drawing (J): new shapes are separate drawing objects. Off =
    /// Merge Drawing: shapes cut and merge with the layer's shape (Animate's default).
    pub object_draw: bool,
    pub brush_mode: ac_core::merge::PaintMode,
    pub eraser_mode: ac_core::merge::PaintMode,
    /// Eraser Faucet: click a fill or line segment to delete it.
    pub faucet: bool,
    /// Paint Bucket gap size: 0 don't close, 1 small, 2 medium, 3 large.
    pub bucket_gap: u8,
    /// Lock Fill: gradients span the stage instead of each shape.
    pub lock_fill: bool,
    /// Deformation tools (OpenToonz): radius / reach, Pinch corner, Iron strength.
    pub sculpt_size: f32,
    pub sculpt_sharp: bool,
    pub sculpt_strength: f32,
    /// Raster Brush: brush (index into `ac_core::paint::PRESETS`), radius
    /// (0 = the brush's own), opacity, erase.
    pub raster_brush: usize,
    pub raster_size: f32,
    pub raster_opacity: f32,
    pub raster_eraser: bool,
    /// Pressure the brush sees (mouse input has none).
    pub raster_pressure: f32,
    /// Use the drawing assistants (guides + symmetry) with freehand tools.
    pub assist_on: bool,
    /// Remembered tool per slot (slot index → tool).
    pub slot_memory: Vec<Tool>,
    pub hidden: BTreeSet<Tool>,
    pub flyout: Option<(usize, Rect)>,
    pub edit_toolbar_open: bool,
}

impl Default for ToolOpts {
    fn default() -> Self {
        ToolOpts {
            rect_radius: 0.0,
            poly_sides: 5,
            poly_star: true,
            poly_inner: 0.5,
            oval_start: 0.0,
            oval_end: 0.0,
            oval_inner: 0.0,
            pencil_mode: PencilMode::Smooth,
            brush_size: 10.0,
            brush_smooth: 50.0,
            brush_shape: Default::default(),
            wand_threshold: 10.0,
            eraser_size: 12.0,
            ft_mode: FtMode::Normal,
            fluid_size: 14.0,
            fluid_sensitivity: 50.0,
            show_bones: true,
            fill_gradient: None,
            fill2: Color32::WHITE,
            grad_stops: None,
            text_size: 28.0,
            text_kind: ac_core::text::TextKind::Static,
            text_font: FontFace::Sans,
            text_bold: false,
            text_italic: false,
            text_align: Align::Left,
            snap: true,
            object_draw: false,
            brush_mode: Default::default(),
            eraser_mode: Default::default(),
            faucet: false,
            bucket_gap: 1,
            lock_fill: false,
            raster_brush: ac_core::paint::PRESETS.iter().position(|p| p.id == "classic_pencil").unwrap_or(0),
            raster_size: 0.0,
            raster_opacity: 1.0,
            raster_eraser: false,
            raster_pressure: 0.6,
            assist_on: true,
            sculpt_size: 40.0,
            sculpt_sharp: false,
            sculpt_strength: 0.5,
            slot_memory: SECTIONS.iter().flat_map(|s| s.iter()).map(|slot| slot[0]).collect(),
            hidden: BTreeSet::new(),
            flyout: None,
            edit_toolbar_open: false,
        }
    }
}

/// Keyboard shortcuts for tools (call when no text field has focus).
pub fn tool_shortcuts(app: &mut App, ctx: &egui::Context) {
    // egui ignores extra Shift when matching, so Shift+key tools must be checked first.
    let mut tools: Vec<Tool> = all_tools().collect();
    tools.sort_by_key(|t| !t.info().shift);
    for tool in tools {
        let i = tool.info();
        let Some(k) = i.key else { continue };
        let m = if i.shift { Modifiers::SHIFT } else { Modifiers::NONE };
        if ctx.input_mut(|inp| inp.consume_key(m, k)) {
            app.set_tool(tool);
        }
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::X)) {
        std::mem::swap(&mut app.fill, &mut app.stroke);
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::D)) {
        app.stroke = Color32::BLACK;
        app.fill = Color32::WHITE;
        app.stroke_on = true;
        app.fill_on = true;
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::J)) {
        app.topts.object_draw = !app.topts.object_draw;
        app.status = if app.topts.object_draw { "Object Drawing on: each shape is a separate object" } else { "Merge Drawing: shapes cut and merge with the layer's shape" }.into();
    }
}

pub fn panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::new(app.layout.tools_side(), "tools").exact_width(92.0).resizable(false).show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            crate::dock::grip(app, ui, crate::dock::Module::Tools, None);
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                let mut slot_index = 0usize;
                for (si, section) in SECTIONS.iter().enumerate() {
                    // Two columns, like Animate's expanded toolbar.
                    let mut slots: Vec<(usize, Vec<Tool>)> = Vec::new();
                    for slot in section.iter() {
                        let visible: Vec<Tool> = slot.iter().copied().filter(|t| !app.topts.hidden.contains(t)).collect();
                        if !visible.is_empty() {
                            slots.push((slot_index, visible));
                        }
                        slot_index += 1;
                    }
                    for pair in slots.chunks(2) {
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            for (idx, tools) in pair {
                                slot_button(app, ui, *idx, tools);
                            }
                        });
                    }
                    if !slots.is_empty() && si + 1 < SECTIONS.len() {
                        ui.add_space(3.0);
                        let (r, _) = ui.allocate_exact_size(vec2(76.0, 1.0), Sense::hover());
                        ui.painter().rect_filled(r, 0.0, Color32::from_gray(0x48));
                        ui.add_space(3.0);
                    }
                }
                // Edit Toolbar
                if ui.add(egui::Button::new("…").min_size(vec2(76.0, 18.0))).on_hover_text("Edit Toolbar: show or hide tools").clicked() {
                    app.topts.edit_toolbar_open = true;
                }
                ui.add_space(6.0);
                color_controls(app, ui);
            });
        });
    });
    flyout(app, ctx);
    edit_toolbar(app, ctx);
}

fn slot_button(app: &mut App, ui: &mut egui::Ui, idx: usize, tools: &[Tool]) {
    let remembered = app.topts.slot_memory.get(idx).copied().unwrap_or(tools[0]);
    let shown = if tools.contains(&app.tool) { app.tool } else if tools.contains(&remembered) { remembered } else { tools[0] };
    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 32.0), Sense::click_and_drag());
    let active = app.tool == shown;
    let bg = if active { Color32::from_gray(0x1E) } else if resp.hovered() { Color32::from_gray(0x44) } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 4.0, bg);
    let col = if !shown.usable() { Color32::from_gray(95) } else if active { ACCENT } else { Color32::from_gray(210) };
    draw_icon(ui.painter(), rect, shown, col);
    if tools.len() > 1 {
        // flyout indicator (small corner triangle)
        let c = rect.right_bottom() - vec2(4.0, 4.0);
        ui.painter().add(egui::Shape::convex_polygon(vec![c, c - vec2(5.0, 0.0), c - vec2(0.0, 5.0)], Color32::from_gray(150), Stroke::NONE));
    }
    let held = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.press_start_time().map(|t| i.time - t > 0.45).unwrap_or(false));
    let resp = resp.on_hover_text(shown.tooltip());
    if (resp.secondary_clicked() || held) && tools.len() > 1 {
        app.topts.flyout = Some((idx, rect));
    } else if resp.clicked() {
        app.set_tool(shown);
    }
}

fn flyout(app: &mut App, ctx: &egui::Context) {
    let Some((idx, anchor)) = app.topts.flyout else { return };
    let slot: Vec<Tool> = SECTIONS.iter().flat_map(|s| s.iter()).nth(idx).map(|s| s.iter().copied().filter(|t| !app.topts.hidden.contains(t)).collect()).unwrap_or_default();
    let area = egui::Area::new(egui::Id::new("tool-flyout")).order(egui::Order::Foreground).fixed_pos(anchor.right_top() + vec2(4.0, 0.0));
    let r = area.show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            for tool in slot {
                let label = format!("{:<24} {}", tool.info().name, tool.shortcut_label());
                let txt = if tool.usable() { RichText::new(label) } else { RichText::new(format!("{label}   (planned)")).color(Color32::from_gray(120)) };
                if ui.selectable_label(app.tool == tool, txt).on_hover_text(tool.tooltip()).clicked() {
                    app.set_tool(tool);
                    app.topts.flyout = None;
                }
            }
        });
    });
    let clicked_outside = ctx.input(|i| i.pointer.any_pressed()) && !r.response.rect.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default()) && !anchor.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default());
    if clicked_outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.topts.flyout = None;
    }
}

fn edit_toolbar(app: &mut App, ctx: &egui::Context) {
    if !app.topts.edit_toolbar_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Edit Toolbar").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.label("Untick tools to hide them from the Tools panel.");
        ui.label(RichText::new("Dimmed tools are planned and not usable yet.").small().color(Color32::from_gray(150)));
        ui.separator();
        egui::Grid::new("edit-toolbar").num_columns(2).show(ui, |ui| {
            for (i, tool) in all_tools().enumerate() {
                let mut shown = !app.topts.hidden.contains(&tool);
                let txt = if tool.usable() { RichText::new(tool.info().name) } else { RichText::new(tool.info().name).color(Color32::from_gray(120)) };
                if ui.checkbox(&mut shown, txt).changed() {
                    if shown { app.topts.hidden.remove(&tool); } else { app.topts.hidden.insert(tool); }
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Show all").clicked() { app.topts.hidden.clear(); }
            if ui.button("Hide planned tools").clicked() {
                for tl in all_tools().filter(|t| !t.usable()) { app.topts.hidden.insert(tl); }
            }
        });
    });
    app.topts.edit_toolbar_open = open;
}

fn color_controls(app: &mut App, ui: &mut egui::Ui) {
    let sw = |ui: &mut egui::Ui, label: &str, c: &mut Color32, on: &mut bool, tip: &str| {
        ui.label(RichText::new(label).small());
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.add_enabled_ui(*on, |ui| ui.color_edit_button_srgba(c)).response.on_hover_text(tip);
        });
        let (r, resp) = ui.allocate_exact_size(vec2(36.0, 14.0), Sense::click());
        ui.painter().rect_stroke(r.shrink(1.0), 2.0, Stroke::new(1.0_f32, Color32::from_gray(120)));
        if !*on {
            ui.painter().line_segment([r.left_bottom(), r.right_top()], Stroke::new(1.5_f32, Color32::from_rgb(0xE5, 0x3B, 0x3B)));
        }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, if *on { "on" } else { "none" }, FontId::proportional(9.0), Color32::from_gray(170));
        if resp.on_hover_text("No colour on/off").clicked() {
            *on = !*on;
        }
    };
    sw(ui, "Stroke", &mut app.stroke, &mut app.stroke_on, "Stroke colour");
    sw(ui, "Fill", &mut app.fill, &mut app.fill_on, "Fill colour");
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        if ui.small_button("Swap").on_hover_text("Swap stroke and fill colours (X)").clicked() {
            std::mem::swap(&mut app.fill, &mut app.stroke);
        }
        if ui.small_button("B/W").on_hover_text("Default black stroke, white fill (D)").clicked() {
            app.stroke = Color32::BLACK;
            app.fill = Color32::WHITE;
            app.stroke_on = true;
            app.fill_on = true;
        }
    });
    ui.add(egui::DragValue::new(&mut app.stroke_w).range(0.1..=100.0).speed(0.1).suffix(" px")).on_hover_text("Stroke size");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.toggle_value(&mut app.topts.snap, "Snap").on_hover_text("Snap to objects and the stage (hold Alt to bypass)");
        let obj = app.topts.object_draw;
        let tip = if obj { "Object Drawing is on: each new shape is its own object (J toggles Merge Drawing)" } else { "Merge Drawing: new shapes cut and merge with the layer's shape (J toggles Object Drawing)" };
        if ui.selectable_label(obj, "Obj").on_hover_text(tip).clicked() {
            app.topts.object_draw = !obj;
            app.status = if app.topts.object_draw { "Object Drawing on: each shape is a separate object" } else { "Merge Drawing: shapes cut and merge with the layer's shape" }.into();
        }
    });
}

/// Flare's own tool icons (`assets/flare-icons/tools`, from the Flare repo,
/// OpenToonz lineage, BSD-3-Clause) where Flare has one for the tool; the rest
/// keep the drawn icons below.
fn flare_icon_svg(t: Tool) -> Option<(&'static str, &'static str)> {
    macro_rules! svg {
        ($n:literal) => {
            Some(($n, include_str!(concat!("../assets/flare-icons/tools/", $n, ".svg"))))
        };
    }
    // our own icons in the same style, for tools Flare has none for
    macro_rules! own {
        ($n:literal) => {
            Some(($n, include_str!(concat!("../assets/icons/tools/", $n, ".svg"))))
        };
    }
    match t {
        Tool::Lasso => svg!("selection_freehand"),
        Tool::PolyLasso => svg!("selection_polyline"),
        Tool::Line => svg!("geometric_line"),
        Tool::Rect => svg!("geometric_rectangle"),
        Tool::Oval => svg!("geometric_ellipse"),
        Tool::PolyStar => svg!("geometric_polygon"),
        Tool::Camera => svg!("camera"),
        Tool::Pen => own!("pen"),
        Tool::AddAnchor => own!("pen_add"),
        Tool::DeleteAnchor => own!("pen_delete"),
        Tool::ConvertAnchor => own!("convert_anchor"),
        Tool::Pencil => own!("pencil"),
        Tool::FluidBrush => own!("fluid_brush"),
        Tool::Width => own!("width"),
        Tool::InkBottle => own!("ink_bottle"),
        Tool::GradientTransform => own!("gradient_transform"),
        Tool::Rotate3D => own!("rotate_3d"),
        Tool::Translate3D => own!("translate_3d"),
        Tool::MagicWand => own!("magic_wand"),
        Tool::RectPrimitive => own!("rect_primitive"),
        Tool::OvalPrimitive => own!("oval_primitive"),
        Tool::Particles => own!("particles"),
        Tool::Select => svg!("selection"),
        Tool::Subselect => svg!("controlpointeditor"),
        Tool::FreeTransform => svg!("animate"),
        Tool::Text => svg!("type"),
        Tool::Brush => svg!("brush"),
        Tool::RasterBrush => svg!("paintbrush"),
        Tool::Bone => svg!("skeleton"),
        Tool::Bind => svg!("hook"),
        Tool::Bucket => svg!("fill"),
        Tool::Eyedropper => svg!("rgbpicker"),
        Tool::Eraser => svg!("eraser"),
        Tool::AssetWarp => svg!("plastic"),
        Tool::Magnet => svg!("magnet"),
        Tool::Pinch => svg!("pinch"),
        Tool::Pump => svg!("pump"),
        Tool::Bender => svg!("bender"),
        Tool::Iron => svg!("iron"),
        Tool::Assistant => svg!("assistant"),
        Tool::Hand => svg!("hand"),
        Tool::RotateView => svg!("rotate"),
        Tool::Zoom => svg!("zoom"),
    }
}

/// The icon as a white alpha mask texture (rendered once, tinted when drawn).
fn flare_icon_texture(ctx: &egui::Context, t: Tool) -> Option<egui::TextureHandle> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static CACHE: RefCell<HashMap<Tool, Option<egui::TextureHandle>>> = RefCell::new(HashMap::new());
    }
    CACHE.with(|c| {
        c.borrow_mut()
            .entry(t)
            .or_insert_with(|| {
                let (name, svg) = flare_icon_svg(t)?;
                let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
                const PX: u32 = 60; // 3× a 20 px icon: sharp on HiDPI too
                let mut pm = tiny_skia::Pixmap::new(PX, PX)?;
                let sz = tree.size();
                let k = PX as f32 / sz.width().max(sz.height()).max(1.0);
                resvg::render(&tree, tiny_skia::Transform::from_scale(k, k), &mut pm.as_mut());
                // ink = dark and opaque: white parts of an icon are cut-outs
                let px: Vec<Color32> = pm
                    .pixels()
                    .iter()
                    .map(|p| {
                        let c = p.demultiply();
                        let lum = (c.red() as u32 * 3 + c.green() as u32 * 6 + c.blue() as u32) / 10;
                        Color32::from_rgba_unmultiplied(255, 255, 255, ((c.alpha() as u32 * (255 - lum)) / 255) as u8)
                    })
                    .collect();
                let img = egui::ColorImage { size: [PX as usize, PX as usize], pixels: px };
                Some(ctx.load_texture(format!("flare-tool-{name}"), img, egui::TextureOptions::LINEAR))
            })
            .clone()
    })
}

pub fn draw_icon(p: &egui::Painter, r: Rect, t: Tool, col: Color32) {
    if let Some(tex) = flare_icon_texture(p.ctx(), t) {
        let side = r.width().min(r.height()).min(24.0) * 0.86;
        p.image(tex.id(), Rect::from_center_size(r.center(), vec2(side, side)), Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), col);
        return;
    }
    let c = r.center();
    let s = Stroke::new(1.6_f32, col);
    let thin = Stroke::new(1.0_f32, col);
    let poly = |pts: Vec<egui::Pos2>, fill: Color32, st: Stroke| egui::Shape::convex_polygon(pts, fill, st);
    let arrow = |off: egui::Vec2, filled: bool| {
        let pts = vec![c + off + vec2(-5.0, -9.0), c + off + vec2(6.0, 2.0), c + off + vec2(1.0, 2.5), c + off + vec2(4.0, 9.0), c + off + vec2(1.5, 10.0), c + off + vec2(-1.5, 3.5), c + off + vec2(-5.0, 7.0)];
        if filled { poly(pts, col, Stroke::NONE) } else { egui::Shape::closed_line(pts, thin) }
    };
    match t {
        Tool::Select => { p.add(arrow(vec2(0.0, 0.0), true)); }
        Tool::Subselect => { p.add(arrow(vec2(0.0, 0.0), false)); }
        Tool::FreeTransform | Tool::GradientTransform => {
            let rr = Rect::from_center_size(c, vec2(16.0, 14.0));
            p.rect_stroke(rr, 0.0, thin);
            for k in [rr.left_top(), rr.right_top(), rr.left_bottom(), rr.right_bottom()] {
                p.rect_filled(Rect::from_center_size(k, vec2(4.0, 4.0)), 0.0, col);
            }
            if t == Tool::GradientTransform { p.circle_stroke(c, 3.0, thin); }
        }
        Tool::Rotate3D => {
            p.circle_stroke(c, 8.0, Stroke::new(1.4_f32, Color32::from_rgb(0x40, 0x90, 0xFF).gamma_multiply(if col == Color32::from_gray(95) { 0.4 } else { 1.0 })));
            p.line_segment([c + vec2(0.0, -10.0), c + vec2(0.0, 10.0)], Stroke::new(1.6_f32, Color32::from_rgb(0xE0, 0x40, 0x40)));
            p.line_segment([c + vec2(-10.0, 0.0), c + vec2(10.0, 0.0)], Stroke::new(1.6_f32, Color32::from_rgb(0x40, 0xC0, 0x40)));
            if col == ACCENT { p.circle_stroke(c, 10.5, Stroke::new(1.0_f32, ACCENT)); }
        }
        Tool::Translate3D => {
            p.arrow(c, vec2(11.0, 0.0), Stroke::new(1.6_f32, Color32::from_rgb(0xE0, 0x40, 0x40)));
            p.arrow(c, vec2(0.0, -11.0), Stroke::new(1.6_f32, Color32::from_rgb(0x40, 0xC0, 0x40)));
            p.circle_filled(c, 2.5, col);
        }
        Tool::Lasso => {
            let pts: Vec<_> = (0..=20).map(|i| { let a = i as f32 / 20.0 * 5.6; c + vec2(a.cos() * 8.0, a.sin() * 5.0 - 2.0) }).collect();
            p.add(egui::Shape::line(pts, s));
            p.line_segment([c + vec2(6.0, 1.0), c + vec2(4.0, 9.0)], s);
        }
        Tool::MagicWand => {
            p.line_segment([c + vec2(-8.0, 8.0), c + vec2(3.0, -3.0)], Stroke::new(2.2_f32, col));
            for (d, r) in [(vec2(6.0, -6.0), 3.5_f32), (vec2(-2.0, -8.0), 1.8), (vec2(8.0, 2.0), 1.8)] {
                let q = c + d;
                p.line_segment([q - vec2(r, 0.0), q + vec2(r, 0.0)], thin);
                p.line_segment([q - vec2(0.0, r), q + vec2(0.0, r)], thin);
            }
        }
        Tool::PolyLasso => {
            p.add(egui::Shape::closed_line(vec![c + vec2(-8.0, -4.0), c + vec2(0.0, -8.0), c + vec2(8.0, -2.0), c + vec2(3.0, 6.0), c + vec2(-6.0, 4.0)], s));
        }
        Tool::Pen | Tool::AddAnchor | Tool::DeleteAnchor | Tool::ConvertAnchor => {
            p.add(poly(vec![c + vec2(0.0, -9.0), c + vec2(6.0, 3.0), c + vec2(0.0, 6.0), c + vec2(-6.0, 3.0)], Color32::TRANSPARENT, s));
            p.line_segment([c + vec2(0.0, -9.0), c + vec2(0.0, 1.0)], thin);
            let badge = c + vec2(8.0, -7.0);
            match t {
                Tool::AddAnchor => { p.line_segment([badge - vec2(3.0, 0.0), badge + vec2(3.0, 0.0)], s); p.line_segment([badge - vec2(0.0, 3.0), badge + vec2(0.0, 3.0)], s); }
                Tool::DeleteAnchor => { p.line_segment([badge - vec2(3.0, 0.0), badge + vec2(3.0, 0.0)], s); }
                Tool::ConvertAnchor => { p.add(egui::Shape::line(vec![badge + vec2(-3.0, 3.0), badge + vec2(0.0, -3.0), badge + vec2(3.0, 3.0)], thin)); }
                _ => {}
            }
        }
        Tool::Text => { p.text(c, Align2::CENTER_CENTER, "T", FontId::proportional(20.0), col); }
        Tool::Line => { p.line_segment([c + vec2(-8.0, 8.0), c + vec2(8.0, -8.0)], s); }
        Tool::Rect => { p.rect_stroke(Rect::from_center_size(c, vec2(18.0, 14.0)), 0.0, s); }
        Tool::Oval => { p.circle_stroke(c, 8.0, s); }
        Tool::RectPrimitive => {
            p.rect_stroke(Rect::from_center_size(c, vec2(18.0, 14.0)), 4.0, s);
            for d in [vec2(-9.0, -7.0), vec2(9.0, -7.0), vec2(9.0, 7.0), vec2(-9.0, 7.0)] {
                p.circle_filled(c + d, 2.0, col);
            }
        }
        Tool::OvalPrimitive => {
            p.circle_stroke(c, 8.0, s);
            p.circle_filled(c + vec2(8.0, 0.0), 2.2, col);
            p.circle_filled(c, 1.8, col);
        }
        Tool::PolyStar => {
            let pts: Vec<_> = (0..10).map(|i| { let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0; let rr = if i % 2 == 0 { 9.0 } else { 4.0 }; c + vec2(a.cos() * rr, a.sin() * rr) }).collect();
            p.add(egui::Shape::closed_line(pts, s));
        }
        Tool::Pencil => {
            p.line_segment([c + vec2(-6.0, 6.0), c + vec2(7.0, -7.0)], Stroke::new(4.0_f32, col));
            p.add(poly(vec![c + vec2(-6.5, 3.5), c + vec2(-3.5, 6.5), c + vec2(-9.0, 9.0)], col, Stroke::NONE));
        }
        Tool::RasterBrush => {
            p.line_segment([c + vec2(-1.0, 1.0), c + vec2(8.0, -8.0)], Stroke::new(2.0_f32, col));
            // a dabbed, grainy blob
            for (d, r) in [(vec2(-4.0, 4.0), 3.2_f32), (vec2(-7.5, 6.5), 1.6), (vec2(-2.0, 8.0), 1.4), (vec2(-8.0, 1.5), 1.2), (vec2(-5.5, 9.5), 0.9)] {
                p.circle_filled(c + d, r, col);
            }
        }
        Tool::Brush | Tool::FluidBrush => {
            p.line_segment([c + vec2(-1.0, 1.0), c + vec2(8.0, -8.0)], Stroke::new(2.0_f32, col));
            p.circle_filled(c + vec2(-4.0, 4.0), 4.5, col);
            if t == Tool::FluidBrush { p.circle_stroke(c + vec2(-4.0, 4.0), 7.0, thin); }
        }
        Tool::Bone | Tool::Bind => {
            p.line_segment([c + vec2(-7.0, 7.0), c + vec2(7.0, -7.0)], Stroke::new(3.0_f32, col));
            p.circle_filled(c + vec2(-7.0, 7.0), 3.0, col);
            p.circle_filled(c + vec2(7.0, -7.0), 3.0, col);
            if t == Tool::Bind {
                for d in [vec2(4.0, 6.0), vec2(8.0, 2.0)] {
                    p.rect_filled(Rect::from_center_size(c + d, vec2(3.5, 3.5)), 0.0, Color32::from_rgb(0xFF, 0xD0, 0x40));
                }
            }
        }
        Tool::Bucket => {
            p.add(poly(vec![c + vec2(-7.0, -1.0), c + vec2(0.0, -8.0), c + vec2(7.0, -1.0), c + vec2(0.0, 6.0)], Color32::TRANSPARENT, s));
            p.circle_filled(c + vec2(8.0, 6.0), 2.5, col);
        }
        Tool::InkBottle => {
            p.rect_stroke(Rect::from_center_size(c + vec2(0.0, 3.0), vec2(12.0, 11.0)), 2.0, s);
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, -6.0), vec2(6.0, 5.0)), 1.0, col);
        }
        Tool::Eyedropper => {
            p.line_segment([c + vec2(-7.0, 7.0), c + vec2(4.0, -4.0)], Stroke::new(2.5_f32, col));
            p.circle_filled(c + vec2(6.0, -6.0), 3.5, col);
        }
        Tool::Eraser => {
            p.add(poly(vec![c + vec2(-9.0, 4.0), c + vec2(1.0, -7.0), c + vec2(9.0, 0.0), c + vec2(0.0, 9.0)], Color32::TRANSPARENT, s));
            p.line_segment([c + vec2(-4.0, -1.5), c + vec2(4.0, 5.5)], thin);
        }
        Tool::Width => {
            p.add(egui::Shape::line(vec![c + vec2(-9.0, 2.0), c + vec2(-3.0, -4.0), c + vec2(3.0, 4.0), c + vec2(9.0, -2.0)], Stroke::new(3.0_f32, col)));
        }
        Tool::AssetWarp => {
            p.circle_filled(c + vec2(0.0, -3.0), 4.0, col);
            p.line_segment([c + vec2(0.0, 1.0), c + vec2(0.0, 9.0)], s);
        }
        Tool::Magnet => {
            // horseshoe magnet
            let pts: Vec<_> = (0..=12).map(|i| { let a = std::f32::consts::PI * i as f32 / 12.0; c + vec2(a.cos() * 6.0, a.sin() * 6.0 + 0.0) }).collect();
            p.add(egui::Shape::line(pts, Stroke::new(3.0_f32, col)));
            p.line_segment([c + vec2(-6.0, 0.0), c + vec2(-6.0, -7.0)], Stroke::new(3.0_f32, col));
            p.line_segment([c + vec2(6.0, 0.0), c + vec2(6.0, -7.0)], Stroke::new(3.0_f32, col));
            p.line_segment([c + vec2(-7.5, -8.0), c + vec2(-4.5, -8.0)], thin);
            p.line_segment([c + vec2(4.5, -8.0), c + vec2(7.5, -8.0)], thin);
        }
        Tool::Pinch => {
            p.add(egui::Shape::line(vec![c + vec2(-10.0, 6.0), c + vec2(-4.0, 5.0), c + vec2(0.0, -6.0), c + vec2(4.0, 5.0), c + vec2(10.0, 6.0)], s));
            p.circle_filled(c + vec2(0.0, -6.0), 2.2, col);
        }
        Tool::Pump => {
            let pts: Vec<_> = (0..=16).map(|i| { let x = -10.0 + i as f32 * 1.25; c + vec2(x, 2.0) }).collect();
            p.add(egui::Shape::line(pts, thin));
            p.add(poly(vec![c + vec2(-6.0, 2.0), c + vec2(0.0, -3.0), c + vec2(6.0, 2.0), c + vec2(0.0, 7.0)], col, Stroke::NONE));
            p.arrow(c + vec2(0.0, -4.0), vec2(0.0, -5.0), thin);
        }
        Tool::Bender => {
            let pts: Vec<_> = (0..=12).map(|i| { let a = std::f32::consts::FRAC_PI_2 * i as f32 / 12.0; c + vec2(-8.0 + (1.0 - a.cos()) * 0.0 + a.sin() * 12.0, 8.0 - (1.0 - a.cos()) * 12.0) }).collect();
            p.add(egui::Shape::line(pts, s));
            p.line_segment([c + vec2(-3.0, -9.0), c + vec2(-3.0, 9.0)], Stroke::new(1.0_f32, col.gamma_multiply(0.6)));
        }
        Tool::Iron => {
            p.add(poly(vec![c + vec2(-9.0, 4.0), c + vec2(7.0, 4.0), c + vec2(9.0, -1.0), c + vec2(1.0, -4.0), c + vec2(-6.0, -4.0)], col, Stroke::NONE));
            p.line_segment([c + vec2(-3.0, -4.0), c + vec2(-1.0, -9.0)], s);
            p.line_segment([c + vec2(-1.0, -9.0), c + vec2(6.0, -9.0)], s);
            p.line_segment([c + vec2(-10.0, 8.0), c + vec2(10.0, 8.0)], thin);
        }
        Tool::Assistant => {
            // a dashed mirror line between a stroke and its reflection
            for k in 0..5 {
                let y = -10.0 + k as f32 * 4.5;
                p.line_segment([c + vec2(0.0, y), c + vec2(0.0, y + 2.5)], thin);
            }
            let arc = |sx: f32| -> Vec<egui::Pos2> { (0..=10).map(|i| { let a = -1.2 + i as f32 * 0.24; c + vec2(sx * (3.0 + a.cos() * 5.5), a.sin() * 8.0) }).collect() };
            p.add(egui::Shape::line(arc(-1.0), s));
            p.add(egui::Shape::line(arc(1.0), s));
        }
        Tool::Particles => {
            // a spray of dots from a nozzle
            p.circle_filled(c + vec2(-7.0, 7.0), 2.5, col);
            for (dx, dy, r) in [(-1.0, 1.0, 1.6), (3.0, -1.0, 1.4), (-2.0, -4.0, 1.3), (6.0, -6.0, 1.1), (1.0, -8.0, 1.0), (8.0, 0.0, 1.0), (-6.0, -8.0, 0.9)] {
                p.circle_filled(c + vec2(dx, dy), r, col);
            }
        }
        Tool::Camera => {
            p.rect_stroke(Rect::from_center_size(c + vec2(-2.0, 1.0), vec2(14.0, 10.0)), 2.0, s);
            p.add(poly(vec![c + vec2(5.0, 1.0), c + vec2(10.0, -3.0), c + vec2(10.0, 5.0)], col, Stroke::NONE));
        }
        Tool::Hand => {
            for (i, h) in [6.0, 8.0, 8.0, 6.5].iter().enumerate() {
                let x = -4.5 + i as f32 * 3.0;
                p.line_segment([c + vec2(x, 2.0), c + vec2(x, 2.0 - h)], Stroke::new(2.0_f32, col));
            }
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, 4.0), vec2(12.0, 8.0)), 3.0, col);
        }
        Tool::RotateView => {
            let pts: Vec<_> = (0..=16).map(|i| { let a = 0.6 + i as f32 / 16.0 * 4.8; c + vec2(a.cos() * 8.0, a.sin() * 8.0) }).collect();
            p.add(egui::Shape::line(pts, s));
            p.add(poly(vec![c + vec2(6.0, -9.0), c + vec2(10.0, -3.0), c + vec2(3.0, -3.0)], col, Stroke::NONE));
        }
        Tool::Zoom => {
            p.circle_stroke(c + vec2(-2.0, -2.0), 6.0, s);
            p.line_segment([c + vec2(2.5, 2.5), c + vec2(8.0, 8.0)], Stroke::new(2.5_f32, col));
        }
    }
    let _ = pos2;
}
