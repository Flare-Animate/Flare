//! The Library panel (docs/animate-ui.md §7.3): the document's symbols, in the
//! folders their names give them ("Folder/Name" sits in Folder), with a
//! preview, search, sortable columns, inline rename and the bottom toolbar.

use std::collections::HashMap;

use eframe::egui::{self, Align2, Color32, FontId, Id, Margin, Rect, Sense, Stroke, TextEdit, Ui, WidgetInfo, WidgetType, pos2, vec2};
use minnamate_core::doc::{Document, Element, SymbolKind};

use crate::app::Editor;
use crate::icons::{self, Icon};
use crate::theme;

pub fn kind_icon(kind: SymbolKind) -> Icon {
    match kind {
        SymbolKind::MovieClip => Icon::MovieClip,
        SymbolKind::Graphic => Icon::Graphic,
        SymbolKind::Button => Icon::Button,
    }
}

fn kind_name(kind: Option<SymbolKind>) -> &'static str {
    match kind {
        Some(SymbolKind::MovieClip) => "Movie Clip",
        Some(SymbolKind::Graphic) => "Graphic",
        Some(SymbolKind::Button) => "Button",
        None => "Folder",
    }
}

/// The folder holding `path`, and its own name.
fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

/// A line of the list: a folder (no kind) or a symbol, by its full path.
struct Row {
    path: String,
    depth: usize,
    kind: Option<SymbolKind>,
}

/// The columns, and how wide each is (Name takes what's left, at least 120).
const COLUMNS: [(&str, f32); 5] = [("Name", 0.0), ("Linkage", 90.0), ("Use Count", 70.0), ("Date Modified", 120.0), ("Type", 80.0)];

/// How the list is sorted: a column, and whether it runs backwards.
#[derive(Clone, Copy, Default)]
struct Sort {
    column: usize,
    descending: bool,
}

/// Every folder the symbols' names make, outermost first.
fn folders(doc: &Document) -> Vec<String> {
    let mut all: Vec<String> = doc.library.keys().flat_map(|name| name.match_indices('/').map(|(i, _)| name[..i].to_string())).collect();
    all.sort();
    all.dedup();
    all
}

/// How many instances of each symbol the document holds, in scenes and symbols.
fn use_counts(doc: &Document) -> HashMap<String, usize> {
    fn walk(elements: &[Element], counts: &mut HashMap<String, usize>) {
        for el in elements {
            match el {
                Element::Instance(i) => *counts.entry(i.symbol.clone()).or_default() += 1,
                Element::Group(g) => walk(g, counts),
                _ => {}
            }
        }
    }
    let mut counts = HashMap::new();
    for tl in doc.scenes.iter().chain(doc.library.values().map(|s| &s.timeline)) {
        for key in tl.layers.iter().flat_map(|l| &l.frames) {
            walk(&key.elements, &mut counts);
        }
    }
    counts
}

/// "MM/DD/YYYY hh:mm" (UTC) for XFL's lastModified seconds.
fn date(seconds: &str) -> String {
    let Ok(s) = seconds.parse::<i64>() else { return String::new() };
    // Days since 1970 to a civil date (Howard Hinnant's algorithm).
    let (days, rest) = (s.div_euclid(86400), s.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{m:02}/{d:02}/{y} {:02}:{:02}", rest / 3600, rest / 60 % 60)
}

/// The list's lines: each folder's contents under it while it's open, sorted
/// within each folder; a search lists every matching symbol on its own.
fn rows(ed: &Editor, sort: Sort, uses: &HashMap<String, usize>) -> Vec<Row> {
    let doc = &ed.doc;
    let folders = folders(doc);
    let cell = |path: &str, kind: Option<SymbolKind>| -> String {
        let symbol = doc.library.get(path);
        match sort.column {
            1 => symbol.and_then(|s| s.extra.get("linkageClassName")).unwrap_or_default().to_lowercase(),
            2 => format!("{:010}", uses.get(path).copied().unwrap_or(0)),
            3 => format!("{:020}", symbol.and_then(|s| s.extra.get("lastModified")).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0)),
            4 => kind_name(kind).to_string(),
            _ => split(path).1.to_lowercase(),
        }
    };
    let mut items: Vec<(String, Option<SymbolKind>)> = folders.iter().map(|f| (f.clone(), None)).collect();
    items.extend(doc.library.iter().map(|(name, s)| (name.clone(), Some(s.kind))));
    let order = |a: &(String, Option<SymbolKind>), b: &(String, Option<SymbolKind>)| {
        let o = (cell(&a.0, a.1), split(&a.0).1.to_lowercase()).cmp(&(cell(&b.0, b.1), split(&b.0).1.to_lowercase()));
        if sort.descending { o.reverse() } else { o }
    };
    let search = ed.library_search.trim().to_lowercase();
    if !search.is_empty() {
        let linkage = |p: &str| doc.library.get(p).and_then(|s| s.extra.get("linkageClassName")).unwrap_or_default().to_lowercase();
        let mut found: Vec<_> = items.into_iter().filter(|(p, k)| k.is_some() && (split(p).1.to_lowercase().contains(&search) || linkage(p).contains(&search))).collect();
        found.sort_by(order);
        return found.into_iter().map(|(path, kind)| Row { path, depth: 0, kind }).collect();
    }
    items.sort_by(order);
    fn add(items: &[(String, Option<SymbolKind>)], parent: &str, depth: usize, ed: &Editor, out: &mut Vec<Row>) {
        for (path, kind) in items.iter().filter(|(p, _)| split(p).0 == parent) {
            out.push(Row { path: path.clone(), depth, kind: *kind });
            if kind.is_none() && ed.library_open.contains(path) {
                add(items, path, depth + 1, ed, out);
            }
        }
    }
    let mut out = Vec::new();
    add(&items, "", 0, ed, &mut out);
    out
}

pub fn panel(ui: &mut Ui, ed: &mut Editor) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let uses = use_counts(&ed.doc);
    let item_count = ed.doc.library.len() + folders(&ed.doc).len();

    // Document row: the library shown (this document's), Pin and New Library Panel.
    egui::Frame::NONE.inner_margin(Margin { left: 8, right: 4, top: 3, bottom: 3 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let title = ed.title.clone();
            crate::widgets::pick(ui, "Library document", 0, &[(0, title.as_str(), true)], ui.available_width() - 52.0);
            // ponytail: one document at a time, so there's no other library to pin or open.
            theme::icon_button(ui, theme::ICON_BUTTON, false, false, "Pin current library", |p, r, c| icons::paint(p, r.shrink(3.0), Icon::Pin, c));
            theme::icon_button(ui, theme::ICON_BUTTON, false, false, "New library panel", |p, r, c| icons::paint(p, r.shrink(3.0), Icon::NewLibrary, c));
        });
    });

    // Preview: the selected symbol's first frame, fitted, on the stage colour.
    let (preview, _) = ui.allocate_exact_size(vec2(ui.available_width(), 140.0), Sense::hover());
    let [r, g, b, _] = ed.doc.background.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8);
    ui.painter().rect_filled(preview, 0.0, Color32::from_rgb(r, g, b));
    if let Some(name) = ed.library_selection.clone().filter(|n| ed.doc.library.contains_key(n)) {
        let ppp = ui.ctx().pixels_per_point();
        if let Some(tex) = ed.library_preview(ui.ctx(), &name, preview.size() * ppp) {
            ui.painter().image(tex, preview, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
    }

    // Info row: the count, then the search box.
    let (info, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    let count = if item_count == 1 { "1 item".to_string() } else { format!("{item_count} items") };
    ui.painter().text(info.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, count, FontId::proportional(12.0), theme::TEXT_DIM);
    let field = Rect::from_min_size(pos2(info.right() - 146.0, info.center().y - 10.0), vec2(140.0, 20.0));
    let search = TextEdit::singleline(&mut ed.library_search).margin(Margin { left: 20, right: 4, top: 2, bottom: 2 }).min_size(field.size());
    ui.put(field, search);
    icons::paint(ui.painter(), Rect::from_center_size(field.left_center() + vec2(10.0, 0.0), vec2(12.0, 12.0)), Icon::Zoom, theme::TEXT_DIM);

    // Column headers; a click sorts by one, the triangle at the end reverses.
    let sort_id = Id::new("library-sort");
    let mut sort: Sort = ui.data(|d| d.get_temp(sort_id)).unwrap_or_default();
    let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let name_w = (head.width() * 0.5).max(120.0);
    let columns = column_x(head.left(), name_w);
    {
        let p = ui.painter_at(head);
        p.rect_filled(head, 0.0, theme::PANEL_HEADER);
        p.hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0, theme::BORDER));
    }
    for (i, &(label, _)) in COLUMNS.iter().enumerate() {
        let x = columns[i];
        let end = columns.get(i + 1).copied().unwrap_or(head.right()).min(head.right() - 16.0);
        if x >= end {
            continue;
        }
        let cell = Rect::from_x_y_ranges(x..=end, head.y_range());
        let resp = ui.interact(cell, ui.id().with(("library-column", i)), Sense::click());
        let p = ui.painter_at(cell);
        let color = if sort.column == i || resp.hovered() { theme::TEXT } else { theme::TEXT_DIM };
        p.text(cell.left_center() + vec2(if i == 0 { 26.0 } else { 6.0 }, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(11.0), color);
        if i > 0 {
            p.vline(x, head.y_range(), Stroke::new(1.0, theme::BORDER));
        }
        if resp.clicked() {
            sort.column = i;
        }
    }
    let flip = Rect::from_min_max(pos2(head.right() - 16.0, head.top()), head.max);
    let flip_resp = ui.interact(flip, ui.id().with("library-sort-order"), Sense::click()).on_hover_text("Toggle sort order");
    let tri = if sort.descending { [pos2(-4.0, -2.0), pos2(4.0, -2.0), pos2(0.0, 3.0)] } else { [pos2(-4.0, 2.0), pos2(4.0, 2.0), pos2(0.0, -3.0)] };
    let c = flip.center();
    ui.painter().add(egui::Shape::convex_polygon(tri.iter().map(|p| c + p.to_vec2()).collect(), theme::TEXT_DIM, Stroke::NONE));
    if flip_resp.clicked() {
        sort.descending = !sort.descending;
    }
    ui.data_mut(|d| d.insert_temp(sort_id, sort));

    // The list.
    let rows = rows(ed, sort, &uses);
    let mut action = None;
    let list_h = ui.available_height() - 25.0;
    egui::ScrollArea::vertical().max_height(list_h).auto_shrink(false).show(ui, |ui| {
        for row in &rows {
            if let Some(a) = list_row(ui, ed, row, &columns, &uses) {
                action = Some(a);
            }
        }
        // A click below the rows selects nothing.
        let rest = ui.allocate_response(vec2(ui.available_width(), ui.available_height().max(0.0)), Sense::click());
        if rest.clicked() {
            ed.library_selection = None;
        }
        theme::context_menu(&rest, |ui| commands(ui, ed, None, true));
    });
    if let Some(a) = action {
        run(ed, a);
    }

    // Bottom toolbar: New Symbol, New Folder, Properties, Delete.
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 25.0), Sense::hover());
    ui.painter().hline(bar.x_range(), bar.top() + 0.5, Stroke::new(1.0, theme::BORDER));
    let mut x = bar.left() + 6.0;
    let selected = ed.library_selection.clone();
    // ponytail: folders exist only through the names in them, and there's no Symbol Properties dialog.
    let buttons = [(Icon::Layer, "New Symbol", true, Some(Command::NewSymbol)), (Icon::NewFolder, "New Folder", false, None), (Icon::Info, "Properties", false, None), (Icon::Delete, "Delete", selected.is_some(), selected.clone().map(Command::Delete))];
    for (icon, tip, enabled, command) in buttons {
        let rect = Rect::from_min_size(pos2(x, bar.top() + 1.5), vec2(theme::ICON_BUTTON, theme::ICON_BUTTON));
        let resp = ui.put(rect, |ui: &mut Ui| theme::icon_button(ui, theme::ICON_BUTTON, false, enabled, tip, |p, r, c| icons::paint(p, r.shrink(3.0), icon, c)));
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, tip));
        if resp.clicked()
            && let Some(command) = command
        {
            run(ed, command);
        }
        x += theme::ICON_BUTTON + 2.0;
    }
}

/// Where each column starts, Name first.
fn column_x(left: f32, name_w: f32) -> Vec<f32> {
    let mut x = left;
    COLUMNS.iter().map(|&(_, w)| {
        let at = x;
        x += if w == 0.0 { name_w } else { w };
        at
    }).collect()
}

/// What a row, the toolbar or a menu asks the Library to do.
#[derive(Clone, PartialEq, Debug)]
enum Command {
    NewSymbol,
    Edit(String),
    /// Starts renaming an item in place.
    Rename(String),
    /// Gives the item at the first path the name typed, the second.
    Renamed(String, String),
    Delete(String),
    Duplicate(String),
    /// Opens (true) or closes a folder.
    Fold(String, bool),
    FoldAll(bool),
}

/// One line of the list: indent, disclosure triangle for a folder, glyph and
/// name, then the other columns. Returns what it asks for.
fn list_row(ui: &mut Ui, ed: &mut Editor, row: &Row, columns: &[f32], uses: &HashMap<String, usize>) -> Option<Command> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
    let leaf = split(&row.path).1.to_string();
    resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, ed.library_selection.as_deref() == Some(row.path.as_str()), &leaf));
    let x0 = rect.left() + 6.0 + row.depth as f32 * 14.0;
    let open = ed.library_open.contains(&row.path);
    let selected = ed.library_selection.as_deref() == Some(row.path.as_str());
    let p = ui.painter_at(rect);
    if selected {
        p.rect_filled(rect, 0.0, theme::SELECTION);
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, theme::BUTTON_HOVER);
    }
    let triangle = Rect::from_min_size(pos2(x0, rect.top()), vec2(12.0, rect.height()));
    let glyph = Rect::from_min_size(pos2(x0 + 12.0, rect.top() + 2.0), vec2(16.0, 16.0));
    let name_x = glyph.right() + 4.0;
    let color = if selected { Color32::WHITE } else { theme::TEXT };
    let icon = match row.kind {
        Some(kind) => kind_icon(kind),
        None => {
            let c = triangle.center();
            let pts = if open { vec![c + vec2(-3.5, -2.0), c + vec2(3.5, -2.0), c + vec2(0.0, 2.5)] } else { vec![c + vec2(-2.0, -3.5), c + vec2(2.5, 0.0), c + vec2(-2.0, 3.5)] };
            p.add(egui::Shape::convex_polygon(pts, theme::TEXT_DIM, Stroke::NONE));
            if open { Icon::FolderOpen } else { Icon::Folder }
        }
    };
    icons::paint(&p, glyph, icon, color);
    let renaming = ed.library_rename.as_ref().is_some_and(|(path, _)| *path == row.path);
    let name_end = columns.get(1).copied().unwrap_or(rect.right()) - 4.0;
    if !renaming {
        p.with_clip_rect(Rect::from_x_y_ranges(rect.left()..=name_end, rect.y_range())).text(pos2(name_x, rect.center().y), Align2::LEFT_CENTER, &leaf, FontId::proportional(12.0), color);
    }
    let symbol = ed.doc.library.get(&row.path);
    let cells = [
        symbol.and_then(|s| s.extra.get("linkageClassName")).unwrap_or_default().to_string(),
        symbol.map(|_| uses.get(&row.path).copied().unwrap_or(0).to_string()).unwrap_or_default(),
        symbol.and_then(|s| s.extra.get("lastModified")).map(date).unwrap_or_default(),
        kind_name(row.kind).to_string(),
    ];
    for (i, text) in cells.iter().enumerate() {
        let x = columns[i + 1];
        let end = columns.get(i + 2).copied().unwrap_or(rect.right());
        if x < rect.right() {
            p.with_clip_rect(Rect::from_x_y_ranges(x..=end - 4.0, rect.y_range())).text(pos2(x + 6.0, rect.center().y), Align2::LEFT_CENTER, text, FontId::proportional(12.0), theme::TEXT_DIM);
        }
    }

    if renaming {
        let field = Rect::from_x_y_ranges(name_x - 3.0..=name_end, rect.top() + 1.0..=rect.bottom() - 1.0);
        let id = ui.id().with("library-rename");
        let (path, mut text) = ed.library_rename.clone().unwrap_or_default();
        let edit = ui.put(field, TextEdit::singleline(&mut text).id(id).margin(Margin::symmetric(2, 1)));
        if !edit.has_focus() && !edit.lost_focus() {
            edit.request_focus();
        }
        if edit.lost_focus() {
            ed.library_rename = None;
            if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                return Some(Command::Renamed(path, text.trim().to_string()));
            }
        } else {
            ed.library_rename = Some((path, text));
        }
        return None;
    }

    let mut command = None;
    let on_triangle = resp.interact_pointer_pos().is_some_and(|p| triangle.contains(p));
    if resp.clicked() || resp.secondary_clicked() {
        ed.library_selection = Some(row.path.clone());
        if row.kind.is_none() && on_triangle && resp.clicked() {
            command = Some(Command::Fold(row.path.clone(), !open));
        }
    }
    if resp.double_clicked() {
        let on_name = resp.interact_pointer_pos().is_some_and(|p| p.x >= name_x);
        command = Some(match row.kind {
            None => Command::Fold(row.path.clone(), !open),
            // Double-clicking the glyph edits the symbol, the name renames it.
            Some(_) if on_name => Command::Rename(row.path.clone()),
            Some(_) => Command::Edit(row.path.clone()),
        });
    }
    theme::context_menu(&resp, |ui| commands(ui, ed, Some(&row.path), true));
    command
}

/// The commands on `target` (or the selection), as the panel menu lists them,
/// or with `context` as an item's context menu does; greyed ones aren't built.
fn commands(ui: &mut Ui, ed: &mut Editor, target: Option<&str>, context: bool) {
    let target = target.map(str::to_string).or_else(|| ed.library_selection.clone());
    let symbol = target.clone().filter(|t| ed.doc.library.contains_key(t));
    let folder = target.clone().filter(|t| !ed.doc.library.contains_key(t) && !t.is_empty());
    let mut picked = None;
    let mut item = |ui: &mut Ui, label: &str, command: Option<Command>| {
        if theme::menu_item(ui, command.is_some(), label, "").clicked() {
            picked = command;
            ui.close();
        }
    };
    let greyed = |item: &mut dyn FnMut(&mut Ui, &str, Option<Command>), ui: &mut Ui, labels: &[&str]| {
        for label in labels {
            item(ui, label, None);
        }
    };
    item(ui, "New Symbol...", Some(Command::NewSymbol));
    greyed(&mut item, ui, if context { &["New Folder"] } else { &["New Folder", "New Font...", "New Video..."] });
    ui.separator();
    item(ui, "Rename", target.clone().map(Command::Rename));
    item(ui, "Delete", target.clone().map(Command::Delete));
    ui.separator();
    item(ui, "Duplicate...", symbol.clone().map(Command::Duplicate));
    greyed(&mut item, ui, &["Move to..."]);
    ui.separator();
    item(ui, "Edit", symbol.clone().map(Command::Edit));
    greyed(&mut item, ui, &["Edit with...", "Edit Class", "Play"]);
    ui.separator();
    if context {
        greyed(&mut item, ui, &["Properties..."]);
        ui.separator();
        greyed(&mut item, ui, &["Generate Sprite Sheet...", "Generate Texture Atlas...", "Export PNG Sequence..."]);
        ui.separator();
        greyed(&mut item, ui, &["Export Asset...", "Save As Asset..."]);
    } else {
        greyed(&mut item, ui, &["Update...", "Properties...", "Component Definition...", "Runtime Shared Library URL..."]);
    }
    ui.separator();
    greyed(&mut item, ui, &["Select Unused Items"]);
    ui.separator();
    item(ui, "Expand Folder", folder.clone().filter(|f| !ed.library_open.contains(f)).map(|f| Command::Fold(f, true)));
    item(ui, "Collapse Folder", folder.filter(|f| ed.library_open.contains(f)).map(|f| Command::Fold(f, false)));
    item(ui, "Expand All Folders", Some(Command::FoldAll(true)));
    item(ui, "Collapse All Folders", Some(Command::FoldAll(false)));
    if let Some(c) = picked {
        run(ed, c);
    }
}

/// The Library's panel menu (≡).
pub fn panel_menu(ui: &mut Ui, ed: &mut Editor) {
    commands(ui, ed, None, false);
    ui.separator();
    crate::panels::menu_tail(ui, ed);
}

fn run(ed: &mut Editor, command: Command) {
    match command {
        // The Create New Symbol dialog, as Insert > New Symbol opens it.
        Command::NewSymbol => crate::convert_symbol::open_new(ed),
        Command::Edit(name) => ed.edit_library_symbol(&name),
        Command::Rename(path) => ed.library_rename = Some((path.clone(), split(&path).1.to_string())),
        Command::Renamed(old, new) => rename(ed, &old, &new),
        Command::Delete(path) => delete(ed, &path),
        Command::Duplicate(name) => duplicate(ed, &name),
        Command::Fold(path, open) => {
            if open {
                ed.library_open.insert(path);
            } else {
                ed.library_open.remove(&path);
            }
        }
        Command::FoldAll(open) => {
            ed.library_open = if open { folders(&ed.doc).into_iter().collect() } else { Default::default() };
        }
    }
}

/// Duplicate: a copy named "<name> copy" (then "copy 2"...) in the same folder.
// ponytail: Animate asks for the name and type first.
fn duplicate(ed: &mut Editor, name: &str) {
    let Some(mut symbol) = ed.doc.library.get(name).cloned() else { return };
    let base = format!("{name} copy");
    let copy = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base} {n}") }).find(|n| !ed.doc.library.contains_key(n)).unwrap_or(base);
    symbol.timeline.name = split(&copy).1.to_string();
    ed.checkpoint();
    ed.doc.library.insert(copy.clone(), symbol);
    ed.library_selection = Some(copy);
}

/// Renames the item (symbol or folder) at `old` to `new` in the same folder,
/// and every instance and folder path that names it.
fn rename(ed: &mut Editor, old: &str, new: &str) {
    let parent = split(old).0;
    let to = if parent.is_empty() { new.to_string() } else { format!("{parent}/{new}") };
    if new.is_empty() || to == old {
        return;
    }
    let under = |name: &str| name == old || name.starts_with(&format!("{old}/"));
    let taken = ed.doc.library.keys().any(|n| n == &to || n.starts_with(&format!("{to}/")));
    if taken {
        ed.error = Some(format!("The name \"{new}\" is already in use in the library."));
        return;
    }
    let renamed = |name: &str| format!("{to}{}", &name[old.len()..]);
    ed.checkpoint();
    let moved: Vec<String> = ed.doc.library.keys().filter(|n| under(n)).cloned().collect();
    for name in moved {
        if let Some(mut symbol) = ed.doc.library.remove(&name) {
            let new_name = renamed(&name);
            symbol.timeline.name = split(&new_name).1.to_string();
            ed.doc.library.insert(new_name, symbol);
        }
    }
    for_instances(&mut ed.doc, &mut |symbol: &mut String| {
        if under(symbol) {
            *symbol = renamed(symbol);
        }
    });
    for target in &mut ed.editing {
        if under(&target.symbol) {
            target.symbol = renamed(&target.symbol);
        }
    }
    ed.library_open = std::mem::take(&mut ed.library_open).into_iter().map(|f| if under(&f) { renamed(&f) } else { f }).collect();
    ed.library_selection = Some(to);
}

/// Runs `f` on the symbol name of every instance in the document.
fn for_instances(doc: &mut Document, f: &mut dyn FnMut(&mut String)) {
    fn walk(elements: &mut [Element], f: &mut dyn FnMut(&mut String)) {
        for el in elements {
            match el {
                Element::Instance(i) => f(&mut i.symbol),
                Element::Group(g) => walk(g, f),
                _ => {}
            }
        }
    }
    let timelines = doc.scenes.iter_mut().chain(doc.library.values_mut().map(|s| &mut s.timeline));
    for key in timelines.flat_map(|tl| &mut tl.layers).flat_map(|l| &mut l.frames) {
        walk(&mut key.elements, f);
    }
}

/// Deletes the item at `path` (a folder takes everything in it) and every
/// instance of what's deleted, as Animate's trash does.
fn delete(ed: &mut Editor, path: &str) {
    let gone: Vec<String> = ed.doc.library.keys().filter(|n| *n == path || n.starts_with(&format!("{path}/"))).cloned().collect();
    if gone.is_empty() {
        return;
    }
    ed.checkpoint();
    for name in &gone {
        ed.doc.library.remove(name);
    }
    fn strip(elements: &mut Vec<Element>, gone: &[String]) {
        elements.retain(|el| !matches!(el, Element::Instance(i) if gone.contains(&i.symbol)));
        for el in elements {
            if let Element::Group(g) = el {
                strip(g, gone);
            }
        }
    }
    let doc = &mut ed.doc;
    let timelines = doc.scenes.iter_mut().chain(doc.library.values_mut().map(|s| &mut s.timeline));
    for key in timelines.flat_map(|tl| &mut tl.layers).flat_map(|l| &mut l.frames) {
        strip(&mut key.elements, &gone);
    }
    // Leave any symbol being edited that went with it.
    while ed.editing.iter().any(|t| !ed.doc.library.contains_key(&t.symbol)) {
        ed.exit_symbol();
    }
    ed.selection.clear();
    ed.library_selection = None;
    ed.library_open.retain(|f| f != path && !f.starts_with(&format!("{path}/")));
}

#[cfg(test)]
mod tests {
    use minnamate_core::doc::{Document, Element, Instance, Symbol, SymbolKind};

    use super::*;
    use crate::app::testkit::*;

    /// A document whose library holds "Star" and "Folder/Ball", with a Ball on stage.
    pub(crate) fn library_doc() -> Document {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        let symbol = |name: &str| {
            let mut timeline = Document::new(1.0, 1.0, 1.0).scenes.remove(0);
            timeline.name = name.into();
            Symbol { kind: SymbolKind::MovieClip, timeline, extra: Default::default() }
        };
        doc.library.insert("Star".into(), symbol("Star"));
        doc.library.insert("Folder/Ball".into(), symbol("Ball"));
        let instance = Instance {
            symbol: "Folder/Ball".into(),
            kind: SymbolKind::MovieClip,
            matrix: minnamate_core::doc::Matrix::IDENTITY,
            pivot: (0.0, 0.0),
            color: minnamate_core::doc::ColorTransform::IDENTITY,
            looping: minnamate_core::doc::Loop::Loop,
            first_frame: 0,
            extra: Default::default(),
        };
        doc.scenes[0].layers[0].frames[0].elements.push(Element::Instance(instance));
        doc
    }

    #[test]
    fn folders_expand_and_collapse() {
        use egui_kittest::kittest::Queryable;
        let mut h = blank();
        h.state_mut().load(library_doc(), "lib".into(), None);
        h.state_mut().right_tab = 1;
        h.run();
        assert!(h.query_by_label("Ball").is_none(), "folders start closed");
        assert!(h.query_by_label("Star").is_some());
        let row = h.get_by_label("Folder").rect();
        click_at(&mut h, pos2(row.left() + 12.0, row.center().y));
        h.run();
        assert!(h.query_by_label("Ball").is_some(), "the disclosure triangle opens the folder");
        assert_eq!(h.state().library_selection.as_deref(), Some("Folder"));
        h.state_mut().library_open.clear();
        h.run();
        assert!(h.query_by_label("Ball").is_none());

        // The panel menu (≡) opens them all.
        h.get_by_label("Library panel menu").click();
        h.run();
        // (Menu items carry their empty shortcut in their names, so match by prefix.)
        h.get_by_label_contains("Expand All Folders").click();
        h.run();
        assert!(h.query_by_label("Ball").is_some(), "Expand All Folders");
    }

    #[test]
    fn double_click_renames_by_the_name_and_edits_by_the_glyph() {
        use egui_kittest::kittest::Queryable;
        // Short frames, so two clicks in a row are a double-click.
        let builder = Harness::builder().with_size(eframe::egui::vec2(1600.0, 900.0)).with_step_dt(0.05);
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().load(library_doc(), "lib".into(), None);
        h.state_mut().right_tab = 1;
        h.run();
        let row = h.get_by_label("Star").rect();
        let name = pos2(row.left() + 50.0, row.center().y);
        click_at(&mut h, name);
        click_at(&mut h, name);
        h.run();
        assert_eq!(h.state().library_rename.as_ref().map(|r| r.0.as_str()), Some("Star"), "the name turns into a field");
        h.event(eframe::egui::Event::Text("s".into()));
        h.key_press(eframe::egui::Key::Enter);
        h.run();
        assert!(h.state().doc.library.contains_key("Stars") && h.state().library_rename.is_none());

        let row = h.get_by_label("Stars").rect();
        let glyph = pos2(row.left() + 26.0, row.center().y);
        click_at(&mut h, glyph);
        click_at(&mut h, glyph);
        h.run();
        assert_eq!(h.state().editing.last().map(|t| t.symbol.as_str()), Some("Stars"), "the glyph opens the symbol");
    }

    #[test]
    fn rename_and_delete_follow_instances() {
        let mut h = blank();
        h.state_mut().load(library_doc(), "lib".into(), None);
        let ed = h.state_mut();
        rename(ed, "Folder", "Props");
        assert!(ed.doc.library.contains_key("Props/Ball") && !ed.doc.library.contains_key("Folder/Ball"));
        let Element::Instance(i) = &ed.doc.scenes[0].layers[0].frames[0].elements[0] else { panic!() };
        assert_eq!(i.symbol, "Props/Ball", "instances follow the rename");
        rename(ed, "Star", "Ball");
        assert!(ed.doc.library.contains_key("Ball"), "a name taken only in another folder is free");
        delete(ed, "Props");
        assert!(ed.doc.scenes[0].layers[0].frames[0].elements.is_empty(), "deleting removes the instances too");
        assert_eq!(ed.doc.library.len(), 1);
        let ctx = h.ctx.clone();
        h.state_mut().run(crate::app::Action::Undo, &ctx);
        assert!(h.state().doc.library.contains_key("Props/Ball"), "one undo step");
        assert_eq!(date("0"), "01/01/1970 00:00");
        assert_eq!(date("1700000000"), "11/14/2023 22:13");
    }

    /// Writes target/screenshots/library.png: the Library with a folder open and a symbol selected.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn library_screenshot() {
        let mut h = harness(true);
        h.run();
        let ed = h.state_mut();
        ed.doc.library.insert("Props/Star".into(), ed.doc.library["Ball"].clone());
        ed.library_open.insert("Props".into());
        ed.library_selection = Some("Ball".into());
        ed.right_tab = 1;
        h.run();
        screenshot(&mut h, "library");
    }
}
