//! Symbol Properties (the Library's Properties), Duplicate Symbol (Modify > Symbol and the
//! Library's Duplicate) and Swap Symbol (Modify > Symbol and Properties > Object's Swap): a
//! symbol's name, type, folder and linkage, and which symbol the selected instances show
//! (docs/animate-ui.md §7.3, §7.2.4). The dialogs stack: Swap Symbol can open Duplicate Symbol.

use eframe::egui::{self, Align, Align2, Color32, FontId, Frame, Key, Layout, Margin, Rect, RichText, Sense, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use minnamate_core::doc::{Element, SymbolKind};
use minnamate_core::hit::ElementRef;
use minnamate_core::{library, symbols};

use crate::app::{Action, Editor};
use crate::icons::{self, Icon};
use crate::{home, theme, widgets};

pub enum Dialog {
    Props(Props),
    Swap(Swap),
}

/// Symbol Properties of the symbol at `path`, or Duplicate Symbol copying it.
pub struct Props {
    path: String,
    /// Duplicate Symbol, the copy then replacing the selected instances when true.
    duplicate: Option<bool>,
    pub name: String,
    pub kind: SymbolKind,
    /// "" for the library's root.
    pub folder: String,
    pub export: bool,
    pub class: String,
    /// The class it was exported as when the dialog opened.
    linked: Option<String>,
}

pub struct Swap {
    refs: Vec<ElementRef>,
    /// The symbol they shall show.
    pub picked: String,
}

/// Modify > Symbol's commands on the selected instances.
pub fn command(ed: &mut Editor, action: Action) {
    let instances = selected_instances(ed);
    let Some((_, first)) = instances.first().cloned() else { return };
    match action {
        Action::SwapSymbol => swap(ed),
        Action::DuplicateSymbol => duplicate(ed, &first, true),
        _ => {}
    }
}

/// The selected instances, with their symbols.
fn selected_instances(ed: &Editor) -> Vec<(ElementRef, String)> {
    ed.selection.iter().filter_map(|&r| match ed.element(r) {
        Some(Element::Instance(i)) => Some((r, i.symbol.clone())),
        _ => None,
    }).collect()
}

/// Symbol Properties for the symbol at `path`.
pub fn properties(ed: &mut Editor, path: &str) {
    let Some(symbol) = ed.doc.library.get(path) else { return };
    let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
    let class = library::linkage(symbol).map(str::to_string);
    let props = Props {
        path: path.to_string(),
        duplicate: None,
        name: name.to_string(),
        kind: symbol.kind,
        folder: folder.to_string(),
        export: class.is_some(),
        class: class.clone().unwrap_or_else(|| name.replace(' ', "_")),
        linked: class,
    };
    ed.symbol_dialogs.push(Dialog::Props(props));
}

/// Duplicate Symbol for the symbol at `path`: the copy replaces the selected instances when `swap`.
pub fn duplicate(ed: &mut Editor, path: &str, swap: bool) {
    properties(ed, path);
    let copy = library::copy_name(&ed.doc, path);
    if let Some(Dialog::Props(p)) = ed.symbol_dialogs.last_mut() {
        p.duplicate = Some(swap);
        p.name = library::leaf(&copy).to_string();
    }
}

/// Swap Symbol for the selected instances.
pub fn swap(ed: &mut Editor) {
    let instances = selected_instances(ed);
    let Some((_, picked)) = instances.first().cloned() else { return };
    ed.symbol_dialogs.push(Dialog::Swap(Swap { refs: instances.into_iter().map(|(r, _)| r).collect(), picked }));
}

pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    let mut stack = std::mem::take(&mut ed.symbol_dialogs);
    let mut outcome = None;
    let count = stack.len();
    for (i, dialog) in stack.iter_mut().enumerate() {
        let done = match dialog {
            Dialog::Props(p) => props_dialog(ctx, ed, p, i),
            Dialog::Swap(s) => swap_dialog(ctx, ed, s, i),
        };
        if i + 1 == count {
            outcome = done;
        }
    }
    // Enter and Escape are the top dialog's, unless a message is up over it.
    let popup_open = egui::Popup::is_any_open(ctx) || ed.error.is_some();
    if outcome.is_none() && count > 0 && !popup_open {
        outcome = ctx.input(|i| if i.key_pressed(Key::Escape) { Some(false) } else if i.key_pressed(Key::Enter) { Some(true) } else { None });
    }
    match (outcome, stack.pop()) {
        (Some(true), Some(Dialog::Props(p))) => match apply_props(ed, &p) {
            // A copy made from Swap Symbol is the one picked there.
            Ok(made) => {
                if let (Some(Dialog::Swap(s)), Some(_)) = (stack.last_mut(), p.duplicate) {
                    s.picked = made;
                }
            }
            Err(e) => {
                ed.error = Some(e);
                stack.push(Dialog::Props(p));
            }
        },
        (Some(true), Some(Dialog::Swap(s))) => apply_swap(ed, &s),
        (Some(false), Some(_)) => {}
        (None, Some(top)) => stack.push(top),
        (_, None) => {}
    }
    // Dialogs opened meanwhile (Duplicate from Swap) go on top.
    stack.append(&mut ed.symbol_dialogs);
    ed.symbol_dialogs = stack;
}

fn kind_name(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::MovieClip => "Movie Clip",
        SymbolKind::Button => "Button",
        SymbolKind::Graphic => "Graphic",
    }
}

/// OK and Cancel at the right, the main one blue; Some(true) for OK.
fn ok_cancel(ui: &mut Ui) -> Option<bool> {
    let mut picked = None;
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if ui.add(egui::Button::new("Cancel").min_size(vec2(72.0, 28.0))).clicked() {
            picked = Some(false);
        }
        let (_, rect) = ui.allocate_space(vec2(72.0, 28.0));
        if home::push(ui, rect, "OK", true, 4.0).clicked() {
            picked = Some(true);
        }
    });
    picked
}

fn props_dialog(ctx: &egui::Context, ed: &mut Editor, p: &mut Props, i: usize) -> Option<bool> {
    let title = if p.duplicate.is_some() { "Duplicate Symbol" } else { "Symbol Properties" };
    let folders = library::folders(&ed.doc);
    let mut done = None;
    let mut edit = false;
    home::dialog(&format!("symbol-props-{i}"), ctx, theme::PANEL_BG).show(ctx, |ui| {
        if home::title_bar(ui, title, 460.0) {
            done = Some(false);
        }
        Frame::NONE.inner_margin(Margin { left: 20, right: 20, top: 16, bottom: 14 }).show(ui, |ui| {
            ui.set_width(420.0);
            let label = |ui: &mut Ui, text: &str| ui.label(RichText::new(text).color(theme::TEXT_DIM));
            egui::Grid::new(("symbol-props-grid", i)).num_columns(2).spacing([14.0, 10.0]).show(ui, |ui| {
                label(ui, "Name");
                ui.horizontal(|ui| {
                    widgets::text_input(ui, &mut p.name, "Name", 230.0);
                    if p.duplicate.is_none() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| edit = ui.add(egui::Button::new("Edit").min_size(vec2(64.0, 24.0))).clicked());
                    }
                });
                ui.end_row();
                label(ui, "Type");
                let kinds = [SymbolKind::MovieClip, SymbolKind::Button, SymbolKind::Graphic].map(|k| (k, kind_name(k), true));
                if let Some(k) = widgets::pick(ui, "Symbol type", p.kind, &kinds, 160.0) {
                    p.kind = k;
                }
                ui.end_row();
                label(ui, "Folder");
                let mut items: Vec<(&str, &str, bool)> = vec![("", "Library root", true)];
                items.extend(folders.iter().map(|f| (f.as_str(), f.as_str(), true)));
                if let Some(f) = widgets::pick(ui, "Folder", p.folder.as_str(), &items, 230.0) {
                    p.folder = f.to_string();
                }
                ui.end_row();
            });
            ui.add_space(10.0);
            theme::section_with(ui, "Advanced", p.export, |ui| {
                ui.label(RichText::new("ActionScript Linkage").color(theme::SECTION_TITLE));
                widgets::row(ui, |ui| widgets::checkbox(ui, &mut p.export, "Export for ActionScript"));
                widgets::row(ui, |ui| {
                    widgets::label(ui, "Class:", 48.0);
                    ui.add_enabled_ui(p.export, |ui| widgets::text_input(ui, &mut p.class, "Class", 260.0));
                });
            });
            ui.add_space(12.0);
            if let Some(answer) = ok_cancel(ui) {
                done = Some(answer);
            }
        });
    });
    if edit {
        let path = p.path.clone();
        ed.edit_library_symbol(&path);
        done = Some(false);
    }
    done
}

/// Applies Symbol Properties or Duplicate Symbol as one undo step. Returns the symbol's path now.
fn apply_props(ed: &mut Editor, p: &Props) -> Result<String, String> {
    let name = p.name.trim();
    let to = if p.folder.is_empty() { name.to_string() } else { format!("{}/{name}", p.folder) };
    let class = p.class.trim();
    if p.export && class.is_empty() {
        return Err("Exported for ActionScript, a symbol needs a class name.".into());
    }
    // On a copy, so a refused name leaves no undo step behind.
    let mut doc = ed.doc.clone();
    match p.duplicate {
        Some(_) => library::duplicate(&mut doc, &p.path, &to),
        None => library::rename(&mut doc, &p.path, &to),
    }
    .map_err(|e| e.to_string())?;
    if let Some(symbol) = doc.library.get_mut(&to) {
        symbol.kind = p.kind;
        // Linkage changes as the dialog changed it, so what it doesn't show (runtime sharing...) stays.
        let class = p.export.then_some(class);
        if class != p.linked.as_deref() {
            library::set_linkage(symbol, class);
        }
    }
    if doc != ed.doc {
        ed.checkpoint();
        ed.doc = doc;
        match p.duplicate {
            // Modify > Symbol > Duplicate Symbol: the copy replaces the selected instances, in the same step.
            Some(true) => {
                for (r, _) in selected_instances(ed) {
                    if let Some(Element::Instance(inst)) = ed.element_mut(r) {
                        inst.symbol = to.clone();
                    }
                }
            }
            Some(false) => {}
            None => crate::library::follow_rename(ed, &p.path, &to),
        }
        ed.library_selection = Some(to.clone());
    }
    Ok(to)
}

fn swap_dialog(ctx: &egui::Context, ed: &mut Editor, s: &mut Swap, i: usize) -> Option<bool> {
    let current: Vec<String> = s.refs.iter().filter_map(|&r| match ed.element(r) {
        Some(Element::Instance(inst)) => Some(inst.symbol.clone()),
        _ => None,
    }).collect();
    let mut done = None;
    let mut duplicate = false;
    // The library as a tree, every folder open.
    let mut items: Vec<(String, Option<SymbolKind>)> = library::folders(&ed.doc).into_iter().map(|f| (f, None)).collect();
    items.extend(ed.doc.library.iter().map(|(name, sym)| (name.clone(), Some(sym.kind))));
    items.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    home::dialog(&format!("swap-symbol-{i}"), ctx, theme::PANEL_BG).show(ctx, |ui| {
        if home::title_bar(ui, "Swap Symbol", 560.0) {
            done = Some(false);
        }
        Frame::NONE.inner_margin(Margin { left: 16, right: 16, top: 14, bottom: 14 }).show(ui, |ui| {
            ui.set_width(528.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                // The picked symbol's preview, on the stage colour.
                let (preview, _) = ui.allocate_exact_size(vec2(120.0, 120.0), Sense::hover());
                let [r, g, b, _] = ed.doc.background.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8);
                ui.painter().rect_filled(preview, 0.0, Color32::from_rgb(r, g, b));
                let ppp = ui.ctx().pixels_per_point();
                if let Some(tex) = ed.library_preview(ui.ctx(), &s.picked.clone(), preview.size() * ppp) {
                    ui.painter().image(tex, preview, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                Frame::NONE.fill(theme::INSET).stroke(Stroke::new(1.0, theme::BORDER)).show(ui, |ui| {
                    ui.set_width(280.0);
                    egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, false]).show(ui, |ui| ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (path, kind) in &items {
                            let depth = path.matches('/').count() as f32;
                            let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), if kind.is_some() { Sense::click() } else { Sense::hover() });
                            let leaf = library::leaf(path);
                            let picked = kind.is_some() && *path == s.picked;
                            let p = ui.painter();
                            if picked {
                                p.rect_filled(row, 0.0, theme::SELECTION);
                            } else if kind.is_some() && resp.hovered() {
                                p.rect_filled(row, 0.0, theme::BUTTON_HOVER);
                            }
                            // The instances' symbol now: a dot before it.
                            if current.contains(path) {
                                p.circle_filled(pos2(row.left() + 8.0, row.center().y), 2.5, theme::TEXT);
                            }
                            let x = row.left() + 16.0 + depth * 14.0;
                            let icon = kind.map_or(Icon::FolderOpen, crate::library::kind_icon);
                            icons::paint(p, Rect::from_min_size(pos2(x, row.top() + 2.0), vec2(16.0, 16.0)), icon, theme::TEXT);
                            let color = if picked { Color32::WHITE } else { theme::TEXT };
                            p.text(pos2(x + 20.0, row.center().y), Align2::LEFT_CENTER, leaf, FontId::proportional(12.0), color);
                            resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, kind.is_some(), picked, leaf));
                            if resp.clicked() {
                                s.picked = path.clone();
                            }
                            if resp.double_clicked() {
                                done = Some(true);
                            }
                        }
                    }));
                });
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    let (_, rect) = ui.allocate_space(vec2(72.0, 28.0));
                    if home::push(ui, rect, "OK", true, 4.0).clicked() {
                        done = Some(true);
                    }
                    if ui.add(egui::Button::new("Cancel").min_size(vec2(72.0, 28.0))).clicked() {
                        done = Some(false);
                    }
                });
            });
            ui.add_space(10.0);
            duplicate = ui.add(egui::Button::new("Duplicate Symbol...").min_size(vec2(0.0, 28.0))).clicked();
        });
    });
    if duplicate {
        let picked = s.picked.clone();
        self::duplicate(ed, &picked, false);
    }
    done
}

/// Swap Symbol's OK: the instances show the symbol picked, in place, their colour and all.
fn apply_swap(ed: &mut Editor, s: &Swap) {
    let picked = s.picked.clone();
    // Inside a symbol, one that draws it would draw itself without end.
    if let Some(edited) = ed.editing.last().map(|t| t.symbol.clone())
        && (picked == edited || ed.doc.library.get(&picked).is_some_and(|sym| symbols::draws(&ed.doc, sym.timeline.layers.iter().flat_map(|l| &l.frames).flat_map(|k| &k.elements), &edited)))
    {
        ed.log("Swapping that symbol in here would put a symbol inside itself.");
        return;
    }
    let refs = s.refs.clone();
    ed.edit_timeline(|_, tl| {
        for &r in &refs {
            if let Some(Element::Instance(inst)) = tl.element_mut(r) {
                inst.symbol = picked.clone();
            }
        }
        refs.clone()
    });
}

#[cfg(test)]
mod tests {
    use eframe::egui::Modifiers;
    use egui_kittest::kittest::Queryable;
    use minnamate_core::doc::Matrix;

    use super::*;
    use crate::app::testkit::*;

    // The tween fixture's Ball instance, on layer 1.
    const BALL: ElementRef = ElementRef { layer: 1, keyframe: 0, element: 0 };

    #[test]
    fn symbol_properties_rename_retype_and_link() {
        let mut h = harness(false);
        h.run();
        properties(h.state_mut(), "Ball");
        h.run();
        let Some(Dialog::Props(p)) = h.state_mut().symbol_dialogs.last_mut() else { panic!("Symbol Properties is up") };
        (p.name, p.kind, p.export, p.class) = ("Planet".into(), SymbolKind::Graphic, true, "Planet".into());
        h.run();
        h.get_by_label("OK").click();
        h.run();
        let ed = h.state();
        assert!(ed.symbol_dialogs.is_empty());
        let planet = &ed.doc.library["Planet"];
        assert_eq!((planet.kind, library::linkage(planet), planet.timeline.name.as_str()), (SymbolKind::Graphic, Some("Planet"), "Planet"));
        let Some(Element::Instance(i)) = ed.element(BALL) else { panic!() };
        assert_eq!(i.symbol, "Planet", "the instances follow");
        // One undo step takes it all back.
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert!(h.state().doc.library.contains_key("Ball") && !h.state().doc.library.contains_key("Planet"));
        // A name in use is refused, and the dialog stays.
        properties(h.state_mut(), "Ball");
        let Some(Dialog::Props(p)) = h.state_mut().symbol_dialogs.last_mut() else { panic!() };
        p.name = "Blink".into();
        h.run();
        h.key_press(Key::Enter);
        h.run();
        assert!(h.state().error.is_some() && h.state().symbol_dialogs.len() == 1);
    }

    #[test]
    fn a_rename_leaves_linkage_it_doesnt_show_alone() {
        let mut h = harness(false);
        h.run();
        // Imported for runtime sharing: a class, but not exported for ActionScript here.
        let imported = [("linkageImportForRS", "true"), ("linkageURL", "shared.swf"), ("linkageClassName", "Ball")];
        let ball = h.state_mut().doc.library.get_mut("Ball").unwrap();
        imported.iter().for_each(|(k, v)| ball.extra.set(k, Some(v)));
        properties(h.state_mut(), "Ball");
        let Some(Dialog::Props(p)) = h.state_mut().symbol_dialogs.last_mut() else { panic!() };
        p.name = "Planet".into();
        h.run();
        h.get_by_label("OK").click();
        h.run();
        let planet = &h.state().doc.library["Planet"];
        assert!(imported.iter().all(|(k, v)| planet.extra.get(k) == Some(v)), "{:?}", planet.extra.attrs);
    }

    #[test]
    fn swap_symbol_keeps_the_instance_where_it_was() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        let matrix = |h: &Harness<'_, Editor>| match h.state().element(BALL) {
            Some(Element::Instance(i)) => (i.symbol.clone(), i.matrix),
            _ => panic!("the ball"),
        };
        let (_, before): (String, Matrix) = matrix(&h);
        h.state_mut().selection = vec![BALL];
        h.state_mut().run(Action::SwapSymbol, &ctx);
        h.run();
        h.get_by_label("Blink").click();
        h.run();
        h.get_by_label("OK").click();
        h.run();
        assert_eq!(matrix(&h), ("Blink".to_string(), before));
        assert!(h.state().symbol_dialogs.is_empty() && h.state().can_undo());

        // Duplicate Symbol from Swap Symbol: the copy is the one picked.
        h.state_mut().run(Action::SwapSymbol, &ctx);
        h.run();
        h.get_by_label("Duplicate Symbol...").click();
        h.run();
        assert_eq!(h.state().symbol_dialogs.len(), 2);
        h.key_press(Key::Enter);
        h.run();
        let Some(Dialog::Swap(s)) = h.state().symbol_dialogs.last() else { panic!("back on Swap Symbol") };
        assert_eq!(s.picked, "Blink copy");
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(matrix(&h).0, "Blink copy");
    }

    /// Writes target/screenshots/swap_symbol.png and symbol_properties.png.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn symbol_dialogs_screenshot() {
        let mut h = harness(true);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().selection = vec![BALL];
        h.state_mut().run(Action::SwapSymbol, &ctx);
        h.run();
        screenshot(&mut h, "swap_symbol");
        h.key_press(Key::Escape);
        h.run();
        properties(h.state_mut(), "Ball");
        h.run();
        screenshot(&mut h, "symbol_properties");
    }
}
