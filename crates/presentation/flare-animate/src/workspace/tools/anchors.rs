//! The subselection tool: dragging the points of a selected shape.

use flare_document::Content;
use glamx::DVec2;

use crate::workspace::*;

/// A point being dragged: which shape, which segment's end, and where it is now.
#[derive(Clone, Copy, Debug)]
pub(in crate::workspace) struct AnchorDrag {
    pub(in crate::workspace) at: SpanRef,
    pub(in crate::workspace) segment: usize,
    /// Where the point was when it was grabbed.
    pub(in crate::workspace) from: Pos2,
    pub(in crate::workspace) to: Pos2,
}

impl Workspace {
    /// The points of the selected shape that can be dragged (none for instances).
    pub(in crate::workspace) fn anchor_points(&self) -> Vec<(usize, Pos2)> {
        let (Some(at), Some(scene)) = (self.selected, self.scene_id()) else {
            return Vec::new();
        };
        match ops::find_span(self.document(), scene, at).map(|s| s.content()) {
            Some(Content::Shape(shape)) => ops::anchors(shape),
            _ => Vec::new(),
        }
    }

    /// The point of the selected shape nearest `point`, if one is close enough to grab.
    fn anchor_near(&self, point: Pos2) -> Option<(SpanRef, usize)> {
        let at = self.selected?;
        let grab = self.handle_radius.max(f32::EPSILON);
        self.anchor_points()
            .into_iter()
            .map(|(segment, p)| (segment, (p - point).length()))
            .filter(|(_, distance)| *distance <= grab)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(segment, _)| (at, segment))
    }

    /// Chooses the point of the selected shape under `point`, for Delete. False when there is
    /// none there.
    pub(in crate::workspace) fn select_anchor_at(&mut self, point: Pos2) -> bool {
        let found = self.anchor_near(point);
        if let Some((at, segment)) = found {
            self.selected_anchor = Some((at, segment, self.active().revision()));
        }
        found.is_some()
    }

    /// Delete with a point chosen: takes it out of the outline, as one undo step. False when
    /// no point is chosen (the key then does what it always does).
    pub(in crate::workspace) fn delete_selected_anchor(&mut self) -> bool {
        let Some((at, segment, revision)) = self.selected_anchor.take() else {
            return false;
        };
        // A point chosen before some other edit may not be there any more.
        if revision != self.active().revision() || self.selected != Some(at) {
            return false;
        }
        let Some(scene) = self.scene_id() else {
            return false;
        };
        match ops::delete_anchor(self.document(), scene, at, segment) {
            Some(commands) => self.run_batch(commands, "delete the point"),
            None => self.report("A shape keeps at least two points, three when it is closed"),
        }
        true
    }

    /// Starts dragging the point nearest `point`, if one is close enough to grab.
    pub(in crate::workspace) fn begin_anchor_drag(&mut self, point: Pos2) {
        let Some(at) = self.selected else { return };
        let grab = self.handle_radius.max(f32::EPSILON);
        let nearest = self
            .anchor_points()
            .into_iter()
            .map(|(segment, p)| (segment, p, (p - point).length()))
            .filter(|(_, _, distance)| *distance <= grab)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((segment, from, _)) = nearest {
            self.anchor_drag = Some(AnchorDrag {
                at,
                segment,
                from,
                to: from,
            });
        }
    }

    pub(in crate::workspace) fn update_anchor_drag(&mut self, point: Pos2) {
        if let Some(drag) = &mut self.anchor_drag {
            drag.to = point;
        }
    }

    /// Applies the drag as one undo step, unless it was cancelled.
    pub(in crate::workspace) fn end_anchor_drag(&mut self, cancelled: bool) {
        let (Some(drag), Some(scene)) = (self.anchor_drag.take(), self.scene_id()) else {
            return;
        };
        if cancelled {
            return;
        }
        if self.tool == Tool::ConvertAnchor {
            // A drag from a point makes it smooth, along the way it was dragged.
            let to = DVec2::new(f64::from(drag.to.x), f64::from(drag.to.y));
            if let Some(commands) =
                ops::convert_anchor(self.document(), scene, drag.at, drag.segment, Some(to))
            {
                self.run_batch(commands, "make the point smooth");
            }
            return;
        }
        let to = DVec2::new(f64::from(drag.to.x), f64::from(drag.to.y));
        if let Some(commands) = ops::move_anchor(self.document(), scene, drag.at, drag.segment, to)
        {
            self.run_batch(commands, "move the point");
        }
    }
}

impl Workspace {
    /// The shapes a click with an anchor tool may mean: the selected one, then the one under the
    /// pointer.
    fn anchor_targets(&self, point: Pos2) -> Vec<SpanRef> {
        let Some(scene) = self.scene_id() else {
            return Vec::new();
        };
        let is_shape = |at: &SpanRef| {
            matches!(
                ops::find_span(self.document(), scene, *at).map(|s| s.content()),
                Some(Content::Shape(_))
            )
        };
        let mut targets: Vec<SpanRef> = self.selected.into_iter().filter(is_shape).collect();
        if let Some(hit) = self.hit(point)
            && !targets.contains(&hit)
            && is_shape(&hit)
        {
            targets.push(hit);
        }
        targets
    }

    /// The point of the shape at `at` that is nearest `point`, if one is close enough to grab.
    fn nearest_anchor(&self, at: SpanRef, point: Pos2) -> Option<usize> {
        let scene = self.scene_id()?;
        let Some(Content::Shape(shape)) =
            ops::find_span(self.document(), scene, at).map(|s| s.content())
        else {
            return None;
        };
        let grab = self.handle_radius.max(f32::EPSILON);
        ops::anchors(shape)
            .into_iter()
            .map(|(segment, p)| (segment, (p - point).length()))
            .filter(|(_, distance)| *distance <= grab)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(segment, _)| segment)
    }

    /// A click with the Add, Delete or Convert Anchor Point tool: it edits the outline of the
    /// selected shape, or of the one under the pointer, as one undo step.
    pub(in crate::workspace) fn anchor_click(&mut self, point: Pos2) {
        if !matches!(
            self.tool,
            Tool::AddAnchor | Tool::DeleteAnchor | Tool::ConvertAnchor
        ) {
            return;
        }
        let Some(scene) = self.scene_id() else { return };
        let here = DVec2::new(f64::from(point.x), f64::from(point.y));
        let grab = self.handle_radius.max(f32::EPSILON);
        for at in self.anchor_targets(point) {
            let near = self.nearest_anchor(at, point);
            let (commands, what) = match self.tool {
                Tool::AddAnchor => (
                    ops::add_anchor(self.document(), scene, at, here, f64::from(grab)),
                    "add the point",
                ),
                Tool::DeleteAnchor => (
                    near.and_then(|s| ops::delete_anchor(self.document(), scene, at, s)),
                    "delete the point",
                ),
                _ => (
                    near.and_then(|s| ops::convert_anchor(self.document(), scene, at, s, None)),
                    "make the point a corner",
                ),
            };
            if let Some(commands) = commands {
                self.selected = Some(at);
                self.also_selected.clear();
                self.run_batch(commands, what);
                return;
            }
            if self.tool == Tool::DeleteAnchor && near.is_some() {
                self.report("A shape keeps at least two points, three when it is closed");
                return;
            }
        }
        // Nothing to edit there: another shape under the pointer is selected, so the next click
        // can edit it. Empty stage leaves the selection as it is.
        if let Some(hit) = self.hit(point)
            && self.selected != Some(hit)
        {
            self.select_at(point, false);
        }
    }
}
