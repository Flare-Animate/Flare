//! The subselection tool: dragging the points of a selected shape.

use flare_document::Content;
use glamx::DVec2;

use super::*;

/// A point being dragged: which shape, which segment's end, and where it is now.
#[derive(Clone, Copy, Debug)]
pub(super) struct AnchorDrag {
    pub(super) at: SpanRef,
    pub(super) segment: usize,
    pub(super) to: Pos2,
}

impl Workspace {
    /// The points of the selected shape that can be dragged (none for instances).
    pub(super) fn anchor_points(&self) -> Vec<(usize, Pos2)> {
        let (Some(at), Some(scene)) = (self.selected, self.scene_id()) else {
            return Vec::new();
        };
        match ops::find_span(self.document(), scene, at).map(|s| s.content()) {
            Some(Content::Shape(shape)) => ops::anchors(shape),
            _ => Vec::new(),
        }
    }

    /// Starts dragging the point nearest `point`, if one is close enough to grab.
    pub(super) fn begin_anchor_drag(&mut self, point: Pos2) {
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
                to: from,
            });
        }
    }

    pub(super) fn update_anchor_drag(&mut self, point: Pos2) {
        if let Some(drag) = &mut self.anchor_drag {
            drag.to = point;
        }
    }

    /// Applies the drag as one undo step, unless it was cancelled.
    pub(super) fn end_anchor_drag(&mut self, cancelled: bool) {
        let (Some(drag), Some(scene)) = (self.anchor_drag.take(), self.scene_id()) else {
            return;
        };
        if cancelled {
            return;
        }
        let to = DVec2::new(f64::from(drag.to.x), f64::from(drag.to.y));
        if let Some(commands) = ops::move_anchor(self.document(), scene, drag.at, drag.segment, to)
        {
            self.run_batch(commands, "move the point");
        }
    }
}
