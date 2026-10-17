//! How layers hang together (docs/animate-ui-interaction.md §2.4, §2.5). A folder
//! holds layers, a mask clips the layers under it and a guide steers their classic
//! tweens: each of those layers names its folder, mask or guide in XFL's
//! `parentLayerIndex`. Layer parenting, where a layer's objects follow another
//! layer's object, is a separate link, kept per keyframe.

use std::iter::successors;

use crate::doc::*;

/// A layer's folder, mask or guide, by index: a DOMLayer attribute.
const PARENT: &str = "parentLayerIndex";
/// Layer parenting, as public notes on Animate-saved files read it: the parent's DOMLayer
/// carries an id, and each keyframe (DOMFrame) of the child names it.
// ponytail: reverse-engineered from community notes, not a published schema; it round-trips
// here. Check it against a file Animate saves with parenting before relying on it there.
const RIG_ID: &str = "layerRiggingIndex";
const RIG_PARENT: &str = "parentLayerIndex";

impl Layer {
    /// A folder shown open in the timeline, its layers listed under it.
    // ponytail: saved as `open="false"` on a closed folder's DOMLayer; the attribute's name is
    // unconfirmed against an Animate-saved file, so Animate may open such folders. It round-trips here.
    pub fn open(&self) -> bool {
        self.extra.get("open") != Some("false")
    }

    pub fn set_open(&mut self, on: bool) {
        self.extra.set("open", (!on).then_some("false"));
    }
}

impl Timeline {
    /// The folder, mask or guide layer `l` sits in: its parentLayerIndex, if that names one above it.
    pub fn parent(&self, l: usize) -> Option<usize> {
        let p: usize = self.layers.get(l)?.extra.get(PARENT)?.parse().ok()?;
        (p < l && matches!(self.layers[p].kind, LayerKind::Folder | LayerKind::Mask | LayerKind::Guide)).then_some(p)
    }

    pub fn set_parent(&mut self, l: usize, parent: Option<usize>) {
        self.layers[l].extra.set(PARENT, parent.map(|p| p.to_string()).as_deref());
    }

    /// The folders, masks and guides `l` sits in, innermost first.
    fn ancestors(&self, l: usize) -> impl Iterator<Item = usize> + '_ {
        successors(self.parent(l), |&p| self.parent(p))
    }

    /// How many of those there are: how far the timeline indents `l`.
    pub fn depth(&self, l: usize) -> usize {
        self.ancestors(l).count()
    }

    /// The mask clipping `l`, if `l` is one of its masked layers.
    pub fn mask_of(&self, l: usize) -> Option<usize> {
        self.parent(l).filter(|&p| self.layers[p].kind == LayerKind::Mask)
    }

    /// The motion guide steering `l`'s classic tweens.
    pub fn guide_of(&self, l: usize) -> Option<usize> {
        self.parent(l).filter(|&p| self.layers[p].kind == LayerKind::Guide)
    }

    /// Every layer under `l`: a folder's contents, a mask's or a guide's layers.
    pub fn descendants(&self, l: usize) -> Vec<usize> {
        (l + 1..self.layers.len()).filter(|&d| self.ancestors(d).any(|p| p == l)).collect()
    }

    /// Whether the timeline lists `l`: no folder it's in is closed.
    pub fn listed(&self, l: usize) -> bool {
        self.ancestors(l).all(|p| self.layers[p].kind != LayerKind::Folder || self.layers[p].open())
    }

    /// `rows` and the contents of the folders among them, in order: what Delete Layers removes.
    /// A mask's or guide's layers stay, becoming normal layers.
    pub fn with_contents(&self, rows: &[usize]) -> Vec<usize> {
        let folder = |l: usize| self.layers.get(l).is_some_and(|x| x.kind == LayerKind::Folder);
        let mut all: Vec<usize> = rows.iter().flat_map(|&l| [l].into_iter().chain(if folder(l) { self.descendants(l) } else { Vec::new() })).collect();
        all.sort_unstable();
        all.dedup();
        all
    }

    /// Dragging layers: moves `rows` and everything under them into the gap above layer `gap`
    /// (`layers.len()` being the bottom), into `parent` (a folder, mask or guide, by its index
    /// before the move) or out of any. A mask or guide takes normal layers only: with others
    /// among them, they all go after its layers instead, into its folder. Returns where each
    /// layer went; None, changing nothing, if `parent` is one of those moving.
    pub fn move_into(&mut self, rows: &[usize], gap: usize, parent: Option<usize>) -> Option<Vec<usize>> {
        let n = self.layers.len();
        let mut moving: Vec<usize> = rows.iter().filter(|&&l| l < n).flat_map(|&l| [l].into_iter().chain(self.descendants(l))).collect();
        moving.sort_unstable();
        moving.dedup();
        let parent = parent.filter(|&p| self.layers.get(p).is_some_and(|x| x.kind != LayerKind::Normal));
        if moving.is_empty() || parent.is_some_and(|p| moving.contains(&p)) {
            return None;
        }
        // The moved layers whose folder, mask or guide stays behind: they join `parent`.
        let roots: Vec<usize> = moving.iter().copied().filter(|&l| self.parent(l).is_none_or(|p| !moving.contains(&p))).collect();
        let (gap, parent) = match parent {
            Some(p) if self.layers[p].kind != LayerKind::Folder && roots.iter().any(|&r| self.layers[r].kind != LayerKind::Normal) => {
                (p + 1 + self.descendants(p).len(), self.folder_of(p))
            }
            _ => (gap.min(n), parent),
        };
        let to = gap - moving.iter().filter(|&&l| l < gap).count();
        let mut order: Vec<usize> = (0..n).filter(|i| !moving.contains(i)).collect();
        order.splice(to..to, moving.iter().copied());
        let mut went = vec![0; n];
        for (new, &old) in order.iter().enumerate() {
            went[old] = new;
        }
        self.move_layers(&moving, gap);
        for l in roots {
            self.set_parent(went[l], parent.map(|p| went[p]));
        }
        Some(went)
    }

    /// New Layer and Insert Folder: the layer just inserted at `l` joins the folder (or, a
    /// normal layer, the mask or guide) of the layer under it, the one that was selected.
    pub fn adopt(&mut self, l: usize) {
        let normal = self.layers[l].kind == LayerKind::Normal;
        let p = successors(self.parent(l + 1), |&p| self.parent(p)).find(|&p| normal || self.layers[p].kind == LayerKind::Folder);
        self.set_parent(l, p);
    }

    /// Copy Layers, Cut Layers and Duplicate Layers: `rows` and what their folders hold, top
    /// first, each naming its folder, mask or guide by its place among them (or none).
    pub fn copy_layers(&self, rows: &[usize]) -> Vec<Layer> {
        let all = self.with_contents(rows);
        let place = |p: usize| all.iter().position(|&x| x == p);
        let copy = |&l: &usize| {
            let mut layer = self.layers[l].clone();
            layer.extra.set(PARENT, self.parent(l).and_then(place).map(|p| p.to_string()).as_deref());
            layer
        };
        all.iter().map(copy).collect()
    }

    /// Paste Layers and Duplicate Layers: `new`, as `copy_layers` gives them, go into the gap
    /// above layer `gap` and into `parent`, as dragging them there would, their folders, masks
    /// and guides holding what they held. A parenting id another layer has already is made
    /// anew, and the copies that followed it follow the copy. Returns where the first went.
    pub fn paste_layers(&mut self, gap: usize, parent: Option<usize>, new: Vec<Layer>) -> usize {
        let links: Vec<Option<usize>> = new.iter().map(|l| l.extra.get(PARENT).and_then(|p| p.parse().ok())).collect();
        // At the bottom first, so `gap` and `parent` still name the layers they did.
        let (at, count) = (self.layers.len(), new.len());
        self.insert_layers(at, new);
        let pasted = at..at + count;
        for (i, link) in links.into_iter().enumerate() {
            self.set_parent(at + i, link.filter(|&j| j < i).map(|j| at + j));
        }
        for l in pasted.clone() {
            let Some(old) = self.layers[l].extra.get(RIG_ID).map(str::to_string) else { continue };
            if !self.layers.iter().enumerate().any(|(x, other)| !pasted.contains(&x) && other.extra.get(RIG_ID) == Some(&old)) {
                continue;
            }
            self.layers[l].extra.set(RIG_ID, None);
            let follows = |k: &Frame| k.extra.get(RIG_PARENT) == Some(old.as_str());
            if self.layers[pasted.clone()].iter().flat_map(|x| &x.frames).any(follows) {
                let id = self.rig_id(l);
                for k in self.layers[pasted.clone()].iter_mut().flat_map(|x| &mut x.frames).filter(|k| follows(k)) {
                    k.extra.set(RIG_PARENT, Some(&id));
                }
            }
        }
        let roots: Vec<usize> = pasted.filter(|&l| self.parent(l).is_none()).collect();
        self.move_into(&roots, gap, parent).map_or(at, |went| went[at])
    }

    /// The nearest folder holding `l`, where a mask or guide made of it sits.
    fn folder_of(&self, l: usize) -> Option<usize> {
        self.ancestors(l).find(|&p| self.layers[p].kind == LayerKind::Folder)
    }

    /// The layers in mask or guide `l` move out to where `l` sits; run while `l` still is one.
    fn release(&mut self, l: usize) {
        let outer = self.parent(l);
        for c in (l + 1..self.layers.len()).filter(|&c| self.parent(c) == Some(l)).collect::<Vec<_>>() {
            self.set_parent(c, outer);
        }
    }

    /// The layer menu's Mask: `l` becomes a mask over the normal layer under it, both locked so
    /// the stage shows the masking. Off, it's a normal layer again, and so are the layers it masked.
    pub fn set_mask(&mut self, l: usize, on: bool) {
        if self.layers.get(l).is_none_or(|x| (x.kind == LayerKind::Mask) == on) {
            return;
        }
        let outer = self.folder_of(l);
        self.release(l);
        self.layers[l].kind = if on { LayerKind::Mask } else { LayerKind::Normal };
        if on {
            self.set_parent(l, outer);
            self.layers[l].set_locked(true);
            if self.layers.get(l + 1).is_some_and(|b| b.kind == LayerKind::Normal) && self.parent(l + 1) == outer {
                self.set_parent(l + 1, Some(l));
                self.layers[l + 1].set_locked(true);
            }
        }
    }

    /// The layer menu's Guide: `l` becomes a guide layer (outside any mask or guide), or a
    /// normal layer again, as do the layers it guided.
    pub fn set_guide(&mut self, l: usize, on: bool) {
        if self.layers.get(l).is_none_or(|x| (x.kind == LayerKind::Guide) == on) {
            return;
        }
        let outer = self.folder_of(l);
        self.release(l);
        self.layers[l].kind = if on { LayerKind::Guide } else { LayerKind::Normal };
        if on {
            self.set_parent(l, outer);
        }
    }

    /// Add Classic Motion Guide: a guide layer `name` above `l`, steering `l`'s classic tweens.
    /// Returns where it went.
    pub fn add_motion_guide(&mut self, l: usize, name: &str) -> usize {
        let outer = self.folder_of(l);
        let at = self.insert_layer(l, name);
        self.layers[at].kind = LayerKind::Guide;
        self.set_parent(at, outer);
        if at + 1 < self.layers.len() {
            self.set_parent(at + 1, Some(at));
        }
        at
    }

    /// The layer whose object keyframe `k` of layer `l` follows (layer parenting).
    pub fn rig_parent(&self, l: usize, k: usize) -> Option<usize> {
        let id = self.layers.get(l)?.frames.get(k)?.extra.get(RIG_PARENT)?;
        let p = self.layers.iter().position(|p| p.extra.get(RIG_ID) == Some(id))?;
        (p != l && self.layers[p].kind == LayerKind::Normal).then_some(p)
    }

    /// The layer `l` follows at `frame` (past its end, as on its last keyframe): the parenting view's link.
    pub fn rig_parent_at(&self, l: usize, frame: u32) -> Option<usize> {
        let layer = self.layers.get(l)?;
        self.rig_parent(l, layer.keyframe_at(frame).or(layer.frames.len().checked_sub(1))?)
    }

    /// The parenting view: every keyframe of layer `l` follows `parent`'s object from here on, or
    /// nothing. Returns false, changing nothing, unless both are normal layers and `parent`
    /// doesn't follow `l` already (that would loop).
    // ponytail: Animate can give each keyframe span a parent of its own; the view sets the whole layer's.
    pub fn set_rig_parent(&mut self, l: usize, parent: Option<usize>) -> bool {
        let normal = |i: usize| self.layers.get(i).is_some_and(|x| x.kind == LayerKind::Normal);
        if !normal(l) || parent.is_some_and(|p| p == l || !normal(p) || self.follows(p, l)) {
            return false;
        }
        let id = parent.map(|p| self.rig_id(p));
        for k in &mut self.layers[l].frames {
            k.extra.set(RIG_PARENT, id.as_deref());
        }
        true
    }

    /// Whether `a` follows `b`, directly or through other layers, on any keyframe.
    fn follows(&self, a: usize, b: usize) -> bool {
        let (mut seen, mut todo) = (vec![false; self.layers.len()], vec![a]);
        while let Some(x) = todo.pop() {
            if x == b {
                return true;
            }
            if !std::mem::replace(&mut seen[x], true) {
                todo.extend((0..self.layers[x].frames.len()).filter_map(|k| self.rig_parent(x, k)));
            }
        }
        false
    }

    /// Layer `p`'s parenting id: its own, or a new one if it has none or shares one with a layer above it.
    fn rig_id(&mut self, p: usize) -> String {
        if let Some(id) = self.layers[p].extra.get(RIG_ID)
            && self.layers.iter().position(|x| x.extra.get(RIG_ID) == Some(id)) == Some(p)
        {
            return id.to_string();
        }
        // Past every id named anywhere, so keyframes still naming a deleted parent don't take to this one.
        let named = self.layers.iter().flat_map(|l| l.extra.get(RIG_ID).into_iter().chain(l.frames.iter().filter_map(|k| k.extra.get(RIG_PARENT))));
        let id = named.filter_map(|v| v.parse::<u32>().ok()).max().map_or(0, |m| m + 1).to_string();
        self.layers[p].extra.set(RIG_ID, Some(&id));
        id
    }
}

/// What a blank keyframe split from a keyframe keeps of it: whom it follows.
pub(crate) fn rig_link(extra: &Extra) -> Extra {
    let mut kept = Extra::default();
    kept.set(RIG_PARENT, extra.get(RIG_PARENT));
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Layers named by `names`, top first, one frame each; "F" folders, "M" masks, "G" guides.
    fn timeline(names: &[&str]) -> Timeline {
        let mut tl = Timeline::new("Scene 1");
        let layer = tl.layers.remove(0);
        for name in names {
            let kind = match name.chars().next() {
                Some('F') => LayerKind::Folder,
                Some('M') => LayerKind::Mask,
                Some('G') => LayerKind::Guide,
                _ => LayerKind::Normal,
            };
            tl.layers.push(Layer { name: name.to_string(), kind, ..layer.clone() });
        }
        tl
    }

    /// Each layer as its name, ">" and its parent's name when it has one, and "-" per indent.
    fn tree(tl: &Timeline) -> String {
        let row = |(l, x): (usize, &Layer)| {
            let parent = tl.parent(l).map_or(String::new(), |p| format!(">{}", tl.layers[p].name));
            format!("{}{}{parent}", "-".repeat(tl.depth(l)), x.name)
        };
        tl.layers.iter().enumerate().map(row).collect::<Vec<_>>().join(" ")
    }

    fn nest(tl: &mut Timeline, links: &[(usize, usize)]) {
        for &(l, p) in links {
            tl.set_parent(l, Some(p));
        }
    }

    #[test]
    fn links_name_folders_masks_and_guides_above() {
        let mut tl = timeline(&["F1", "M", "a", "b", "x", "G", "c"]);
        nest(&mut tl, &[(1, 0), (2, 1), (3, 1), (6, 5)]);
        assert_eq!(tree(&tl), "F1 -M>F1 --a>M --b>M x G -c>G");
        assert_eq!((tl.mask_of(2), tl.guide_of(6), tl.mask_of(1)), (Some(1), Some(5), None));
        assert_eq!(tl.descendants(0), [1, 2, 3]);
        // A link to a normal layer, or one below, is no link.
        nest(&mut tl, &[(4, 3), (5, 6)]);
        assert_eq!((tl.parent(4), tl.parent(5)), (None, None));
        tl.layers[0].set_open(false);
        assert_eq!((0..7).map(|l| tl.listed(l)).collect::<Vec<_>>(), [true, false, false, false, true, true, true]);
        // Deleting a folder takes its contents; a mask leaves its layers.
        assert_eq!(tl.with_contents(&[0, 5]), [0, 1, 2, 3, 5]);
        assert_eq!(tl.with_contents(&[1]), [1]);
    }

    #[test]
    fn dragging_layers_in_and_out_of_folders() {
        let mut tl = timeline(&["F1", "a", "b", "F2", "c"]);
        nest(&mut tl, &[(1, 0), (2, 0), (4, 3)]);
        // Onto F2: c's folder, as its first layer.
        let went = tl.move_into(&[1], 4, Some(3)).unwrap();
        assert_eq!(tree(&tl), "F1 -b>F1 F2 -a>F2 -c>F2");
        assert_eq!(went, [0, 3, 1, 2, 4]);
        // Out to the bottom: no folder.
        tl.move_into(&[1], 5, None).unwrap();
        assert_eq!(tree(&tl), "F1 F2 -a>F2 -c>F2 b");
        // A folder takes its contents along, into another folder.
        tl.move_into(&[1], 0, None).unwrap();
        assert_eq!(tree(&tl), "F2 -a>F2 -c>F2 F1 b");
        tl.move_into(&[3], 1, Some(0)).unwrap();
        assert_eq!(tree(&tl), "F2 -F1>F2 -a>F2 -c>F2 b");
        tl.move_into(&[4], 2, Some(1)).unwrap();
        assert_eq!(tree(&tl), "F2 -F1>F2 --b>F1 -a>F2 -c>F2");
        // Not into itself.
        let before = tl.clone();
        assert_eq!(tl.move_into(&[0], 3, Some(1)), None);
        assert_eq!(tl, before);
        // A mask takes normal layers; a folder dropped there goes after its layers, in its folder.
        let mut tl = timeline(&["F", "M", "m", "F2", "x"]);
        nest(&mut tl, &[(1, 0), (2, 1)]);
        tl.move_into(&[4], 3, Some(1)).unwrap();
        assert_eq!(tree(&tl), "F -M>F --m>M --x>M F2");
        tl.move_into(&[4], 2, Some(1)).unwrap();
        assert_eq!(tree(&tl), "F -M>F --m>M --x>M -F2>F");
    }

    #[test]
    fn new_layers_join_the_selected_layers_folder() {
        let mut tl = timeline(&["F", "M", "m"]);
        nest(&mut tl, &[(1, 0), (2, 1)]);
        let at = tl.insert_layer(2, "n");
        tl.adopt(at);
        let folder = tl.insert_folder(1, "F2");
        tl.adopt(folder);
        assert_eq!(tree(&tl), "F -F2>F -M>F --n>M --m>M");
    }

    #[test]
    fn mask_guide_and_motion_guide_commands() {
        let mut tl = timeline(&["F", "a", "b", "c"]);
        nest(&mut tl, &[(1, 0), (2, 0)]);
        tl.set_mask(1, true);
        assert_eq!(tree(&tl), "F -a>F --b>a c");
        assert!(tl.layers[1].locked() && tl.layers[2].locked() && !tl.layers[3].locked());
        tl.set_mask(1, false);
        assert_eq!((tree(&tl), tl.layers[1].kind), ("F -a>F -b>F c".into(), LayerKind::Normal));

        let at = tl.add_motion_guide(2, "Guide: b");
        assert_eq!((at, tree(&tl)), (2, "F -a>F -Guide: b>F --b>Guide: b c".into()));
        tl.set_guide(2, false);
        assert_eq!(tree(&tl), "F -a>F -Guide: b>F -b>F c");
        // A masked layer made a guide moves out of the mask, into the mask's folder.
        tl.set_mask(3, true);
        nest(&mut tl, &[(4, 3)]);
        tl.set_guide(4, true);
        assert_eq!((tree(&tl), tl.layers[4].kind), ("F -a>F -Guide: b>F -b>F -c>F".into(), LayerKind::Guide));
    }

    #[test]
    fn layer_parenting_links_and_loops() {
        let mut tl = timeline(&["arm", "hand", "F"]);
        tl.layers[1].insert_keyframe(3);
        assert!(tl.set_rig_parent(1, Some(0)));
        assert_eq!((tl.rig_parent(1, 0), tl.rig_parent(1, 1), tl.rig_parent_at(1, 9)), (Some(0), Some(0), Some(0)));
        assert_eq!(tl.layers[0].extra.get("layerRiggingIndex"), Some("0"));
        assert_eq!(tl.layers[1].frames[1].extra.get("parentLayerIndex"), Some("0"));
        // No loops, no folders, and not itself.
        assert!(!tl.set_rig_parent(0, Some(1)) && !tl.set_rig_parent(1, Some(2)) && !tl.set_rig_parent(1, Some(1)));
        // A copy of the parent gets an id of its own; the child keeps following the first.
        let copy = tl.layers[0].clone();
        tl.layers.push(copy);
        assert!(tl.set_rig_parent(0, Some(3)));
        assert_eq!((tl.rig_parent(0, 0), tl.rig_parent(1, 0)), (Some(3), Some(0)));
        assert_eq!(tl.layers[3].extra.get("layerRiggingIndex"), Some("1"));
        // A blank keyframe split off keeps following; removing the parent unlinks every keyframe.
        assert_eq!(tl.layers[1].insert_blank_keyframe(5), 5);
        assert_eq!(tl.rig_parent(1, 2), Some(0));
        assert!(tl.set_rig_parent(1, None));
        assert!((0..3).all(|k| tl.rig_parent(1, k).is_none()));
    }

    #[test]
    fn copied_layers_paste_with_what_they_hold_and_follow() {
        // F holds M, masking m; hand follows arm.
        let mut tl = timeline(&["F", "M", "m", "arm", "hand"]);
        nest(&mut tl, &[(1, 0), (2, 1)]);
        assert!(tl.set_rig_parent(4, Some(3)));
        // A folder copies with what it holds, and pastes so: the copied mask masks the copied layer.
        let clip = tl.copy_layers(&[0]);
        assert_eq!(tl.paste_layers(3, None, clip), 3);
        assert_eq!(tree(&tl), "F -M>F --m>M F -M>F --m>M arm hand");
        // arm duplicated: hand still follows arm itself, not the copy above it.
        let clip = tl.copy_layers(&[6]);
        assert_eq!(tl.paste_layers(6, None, clip), 6);
        assert_eq!((tl.rig_parent(8, 0), tl.layers[6].extra.get(RIG_ID)), (Some(7), None));
        // arm and hand copied together: the copy of hand follows the copy of arm.
        let clip = tl.copy_layers(&[7, 8]);
        assert_eq!(tl.paste_layers(0, None, clip), 0);
        assert_eq!((tl.rig_parent(1, 0), tl.rig_parent(10, 0)), (Some(0), Some(9)));

        // Pasted into a mask, a normal layer is masked; a folder goes after its masked layers, into its folder.
        let mut tl = timeline(&["F", "M", "m", "x", "F2"]);
        nest(&mut tl, &[(1, 0), (2, 1)]);
        let clip = tl.copy_layers(&[3]);
        assert_eq!(tl.paste_layers(2, Some(1), clip), 2);
        let clip = tl.copy_layers(&[5]);
        assert_eq!(tl.paste_layers(3, Some(1), clip), 4);
        assert_eq!(tree(&tl), "F -M>F --x>M --m>M -F2>F x F2");
    }
}
