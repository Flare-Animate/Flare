//! The Motion Editor: the curve of the selected tween, with its handles to drag. All the rules
//! (what a pixel grabs, where a handle goes, what each handle kind does) are in `flare_tween`;
//! this is the state of one editing session and the commands it ends in.
//!
//! A drag changes a working copy only. The document changes once, when the button is released,
//! through the same validated command as the Properties picker, so a drag is one undo step and a
//! failed one changes nothing.

use crate::workspace::Workspace;
use flare_document::SpanInterp;
use flare_tools::tween::spec;
use flare_tween::{
    Curve, DragOptions, Extrapolation, Grab, Handle, HandleKind, Interpolation, Key, View,
    drag_handle, effective_handles, hit_test, sample_into,
};

/// Samples of the drawn curve. A few dozen points read as smooth at panel size.
pub(super) const SAMPLES: usize = 96;
/// How close, in pixels, a press must be to grab a handle.
pub(super) const GRAB_RADIUS: f64 = 9.0;
/// World range shown at first: a little room around 0..1 for overshoot.
const FIT_X: (f64, f64) = (0.0, 1.0);
const FIT_Y: (f64, f64) = (-0.3, 1.3);
const MARGIN: f64 = 18.0;

/// A handle being dragged, and the span as it would be if released now.
#[derive(Clone, Copy)]
struct Dragging {
    grab: Grab,
    working: SpanInterp,
}

/// The editor's own state; nothing here is saved with the document.
pub(crate) struct MotionEditor {
    view: View,
    size: (f64, f64),
    drag: Option<Dragging>,
    /// The curve copied from a span, for pasting on another.
    clipboard: Option<SpanInterp>,
    /// Samples of the drawn curve, and what they were made for, so an idle panel draws without
    /// evaluating or allocating.
    samples: Vec<f64>,
    sampled_for: Option<SpanInterp>,
    resampled: u32,
}

impl Default for MotionEditor {
    fn default() -> Self {
        Self {
            view: View::default(),
            size: (0.0, 0.0),
            drag: None,
            clipboard: None,
            samples: Vec::new(),
            sampled_for: None,
            resampled: 0,
        }
    }
}

impl MotionEditor {
    pub(crate) fn view(&self) -> &View {
        &self.view
    }
    pub(super) fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }
    /// How many times the curve has been sampled, for tests that check an idle panel does not.
    #[cfg(test)]
    pub(crate) fn resamples(&self) -> u32 {
        self.resampled
    }
    pub(super) fn has_clipboard(&self) -> bool {
        self.clipboard.is_some()
    }

    /// The samples of `interp`, made again only when it differs from the last one drawn.
    pub(super) fn samples_for(&mut self, interp: &SpanInterp) -> &[f64] {
        if self.sampled_for != Some(*interp) {
            let curve = bezier_curve(interp);
            let params = interp.params();
            let (family, easing) = (interp.interpolation(), interp.easing());
            sample_into(
                |t| match &curve {
                    Some(c) => c.evaluate(t).unwrap_or(t),
                    None => flare_tween::ease(family, easing, t, &params).unwrap_or(t),
                },
                SAMPLES,
                &mut self.samples,
            );
            self.sampled_for = Some(*interp);
            self.resampled += 1;
        }
        &self.samples
    }
}

/// The two-key curve of a Bezier span, built once per change of the span.
fn bezier_curve(interp: &SpanInterp) -> Option<Curve> {
    if interp.interpolation() != Interpolation::Bezier {
        return None;
    }
    let h = interp.handles();
    let mut a = Key::new(0.0, 0.0);
    a.handle_kind = interp.handle_kind();
    a.easing = interp.easing();
    a.right = Handle { dt: h[0], dv: h[1] };
    let mut b = Key::new(1.0, 1.0);
    b.handle_kind = interp.handle_kind();
    b.left = Handle { dt: h[2], dv: h[3] };
    Curve::new(vec![a, b], Extrapolation::Constant, interp.params()).ok()
}

impl Workspace {
    pub(crate) fn motion(&self) -> &MotionEditor {
        &self.motion
    }
    pub(super) fn motion_mut(&mut self) -> &mut MotionEditor {
        &mut self.motion
    }

    /// Tells the editor how big its plot is, in pixels. The view fits the unit square again only
    /// when the size changes, so panning and zooming survive a redraw.
    pub fn motion_layout(&mut self, width: f64, height: f64) {
        if self.motion.size != (width, height) {
            self.motion.size = (width, height);
            self.motion.view = View::fit(width, height, MARGIN, FIT_X, FIT_Y);
        }
    }

    /// The span to show: the one being dragged, else the selected span's.
    pub fn motion_shown(&self) -> Option<SpanInterp> {
        self.motion
            .drag
            .map(|d| d.working)
            .or_else(|| self.selection_interp())
    }

    /// A press at `pixel` (relative to the plot): true when it took hold of a handle.
    pub fn motion_press(&mut self, pixel: (f64, f64)) -> bool {
        let Some(interp) = self.selection_interp() else {
            return false;
        };
        if interp.interpolation() != Interpolation::Bezier {
            return false;
        }
        let Ok(handles) = effective_handles(interp.handle_kind(), interp.handles()) else {
            return false;
        };
        let Some(grab) = hit_test(self.motion.view(), handles, pixel, GRAB_RADIUS) else {
            return false;
        };
        self.motion.drag = Some(Dragging {
            grab,
            working: interp,
        });
        true
    }

    /// The pointer moved to `pixel` while holding a handle; `snap` snaps to a grid of tenths.
    pub fn motion_drag(&mut self, pixel: (f64, f64), snap: bool) {
        let Some(drag) = self.motion.drag else {
            return;
        };
        let world = self.motion.view.to_world(pixel);
        let from = drag.working;
        let options = DragOptions {
            snap: snap.then_some(0.1),
        };
        let moved = drag_handle(
            from.handle_kind(),
            from.handles(),
            drag.grab,
            world,
            options,
        )
        .ok()
        .and_then(|d| {
            SpanInterp::new(
                from.interpolation(),
                from.easing(),
                d.kind,
                d.handles,
                from.params(),
            )
            .ok()
        });
        if let Some(working) = moved {
            self.motion.drag = Some(Dragging { working, ..drag });
        }
    }

    /// The button was released: the working copy becomes one command, or nothing if unchanged.
    pub fn motion_release(&mut self) {
        let Some(drag) = self.motion.drag.take() else {
            return;
        };
        self.set_selection_interp(Some(drag.working));
    }

    /// Gives up a drag; the document was never touched.
    pub fn motion_cancel(&mut self) {
        self.motion.drag = None;
    }

    /// Chooses the family of the selected tween, keeping its easing and handles.
    pub fn motion_set_family(&mut self, family: &str) {
        let current = self.selection_interp();
        let easing = current.map(|i| i.easing().name());
        let kind = current.map(|i| i.handle_kind().name());
        let handles = current.map(|i| i.handles());
        match spec::build_interp(family, easing, kind, handles) {
            Ok(interp) => self.set_selection_interp(Some(interp)),
            Err(message) => self.report(message),
        }
    }

    /// Chooses the easing of the selected tween's family.
    pub fn motion_set_easing(&mut self, easing: &str) {
        let Some(current) = self.selection_interp() else {
            return;
        };
        let family = current.interpolation().name();
        let kind = current.handle_kind().name();
        match spec::build_interp(family, Some(easing), Some(kind), Some(current.handles())) {
            Ok(interp) => self.set_selection_interp(Some(interp)),
            Err(message) => self.report(message),
        }
    }

    /// Chooses the handle kind. The handles are kept as they are drawn now, so changing the kind
    /// does not make the curve jump; Auto and Vector then recompute them.
    pub fn motion_set_handle_kind(&mut self, kind: HandleKind) {
        let Some(current) = self.selection_interp() else {
            return;
        };
        let shown = effective_handles(current.handle_kind(), current.handles())
            .unwrap_or_else(|_| current.handles());
        match SpanInterp::new(
            current.interpolation(),
            current.easing(),
            kind,
            shown,
            current.params(),
        ) {
            Ok(interp) => self.set_selection_interp(Some(interp)),
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Remembers the selected tween's curve for `motion_paste`.
    pub fn motion_copy(&mut self) {
        if let Some(interp) = self.selection_interp() {
            self.motion.clipboard = Some(interp);
        }
    }

    /// Puts the copied curve on the selected tween.
    pub fn motion_paste(&mut self) {
        if let Some(interp) = self.motion.clipboard {
            self.set_selection_interp(Some(interp));
        }
    }

    /// Back to a plain linear tween.
    pub fn motion_reset(&mut self) {
        self.set_selection_interp(Some(SpanInterp::default()));
    }
}
