//! Menu actions: whether they apply, what they are called and doing them.

use crate::MenuAction;
use flare_document::{Layer, Tween};
use flare_tools::ops::{self};
use flare_ui::view::{self};
use glamx::{DAffine2, DVec2};

use super::*;

impl Workspace {
    /// Whether `action` can run right now (menu items are greyed out when it cannot).
    pub fn is_enabled(&self, action: MenuAction) -> bool {
        let layer = self.active_layer();
        let editable_layer = layer.is_some_and(|l| !l.is_locked());
        let layer_count = self
            .document()
            .scenes()
            .first()
            .map_or(0, |s| s.timeline().layers().len());
        let position = layer.and_then(|l| {
            self.document()
                .scenes()
                .first()?
                .timeline()
                .layers()
                .iter()
                .position(|c| c.id() == l.id())
        });
        match action {
            MenuAction::Open | MenuAction::Save | MenuAction::SaveAs | MenuAction::Quit => true,
            MenuAction::ImportSound => !self.in_symbol(),
            MenuAction::ExportImage => !self.document().scenes().is_empty(),
            MenuAction::Undo => self.active().can_undo(),
            MenuAction::Redo => self.active().can_redo(),
            MenuAction::Paste => !self.clipboard.is_empty(),
            MenuAction::SelectAll => self.scene_id().is_some_and(|scene| {
                !ops::spans_at_frame(self.document(), scene, self.frame).is_empty()
            }),
            MenuAction::Rewind | MenuAction::StepBackward => self.frame > 0,
            MenuAction::StepForward | MenuAction::ActualSize => true,
            MenuAction::CopyFrames => editable_layer || self.frame_range.is_some(),
            MenuAction::PasteFrames => {
                self.frame_clipboard.is_some() && layer.is_some_and(|l| !l.is_locked())
            }
            MenuAction::Delete
            | MenuAction::Duplicate
            | MenuAction::Copy
            | MenuAction::Cut
            | MenuAction::DeselectAll
            | MenuAction::FlipHorizontal
            | MenuAction::FlipVertical
            | MenuAction::RotateClockwise
            | MenuAction::RotateCounterclockwise
            | MenuAction::ScaleUp
            | MenuAction::ScaleDown => self.has_selection(),
            MenuAction::ConvertToSymbol => self.can_convert_selection(),
            MenuAction::Group => self.selection_all().len() >= 2,
            MenuAction::Ungroup | MenuAction::BreakApart => self.selection_has_instance(),
            MenuAction::PublishSwf => !self.document().scenes().is_empty(),
            MenuAction::DocumentSettings => true,
            MenuAction::EditSymbol => self.can_edit_selected_symbol(),
            MenuAction::ExitSymbol => self.in_symbol(),
            MenuAction::ToolSelect
            | MenuAction::ToolSubselection
            | MenuAction::ToolRectangle
            | MenuAction::ToolEllipse
            | MenuAction::ToolLine
            | MenuAction::ToolPen
            | MenuAction::ToolText
            | MenuAction::ToolHand
            | MenuAction::ToolZoom
            | MenuAction::ToolEyedropper
            | MenuAction::ToolBucket
            | MenuAction::ToolInkBottle
            | MenuAction::ToolEraser
            | MenuAction::ToolPolyStar
            | MenuAction::ToolLasso
            | MenuAction::ToolPencil
            | MenuAction::ToolBrush
            | MenuAction::PlayPause
            | MenuAction::ZoomIn
            | MenuAction::ZoomOut => true,
            MenuAction::ZoomFit => !self.stage_view.is_fit(),
            MenuAction::InsertFrame
            | MenuAction::RemoveFrame
            | MenuAction::InsertKeyframe
            | MenuAction::InsertBlankKeyframe
            | MenuAction::ClearKeyframe => editable_layer,
            MenuAction::ToggleMotionTween => self.selection_tween().is_some(),
            MenuAction::ToggleLayerVisible
            | MenuAction::ToggleLayerLocked
            | MenuAction::ToggleLayerMask => layer.is_some(),
            MenuAction::MoveLayerUp => position.is_some_and(|p| p > 0),
            MenuAction::MoveLayerDown => position.is_some_and(|p| p + 1 < layer_count),
            MenuAction::DeleteLayer => layer.is_some() && layer_count > 1,
            MenuAction::ResetLayout => true,
            MenuAction::ToggleOnionSkin
            | MenuAction::ToggleRulers
            | MenuAction::ToggleGrid
            | MenuAction::ToggleSnap
            | MenuAction::ToggleLoop
            | MenuAction::ToggleMute => true,
            MenuAction::TestMovie => true,
        }
    }

    /// The menu item's current label (some toggle between two meanings).
    pub fn label(&self, action: MenuAction) -> String {
        let layer = self.active_layer();
        let text = match action {
            MenuAction::Open => "Open...",
            MenuAction::ImportSound => "Import Sound...",
            MenuAction::ExportImage => "Export Image...",
            MenuAction::Save => "Save",
            MenuAction::SaveAs => "Save As...",
            MenuAction::Quit => "Quit Flare",
            MenuAction::Undo => "Undo",
            MenuAction::Redo => "Redo",
            MenuAction::Delete => "Delete",
            MenuAction::Duplicate => "Duplicate",
            MenuAction::Copy => "Copy",
            MenuAction::Cut => "Cut",
            MenuAction::SelectAll => "Select All",
            MenuAction::DeselectAll => "Deselect All",
            MenuAction::Rewind => "Rewind",
            MenuAction::StepForward => "Step Forward",
            MenuAction::StepBackward => "Step Backward",
            MenuAction::ActualSize => "Actual Size",
            MenuAction::CopyFrames => "Copy Frames",
            MenuAction::PasteFrames => "Paste Frames",
            MenuAction::Paste => "Paste",
            MenuAction::FlipHorizontal => "Flip Horizontal",
            MenuAction::FlipVertical => "Flip Vertical",
            MenuAction::RotateClockwise => "Rotate 90\u{b0} Clockwise",
            MenuAction::RotateCounterclockwise => "Rotate 90\u{b0} Counterclockwise",
            MenuAction::ScaleUp => "Scale Up",
            MenuAction::ScaleDown => "Scale Down",
            MenuAction::ConvertToSymbol => "Convert to Symbol",
            MenuAction::Group => "Group",
            MenuAction::Ungroup => "Ungroup",
            MenuAction::BreakApart => "Break Apart",
            MenuAction::PublishSwf => "Publish SWF...",
            MenuAction::DocumentSettings => "Document Settings...",
            MenuAction::ToolSubselection => "Subselection",
            MenuAction::EditSymbol => "Edit Symbol",
            MenuAction::ExitSymbol => "Exit Symbol",
            MenuAction::ToolSelect => "Select / Move",
            MenuAction::ToolRectangle => "Rectangle",
            MenuAction::ToolEllipse => "Ellipse",
            MenuAction::ToolLine => "Line",
            MenuAction::ToolPen => "Pen",
            MenuAction::ToolText => "Text",
            MenuAction::ToolHand => "Hand",
            MenuAction::ToolZoom => "Zoom",
            MenuAction::ToolEyedropper => "Eyedropper",
            MenuAction::ToolBucket => "Paint Bucket",
            MenuAction::ToolInkBottle => "Ink Bottle",
            MenuAction::ToolEraser => "Eraser",
            MenuAction::ToolPolyStar => "PolyStar",
            MenuAction::ToolLasso => "Lasso",
            MenuAction::ToolPencil => "Pencil",
            MenuAction::ToolBrush => "Brush",
            MenuAction::ZoomIn => "Zoom In",
            MenuAction::ZoomOut => "Zoom Out",
            MenuAction::ZoomFit => "Fit Stage in Window",
            MenuAction::InsertFrame => "Insert Frame",
            MenuAction::RemoveFrame => "Remove Frame",
            MenuAction::InsertKeyframe => "Insert Keyframe",
            MenuAction::InsertBlankKeyframe => "Insert Blank Keyframe",
            MenuAction::ClearKeyframe => "Clear Keyframe",
            MenuAction::ToggleMotionTween => {
                if self.selection_tween() == Some(Tween::Linear) {
                    "Remove Motion Tween"
                } else {
                    "Create Motion Tween"
                }
            }
            MenuAction::PlayPause => {
                if self.playing {
                    "Pause"
                } else {
                    "Play"
                }
            }
            MenuAction::ToggleLayerVisible => {
                if layer.is_none_or(Layer::is_visible) {
                    "Hide Layer"
                } else {
                    "Show Layer"
                }
            }
            MenuAction::ToggleLayerLocked => {
                if layer.is_some_and(Layer::is_locked) {
                    "Unlock Layer"
                } else {
                    "Lock Layer"
                }
            }
            MenuAction::ToggleLayerMask => {
                if layer.is_some_and(Layer::is_mask) {
                    "Remove Mask"
                } else {
                    "Mask Layer Below"
                }
            }
            MenuAction::MoveLayerUp => "Move Layer Up",
            MenuAction::MoveLayerDown => "Move Layer Down",
            MenuAction::DeleteLayer => "Delete Layer",
            MenuAction::ResetLayout => "Reset Workspace Layout",
            MenuAction::ToggleOnionSkin => {
                if self.onion.enabled {
                    "Hide Onion Skin"
                } else {
                    "Show Onion Skin"
                }
            }
            MenuAction::ToggleRulers => {
                if self.show_rulers {
                    "Hide Rulers"
                } else {
                    "Show Rulers"
                }
            }
            MenuAction::ToggleGrid => {
                if self.show_grid {
                    "Hide Grid"
                } else {
                    "Show Grid"
                }
            }
            MenuAction::ToggleSnap => {
                if self.snap_to_grid {
                    "Snapping: On"
                } else {
                    "Snapping: Off"
                }
            }
            MenuAction::ToggleLoop => {
                if self.loop_playback {
                    "Loop Playback: On"
                } else {
                    "Loop Playback: Off"
                }
            }
            MenuAction::ToggleMute => {
                if self.muted {
                    "Unmute Sounds"
                } else {
                    "Mute Sounds"
                }
            }
            MenuAction::TestMovie => "Test Movie",
        };
        text.to_owned()
    }

    /// Runs `action`. Disabled actions do nothing, so menus need not re-check.
    pub fn perform(&mut self, action: MenuAction) {
        if !self.is_enabled(action) {
            return;
        }
        let layer = self.active_layer_id();
        match action {
            MenuAction::Open => self.dialog = Some(DialogRequest::Open),
            MenuAction::ImportSound => self.dialog = Some(DialogRequest::ImportSound),
            MenuAction::ExportImage => self.dialog = Some(DialogRequest::ExportImage),
            MenuAction::Save => self.save(),
            MenuAction::SaveAs => self.dialog = Some(DialogRequest::SaveAs),
            MenuAction::Quit => self.quit_requested = true,
            MenuAction::Undo => self.undo(),
            MenuAction::Redo => self.redo(),
            MenuAction::Delete => self.delete_selection(),
            MenuAction::Duplicate => self.duplicate_selection(),
            MenuAction::Copy => self.copy_selection(),
            MenuAction::Cut => self.cut_selection(),
            MenuAction::SelectAll => self.select_all(),
            MenuAction::DeselectAll => self.clear_selection(),
            MenuAction::Rewind => self.set_frame(0),
            MenuAction::StepForward => self.set_frame(self.frame.saturating_add(1)),
            MenuAction::StepBackward => self.set_frame(self.frame.saturating_sub(1)),
            MenuAction::ActualSize => self.set_stage_percent(100.0),
            MenuAction::CopyFrames => self.copy_frames(),
            MenuAction::PasteFrames => self.paste_frames(),
            MenuAction::Paste => self.paste(),
            MenuAction::FlipHorizontal => {
                self.transform_selection(DAffine2::from_scale(DVec2::new(-1.0, 1.0)))
            }
            MenuAction::FlipVertical => {
                self.transform_selection(DAffine2::from_scale(DVec2::new(1.0, -1.0)))
            }
            MenuAction::RotateClockwise => {
                self.transform_selection(DAffine2::from_angle(std::f64::consts::FRAC_PI_2));
            }
            MenuAction::RotateCounterclockwise => {
                self.transform_selection(DAffine2::from_angle(-std::f64::consts::FRAC_PI_2));
            }
            MenuAction::ScaleUp => {
                self.transform_selection(DAffine2::from_scale(DVec2::splat(SCALE_STEP)))
            }
            MenuAction::ScaleDown => {
                self.transform_selection(DAffine2::from_scale(DVec2::splat(1.0 / SCALE_STEP)))
            }
            MenuAction::ConvertToSymbol => self.convert_selection_to_symbol(),
            MenuAction::Group => self.group_selection(),
            MenuAction::Ungroup | MenuAction::BreakApart => self.break_apart_selection(),
            MenuAction::PublishSwf => self.dialog = Some(DialogRequest::PublishSwf),
            MenuAction::DocumentSettings => {
                self.props_tab = PropsTab::Doc;
                self.show_pane(Pane::Properties);
            }
            MenuAction::ToolSubselection => self.set_tool(Tool::Subselection),
            MenuAction::EditSymbol => self.edit_selected_symbol(),
            MenuAction::ExitSymbol => {
                self.exit_symbol();
            }
            MenuAction::ToolSelect => self.set_tool(Tool::Select),
            MenuAction::ToolRectangle => self.set_tool(Tool::Rectangle),
            MenuAction::ToolEllipse => self.set_tool(Tool::Ellipse),
            MenuAction::ToolLine => self.set_tool(Tool::Line),
            MenuAction::ToolPen => self.set_tool(Tool::Pen),
            MenuAction::ToolText => self.set_tool(Tool::Text),
            MenuAction::ToolHand => self.set_tool(Tool::Hand),
            MenuAction::ToolZoom => self.set_tool(Tool::Zoom),
            MenuAction::ToolEyedropper => self.set_tool(Tool::Eyedropper),
            MenuAction::ToolBucket => self.set_tool(Tool::Bucket),
            MenuAction::ToolInkBottle => self.set_tool(Tool::InkBottle),
            MenuAction::ToolEraser => self.set_tool(Tool::Eraser),
            MenuAction::ToolPolyStar => self.set_tool(Tool::PolyStar),
            MenuAction::ToolLasso => self.set_tool(Tool::Lasso),
            MenuAction::ToolPencil => self.set_tool(Tool::Pencil),
            MenuAction::ToolBrush => self.set_tool(Tool::Brush),
            MenuAction::InsertFrame => self.frame_edit(FrameOp::InsertFrame),
            MenuAction::RemoveFrame => self.frame_edit(FrameOp::RemoveFrame),
            MenuAction::InsertKeyframe => self.frame_edit(FrameOp::InsertKeyframe),
            MenuAction::InsertBlankKeyframe => self.frame_edit(FrameOp::InsertBlankKeyframe),
            MenuAction::ClearKeyframe => self.frame_edit(FrameOp::ClearKeyframe),
            MenuAction::ToggleMotionTween => {
                let next = if self.selection_tween() == Some(Tween::Linear) {
                    Tween::None
                } else {
                    Tween::Linear
                };
                self.set_selection_tween(next);
            }
            MenuAction::PlayPause => self.playing = !self.playing,
            MenuAction::ZoomIn => self.zoom_stage(view::STEP),
            MenuAction::ZoomOut => self.zoom_stage(1.0 / view::STEP),
            MenuAction::ZoomFit => self.stage_view.reset(),
            MenuAction::ToggleLayerVisible => {
                if let Some(id) = layer {
                    self.toggle_layer_visible(id);
                }
            }
            MenuAction::ToggleLayerLocked => {
                if let Some(id) = layer {
                    self.toggle_layer_locked(id);
                }
            }
            MenuAction::ToggleLayerMask => {
                if let Some(id) = layer {
                    self.toggle_layer_mask(id);
                }
            }
            MenuAction::MoveLayerUp => self.move_active_layer(-1),
            MenuAction::MoveLayerDown => self.move_active_layer(1),
            MenuAction::DeleteLayer => self.delete_active_layer(),
            MenuAction::ResetLayout => self.reset_layout(),
            MenuAction::ToggleOnionSkin => self.set_onion_skin(!self.onion.enabled),
            MenuAction::ToggleRulers => self.show_rulers = !self.show_rulers,
            MenuAction::ToggleGrid => self.show_grid = !self.show_grid,
            MenuAction::ToggleSnap => self.snap_to_grid = !self.snap_to_grid,
            MenuAction::ToggleLoop => self.loop_playback = !self.loop_playback,
            MenuAction::ToggleMute => {
                self.muted = !self.muted;
                // Sounds already started stop now; unmuting waits for the next start.
                self.audio_events.push(AudioEvent::Stop);
            }
            MenuAction::TestMovie => {
                // Pending symbol edits belong in the movie, so apply them first, like Save.
                if self.exit_all_symbols() {
                    self.test_movie_requested = true;
                }
            }
        }
    }
}
