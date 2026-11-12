//! The tools of the strip, as the workspace tells them apart.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rectangle,
    Ellipse,
    Line,
    Pen,
    /// Click the stage, type, and place: the text becomes ordinary shapes.
    Text,
    /// Drag to pan the stage.
    Hand,
    /// Drag to turn the view of the stage; the document does not change.
    RotateView,
    /// Click to zoom in, Alt-click to zoom out.
    Zoom,
    /// Click a shape to take its fill (Shift: its stroke) as the current color.
    Eyedropper,
    /// Click a shape to give it the current fill.
    Bucket,
    /// Click a shape to give it the current stroke.
    InkBottle,
    /// Click a shape to delete it.
    Eraser,
    /// Drag from the center out: a regular polygon, or a star.
    PolyStar,
    /// Drag a freehand loop around shapes to select them.
    Lasso,
    /// Click the corners of a loop around shapes to select them.
    PolygonLasso,
    /// Click a bitmap to pick the pixels of a color like the one clicked.
    MagicWand,
    /// Drag to move the scene's camera; its controls are in Properties.
    Camera,
    /// Drag from one symbol instance to another to link them with a bone.
    Bone,
    /// Select a shape, then drag its points to reshape it.
    Subselection,
    /// Click an outline to put a point there.
    AddAnchor,
    /// Click a point to take it out of the outline.
    DeleteAnchor,
    /// Click a smooth point to make it a corner; drag from a corner to make it smooth.
    ConvertAnchor,
    /// Select, then scale, rotate or skew by the handles around the selection.
    FreeTransform,
    /// Drag to draw a smoothed line in the stroke color and width.
    Pencil,
    /// Drag to paint a thick round stroke in the fill color.
    Brush,
    /// Drag to read the distance and angle between two points.
    Ruler,
    /// Click to set the axis that symmetric drawing and mirror copies use.
    Mirror,
}
