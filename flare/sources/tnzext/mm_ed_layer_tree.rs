//! The timeline's layer tree (docs/animate-ui.md §5.4, docs/animate-ui-interaction.md
//! §2.4–§2.5): folders with their disclosure triangles, masks and guides with their
//! layers indented under them, the layer menu's commands for those, and the parenting
//! view, where a layer's head is dragged onto the layer it should follow.

use eframe::egui::{self, Align, Layout, Painter, Pos2, Rect, Stroke, Ui, pos2, vec2};
use minnamate_core::doc::{LayerKind, Timeline};

use crate::app::{Action, Editor};
use crate::icons::{self, Icon};
use crate::theme;
use crate::timeline::{Geo, TYPE_ICON_X, keep_selection, layer_color, next_color, pin_colors, selected_layers};

/// How far each folder, mask or guide indents the layers in it.
pub(crate) const INDENT: f32 = 20.0;
/// The parenting view's column, between the names and the dot column.
pub(crate) const PARENTING_W: f32 = 60.0;
/// A folder's disclosure triangle's center, from its row's (indented) left edge.
const TRIANGLE_X: f32 = 16.0;

/// How many layers the timeline lists: those in closed folders aren't.
pub(crate) fn listed(tl: &Timeline) -> usize {
    (0..tl.layers.len()).filter(|&l| tl.listed(l)).count()
}

/// The nearest layer listed at or above `l`: `l` itself, or the closed folder hiding it.
fn shown(tl: &Timeline, l: usize) -> usize {
    (0..=l).rev().find(|&x| tl.listed(x)).unwrap_or(0)
}

/// What a point in a layer's row is over, left of the toggle columns.
pub(crate) enum Part {
    Triangle,
    Icon,
    Name,
    /// The layer's head in the parenting view.
    Head,
    /// The rest of the parenting view's column.
    Strip,
}

pub(crate) fn part_at(ed: &Editor, geo: &Geo, l: usize, p: Pos2, left: f32, column_right: f32) -> Part {
    let tl = ed.timeline();
    let (kind, indent, strip) = (tl.layers[l].kind, tl.depth(l) as f32 * INDENT, strip_left(column_right));
    if ed.tl.parenting && kind == LayerKind::Normal && (strip..=strip + PARENTING_W).contains(&p.x) {
        let at = head(tl, ed.frame, l, strip, geo.top_of(l), geo.row_h);
        return if at.expand(3.0).contains(p) { Part::Head } else { Part::Strip };
    }
    if kind == LayerKind::Folder && (p.x - (left + indent + TRIANGLE_X)).abs() <= 6.0 {
        Part::Triangle
    } else if p.x < left + indent + TYPE_ICON_X + 9.0 {
        Part::Icon
    } else {
        Part::Name
    }
}

/// A row's disclosure triangle (a folder's) and type icon, its indented left edge at `at`.
/// Masked and guided layers are plain sheets, indented under their mask or guide.
pub(crate) fn paint_kind(p: &Painter, tl: &Timeline, l: usize, at: Pos2) {
    let layer = &tl.layers[l];
    let icon = match layer.kind {
        LayerKind::Folder => {
            let c = at + vec2(TRIANGLE_X, 0.0);
            let points = if layer.open() {
                vec![c + vec2(-4.0, -2.0), c + vec2(4.0, -2.0), c + vec2(0.0, 3.0)]
            } else {
                vec![c + vec2(-2.0, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.0, 4.0)]
            };
            p.add(egui::Shape::convex_polygon(points, theme::TEXT_DIM, Stroke::NONE));
            if layer.open() { Icon::FolderOpen } else { Icon::Folder }
        }
        LayerKind::Mask => Icon::LayerMask,
        // A motion guide, once it guides a layer.
        LayerKind::Guide if tl.parent(l + 1) == Some(l) => Icon::LayerMotionGuide,
        LayerKind::Guide => Icon::LayerGuide,
        // A layer with a motion tween is a tween layer.
        LayerKind::Normal if layer.frames.iter().any(|k| matches!(k.tween, minnamate_core::doc::Tween::Motion(_))) => Icon::MotionTween,
        LayerKind::Normal => Icon::Layer,
    };
    icons::paint(p, Rect::from_center_size(at + vec2(TYPE_ICON_X, 0.0), vec2(15.0, 15.0)), icon, theme::TEXT_DIM);
}

/// A folder's disclosure triangle: opens or closes it. A layer it hides stops being the active one.
pub(crate) fn toggle_open(ed: &mut Editor, l: usize) {
    ed.checkpoint();
    let layer = &mut ed.timeline_mut().layers[l];
    let open = !layer.open();
    layer.set_open(open);
    ed.layer = shown(ed.timeline(), ed.layer);
    keep_selection(ed);
}

/// Where layers dropped at height `y` of the list go.
pub(crate) struct Landing {
    /// Into the gap above this layer; `layers.len()` is the bottom.
    pub gap: usize,
    /// Into this folder, mask or guide, or none.
    pub parent: Option<usize>,
    /// Dropped onto that row: into it, at its top.
    pub onto: Option<usize>,
    /// Where the gap's insertion line goes.
    pub y: f32,
}

/// Over the middle of a folder's, mask's or guide's row, layers go into it, at its top.
/// Elsewhere they go into the nearest gap, joining the folder, mask or guide of the layer
/// under it, so a gap between a folder's layers is inside the folder.
pub(crate) fn drop_at(ed: &Editor, geo: &Geo, y: f32) -> Landing {
    let tl = ed.timeline();
    let r = geo.row_at(y);
    if let Some(l) = geo.layer_at(y).filter(|&l| tl.layers[l].kind != LayerKind::Normal && (0.25..0.75).contains(&r.fract())) {
        return Landing { gap: l + 1, parent: Some(l), onto: Some(l), y };
    }
    let g = (r.round().max(0.0) as usize).min(geo.rows.len());
    let (gap, parent) = match geo.rows.get(g) {
        Some(&l) => (l, tl.parent(l)),
        None => (tl.layers.len(), None),
    };
    Landing { gap, parent, onto: None, y: geo.top_of_row(g) }
}

/// The mask layer `l` is, or is masked by.
fn masking(tl: &Timeline, l: usize) -> Option<usize> {
    tl.mask_of(l).or(tl.layers.get(l).filter(|x| x.kind == LayerKind::Mask).map(|_| l))
}

/// Whether one of the layer menu's mask, guide and folder commands can run now.
pub(crate) fn available(ed: &Editor, action: Action) -> bool {
    let (tl, l) = (ed.timeline(), ed.layer);
    let kind = tl.layers.get(l).map(|x| x.kind);
    match action {
        // On a normal layer, not one already masked or guided.
        Action::AddMotionGuide => kind == Some(LayerKind::Normal) && tl.parent(l).is_none_or(|p| tl.layers[p].kind == LayerKind::Folder),
        Action::ToggleMask => matches!(kind, Some(LayerKind::Normal | LayerKind::Mask)) && tl.mask_of(l).is_none(),
        Action::ShowMasking => masking(tl, l).is_some(),
        Action::ExpandFolders | Action::CollapseFolders => selected_layers(ed).iter().any(|&f| tl.layers[f].kind == LayerKind::Folder),
        Action::ExpandAllFolders | Action::CollapseAllFolders => tl.layers.iter().any(|x| x.kind == LayerKind::Folder),
        _ => true,
    }
}

/// The layer menu's Mask, Show Masking, Add Classic Motion Guide and folder commands.
pub(crate) fn command(ed: &mut Editor, action: Action) {
    let l = ed.layer;
    let sel = ed.selection.clone();
    match action {
        Action::ToggleMask => {
            ed.checkpoint();
            let tl = ed.timeline_mut();
            let on = tl.layers[l].kind != LayerKind::Mask;
            tl.set_mask(l, on);
        }
        // Locks the mask and what it masks, so the stage shows the masking.
        Action::ShowMasking => {
            let Some(mask) = masking(ed.timeline(), l) else { return };
            ed.edit_timeline(|_, tl| {
                for x in [mask].into_iter().chain(tl.descendants(mask)) {
                    tl.layers[x].set_locked(true);
                }
                sel
            });
        }
        Action::AddMotionGuide => {
            ed.checkpoint();
            let tl = ed.timeline_mut();
            pin_colors(tl);
            let color = next_color(tl);
            let name = format!("Guide: {}", tl.layers[l].name);
            let at = tl.add_motion_guide(l, &name);
            tl.layers[at].set_color(color);
            ed.layer = at;
            ed.follow_layers(|x| Some(if x < at { x } else { x + 1 }));
        }
        Action::ExpandFolders | Action::CollapseFolders | Action::ExpandAllFolders | Action::CollapseAllFolders => {
            let open = matches!(action, Action::ExpandFolders | Action::ExpandAllFolders);
            let all = matches!(action, Action::ExpandAllFolders | Action::CollapseAllFolders);
            let picked = selected_layers(ed);
            ed.edit_timeline(|_, tl| {
                for (f, layer) in tl.layers.iter_mut().enumerate() {
                    if layer.kind == LayerKind::Folder && (all || picked.contains(&f)) {
                        layer.set_open(open);
                    }
                }
                sel
            });
            ed.layer = shown(ed.timeline(), ed.layer);
        }
        _ => return,
    }
    ed.prune_selection();
    keep_selection(ed);
}

/// The parenting view's column's left edge.
fn strip_left(column_right: f32) -> f32 {
    column_right - crate::timeline::DOT_X - 10.0 - PARENTING_W
}

/// Layer `l`'s head in the parenting view, in its row at `top`: right of the head of the layer it follows.
fn head(tl: &Timeline, frame: u32, l: usize, strip: f32, top: f32, row_h: f32) -> Rect {
    let depth = std::iter::successors(tl.rig_parent_at(l, frame), |&p| tl.rig_parent_at(p, frame)).take(3).count();
    Rect::from_min_size(pos2(strip + 8.0 + 12.0 * depth as f32, top + (row_h - 8.0) / 2.0), vec2(14.0, 8.0))
}

/// The parenting view: each normal layer's head in its colour, linked to the head of the
/// layer it follows at the playhead.
pub(crate) fn paint_parenting(p: &Painter, ed: &Editor, geo: &Geo, column_right: f32) {
    let (tl, frame, strip) = (ed.timeline(), ed.frame, strip_left(column_right));
    let head_of = |l: usize| head(tl, frame, l, strip, geo.top_of(l), geo.row_h);
    let heads: Vec<usize> = geo.rows.iter().copied().filter(|&l| tl.layers[l].kind == LayerKind::Normal).collect();
    for &l in &heads {
        if let Some(parent) = tl.rig_parent_at(l, frame).filter(|&p| tl.listed(p)) {
            let (from, to) = (head_of(parent).center(), head_of(l).left_center());
            let link = Stroke::new(1.0, layer_color(&tl.layers[parent], parent));
            p.line(vec![from, pos2(from.x, to.y), to], link);
        }
    }
    for &l in &heads {
        p.rect_filled(head_of(l), 2.0, layer_color(&tl.layers[l], l));
    }
}

/// While a head is dragged: a dotted line from it to the pointer.
pub(crate) fn paint_parent_drag(p: &Painter, ed: &Editor, geo: &Geo, column_right: f32, child: usize, pointer: Option<Pos2>) {
    let Some(to) = pointer else { return };
    let from = head(ed.timeline(), ed.frame, child, strip_left(column_right), geo.top_of(child), geo.row_h).center();
    p.add(egui::Shape::dashed_line(&[from, to], Stroke::new(1.0, theme::TEXT), 2.0, 2.0));
}

/// Layer `child` follows `parent`'s object from now on, or nothing: one undo step if that changes anything.
pub(crate) fn set_rig_parent(ed: &mut Editor, child: usize, parent: Option<usize>) {
    let (sel, mut refused) = (ed.selection.clone(), false);
    ed.edit_timeline(|_, tl| {
        refused = !tl.set_rig_parent(child, parent);
        sel
    });
    if refused {
        ed.log("Only a normal layer can follow another normal layer, and not one that follows it.");
    }
    keep_selection(ed);
}

/// The list a click in the parenting view opens (`open` on that frame): Remove parent, then
/// the layers the clicked one can follow, the one it follows checked.
pub(crate) fn parent_menu(ui: &Ui, ed: &mut Editor, open: bool) {
    let Some((child, at)) = ed.tl.parent_menu else { return };
    let mut picked = None;
    egui::Popup::new(ui.id().with("parent-menu"), ui.ctx().clone(), at, ui.layer_id())
        .kind(egui::PopupKind::Menu)
        .layout(Layout::top_down_justified(Align::Min))
        .style(theme::menu_style)
        .open_memory(open.then_some(egui::SetOpenCommand::Bool(true)))
        .show(|ui| {
            ui.set_min_width(160.0);
            let tl = ed.timeline();
            let current = tl.rig_parent_at(child, ed.frame);
            if theme::menu_item(ui, current.is_some(), "Remove parent", "").clicked() {
                picked = Some(None);
            }
            ui.separator();
            for (l, layer) in tl.layers.iter().enumerate().filter(|&(l, x)| l != child && x.kind == LayerKind::Normal) {
                if theme::menu_toggle(ui, true, current == Some(l), &layer.name).clicked() {
                    picked = Some(Some(l));
                }
            }
        });
    if let Some(parent) = picked {
        set_rig_parent(ed, child, parent);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{self, Modifiers, Pos2, pos2, vec2};
    use egui_kittest::kittest::Queryable;
    use minnamate_core::doc::{Cap, Element, Join, LayerKind, Paint, Path, Point, Seg, Stroke};
    use minnamate_core::draw;

    use super::PARENTING_W;
    use crate::app::testkit::*;
    use crate::app::{Action, Editor};
    use crate::timeline::DOT_X;

    /// The middle of a layer's name, `row` rows down the list (0.5 is the first row's middle).
    fn name(h: &Harness<'_, Editor>, row: f32) -> Pos2 {
        let (rows, row_h) = (h.state().layer_rows, h.state().tl.row_h);
        pos2(rows.left() + 80.0, rows.top() + row_h * row)
    }

    /// A layer's head in the parenting view, while it follows none, `row` rows down.
    fn head(h: &Harness<'_, Editor>, row: f32) -> Pos2 {
        let rows = h.state().layer_rows;
        pos2(rows.right() - DOT_X - 10.0 - PARENTING_W + 15.0, name(h, row).y)
    }

    /// Each layer's name, after a "-" for each folder, mask or guide it's in.
    fn tree(h: &Harness<'_, Editor>) -> String {
        let tl = h.state().timeline();
        let row = |l: usize| format!("{}{}", "-".repeat(tl.depth(l)), tl.layers[l].name);
        (0..tl.layers.len()).map(row).collect::<Vec<_>>().join(" ")
    }

    /// Right-clicks the name `row` rows down, then picks `item` from the layer menu.
    fn layer_menu(h: &mut Harness<'_, Editor>, row: f32, item: &str) {
        let at = name(h, row);
        h.hover_at(at);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
            h.step();
        }
        h.run();
        h.get_by_label(item).click();
        h.run();
    }

    /// Drags the layer name `from` rows down to `to` rows down.
    fn drag_rows(h: &mut Harness<'_, Editor>, from: f32, to: f32) {
        let (a, b) = (name(h, from), name(h, to));
        screen_drag(h, a, b, Modifiers::NONE);
    }

    fn undo(h: &mut Harness<'_, Editor>) {
        h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
        h.run();
    }

    #[test]
    fn the_layer_menu_makes_masks_and_motion_guides() {
        let mut h = harness(false);
        h.run();
        // Mask on Blink: Ball, under it, is masked, and both lock so the stage shows the masking.
        layer_menu(&mut h, 0.5, "Mask");
        let tl = h.state().timeline();
        assert_eq!((tl.layers[0].kind, tl.mask_of(1), tree(&h).as_str()), (LayerKind::Mask, Some(0), "Blink -Ball Background"));
        assert!(tl.layers[0].locked() && tl.layers[1].locked());
        undo(&mut h);
        assert_eq!((h.state().timeline().layers[0].kind, tree(&h).as_str()), (LayerKind::Normal, "Blink Ball Background"));

        // Show Masking, from a masked layer, locks the mask and what it masks again.
        layer_menu(&mut h, 0.5, "Mask");
        for l in [0, 1] {
            h.state_mut().timeline_mut().layers[l].set_locked(false);
        }
        layer_menu(&mut h, 1.5, "Show Masking");
        let layers = &h.state().timeline().layers;
        assert!(layers[0].locked() && layers[1].locked() && !layers[2].locked());

        // A motion guide above Background, guiding it; Guide again makes it a normal layer and lets go.
        layer_menu(&mut h, 2.5, "Add Classic Motion Guide");
        let tl = h.state().timeline();
        assert_eq!((tl.layers[2].kind, tl.guide_of(3), h.state().layer), (LayerKind::Guide, Some(2), 2));
        assert_eq!(tree(&h), "Blink -Ball Guide: Background -Background");
        layer_menu(&mut h, 2.5, "Guide");
        assert_eq!(tree(&h), "Blink -Ball Guide: Background Background");
        assert_eq!(h.state().timeline().layers[2].kind, LayerKind::Normal);
    }

    #[test]
    fn layers_drag_in_and_out_of_folders_which_open_and_close() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::NewFolder, &ctx);
        h.run();
        assert_eq!(tree(&h), "Folder_1 Blink Ball Background");
        // Dropped on the folder's row, Ball goes in, at its top.
        drag_rows(&mut h, 2.5, 0.5);
        assert_eq!((tree(&h).as_str(), h.state().layer), ("Folder_1 -Ball Blink Background", 1));
        // In the gap above Ball, Blink goes in too; to the bottom, Ball comes out again.
        drag_rows(&mut h, 2.5, 1.0);
        assert_eq!(tree(&h), "Folder_1 -Blink -Ball Background");
        drag_rows(&mut h, 2.5, 4.2);
        assert_eq!(tree(&h), "Folder_1 -Blink Background Ball");
        undo(&mut h);
        assert_eq!(tree(&h), "Folder_1 -Blink -Ball Background", "one undo step a drag");
        // The folder dragged below Background takes what's in it along.
        drag_rows(&mut h, 0.5, 4.2);
        assert_eq!(tree(&h), "Background Folder_1 -Blink -Ball");

        // Its triangle closes it: its layers leave the list, so the next row is the next layer.
        let triangle = pos2(h.state().layer_rows.left() + 16.0, name(&h, 1.5).y);
        screen_click(&mut h, triangle, Modifiers::NONE);
        assert!(!h.state().timeline().layers[1].open() && !h.state().timeline().listed(2));
        let at = name(&h, 0.5);
        screen_click(&mut h, at, Modifiers::NONE);
        assert_eq!(h.state().layer, 0);
        layer_menu(&mut h, 1.5, "Expand Folders");
        assert!(h.state().timeline().listed(3));
        layer_menu(&mut h, 0.5, "Collapse All Folders");
        assert!(!h.state().timeline().layers[1].open());
    }

    #[test]
    fn a_folders_switches_and_delete_take_its_layers_along() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::NewFolder, &ctx);
        h.run();
        drag_rows(&mut h, 2.5, 0.5);
        drag_rows(&mut h, 2.5, 1.0);
        assert_eq!(tree(&h), "Folder_1 -Blink -Ball Background");
        // The folder's eye hides what's in it; Alt+click on it hides everything else.
        let eye = pos2(h.state().layer_rows.right() - 51.0, name(&h, 0.5).y);
        screen_click(&mut h, eye, Modifiers::NONE);
        let shown = |h: &Harness<'_, Editor>| h.state().timeline().layers.iter().map(|l| l.visible()).collect::<Vec<_>>();
        assert_eq!(shown(&h), [false, false, false, true]);
        screen_click(&mut h, eye, Modifiers::NONE);
        screen_click(&mut h, eye, Modifiers::ALT);
        assert_eq!(shown(&h), [true, true, true, false]);
        // Deleting the folder deletes what's in it, in one undo step.
        let at = name(&h, 0.5);
        screen_click(&mut h, at, Modifiers::NONE);
        h.state_mut().run(Action::DeleteLayer, &ctx);
        assert_eq!(tree(&h), "Background");
        undo(&mut h);
        assert_eq!(tree(&h), "Folder_1 -Blink -Ball Background");
    }

    #[test]
    fn cut_copy_paste_and_duplicate_take_a_folders_layers_along() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::NewFolder, &ctx);
        h.run();
        drag_rows(&mut h, 2.5, 0.5);
        drag_rows(&mut h, 2.5, 1.0);
        assert_eq!(tree(&h), "Folder_1 -Blink -Ball Background");
        let run = |h: &mut Harness<'_, Editor>, row: f32, action: Action| {
            // Far enough apart in time that two clicks aren't a double-click, which renames.
            (0..20).for_each(|_| h.step());
            let at = name(h, row);
            screen_click(h, at, Modifiers::NONE);
            h.state_mut().run(action, &ctx);
            h.run();
        };
        // Cut, a folder takes what it holds along, and pastes back with it.
        run(&mut h, 0.5, Action::CutLayers);
        assert_eq!(tree(&h), "Background");
        run(&mut h, 0.5, Action::PasteLayers);
        assert_eq!(tree(&h), "Folder_1 -Blink -Ball Background");
        run(&mut h, 0.5, Action::DuplicateLayers);
        assert_eq!(tree(&h), "Folder_1 copy -Blink copy -Ball copy Folder_1 -Blink -Ball Background");
        // Pasted onto a selected folder, layers go into it, at its top.
        run(&mut h, 6.5, Action::CopyLayers);
        run(&mut h, 3.5, Action::PasteLayers);
        assert_eq!(tree(&h), "Folder_1 copy -Blink copy -Ball copy Folder_1 -Background -Blink -Ball Background");
    }

    #[test]
    fn the_parenting_view_links_a_layer_to_another() {
        let mut h = harness(false);
        h.run();
        h.get_by_label("Show Parenting View").click();
        h.run();
        assert!(h.state().tl.parenting);
        // Ball's head dropped on Blink's row: Ball follows Blink, on every keyframe.
        let (from, to) = (head(&h, 1.5), name(&h, 0.5));
        screen_drag(&mut h, from, to, Modifiers::NONE);
        let tl = h.state().timeline();
        assert_eq!((tl.rig_parent(1, 0), tl.rig_parent(1, 1)), (Some(0), Some(0)));
        // Not Blink onto Ball now: that would loop.
        let (from, to) = (head(&h, 0.5), name(&h, 1.5));
        screen_drag(&mut h, from, to, Modifiers::NONE);
        assert_eq!(h.state().timeline().rig_parent(0, 0), None);
        undo(&mut h);
        assert_eq!(h.state().timeline().rig_parent(1, 0), None);
        h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Y);
        h.run();
        assert_eq!(h.state().timeline().rig_parent(1, 0), Some(0));

        // A click beside the head lists the layers to follow, and Remove parent.
        let beside = head(&h, 1.5) + vec2(32.0, 0.0);
        screen_click(&mut h, beside, Modifiers::NONE);
        h.run();
        h.get_by_label("Remove parent").click();
        h.run();
        assert_eq!(h.state().timeline().rig_parent(1, 0), None);
        screen_click(&mut h, beside, Modifiers::NONE);
        h.run();
        h.get_by_label("Background").click();
        h.run();
        assert_eq!(h.state().timeline().rig_parent(1, 0), Some(2));

        // Background duplicated: Ball still follows Background itself, not the copy above it.
        let at = name(&h, 2.5);
        screen_click(&mut h, at, Modifiers::NONE);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::DuplicateLayers, &ctx);
        h.run();
        let tl = h.state().timeline();
        assert_eq!((tl.layers[2].name.as_str(), tl.rig_parent(1, 0)), ("Background copy", Some(3)));
    }

    #[test]
    fn the_new_commands_on_odd_timelines() {
        use Action::*;
        let all = [ToggleMask, ShowMasking, AddMotionGuide, ExpandFolders, CollapseFolders, ExpandAllFolders, CollapseAllFolders, NewFolder, NewLayer, PasteLayers, DuplicateLayers, DeleteLayer];
        // The playhead past the end; no layers at all; only a folder; a mask over nothing.
        let setups: [fn(&mut Editor); 4] = [
            |ed| ed.frame = 40,
            |ed| ed.doc.scenes[0].layers.clear(),
            |ed| {
                ed.doc.scenes[0].layers.clear();
                ed.doc.scenes[0].insert_folder(0, "Folder_1");
            },
            |ed| {
                ed.doc.scenes[0].layers.truncate(1);
                ed.doc.scenes[0].set_mask(0, true);
            },
        ];
        for setup in setups {
            let mut h = harness(false);
            h.run();
            let ctx = h.ctx.clone();
            setup(h.state_mut());
            h.state_mut().tl.parenting = true;
            h.run();
            for action in all {
                h.state_mut().run(action, &ctx);
                h.run();
            }
        }
    }

    /// A curve from `a` to `b` through `c`'s pull, drawn as a guide's stroke.
    fn curve(a: Point, c: Point, b: Point) -> Element {
        let stroke = Stroke { width: 2.0, paint: Paint::Solid([0.2, 0.8, 0.2, 1.0]), cap: Cap::Round, join: Join::Round };
        Element::Shape(draw::shape(vec![Path { start: a, segs: vec![Seg::Quad(c, b)] }], Some(stroke), None))
    }

    /// Writes target/screenshots/masks.png: an oval mask over the tween fixture's ball and
    /// background, all locked, so the stage shows the masking.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn masks_screenshot() {
        let mut h = harness(true);
        let ed = h.state_mut();
        let tl = ed.timeline_mut();
        let mask = tl.insert_layer(1, "Spotlight");
        let oval = draw::oval([(150.0, 110.0), (420.0, 290.0)], None, Some(Paint::Solid([0.3, 0.6, 1.0, 1.0])));
        tl.layers[mask].frames[0].elements.push(Element::Shape(oval));
        tl.set_mask(mask, true);
        tl.set_parent(3, Some(mask));
        tl.layers[3].set_locked(true);
        ed.layer = mask;
        ed.set_frame(5);
        h.run();
        screenshot(&mut h, "masks");
    }

    /// Writes target/screenshots/folders.png: a folder holding a closed folder (Blink in it) and a
    /// motion guide steering Ball along its curve, Ball turning with it; a folder holding a mask.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn folders_screenshot() {
        let mut h = harness(true);
        let ed = h.state_mut();
        let tl = ed.timeline_mut();
        // Blink, Guide: Ball, Ball, Background.
        let guide = tl.add_motion_guide(1, "Guide: Ball");
        tl.layers[guide].frames[0].elements.push(curve((100.0, 200.0), (250.0, 40.0), (400.0, 200.0)));
        tl.layers[guide + 1].frames[0].extra.set("motionTweenOrientToPath", Some("true"));
        // Characters, Old (closed, holding Blink), Guide: Ball, Ball, then Props: Spotlight masking Background.
        let old = tl.insert_folder(0, "Old");
        tl.set_parent(old + 1, Some(old));
        tl.layers[old].set_open(false);
        let characters = tl.insert_folder(0, "Characters");
        for l in [1, 3] {
            tl.set_parent(l, Some(characters));
        }
        let props = tl.insert_folder(5, "Props");
        let spot = tl.insert_layer(6, "Spotlight");
        let oval = draw::oval([(60.0, 60.0), (490.0, 340.0)], None, Some(Paint::Solid([1.0; 4])));
        tl.layers[spot].frames[0].elements.push(Element::Shape(oval));
        tl.set_parent(spot, Some(props));
        tl.set_parent(spot + 1, Some(props));
        tl.set_mask(spot, true);
        ed.layer = 4;
        ed.set_frame(3);
        h.run();
        screenshot(&mut h, "folders");
    }

    /// Writes target/screenshots/parenting_view.png: the parenting view, Ball following Blink
    /// and Background following Ball, the playhead halfway through Ball's tween.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn parenting_view_screenshot() {
        let mut h = harness(true);
        let ed = h.state_mut();
        ed.tl.parenting = true;
        let tl = ed.timeline_mut();
        assert!(tl.set_rig_parent(1, Some(0)) && tl.set_rig_parent(2, Some(1)));
        ed.layer = 2;
        ed.set_frame(5);
        h.run();
        screenshot(&mut h, "parenting_view");
    }
}
