//! The right-hand dock: Properties, Library and Assets (docs/animate-ui.md §7.2,
//! §7.3), and beside it the collapsed icon strip whose panels open as flyouts (§4.6).

use eframe::egui::{self, Align, Color32, FontId, Id, Layout, Margin, Order, Rect, RichText, ScrollArea, Sense, Stroke, Ui, pos2, vec2};
use minnamate_core::doc::{Cap, ColorTransform, Element, Instance, Join, Loop, Paint, Rgba, SymbolKind, TextAlign, Tween};
use minnamate_core::filters::{Blend, CACHE_AS_BITMAP, EXPORT_AS_BITMAP};
use minnamate_core::{fonts, hit};

use crate::app::{Action, Editor, PanelId, PropsTab, rgba};
use crate::color::{self, Style};
use crate::icons::{self, Icon};
use crate::theme;
use crate::tools::Tool;
use crate::widgets::{self, LABEL_W, Number, WORD_W};

pub fn right(ui: &mut Ui, ed: &mut Editor) {
    let tab = ed.right_tab;
    let tabs = [ed.tab_label(PanelId::Properties, "Properties"), ed.tab_label(PanelId::Library, "Library"), ed.tab_label(PanelId::Assets, "Assets")];
    let clicked = theme::tab_strip_menu(ui, &tabs, tab, Some(|ui: &mut Ui| match tab {
        1 => crate::library::panel_menu(ui, ed),
        _ => menu_tail(ui, ed),
    }));
    if let Some(tab) = clicked {
        ed.right_tab = tab;
    }
    match ed.right_tab {
        0 => properties(ui, ed),
        1 => crate::library::panel(ui, ed),
        _ => assets(ui),
    }
}

/// How every panel menu ends: Lock, Help, then Close and Close Group.
pub fn menu_tail(ui: &mut Ui, ed: &mut Editor) {
    // ponytail: docked panels don't move yet, so Lock has nothing to lock; Help has no pages.
    theme::menu_item(ui, false, "Lock", "");
    ui.separator();
    theme::menu_item(ui, false, "Help", "");
    ui.separator();
    // ponytail: the group's panels share one dock, so closing one closes them all.
    let close = theme::menu_item(ui, true, "Close", "").clicked();
    if theme::menu_item(ui, true, "Close Group", "").clicked() || close {
        ed.show_right = false;
    }
}

/// The Assets panel: Animate's ready-made animated and static assets. Ours has none yet.
fn assets(ui: &mut Ui) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            for (i, label) in ["Animated", "Static"].into_iter().enumerate() {
                let text = RichText::new(label).color(if i == 0 { Color32::WHITE } else { theme::TEXT_DIM });
                let resp = ui.add(egui::Label::new(text));
                if i == 0 {
                    ui.painter().hline(resp.rect.x_range(), resp.rect.bottom() + 3.0, Stroke::new(2.0, theme::ACCENT));
                }
                ui.add_space(10.0);
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new("No assets yet.").color(theme::TEXT_DIM));
    });
}

/// A panel of the collapsed icon strip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flyout {
    Color,
    Swatches,
    Align,
    Info,
    Transform,
    MotionPresets,
}

impl Flyout {
    /// The flyout a Window menu item opens, for the panels the icon strip holds.
    pub fn of(panel: PanelId) -> Option<Flyout> {
        Some(match panel {
            PanelId::Color => Flyout::Color,
            PanelId::Swatches => Flyout::Swatches,
            PanelId::Align => Flyout::Align,
            PanelId::Info => Flyout::Info,
            PanelId::Transform => Flyout::Transform,
            _ => return None,
        })
    }

    const ALL: [(Flyout, Icon, &'static str); 6] = [
        (Flyout::Color, Icon::ColorPanel, "Color"),
        (Flyout::Swatches, Icon::Swatches, "Swatches"),
        (Flyout::Align, Icon::Align, "Align"),
        (Flyout::Info, Icon::Info, "Info"),
        (Flyout::Transform, Icon::TransformPanel, "Transform"),
        (Flyout::MotionPresets, Icon::MotionPresets, "Motion Presets"),
    ];
}

/// The collapsed-panel strip beside Properties: a 32×32 button per panel; a
/// click opens that panel as a flyout to the strip's left, another closes it.
pub fn icon_strip(ui: &mut Ui, ed: &mut Editor) {
    let mut anchor = None;
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.add_space(4.0);
        for (panel, icon, name) in Flyout::ALL {
            let open = ed.flyout == Some(panel);
            // Glyphs rest in the dim text colour, brightening on hover or while open.
            let resp = theme::icon_button(ui, 32.0, open, true, name, |p, r, c| {
                icons::paint(p, r.shrink(7.0), icon, if c == theme::TEXT { theme::TEXT_DIM } else { c })
            });
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, name));
            if resp.clicked() {
                ed.flyout = if open { None } else { Some(panel) };
            }
            if ed.flyout == Some(panel) {
                anchor = Some(resp.rect);
            }
        }
    });
    if let (Some(panel), Some(at)) = (ed.flyout, anchor) {
        flyout(ui, ed, panel, pos2(ui.max_rect().left() - theme::GAP as f32, at.top()));
    }
}

/// An icon-strip panel, open to the left of `right_top`: its tab (a click
/// collapses it) and panel menu, then the panel.
fn flyout(ui: &Ui, ed: &mut Editor, panel: Flyout, right_top: egui::Pos2) {
    let (_, _, name) = Flyout::ALL.into_iter().find(|f| f.0 == panel).unwrap_or(Flyout::ALL[0]);
    let width = match panel {
        Flyout::Color => 270.0,
        Flyout::Align | Flyout::Info | Flyout::Transform => 264.0,
        _ => 250.0,
    };
    let frame = egui::Frame::NONE.fill(theme::PANEL_BG).stroke(Stroke::new(1.0, theme::POPUP_BORDER)).shadow(ui.visuals().popup_shadow);
    // Below the swatch popup and its eyedropper, which may open from inside.
    egui::Area::new(Id::new(("flyout", name))).order(Order::Middle).pivot(egui::Align2::RIGHT_TOP).fixed_pos(right_top).show(ui.ctx(), |ui| {
        frame.show(ui, |ui| {
            ui.set_width(width);
            let mut closing = false;
            let tab = theme::tab_strip_menu(ui, &[name], 0, Some(|ui: &mut Ui| {
                match panel {
                    Flyout::Color => {
                        theme::menu_item(ui, false, "Add Swatch", "");
                        ui.separator();
                    }
                    // ponytail: the palette is Web 216 alone until documents keep swatches.
                    Flyout::Swatches => {
                        for group in [&["Duplicate Swatch", "Delete Swatch"][..], &["Add Colors...", "Replace Colors..."], &["Load Default Colors", "Save Colors...", "Save as Default", "Clear Colors"], &["Web 216", "Sort by Color"]] {
                            for item in group {
                                theme::menu_item(ui, false, item, "");
                            }
                            ui.separator();
                        }
                    }
                    _ => {}
                }
                theme::menu_item(ui, false, "Help", "");
                ui.separator();
                closing = theme::menu_item(ui, true, "Close", "").clicked();
                closing |= theme::menu_item(ui, true, "Close Group", "").clicked();
            }));
            egui::Frame::NONE.inner_margin(Margin::same(8)).show(ui, |ui| match panel {
                Flyout::Color => color::color_panel(ui, ed),
                Flyout::Swatches => {
                    ui.set_min_height(200.0);
                    color::swatches_panel(ui, ed);
                }
                Flyout::Align => crate::arrange_panels::align(ui, ed),
                Flyout::Info => crate::arrange_panels::info(ui, ed),
                Flyout::Transform => crate::arrange_panels::transform(ui, ed),
                Flyout::MotionPresets => coming(ui, "Motion Presets"),
            });
            if tab.is_some() || closing {
                ed.flyout = None;
            }
        });
    });
}

fn coming(ui: &mut Ui, name: &str) {
    ui.set_min_height(120.0);
    ui.label(RichText::new(format!("The {name} panel isn't built yet.")).color(theme::TEXT_DIM));
}

// ---- Properties -------------------------------------------------------------------------

const TABS: [(PropsTab, &str); 4] = [(PropsTab::Tool, "Tool"), (PropsTab::Object, "Object"), (PropsTab::Frame, "Frame"), (PropsTab::Doc, "Doc")];

/// Tools whose settings Properties shows: all but the selecting and viewing ones.
fn has_tool_tab(tool: Tool) -> bool {
    !matches!(tool, Tool::Selection | Tool::Subselection | Tool::Hand | Tool::Zoom)
}

/// The tools with an Object Drawing mode (J).
fn object_drawing(tool: Tool) -> bool {
    matches!(tool, Tool::Pencil | Tool::Line | Tool::Pen | Tool::Brush | Tool::Oval | Tool::Rectangle | Tool::PolyStar)
}

/// Whether a category tab can be picked: Tool when the tool has settings,
/// Object with a selection, Frame for frames picked in the timeline, Doc always.
fn tab_enabled(ed: &Editor, tab: PropsTab) -> bool {
    match tab {
        PropsTab::Tool => has_tool_tab(ed.tool),
        PropsTab::Object => !ed.selection.is_empty(),
        // ponytail: the timeline keeps no frame selection; clicking a frame shows this
        // tab, so it stays on until another is shown. Ask the timeline once it has one.
        PropsTab::Frame => ed.props_tab == PropsTab::Frame,
        PropsTab::Doc => true,
    }
}

fn properties(ui: &mut Ui, ed: &mut Editor) {
    // Picking a tool with settings shows them, as Animate does.
    let last = Id::new("props-last-tool");
    if ui.data(|d| d.get_temp::<Tool>(last)) != Some(ed.tool) {
        ui.data_mut(|d| d.insert_temp(last, ed.tool));
        if has_tool_tab(ed.tool) {
            ed.props_tab = PropsTab::Tool;
        }
    }
    if !tab_enabled(ed, ed.props_tab) {
        ed.props_tab = if ed.selection.is_empty() { PropsTab::Doc } else { PropsTab::Object };
    }
    category_row(ui, ed);
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        match ed.props_tab {
            PropsTab::Doc => doc_props(ui, ed),
            PropsTab::Frame => frame_props(ui, ed),
            PropsTab::Object => object_props(ui, ed),
            PropsTab::Tool => tool_props(ui, ed),
        }
        ui.add_space(12.0);
    });
}

/// Tool | Object | Frame | Doc in four equal columns; the shown one sits on a pill.
fn category_row(ui: &mut Ui, ed: &mut Editor) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    let w = rect.width() / 4.0;
    for (i, (tab, label)) in TABS.into_iter().enumerate() {
        let col = Rect::from_min_size(rect.min + vec2(w * i as f32, 0.0), vec2(w, rect.height()));
        let enabled = tab_enabled(ed, tab);
        let resp = ui.interact(col, ui.id().with(("props-tab", label)), if enabled { Sense::click() } else { Sense::hover() });
        let p = ui.painter();
        let pill = Rect::from_center_size(col.center(), vec2(w - 8.0, 28.0));
        if ed.props_tab == tab {
            p.rect_filled(pill, 5.0, theme::CATEGORY_PILL);
        } else if enabled && resp.hovered() {
            p.rect_filled(pill, 5.0, theme::BUTTON_HOVER);
        }
        let color = if enabled { theme::TEXT } else { theme::TEXT_DISABLED };
        p.text(col.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(13.0), color);
        let selected = ed.props_tab == tab;
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, selected, label));
        if resp.clicked() {
            ed.props_tab = tab;
        }
    }
    let (divider, _) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0), Sense::hover());
    ui.painter().rect_filled(divider, 0.0, theme::BORDER);
}

/// The row under the tabs: a 20 px glyph and the name, 48 px tall; with a
/// second line (Doc's document type) 56.
fn header(ui: &mut Ui, icon: Icon, title: &str, subtitle: Option<&str>) {
    let h = if subtitle.is_some() { 56.0 } else { 48.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title));
    let p = ui.painter();
    icons::paint(p, Rect::from_center_size(pos2(rect.left() + 26.0, rect.center().y), vec2(20.0, 20.0)), icon, theme::TEXT);
    let x = rect.left() + 46.0;
    match subtitle {
        Some(sub) => {
            p.text(pos2(x, rect.center().y - 1.0), egui::Align2::LEFT_BOTTOM, title, FontId::proportional(13.0), theme::TEXT_DIM);
            p.text(pos2(x, rect.center().y + 1.0), egui::Align2::LEFT_TOP, sub, FontId::proportional(12.0), theme::TEXT_DIM);
        }
        None => {
            p.text(pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, title, FontId::proportional(13.0), theme::TEXT);
        }
    }
}

/// The quick-action row: 28 px buttons in groups, 16 px in.
fn quick_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let layout = Layout::left_to_right(Align::Center);
    ui.allocate_ui_with_layout(vec2(ui.available_width(), 44.0), layout, |ui| {
        ui.set_min_height(44.0);
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(16.0);
        add(ui);
    });
}

// ---- Doc ----------------------------------------------------------------------------------

/// The Doc tab's own toggles, which last the session.
#[derive(Clone, Copy, Default)]
struct DocToggles {
    link: bool,
    scale_content: bool,
}

pub(crate) fn color32(c: Rgba) -> Color32 {
    let [r, g, b, a] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn doc_props(ui: &mut Ui, ed: &mut Editor) {
    header(ui, Icon::Layer, &ed.title.clone(), Some("ActionScript 3.0 Document"));
    quick_row(ui, |ui| {
        // ponytail: snapping, rulers and guides belong to the stage view; these light up once it has them.
        for (icon, tip) in [(Icon::Magnet, "Snap to Objects"), (Icon::SnapAlign, "Snap Align"), (Icon::Rulers, "Rulers"), (Icon::Lock, "Lock Guides")] {
            widgets::quick(ui, icon, false, false, tip);
        }
    });
    theme::section(ui, "Publish Settings", |ui| {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Profile", WORD_W);
            widgets::pick(ui, "Profile", 0, &[(0, "Default", true)], ui.available_width() - 4.0);
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Target", WORD_W);
            widgets::pick(ui, "Target", 0, &[(0, "SWF (Ruffle)", true)], ui.available_width() - 4.0);
        });
        // ponytail: there's no Publish Settings dialog yet; File > Publish exports the SWF.
        widgets::row(ui, |ui| ui.add_enabled_ui(false, |ui| widgets::button(ui, "More settings", ui.available_width())));
    });
    theme::section(ui, "Document Settings", |ui| document_settings(ui, ed));
    theme::section_with(ui, "SWF History", false, |ui| {
        ui.label(RichText::new("No SWF published yet.").color(theme::TEXT_DIM));
        widgets::row(ui, |ui| {
            ui.add_enabled_ui(false, |ui| {
                widgets::button(ui, "Log", 60.0);
                widgets::button(ui, "Clear", 60.0);
            })
        });
    });
}

fn document_settings(ui: &mut Ui, ed: &mut Editor) {
    let id = Id::new("doc-toggles");
    let mut t: DocToggles = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let (old_w, old_h) = (ed.doc.width, ed.doc.height);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
        ui.vertical(|ui| {
            ui.set_width(124.0);
            let (mut w, mut h) = (old_w, old_h);
            let rw = widgets::row(ui, |ui| {
                widgets::label(ui, "W", LABEL_W);
                ui.add(Number::new("W", &mut w, 1.0..=8192.0).suffix(" px").width(64.0))
            });
            let rh = widgets::row(ui, |ui| {
                widgets::label(ui, "H", LABEL_W);
                ui.add(Number::new("H", &mut h, 1.0..=8192.0).suffix(" px").width(64.0))
            });
            let linked = t.link || t.scale_content;
            let at = |r: &egui::Response| pos2(r.rect.right() + 4.0, r.rect.center().y);
            if widgets::link_bracket(ui, at(&rw), at(&rh), linked, !t.scale_content).clicked() {
                t.link = !t.link;
            }
            if rw.changed() || rh.changed() {
                if linked && rw.changed() {
                    h = (old_h * w / old_w).clamp(1.0, 8192.0);
                } else if linked {
                    w = (old_w * h / old_h).clamp(1.0, 8192.0);
                }
                resize_stage(ed, w, h, t.scale_content);
            }
            for r in [&rw, &rh] {
                if r.drag_stopped() || r.lost_focus() {
                    ed.end_gesture();
                }
            }
            widgets::row(ui, |ui| {
                widgets::label(ui, "Stage", 34.0);
                let current = color32(ed.doc.background);
                let chip = widgets::chip(ui, current, "Stage color");
                if let Some(c) = color::swatch_popup(ui, ed, &chip, current, false) {
                    ed.checkpoint_gesture("doc-stage");
                    ed.doc.background = rgba(c.to_opaque());
                }
            });
            widgets::row(ui, |ui| {
                widgets::label(ui, "FPS", 34.0);
                let mut fps = ed.doc.frame_rate;
                let r = ui.add(Number::new("FPS", &mut fps, 0.01..=120.0).speed(0.1).width(54.0));
                widgets::undoable(ed, &r, "doc-fps", |ed| ed.doc.frame_rate = fps);
            });
        });
        ui.vertical(|ui| {
            // ponytail: Match contents waits on which of its two recalled behaviours Animate has (home.rs).
            widgets::row(ui, |ui| ui.add_enabled_ui(false, |ui| widgets::button(ui, "Match contents", ui.available_width())));
            widgets::row(ui, |ui| widgets::checkbox(ui, &mut t.scale_content, "Scale Content"));
            widgets::row(ui, |ui| widgets::checkbox(ui, &mut ed.home.apply_to_pasteboard, "Apply to pasteboard"));
            // ponytail: rescaling every frame span to a new rate isn't built.
            widgets::row(ui, |ui| ui.add_enabled_ui(false, |ui| widgets::checkbox(ui, &mut false, "Scale Spans")));
        });
    });
    ui.data_mut(|d| d.insert_temp(id, t));
    widgets::row(ui, |ui| {
        if widgets::button(ui, "More settings", ui.available_width()).clicked() {
            ed.run(Action::DocumentSettings, ui.ctx());
        }
    });
}

/// A new stage size from the Doc tab; Scale Content scales the scenes' content with it.
fn resize_stage(ed: &mut Editor, w: f32, h: f32, scale: bool) {
    let (sx, sy) = (w / ed.doc.width, h / ed.doc.height);
    if (sx, sy) == (1.0, 1.0) {
        return;
    }
    ed.checkpoint_gesture("doc-size");
    if scale {
        let elements = ed.doc.scenes.iter_mut().flat_map(|t| &mut t.layers).flat_map(|l| &mut l.frames).flat_map(|f| &mut f.elements);
        for el in elements {
            minnamate_core::transform::transform(el, &minnamate_core::doc::Matrix::scale(sx, sy));
        }
    }
    (ed.doc.width, ed.doc.height) = (w, h);
}

// ---- Tool ---------------------------------------------------------------------------------

fn tool_props(ui: &mut Ui, ed: &mut Editor) {
    let tool = ed.tool;
    header(ui, tool.icon(), tool.name().trim_end_matches(" Tool"), None);
    if object_drawing(tool) {
        quick_row(ui, |ui| {
            if widgets::quick(ui, Icon::ObjectDrawing, ed.object_drawing, true, "Object Drawing (J)").clicked() {
                ed.object_drawing = !ed.object_drawing;
            }
            if tool == Tool::Brush {
                // ponytail: Brush Mode and Lock Fill need the brush to paint behind, inside, and gradients.
                widgets::quick(ui, Icon::Brush, false, false, "Brush Mode");
                widgets::quick(ui, Icon::Lock, false, false, "Lock Fill");
            }
        });
    }
    let (fill, stroke) = match tool {
        Tool::Rectangle | Tool::Oval | Tool::PolyStar | Tool::Pen => (true, true),
        Tool::Line | Tool::Pencil | Tool::InkBottle => (false, true),
        Tool::Brush | Tool::PaintBucket => (true, false),
        _ => (false, false),
    };
    if fill || stroke {
        let shown = Shown {
            fill: fill.then(|| ed.fill()),
            stroke: stroke.then(|| ed.stroke().paint),
            width: ed.stroke_width,
            cap: ed.stroke_cap,
            join: ed.stroke_join,
        };
        color_and_style(ui, ed, shown);
    }
    match tool {
        Tool::Rectangle => rectangle_options(ui, ed),
        Tool::Oval => oval_options(ui),
        Tool::PolyStar => polystar_options(ui, ed),
        Tool::Brush => brush_options(ui, ed),
        Tool::FreeTransform => crate::free_transform::options(ui, ed),
        Tool::Pen => crate::pen::options(ui, ed),
        Tool::Text => crate::text_tool::options(ui, ed),
        Tool::PaintBucket | Tool::InkBottle | Tool::Eyedropper => crate::paint_tools::options(ui, ed),
        Tool::Eraser => crate::eraser::options(ui, ed),
        _ => {}
    }
}

/// What Color and Style shows: the paints (None hides the row) and the stroke settings.
struct Shown {
    fill: Option<Paint>,
    stroke: Option<Paint>,
    width: f32,
    cap: Cap,
    join: Join,
}

/// "Color and Style" (interaction §3.4): a row per colour (chip, name, alpha),
/// then the stroke's size, style, width profile, scale, cap and join. Edits go
/// to the drawing defaults and the selected shapes.
fn color_and_style(ui: &mut Ui, ed: &mut Editor, s: Shown) {
    theme::section(ui, "Color and Style", |ui| {
        for (name, paint, is_fill) in [("Fill", &s.fill, true), ("Stroke", &s.stroke, false)] {
            let Some(paint) = paint else { continue };
            let c = color::solid(paint);
            let style = |c: Color32| if is_fill { Style::Fill(c) } else { Style::Stroke(c) };
            widgets::row(ui, |ui| {
                let chip = color::paint_chip(ui, paint, &format!("{name} color"));
                color::paint_popup(ui, ed, &chip, paint, is_fill);
                widgets::label(ui, name, 50.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut a = (c.a() as f32 / 2.55).round();
                    let label = format!("{name} alpha");
                    // A gradient's alpha is its stops', which the Color panel edits.
                    let solid = !widgets::is_none(c) && !matches!(paint, Paint::Gradient(_));
                    let r = ui.add_enabled(solid, Number::new(&label, &mut a, 0.0..=100.0).suffix(" %").width(44.0));
                    if r.changed() {
                        let [cr, cg, cb, _] = c.to_srgba_unmultiplied();
                        color::restyle(ed, style(Color32::from_rgba_unmultiplied(cr, cg, cb, (a * 2.55).round() as u8)));
                    }
                    widgets::settle(ed, &r);
                    widgets::checker(ui);
                });
            });
        }
        if s.stroke.is_none() {
            return;
        }
        let mut width = s.width;
        widgets::row(ui, |ui| {
            widgets::label(ui, "Stroke", WORD_W);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let r = ui.add(Number::new("Stroke size", &mut width, 0.1..=200.0).speed(0.1).fixed(2).width(52.0));
                if r.changed() {
                    color::restyle(ed, Style::Width(width));
                }
                widgets::settle(ed, &r);
            });
        });
        let slider = widgets::slider(ui, "Stroke size slider", &mut width, 0.1..=200.0);
        if slider.changed() {
            color::restyle(ed, Style::Width((width * 10.0).round() / 10.0));
        }
        widgets::settle(ed, &slider);
        // ponytail: the model draws solid, uniform strokes; the other styles and profiles are listed, greyed.
        widgets::row(ui, |ui| {
            widgets::label(ui, "Style", WORD_W);
            let styles = [(0, "Hairline", false), (1, "Solid line", true), (2, "Dashed", false), (3, "Dotted", false), (4, "Ragged", false), (5, "Stipple", false), (6, "Hatched", false)];
            widgets::pick(ui, "Stroke style", 1, &styles, ui.available_width() - 34.0);
            widgets::quick(ui, Icon::More, false, false, "Edit stroke style");
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Width", WORD_W);
            widgets::pick(ui, "Width profile", 0, &[(0, "Uniform", true)], ui.available_width() - 34.0);
            widgets::quick(ui, Icon::More, false, false, "Width profiles");
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Scale", WORD_W);
            let scales = [(0, "Normal", true), (1, "Horizontal", false), (2, "Vertical", false), (3, "None", false)];
            widgets::pick(ui, "Stroke scale", 0, &scales, 96.0);
            ui.add_enabled_ui(false, |ui| widgets::checkbox(ui, &mut false, "Hinting"));
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Cap", WORD_W);
            let caps = [(Cap::None, Icon::CapNone, "None"), (Cap::Round, Icon::CapRound, "Round"), (Cap::Square, Icon::CapSquare, "Square")];
            let items: Vec<_> = caps.iter().map(|&(_, icon, tip)| (icon, tip, true)).collect();
            let current = caps.iter().position(|c| c.0 == s.cap).unwrap_or(1);
            if let Some(i) = widgets::segmented(ui, &items, current, 32.0, false) {
                color::restyle(ed, Style::Cap(caps[i].0));
            }
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Join", WORD_W);
            let joins = [(Join::Miter, Icon::JoinMiter, "Miter"), (Join::Bevel, Icon::JoinBevel, "Bevel"), (Join::Round, Icon::JoinRound, "Round")];
            let items: Vec<_> = joins.iter().map(|&(_, icon, tip)| (icon, tip, true)).collect();
            let current = joins.iter().position(|j| j.0 == s.join).unwrap_or(2);
            if let Some(i) = widgets::segmented(ui, &items, current, 32.0, false) {
                color::restyle(ed, Style::Join(joins[i].0));
            }
            // ponytail: strokes have no miter limit in the model; the field shows Animate's default.
            ui.add_enabled_ui(false, |ui| {
                widgets::label(ui, "Miter", 34.0);
                ui.add(Number::new("Miter", &mut 3.0, 1.0..=60.0).width(32.0));
            });
        });
    });
}

fn rectangle_options(ui: &mut Ui, ed: &mut Editor) {
    theme::section(ui, "Rectangle Options", |ui| {
        // ponytail: one radius for all four corners (draw::rectangle's); the lock stays on.
        let mut r = ed.corner_radius;
        let mut changed = false;
        for (row, corners) in [[("Top left", (-1.0, -1.0)), ("Top right", (1.0, -1.0))], [("Bottom left", (-1.0, 1.0)), ("Bottom right", (1.0, 1.0))]].into_iter().enumerate() {
            widgets::row(ui, |ui| {
                for (col, (name, (sx, sy))) in corners.into_iter().enumerate() {
                    let first = row == 0 && col == 0;
                    ui.add_enabled_ui(first, |ui| {
                        // The corner, as a short L.
                        let (g, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                        let (c, d) = (g.center() + vec2(sx, sy) * 4.0, vec2(sx, sy) * -7.0);
                        ui.painter().line(vec![c + vec2(d.x, 0.0), c, c + vec2(0.0, d.y)], Stroke::new(1.2, theme::TEXT_DIM));
                        changed |= ui.add(Number::new(name, &mut r, -100.0..=999.0).speed(0.5).width(52.0)).changed();
                    });
                    ui.add_space(12.0);
                }
                if row == 0 {
                    widgets::quick(ui, Icon::Link, true, false, "Lock corner radius controls");
                }
            });
        }
        let mut slid = r.clamp(-100.0, 100.0);
        if widgets::slider(ui, "Corner radius", &mut slid, -100.0..=100.0).changed() {
            (r, changed) = (slid.round(), true);
        }
        widgets::row(ui, |ui| {
            if widgets::button(ui, "Reset", 60.0).clicked() {
                (r, changed) = (0.0, true);
            }
        });
        if changed {
            ed.corner_radius = r;
        }
    });
}

fn oval_options(ui: &mut Ui) {
    // ponytail: draw::oval makes whole ellipses; arcs and rings are listed, greyed.
    theme::section(ui, "Oval Options", |ui| {
        ui.add_enabled_ui(false, |ui| {
            for name in ["Start angle", "End angle", "Inner radius"] {
                widgets::row(ui, |ui| {
                    widgets::label(ui, name, 80.0);
                    ui.add(Number::new(name, &mut 0.0, 0.0..=360.0).width(44.0));
                });
            }
            widgets::row(ui, |ui| widgets::checkbox(ui, &mut true, "Close path"));
            widgets::row(ui, |ui| widgets::button(ui, "Reset", 60.0));
        });
    });
}

fn polystar_options(ui: &mut Ui, ed: &mut Editor) {
    theme::section(ui, "Tool Options", |ui| {
        // Labels above their fields, as Animate lays this section out.
        let caption = |ui: &mut Ui, text: &str| ui.label(RichText::new(text).color(theme::TEXT_DIM));
        caption(ui, "Style:");
        widgets::row(ui, |ui| {
            if let Some(star) = widgets::pick(ui, "PolyStar style", ed.polystar_star, &[(false, "polygon", true), (true, "star", true)], 120.0) {
                ed.polystar_star = star;
            }
        });
        caption(ui, "Number of Sides:");
        widgets::row(ui, |ui| {
            let mut sides = ed.polystar_sides as f32;
            if ui.add(Number::new("Number of Sides", &mut sides, 3.0..=32.0).speed(0.1)).changed() {
                ed.polystar_sides = sides.round() as u32;
            }
        });
        caption(ui, "Star point size:");
        widgets::row(ui, |ui| {
            ui.add(Number::new("Star point size", &mut ed.polystar_point, 0.0..=1.0).speed(0.01).fixed(2));
        });
    });
}

fn brush_options(ui: &mut Ui, ed: &mut Editor) {
    theme::section(ui, "Brush Options", |ui| {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Size", WORD_W);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(Number::new("Size", &mut ed.brush_size, 1.0..=200.0).speed(0.5).width(48.0));
            });
        });
        widgets::slider(ui, "Size slider", &mut ed.brush_size, 1.0..=200.0);
        ed.brush_size = ed.brush_size.round();
        // ponytail: brushes draw at their size whatever the zoom, and the Eraser keeps its own size.
        ui.add_enabled_ui(false, |ui| {
            widgets::row(ui, |ui| widgets::checkbox(ui, &mut true, "Zoom size with stage"));
            widgets::row(ui, |ui| widgets::checkbox(ui, &mut false, "Sync settings with Eraser"));
        });
    });
}

// ---- Object -------------------------------------------------------------------------------

/// The Object tab: what is selected, by kind.
fn object_props(ui: &mut Ui, ed: &mut Editor) {
    let sel = ed.selection.clone();
    let kinds: Vec<Option<&Element>> = sel.iter().map(|&r| ed.element(r)).collect();
    match kinds.as_slice() {
        [] | [None] => {}
        [Some(Element::Instance(_))] => instance_props(ui, ed, sel[0]),
        [Some(Element::Text(t))] => {
            header(ui, Icon::Text, crate::text_tool::kind_name(t.kind), None);
            position_size(ui, ed);
            text_props(ui, ed);
            crate::filters::section(ui, ed, sel[0]);
        }
        [Some(Element::Group(_))] => {
            header(ui, Icon::Group, "Group", None);
            position_size(ui, ed);
        }
        [Some(Element::Unsupported { name, .. })] => {
            let name = name.trim_start_matches("DOM").replace("Instance", "");
            header(ui, Icon::TlMountains, &name, None);
            position_size(ui, ed);
        }
        all if all.iter().all(|e| matches!(e, Some(Element::Shape(_)))) => shape_props(ui, ed),
        many => {
            header(ui, Icon::Selection, &format!("{} objects", many.len()), None);
            position_size(ui, ed);
        }
    }
}

/// X, Y, W and H of the selection: W above H joined by the link bracket, X
/// above Y. A lone instance's X and Y are its registration point's.
fn position_size(ui: &mut Ui, ed: &mut Editor) {
    let Some(b) = hit::bounds(&ed.doc, ed.timeline(), ed.frame, &ed.selection) else { return };
    let origin = match ed.selection.as_slice() {
        [r] => match ed.element(*r) {
            // A motion tween's target, where it shows now.
            Some(Element::Instance(_)) if let Some(Element::Instance(i)) = crate::tweens::shown_target(ed, *r) => (i.matrix.tx, i.matrix.ty),
            Some(Element::Instance(i)) => (i.matrix.tx, i.matrix.ty),
            _ => b.min,
        },
        _ => b.min,
    };
    let id = Id::new("object-link");
    let mut linked: bool = ui.data(|d| d.get_temp(id)).unwrap_or(false);
    theme::section(ui, "Position and Size", |ui| {
        let (bw, bh) = (b.max.0 - b.min.0, b.max.1 - b.min.1);
        let (mut w, mut h, mut x, mut y) = (bw, bh, origin.0, origin.1);
        let field = |ui: &mut Ui, name: &'static str, v: &mut f32| {
            widgets::label(ui, name, LABEL_W);
            // The range holds what's typed or dragged in; a thinner selection is left as it is.
            let range = if matches!(name, "W" | "H") { 0.05..=f32::INFINITY } else { f32::NEG_INFINITY..=f32::INFINITY };
            ui.add(Number::new(name, v, range).width(64.0))
        };
        let (rw, rx) = widgets::row(ui, |ui| {
            let rw = field(ui, "W", &mut w);
            ui.add_space(24.0);
            (rw, field(ui, "X", &mut x))
        });
        let (rh, ry) = widgets::row(ui, |ui| {
            let rh = field(ui, "H", &mut h);
            ui.add_space(24.0);
            (rh, field(ui, "Y", &mut y))
        });
        let at = |r: &egui::Response| pos2(r.rect.right() + 4.0, r.rect.center().y);
        if widgets::link_bracket(ui, at(&rw), at(&rh), linked, true).clicked() {
            linked = !linked;
        }
        if rx.changed() || ry.changed() {
            ed.nudge(x - origin.0, y - origin.1, "object-position");
        }
        if rw.changed() || rh.changed() {
            if linked && rw.changed() && bw > 0.0 {
                h = bh * w / bw;
            } else if linked && rh.changed() && bh > 0.0 {
                w = bw * h / bh;
            }
            crate::free_transform::resize(ed, b, w, h);
        }
        if [&rw, &rh, &rx, &ry].iter().any(|r| r.drag_stopped() || r.lost_focus()) {
            ed.end_gesture();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, linked));
}

fn shape_props(ui: &mut Ui, ed: &mut Editor) {
    let first = ed.selection.first().and_then(|&r| match ed.element(r) {
        Some(Element::Shape(s)) => Some(s.clone()),
        _ => None,
    });
    let Some(shape) = first else { return };
    let drawing_object = shape.extra.get("isDrawingObject") == Some("true");
    header(ui, Icon::Shape, if drawing_object { "Drawing Object" } else { "Shape" }, None);
    quick_row(ui, |ui| {
        // ponytail: Lines to Fills, Group, Break Apart and Create Brush wait on their commands.
        widgets::quick(ui, Icon::LinesToFills, false, false, "Convert Lines to Fills");
        widgets::quick(ui, Icon::Group, false, false, "Group");
        widgets::quick(ui, Icon::BreakApart, false, false, "Break Apart");
        widgets::group_divider(ui);
        if widgets::quick(ui, Icon::ConvertToSymbol, false, ed.editing.is_empty(), "Convert to Symbol").clicked() {
            ed.run(Action::ConvertToSymbol, ui.ctx());
        }
        widgets::quick(ui, Icon::CreateBrush, false, false, "Create Brush");
    });
    position_size(ui, ed);
    let stroke = shape.strokes.first();
    let shown = Shown {
        fill: shape.fills.first().cloned(),
        stroke: stroke.map(|s| s.paint.clone()),
        width: stroke.map_or(ed.stroke_width, |s| s.width),
        cap: stroke.map_or(ed.stroke_cap, |s| s.cap),
        join: stroke.map_or(ed.stroke_join, |s| s.join),
    };
    if shown.fill.is_some() || shown.stroke.is_some() {
        color_and_style(ui, ed, shown);
    }
    theme::section(ui, "Shape Options", |ui| {
        // ponytail: smoothing and straightening shapes aren't built.
        widgets::row(ui, |ui| {
            widgets::quick(ui, Icon::Straighten, false, false, "Straighten");
            widgets::quick(ui, Icon::Smooth, false, false, "Smoothen");
        });
    });
}

/// A text field's Character and Paragraph settings, applied to every selected text field.
fn text_props(ui: &mut Ui, ed: &mut Editor) {
    let Some(Element::Text(t)) = ed.selection.first().and_then(|&r| ed.element(r)).cloned() else { return };
    let Some(run) = t.runs.first().cloned() else { return };
    let edit = |ed: &mut Editor, field: &'static str, change: &dyn Fn(&mut minnamate_core::doc::Text)| {
        ed.checkpoint_gesture(field);
        for r in ed.selection.clone() {
            if let Some(Element::Text(t)) = ed.element_mut(r) {
                change(t);
            }
        }
    };
    theme::section(ui, "Character", |ui| {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Family", WORD_W);
            let face = fonts::find(&run.face);
            let family = face.map_or(run.face.clone(), |f| f.family.clone());
            let mut picked = None;
            widgets::dropdown(ui, "Font family", &family, ui.available_width() - 4.0, |ui| {
                ui.set_max_height(320.0);
                for name in fonts::families() {
                    if widgets::choice(ui, name == family, true, name).clicked() {
                        picked = fonts::resolve(name, false, false).map(|f| f.postscript.clone());
                    }
                }
            });
            if let Some(face) = picked {
                ed.end_gesture();
                edit(ed, "text-family", &|t| t.runs.iter_mut().for_each(|r| (r.face, r.bold, r.italic) = (face.clone(), false, false)));
                ed.end_gesture();
            }
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Size", WORD_W);
            let mut size = run.size;
            let r = ui.add(Number::new("Size", &mut size, 1.0..=2500.0).speed(0.5).suffix(" pt"));
            if r.changed() {
                edit(ed, "text-size", &|t| t.runs.iter_mut().for_each(|r| r.size = size));
            }
            ui.add_space(8.0);
            widgets::label(ui, "VA", 20.0);
            let mut spacing = run.letter_spacing;
            let s = ui.add(Number::new("Letter spacing", &mut spacing, -60.0..=60.0).speed(0.1).width(40.0));
            if s.changed() {
                edit(ed, "text-spacing", &|t| t.runs.iter_mut().for_each(|r| r.letter_spacing = spacing));
            }
            if [&r, &s].iter().any(|r| r.drag_stopped() || r.lost_focus()) {
                ed.end_gesture();
            }
        });
        widgets::row(ui, |ui| {
            let c = color32(run.color);
            let chip = widgets::chip(ui, c, "Text color");
            if let Some(picked) = color::swatch_popup(ui, ed, &chip, c, false) {
                edit(ed, "text-color", &|t| t.runs.iter_mut().for_each(|r| r.color = rgba(picked)));
            }
            widgets::label(ui, "Fill", 50.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut a = (run.color[3] * 100.0).round();
                let r = ui.add(Number::new("Text alpha", &mut a, 0.0..=100.0).suffix(" %").width(44.0));
                if r.changed() {
                    edit(ed, "text-color", &|t| t.runs.iter_mut().for_each(|r| r.color[3] = a / 100.0));
                }
                widgets::settle(ed, &r);
                widgets::checker(ui);
            });
        });
    });
    theme::section(ui, "Paragraph", |ui| {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Format", WORD_W);
            let aligns = [(TextAlign::Left, Icon::AlignLeft, "Align left"), (TextAlign::Center, Icon::AlignCenter, "Align center"), (TextAlign::Right, Icon::AlignRight, "Align right"), (TextAlign::Justify, Icon::AlignJustify, "Justify")];
            let items: Vec<_> = aligns.iter().map(|&(_, icon, tip)| (icon, tip, true)).collect();
            let current = aligns.iter().position(|a| a.0 == t.align).unwrap_or(0);
            if let Some(i) = widgets::segmented(ui, &items, current, 28.0, false) {
                ed.end_gesture();
                edit(ed, "text-align", &|t| t.align = aligns[i].0);
                ed.end_gesture();
            }
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Spacing", WORD_W);
            let mut spacing = t.line_spacing;
            let r = ui.add(Number::new("Line spacing", &mut spacing, -100.0..=500.0).suffix(" pt"));
            widgets::undoable(ed, &r, "text-line-spacing", |ed| {
                for r in ed.selection.clone() {
                    if let Some(Element::Text(t)) = ed.element_mut(r) {
                        t.line_spacing = spacing;
                    }
                }
            });
        });
    });
}

/// How an instance's Color Effect shows its color transform.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Effect {
    None,
    Brightness,
    Tint,
    Advanced,
    Alpha,
}

const EFFECTS: [(Effect, &str, bool); 5] =
    [(Effect::None, "None", true), (Effect::Brightness, "Brightness", true), (Effect::Tint, "Tint", true), (Effect::Advanced, "Advanced", true), (Effect::Alpha, "Alpha", true)];

/// Whether `effect` can show transform `c` (Advanced shows any).
fn fits(effect: Effect, c: &ColorTransform) -> bool {
    let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
    let alpha_plain = near(c.mult[3], 1.0) && near(c.add[3], 0.0);
    let grey = near(c.mult[0], c.mult[1]) && near(c.mult[1], c.mult[2]);
    let (m, add) = (c.mult[0], c.add[0]);
    match effect {
        Effect::None => *c == ColorTransform::IDENTITY,
        Effect::Alpha => (0..3).all(|i| near(c.mult[i], 1.0) && near(c.add[i], 0.0)) && near(c.add[3], 0.0),
        Effect::Brightness => {
            alpha_plain && grey && near(c.add[1], add) && near(c.add[2], add) && (if add > 0.0 { near(m, 1.0 - add) } else { near(add, 0.0) && m <= 1.0 })
        }
        Effect::Tint => alpha_plain && grey && (0.0..=1.0).contains(&m) && (0..3).all(|i| c.add[i] >= -1e-4 && c.add[i] <= 1.0 - m + 1e-4),
        Effect::Advanced => true,
    }
}

/// The Color Effect section for the instance at `r`, whose transform is `c`.
fn color_effect(ui: &mut Ui, ed: &mut Editor, r: hit::ElementRef, c: ColorTransform) {
    // The style picked is remembered while its instance stays selected, as a transform
    // may fit several (Brightness 0 % is no effect at all); the transform is what's saved.
    let id = Id::new("color-effect");
    let whose = (ed.title.clone(), ed.editing.len(), r);
    let remembered: Option<((String, usize, hit::ElementRef), Effect, Color32)> = ui.data(|d| d.get_temp(id));
    let (remembered, mut tint) = match remembered {
        Some((w, e, t)) if w == whose => (Some(e), t),
        _ => (None, Color32::WHITE),
    };
    let inferred = [Effect::None, Effect::Alpha, Effect::Brightness, Effect::Tint].into_iter().find(|&e| fits(e, &c)).unwrap_or(Effect::Advanced);
    let effect = remembered.filter(|&e| fits(e, &c)).unwrap_or(inferred);
    let mut next = c;
    let mut picked_effect = None;
    // A field or slider let go of: its edits end their undo step.
    let mut done = false;
    let finished = |r: &egui::Response| r.drag_stopped() || r.lost_focus();
    theme::section(ui, "Color Effect", |ui| {
        widgets::row(ui, |ui| {
            let tinted = effect == Effect::Tint;
            let w = ui.available_width() - if tinted { 40.0 } else { 4.0 };
            picked_effect = widgets::pick(ui, "Color effect", effect, &EFFECTS, w);
            if tinted {
                let t = 1.0 - c.mult[0];
                if t > 1e-4 {
                    let u = |v: f32| (v / t * 255.0).round().clamp(0.0, 255.0) as u8;
                    tint = Color32::from_rgb(u(c.add[0]), u(c.add[1]), u(c.add[2]));
                }
                let chip = widgets::chip(ui, tint, "Tint color");
                if let Some(p) = color::swatch_popup(ui, ed, &chip, tint, false) {
                    tint = p.to_opaque();
                    next = tinted_by(tint, t);
                }
            }
        });
        // A percentage: its name and value, then a slider. Returns (changed, let go of).
        let percent_row = |ui: &mut Ui, name: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>| {
            let field = widgets::row(ui, |ui| {
                widgets::label(ui, name, WORD_W);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.add(Number::new(name, v, range.clone()).suffix(" %").width(48.0))).inner
            });
            let slider = widgets::slider(ui, &format!("{name} slider"), v, range);
            if slider.changed() {
                *v = v.round();
            }
            (field.changed() || slider.changed(), finished(&field) || finished(&slider))
        };
        match effect {
            Effect::Brightness => {
                let mut b = if c.add[0] > 0.0 { c.add[0] } else { c.mult[0] - 1.0 } * 100.0;
                let (changed, let_go) = percent_row(ui, "Brightness", &mut b, -100.0..=100.0);
                if changed {
                    let b = b / 100.0;
                    next = ColorTransform { mult: [1.0 - b.abs(), 1.0 - b.abs(), 1.0 - b.abs(), 1.0], add: [b.max(0.0), b.max(0.0), b.max(0.0), 0.0] };
                }
                done |= let_go;
            }
            Effect::Tint => {
                let mut t = (1.0 - c.mult[0]) * 100.0;
                let (changed, let_go) = percent_row(ui, "Tint", &mut t, 0.0..=100.0);
                if changed {
                    next = tinted_by(tint, t / 100.0);
                }
                done |= let_go;
                let mut rgb = tint.to_array().map(|v| v as f32);
                let mut changed = false;
                for (i, name) in ["Red", "Green", "Blue"].into_iter().enumerate() {
                    widgets::row(ui, |ui| {
                        widgets::label(ui, name, WORD_W);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let field = ui.add(Number::new(name, &mut rgb[i], 0.0..=255.0).width(48.0));
                            let slider = widgets::slider(ui, &format!("{name} slider"), &mut rgb[i], 0.0..=255.0);
                            changed |= field.changed() || slider.changed();
                            done |= finished(&field) || finished(&slider);
                        });
                    });
                }
                if changed {
                    tint = Color32::from_rgb(rgb[0].round() as u8, rgb[1].round() as u8, rgb[2].round() as u8);
                    next = tinted_by(tint, t / 100.0);
                }
            }
            Effect::Alpha => {
                let mut a = c.mult[3] * 100.0;
                let (changed, let_go) = percent_row(ui, "Alpha", &mut a, 0.0..=100.0);
                if changed {
                    next = ColorTransform { mult: [1.0, 1.0, 1.0, a / 100.0], add: [0.0; 4] };
                }
                done |= let_go;
            }
            Effect::Advanced => {
                // Result = value × % + offset, per channel: `[100] % × A + [0]`.
                for (i, ch) in [3, 0, 1, 2].into_iter().enumerate() {
                    let name = ["Alpha", "Red", "Green", "Blue"][i];
                    widgets::row(ui, |ui| {
                        let mut m = c.mult[ch] * 100.0;
                        let mut o = c.add[ch] * 255.0;
                        let times = ui.add(Number::new(name, &mut m, -100.0..=100.0).suffix(" %").width(52.0));
                        if times.changed() {
                            next.mult[ch] = m / 100.0;
                        }
                        widgets::label(ui, &format!("× {} +", &name[..1]), 34.0);
                        let plus = ui.add(Number::new(&format!("{name} offset"), &mut o, -255.0..=255.0).width(44.0));
                        if plus.changed() {
                            next.add[ch] = o / 255.0;
                        }
                        done |= finished(&times) || finished(&plus);
                    });
                }
            }
            Effect::None => {}
        }
    });
    if let Some(e) = picked_effect {
        // Each style starts from no change at all; Advanced keeps what's there.
        let start = if e == Effect::Advanced { c } else { ColorTransform::IDENTITY };
        if start != c {
            ed.checkpoint();
            crate::tweens::set_color(ed, r, start);
        }
        ui.data_mut(|d| d.insert_temp(id, (whose, e, tint)));
        return;
    }
    ui.data_mut(|d| d.insert_temp(id, (whose, effect, tint)));
    if next != c {
        ed.checkpoint_gesture("color-effect");
        crate::tweens::set_color(ed, r, next);
    }
    if done {
        ed.end_gesture();
    }
}

/// Tint: every channel `amount` of the way to `color`.
fn tinted_by(color: Color32, amount: f32) -> ColorTransform {
    let [r, g, b, _] = color.to_array().map(|v| v as f32 / 255.0 * amount);
    ColorTransform { mult: [1.0 - amount, 1.0 - amount, 1.0 - amount, 1.0], add: [r, g, b, 0.0] }
}

fn instance_props(ui: &mut Ui, ed: &mut Editor, r: hit::ElementRef) {
    let Some(Element::Instance(inst)) = ed.element(r).cloned() else { return };
    let set = |ed: &mut Editor, change: &dyn Fn(&mut Instance)| {
        if let Some(Element::Instance(i)) = ed.element_mut(r) {
            change(i);
        }
    };
    // Header: the type glyph; the behaviour and (but for graphics) the instance name beside it.
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        let (glyph, _) = ui.allocate_exact_size(vec2(46.0, 56.0), Sense::hover());
        icons::paint(ui.painter(), Rect::from_center_size(pos2(glyph.left() + 26.0, glyph.top() + 22.0), vec2(20.0, 20.0)), crate::library::kind_icon(inst.kind), theme::TEXT);
        ui.vertical(|ui| {
            ui.add_space(10.0);
            let width = ui.available_width() - 16.0;
            let kinds = [(SymbolKind::MovieClip, "Movie Clip", true), (SymbolKind::Button, "Button", true), (SymbolKind::Graphic, "Graphic", true)];
            if let Some(kind) = widgets::pick(ui, "Symbol behavior", inst.kind, &kinds, width) {
                ed.checkpoint();
                set(ed, &|i| i.kind = kind);
            }
            if inst.kind != SymbolKind::Graphic {
                let mut name = inst.extra.get("name").unwrap_or_default().to_string();
                let resp = widgets::text_input(ui, &mut name, "Instance Name", width);
                if resp.changed() {
                    ed.checkpoint_gesture("instance-name");
                    set(ed, &|i| i.extra.set("name", Some(name.as_str()).filter(|n| !n.is_empty())));
                }
                if resp.lost_focus() {
                    ed.end_gesture();
                }
            }
            ui.add_space(8.0);
        });
    });
    widgets::row(ui, |ui| {
        ui.add_space(16.0);
        widgets::label(ui, "Instance of:", 72.0);
        ui.label(RichText::new(&inst.symbol).color(theme::TEXT_DISABLED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(16.0);
            // ponytail: no Swap Symbol or Symbol Properties dialogs yet.
            widgets::quick(ui, Icon::Swap, false, false, "Swap symbol");
            widgets::quick(ui, Icon::Sliders, false, false, "Symbol properties");
        });
    });
    quick_row(ui, |ui| {
        if widgets::quick(ui, Icon::ConvertToSymbol, false, ed.editing.is_empty(), "Convert to Symbol").clicked() {
            ed.run(Action::ConvertToSymbol, ui.ctx());
        }
    });
    position_size(ui, ed);
    // A motion tween's target shows the effect keyed where the playhead is.
    let shown = crate::tweens::shown_target(ed, r).and_then(|el| if let Element::Instance(i) = el { Some(i.color) } else { None });
    color_effect(ui, ed, r, shown.unwrap_or(inst.color));
    if inst.kind == SymbolKind::Graphic {
        looping(ui, ed, r, &inst);
    } else {
        display(ui, ed, r, &inst);
        crate::filters::section(ui, ed, r);
    }
}

/// Display (movie clips and buttons): Visible, Blending and Render, kept in the
/// instance's XFL attributes, which minnamate_core::filters draws and publishes.
// ponytail: no Transparent / Opaque background choice once Render caches.
fn display(ui: &mut Ui, ed: &mut Editor, r: hit::ElementRef, inst: &Instance) {
    let attr = |ed: &mut Editor, name: &'static str, value: Option<&str>| {
        ed.checkpoint();
        if let Some(Element::Instance(i)) = ed.element_mut(r) {
            i.extra.set(name, value);
        }
    };
    theme::section(ui, "Display", |ui| {
        widgets::row(ui, |ui| {
            let mut visible = inst.extra.get("isVisible") != Some("false");
            if widgets::checkbox(ui, &mut visible, "Visible").changed() {
                attr(ed, "isVisible", (!visible).then_some("false"));
            }
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Blending", WORD_W);
            let current = inst.extra.get("blendMode").unwrap_or("normal");
            let shown = Blend::ALL.iter().find(|m| m.1 == current).map_or("Normal", |m| m.2);
            let mut picked = None;
            widgets::dropdown(ui, "Blending", shown, ui.available_width() - 4.0, |ui| {
                for (i, (_, value, text)) in Blend::ALL.into_iter().enumerate() {
                    // Separators after Normal, Layer, Multiply, Screen, Hard Light and Difference.
                    if [2, 4, 6, 8, 11].contains(&i) || i == 1 {
                        ui.separator();
                    }
                    if widgets::choice(ui, value == current, true, text).clicked() && value != current {
                        picked = Some(value);
                    }
                }
            });
            if let Some(mode) = picked {
                attr(ed, "blendMode", (mode != "normal").then_some(mode));
            }
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Render", WORD_W);
            let modes = [CACHE_AS_BITMAP, EXPORT_AS_BITMAP];
            let current = modes.iter().position(|&m| inst.extra.get(m) == Some("true")).map_or(0, |k| k + 1);
            let renders = [(0, "Original (No Change)", true), (1, "Cache as Bitmap", true), (2, "Export as Bitmap", true)];
            if let Some(picked) = widgets::pick(ui, "Render", current, &renders, ui.available_width() - 4.0) {
                ed.checkpoint();
                if let Some(Element::Instance(i)) = ed.element_mut(r) {
                    for (k, mode) in modes.into_iter().enumerate() {
                        i.extra.set(mode, (picked == k + 1).then_some("true"));
                    }
                }
            }
        });
    });
}

/// Looping (graphic instances): how the symbol's timeline plays, and from which frame.
fn looping(ui: &mut Ui, ed: &mut Editor, r: hit::ElementRef, inst: &Instance) {
    let frames = ed.doc.library.get(&inst.symbol).map_or(1, |s| s.timeline.frame_count().max(1));
    theme::section(ui, "Looping", |ui| {
        widgets::row(ui, |ui| {
            // ponytail: the model has no reverse modes; they're shown, greyed.
            let modes = [
                (Icon::Loop, "Loop", true),
                (Icon::PlayOnce, "Play Once", true),
                (Icon::SingleFrame, "Single Frame", true),
                (Icon::ReversePlayOnce, "Play Once Reverse", false),
                (Icon::ReverseLoop, "Loop Reverse", false),
            ];
            let all = [Loop::Loop, Loop::PlayOnce, Loop::SingleFrame];
            let current = all.iter().position(|l| *l == inst.looping).unwrap_or(0);
            if let Some(i) = widgets::segmented(ui, &modes, current, 30.0, true) {
                ed.checkpoint();
                if let Some(Element::Instance(i2)) = ed.element_mut(r) {
                    i2.looping = all[i];
                }
            }
        });
        widgets::row(ui, |ui| {
            // ponytail: no Frame Picker panel yet.
            ui.add_enabled_ui(false, |ui| widgets::button(ui, "Use Frame Picker", 110.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled_ui(false, |ui| widgets::text_input(ui, &mut String::new(), "", 32.0));
                widgets::label(ui, "Last", 26.0);
                let mut first = inst.first_frame as f32 + 1.0;
                let resp = ui.add(Number::new("First", &mut first, 1.0..=frames as f32).speed(0.2).width(32.0));
                widgets::undoable(ed, &resp, "first-frame", |ed| {
                    if let Some(Element::Instance(i)) = ed.element_mut(r) {
                        i.first_frame = first.round() as u32 - 1;
                    }
                });
                widgets::label(ui, "First", 26.0);
            });
        });
        // ponytail: lip syncing needs sound layers.
        widgets::row(ui, |ui| ui.add_enabled_ui(false, |ui| widgets::button(ui, "Lip Syncing", ui.available_width())));
    });
}

// ---- Frame --------------------------------------------------------------------------------

/// The Frame tab: the keyframe's label and its tween, both editable.
fn frame_props(ui: &mut Ui, ed: &mut Editor) {
    use minnamate_core::doc::Rotate;
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width() - 48.0, 48.0), Sense::hover());
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        header(&mut child, Icon::Frame, "Frame", None);
        // ponytail: no Actions panel yet.
        widgets::quick(ui, Icon::Actions, false, false, "Actions");
    });
    let (l, f) = (ed.layer, ed.frame);
    let key = ed.timeline().layers.get(l).and_then(|layer| layer.keyframe_at(f).map(|k| (k, layer.frames[k].clone())));
    quick_row(ui, |ui| {
        let can = key.as_ref().is_some_and(|(_, k)| k.tween == Tween::None);
        if widgets::quick(ui, Icon::TlClassicTween, false, can, "Create Classic Tween").clicked() {
            ed.run(Action::CreateClassicTween, ui.ctx());
        }
        for (icon, tip, action) in [(Icon::MotionTween, "Create Motion Tween", Action::CreateMotionTween), (Icon::ShapeTween, "Create Shape Tween", Action::CreateShapeTween)] {
            if widgets::quick(ui, icon, false, crate::timeline::available(ed, action), tip).clicked() {
                ed.run(action, ui.ctx());
            }
        }
    });
    let Some((k, key)) = key else {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(RichText::new("No frame here").color(theme::TEXT_DIM));
        });
        return;
    };
    let label = key.label().map_or((String::new(), "name".to_string()), |(n, t)| (n.to_string(), t.to_string()));
    let (mut name, mut kind) = label.clone();
    let mut tween = key.tween.clone();
    // Orient to path, kept as XFL's own frame attribute.
    const ORIENT: &str = "motionTweenOrientToPath";
    let mut orient = key.extra.get(ORIENT) == Some("true");
    let mut done_typing = false;
    // A tween field let go of: its edits end their undo step.
    let mut let_go = false;

    theme::section(ui, "Label", |ui| {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Name", WORD_W);
            done_typing = widgets::text_input(ui, &mut name, "Type here", ui.available_width() - 4.0).lost_focus();
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Type", WORD_W);
            let kinds = [("name", "Name", true), ("comment", "Comment", true), ("anchor", "Anchor", true)];
            ui.add_enabled_ui(!name.trim().is_empty(), |ui| {
                if let Some(k) = widgets::pick(ui, "Label type", kind.as_str(), &kinds, ui.available_width() - 4.0) {
                    kind = k.to_string();
                }
            });
        });
    });
    if tween != Tween::None {
        theme::section(ui, "Tweening", |ui| match &mut tween {
            Tween::Unsupported(other) => {
                let what = if other == "shape" { "Shape tween".to_string() } else { format!("Motion tween ({other})") };
                ui.label(RichText::new(what).color(theme::TEXT_DIM));
            }
            Tween::Motion(m) => crate::tweens::motion_settings(ui, m, &mut let_go),
            Tween::Shape { ease, angular, .. } => crate::tweens::shape_settings(ui, ease, angular, &mut let_go),
            Tween::Classic { ease, rotate } => {
                widgets::row(ui, |ui| {
                    // ponytail: one ease for every property; easing them separately isn't modelled.
                    let modes = [(0, "All properties together", true), (1, "Each property separately", false)];
                    widgets::pick(ui, "Ease mode", 0, &modes, ui.available_width() - 4.0);
                });
                widgets::row(ui, |ui| {
                    // ponytail: no preset ease curves; the intensity is the classic ease.
                    ui.add_enabled_ui(false, |ui| widgets::button(ui, "Classic Ease", 96.0));
                    let mut percent = (*ease * 100.0).round();
                    let field = ui.add(Number::new("Ease", &mut percent, -100.0..=100.0));
                    if field.changed() {
                        *ease = percent / 100.0;
                    }
                    let_go |= field.drag_stopped() || field.lost_focus();
                });
                widgets::row(ui, |ui| {
                    widgets::label(ui, "Rotate", WORD_W);
                    let (way, mut times) = match *rotate {
                        Rotate::Auto => (0, 1),
                        Rotate::Cw(n) => (1, n),
                        Rotate::Ccw(n) => (2, n),
                    };
                    let ways = [(0, "Auto", true), (1, "CW", true), (2, "CCW", true)];
                    let way = widgets::pick(ui, "Rotate", way, &ways, 80.0).unwrap_or(way);
                    if way > 0 {
                        let mut t = times as f32;
                        let field = ui.add(Number::new("Rotations", &mut t, 0.0..=99.0).speed(0.1).suffix(" times").width(64.0));
                        if field.changed() {
                            times = t.round() as u32;
                        }
                        let_go |= field.drag_stopped() || field.lost_focus();
                    }
                    *rotate = match way {
                        1 => Rotate::Cw(times),
                        2 => Rotate::Ccw(times),
                        _ => Rotate::Auto,
                    };
                });
                // ponytail: the classic tween model has ease, rotation and Orient to path only; the rest
                // show their defaults. Snap always holds: a guided tween runs from and to the guide.
                widgets::row(ui, |ui| {
                    ui.add_enabled_ui(false, |ui| ui.allocate_ui(vec2(120.0, 22.0), |ui| widgets::checkbox(ui, &mut true, "Snap")));
                    ui.allocate_ui(vec2(120.0, 22.0), |ui| widgets::checkbox(ui, &mut orient, "Orient to path"));
                });
                ui.add_enabled_ui(false, |ui| {
                    for pair in [[("Color along path", false), ("Scale along path", false)], [("Sync", false), ("Scale", true)]] {
                        widgets::row(ui, |ui| {
                            for (text, mut on) in pair {
                                ui.allocate_ui(vec2(120.0, 22.0), |ui| widgets::checkbox(ui, &mut on, text));
                            }
                        });
                    }
                });
            }
            Tween::None => {}
        });
        widgets::row(ui, |ui| {
            ui.add_space(16.0);
            if widgets::button(ui, "Remove Tween", ui.available_width() - 16.0).clicked() {
                ed.run(Action::RemoveTween, ui.ctx());
            }
        });
    }
    theme::section_with(ui, "Sound", false, |ui| {
        // ponytail: sound layers aren't modelled.
        ui.add_enabled_ui(false, |ui| {
            for (i, (name, value)) in [("Name", "None"), ("Effect", "None"), ("Sync", "Event"), ("", "Repeat")].into_iter().enumerate() {
                widgets::row(ui, |ui| {
                    widgets::label(ui, name, WORD_W);
                    widgets::pick(ui, &format!("Sound {i}"), 0, &[(0, value, true)], ui.available_width() - 4.0);
                });
            }
        });
    });

    // One undo step per edited field, until typing or dragging stops. The name
    // is stored as typed (so spaces between words survive) and trimmed at the end.
    if (name.as_str(), kind.as_str()) != (label.0.as_str(), label.1.as_str()) {
        ed.checkpoint_gesture("frame-label");
        ed.timeline_mut().layers[l].frames[k].set_label(&name, &kind);
    }
    if done_typing {
        if name.trim() != name {
            ed.checkpoint_gesture("frame-label");
            ed.timeline_mut().layers[l].frames[k].set_label(name.trim(), &kind);
        }
        ed.end_gesture();
    }
    if tween != key.tween {
        ed.checkpoint_gesture("frame-tween");
        ed.timeline_mut().layers[l].frames[k].tween = tween;
    }
    if orient != (key.extra.get(ORIENT) == Some("true")) {
        ed.checkpoint();
        ed.timeline_mut().layers[l].frames[k].extra.set(ORIENT, orient.then_some("true"));
    }
    if let_go {
        ed.end_gesture();
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Pos2, vec2};
    use egui_kittest::kittest::Queryable;
    use minnamate_core::hit::ElementRef;

    use super::*;
    use crate::app::testkit::*;

    /// The editor on a new document, with a renderer when `gpu`.
    fn new_doc(gpu: bool) -> Harness<'static, Editor> {
        let builder = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0);
        let builder = if gpu { builder.wgpu() } else { builder };
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h
    }

    /// The tween fixture with its movie clip (on the Blink layer) selected, on the Object tab.
    fn instance_selected(gpu: bool) -> Harness<'static, Editor> {
        let mut h = harness(gpu);
        h.run();
        let ed = h.state_mut();
        ed.selection = vec![ElementRef { layer: 0, keyframe: 0, element: 0 }];
        ed.props_tab = PropsTab::Object;
        h.run();
        h
    }

    /// A press, a few moves and a release, in screen points.
    fn drag(h: &mut Harness<'_, Editor>, from: Pos2, to: Pos2) {
        h.hover_at(from);
        h.step();
        h.drag_at(from);
        h.step();
        for t in [0.25, 0.5, 0.75, 1.0] {
            h.hover_at(from.lerp(to, t));
            h.step();
        }
        h.drop_at(to);
        h.step();
    }

    #[test]
    fn scrubbing_a_number_is_one_undo_step() {
        let mut h = new_doc(false);
        assert_eq!(h.state().props_tab, PropsTab::Doc, "a new document shows Doc");
        let w = h.get_by_label("W").rect().center();
        drag(&mut h, w, w + vec2(40.0, 0.0));
        h.run();
        let width = h.state().doc.width;
        assert!(width > 560.0, "dragging right scrubs the width up: {width}");
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!(h.state().doc.width, 550.0, "the whole scrub undoes at once");
        assert!(!h.state().can_undo());
    }

    #[test]
    fn typing_a_number_commits_on_enter() {
        let mut h = new_doc(false);
        h.get_by_label("FPS").click();
        h.run();
        h.get_by_label("FPS").type_text("30");
        h.key_press(egui::Key::Enter);
        h.run();
        assert_eq!(h.state().doc.frame_rate, 30.0);
    }

    #[test]
    fn picking_a_swatch_sets_the_fill_and_the_selection() {
        let mut h = new_doc(false);
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        let ed = h.state_mut();
        ed.selection = ed.all_elements();
        h.run();
        assert_eq!(h.state().props_tab, PropsTab::Tool);
        h.get_by_label("Fill color").click();
        h.run();
        // Row 6 starts the lower half: red 0x99 in the first cube column, green 0x66 two columns on.
        let grid = h.get_by_label("Swatch grid").rect();
        let cell = |col: f32, row: f32| grid.min + vec2(1.0 + col * 11.0 + 5.0, 1.0 + row * 11.0 + 5.0);
        click_at(&mut h, cell(4.0, 6.0));
        h.run();
        let picked = Color32::from_rgb(0x99, 0x66, 0x00);
        assert_eq!(h.state().fill_color, picked, "the swatch under the pointer");
        assert!(h.query_by_label("Swatch grid").is_none(), "picking closes the popup");
        let fills = |h: &Harness<'_, Editor>| match h.state().element(h.state().selection[0]) {
            Some(Element::Shape(s)) => s.fills.clone(),
            _ => panic!("no shape"),
        };
        assert_eq!(fills(&h), [Paint::Solid(rgba(picked))], "the selected shape takes it");
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        assert_ne!(fills(&h), [Paint::Solid(rgba(picked))], "one undo step");
    }

    #[test]
    fn escape_closes_the_swatch_popup_unchanged() {
        let mut h = new_doc(false);
        h.state_mut().tool = Tool::Oval;
        h.run();
        let before = h.state().stroke_color;
        h.get_by_label("Stroke color").click();
        h.run();
        assert!(h.query_by_label("Swatch grid").is_some());
        h.key_press(egui::Key::Escape);
        h.run();
        assert!(h.query_by_label("Swatch grid").is_none());
        assert_eq!(h.state().stroke_color, before);
    }

    #[test]
    fn an_instance_alpha_color_effect() {
        let mut h = instance_selected(false);
        let r = h.state().selection[0];
        let color = |h: &Harness<'_, Editor>| match h.state().element(r) {
            Some(Element::Instance(i)) => i.color,
            _ => panic!("not an instance"),
        };
        assert_eq!(color(&h), ColorTransform::IDENTITY);
        h.get_by_label("Color effect").click();
        h.run();
        h.get_by_label("Alpha").click();
        h.run();
        h.get_by_label("Alpha").click();
        h.run();
        h.get_by_label("Alpha").type_text("40");
        h.key_press(egui::Key::Enter);
        h.run();
        assert_eq!(color(&h).mult, [1.0, 1.0, 1.0, 0.4]);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!(color(&h).mult, [1.0, 1.0, 1.0, 1.0], "typing 40 is one step");
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!(color(&h), ColorTransform::IDENTITY);
    }

    #[test]
    fn color_effect_styles_read_back_from_transforms() {
        let brightness = ColorTransform { mult: [0.6, 0.6, 0.6, 1.0], add: [0.4, 0.4, 0.4, 0.0] };
        let alpha = ColorTransform { mult: [1.0, 1.0, 1.0, 0.3], add: [0.0; 4] };
        let tint = tinted_by(Color32::from_rgb(255, 0, 0), 0.5);
        assert!(fits(Effect::Brightness, &brightness) && !fits(Effect::Alpha, &brightness));
        assert!(fits(Effect::Alpha, &alpha) && !fits(Effect::Brightness, &alpha) && !fits(Effect::Tint, &alpha));
        assert!(fits(Effect::Tint, &tint) && !fits(Effect::Brightness, &tint));
        assert!(fits(Effect::None, &ColorTransform::IDENTITY) && fits(Effect::Brightness, &ColorTransform::IDENTITY));
    }

    #[test]
    fn instance_name_and_behaviour() {
        let mut h = instance_selected(false);
        let r = h.state().selection[0];
        h.get_by_label("Instance Name").click();
        h.run();
        h.get_by_label("Instance Name").type_text("blinker");
        h.key_press(egui::Key::Enter);
        h.run();
        let inst = |h: &Harness<'_, Editor>| match h.state().element(r) {
            Some(Element::Instance(i)) => i.clone(),
            _ => panic!("not an instance"),
        };
        assert_eq!(inst(&h).extra.get("name"), Some("blinker"));
        h.get_by_label("Symbol behavior").click();
        h.run();
        h.get_by_label("Graphic").click();
        h.run();
        assert_eq!(inst(&h).kind, SymbolKind::Graphic);
        assert!(h.query_by_label("Instance Name").is_none(), "graphics have no instance name");
        // The timeline toolbar has a Loop button too.
        assert!(h.query_all_by_label("Loop").count() == 2, "but they loop");
    }

    #[test]
    fn tabs_follow_the_tool_and_the_selection() {
        let mut h = new_doc(false);
        let enabled = |h: &Harness<'_, Editor>, tab| tab_enabled(h.state(), tab);
        assert!(!enabled(&h, PropsTab::Tool) && !enabled(&h, PropsTab::Object) && !enabled(&h, PropsTab::Frame));
        h.state_mut().tool = Tool::Rectangle;
        h.run();
        assert_eq!(h.state().props_tab, PropsTab::Tool, "picking a drawing tool shows Tool");
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Tool::Selection;
        h.run();
        assert_eq!(h.state().props_tab, PropsTab::Doc, "the Selection tool has no Tool tab");
        mouse_click(&mut h, (150.0, 125.0));
        h.run();
        assert_eq!(h.state().props_tab, PropsTab::Object);
        assert!(h.query_by_label("Fill color").is_some(), "a shape's Color and Style");
    }

    #[test]
    fn stroke_settings_restyle_the_selection() {
        let mut h = new_doc(false);
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Tool::Selection;
        mouse_click(&mut h, (150.0, 125.0));
        h.run();
        h.get_by_label("Square").click();
        h.run();
        h.get_by_label("Bevel").click();
        h.run();
        let Some(Element::Shape(s)) = h.state().element(h.state().selection[0]) else { panic!("no shape") };
        assert_eq!((s.strokes[0].cap, s.strokes[0].join), (Cap::Square, Join::Bevel));
        assert_eq!((h.state().stroke_cap, h.state().stroke_join), (Cap::Square, Join::Bevel), "and the defaults");
    }

    #[test]
    fn icon_strip_opens_flyouts() {
        let mut h = new_doc(false);
        h.get_by_label("Swatches").click();
        h.run();
        assert_eq!(h.state().flyout, Some(Flyout::Swatches));
        // The Swatches panel's grid gives a swatch to the colour the Color panel edits: the fill.
        let grid = h.get_all_by_label("Swatches").map(|n| n.rect()).find(|r| r.width() > 100.0).expect("the grid");
        click_at(&mut h, grid.min + vec2(5.0 + 11.0 * 6.0, 5.0));
        h.run();
        assert_ne!(h.state().fill_color, Color32::from_rgb(0x00, 0x99, 0xFF));
        h.get_by_label("Color").click();
        h.run();
        assert_eq!(h.state().flyout, Some(Flyout::Color));
        h.get_by_label("Color").click();
        h.run();
        assert_eq!(h.state().flyout, None, "a second click collapses it");
    }

    /// Writes target/screenshots/properties_doc.png: Properties on launch, the Doc tab.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn properties_doc_screenshot() {
        let mut h = new_doc(true);
        screenshot(&mut h, "properties_doc");
    }

    /// Writes target/screenshots/properties_object.png (a movie clip selected)
    /// and properties_graphic.png (a graphic instance).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn properties_object_screenshot() {
        let mut h = instance_selected(true);
        screenshot(&mut h, "properties_object");
        h.state_mut().selection = vec![ElementRef { layer: 1, keyframe: 0, element: 0 }];
        h.run();
        screenshot(&mut h, "properties_graphic");
    }

    /// Writes target/screenshots/properties_shape.png: a drawn rectangle selected.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn properties_shape_screenshot() {
        let mut h = new_doc(true);
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Tool::Selection;
        mouse_click(&mut h, (150.0, 125.0));
        h.run();
        screenshot(&mut h, "properties_shape");
    }

    /// Writes target/screenshots/swatch_popup.png: the Rectangle tool's fill chip open.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn swatch_popup_screenshot() {
        let mut h = new_doc(true);
        h.state_mut().tool = Tool::Rectangle;
        h.run();
        h.get_by_label("Fill color").click();
        h.run();
        let grid = h.get_by_label("Swatch grid").rect();
        h.hover_at(grid.min + vec2(60.0, 72.0));
        h.run();
        screenshot(&mut h, "swatch_popup");
    }

    /// Writes target/screenshots/color_panel.png and swatches_panel.png: the icon strip's flyouts.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn color_panel_screenshot() {
        let mut h = new_doc(true);
        h.state_mut().flyout = Some(Flyout::Color);
        h.run();
        screenshot(&mut h, "color_panel");
        h.state_mut().flyout = Some(Flyout::Swatches);
        h.run();
        screenshot(&mut h, "swatches_panel");
    }
}
