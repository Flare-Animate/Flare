//! Menu actions: whether they apply, what they are called and doing them.

use flare_dock::Preset;
use flare_document::Tween;
use flare_menubar::MenuAction;
use flare_tools::ops::{self, Stack};
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
        // Home hides the document, so only commands that do not act on it are offered.
        if self.windows.home
            && !matches!(
                action,
                MenuAction::Open
                    | MenuAction::New
                    | MenuAction::OpenRecent
                    | MenuAction::Quit
                    | MenuAction::ShowHome
                    | MenuAction::Preferences
                    | MenuAction::About
                    | MenuAction::KeyboardShortcuts
            )
        {
            return false;
        }
        match action {
            MenuAction::Open | MenuAction::Save | MenuAction::SaveAs | MenuAction::Quit => true,
            MenuAction::ImportSound => !self.in_symbol(),
            MenuAction::NewLayer | MenuAction::NewLayerFolder => {
                !self.document().scenes().is_empty()
            }
            MenuAction::New | MenuAction::Close => !self.in_symbol(),
            MenuAction::SaveAll => self.is_dirty(),
            MenuAction::OpenRecent => !self.home.recent.is_empty(),
            MenuAction::PublishSettings => true,
            MenuAction::Revert => self.path.is_some() && self.is_dirty(),
            MenuAction::ExportImage | MenuAction::ExportMovie | MenuAction::ExportFla => {
                !self.document().scenes().is_empty()
            }
            MenuAction::Undo => self.active().can_undo(),
            MenuAction::Redo => self.active().can_redo(),
            MenuAction::Paste | MenuAction::PasteInPlace => !self.clipboard.is_empty(),
            MenuAction::SelectAll => self.scene_id().is_some_and(|scene| {
                !ops::spans_at_frame(self.document(), scene, self.frame).is_empty()
            }),
            MenuAction::Rewind | MenuAction::StepBackward => self.frame > 0,
            MenuAction::GoToEnd => self.frame + 1 < self.scene_frames(),
            MenuAction::StepForward | MenuAction::ActualSize => true,
            MenuAction::CopyFrames => editable_layer || self.frame_range.is_some(),
            MenuAction::CutFrames => editable_layer,
            MenuAction::SelectAllFrames => layer.is_some_and(|l| !l.spans().is_empty()),
            MenuAction::ReverseFrames => editable_layer && self.frame_range.is_some(),
            MenuAction::PasteFrames => {
                self.frame_clipboard.is_some() && layer.is_some_and(|l| !l.is_locked())
            }
            // The Magic Wand's picked pixels are a selection too.
            MenuAction::Delete | MenuAction::DeselectAll => {
                self.has_selection() || self.wand_has_pixels()
            }
            MenuAction::Duplicate
            | MenuAction::Copy
            | MenuAction::Cut
            | MenuAction::FlipHorizontal
            | MenuAction::FlipVertical
            | MenuAction::MirrorCopyHorizontal
            | MenuAction::MirrorCopyVertical
            | MenuAction::RotateClockwise
            | MenuAction::RotateCounterclockwise
            | MenuAction::ScaleUp
            | MenuAction::ScaleDown => self.has_selection(),
            MenuAction::ConvertToSymbol => self.can_convert_selection(),
            MenuAction::BringToFront
            | MenuAction::BringForward
            | MenuAction::SendBackward
            | MenuAction::SendToBack => self.has_selection(),
            MenuAction::Group => self.selection_all().len() >= 2,
            MenuAction::Ungroup => self.selection_has_instance(),
            MenuAction::BreakApart => self.selection_can_break_apart(),
            MenuAction::IncrementInstances => self.selected_instance_symbol().is_some(),
            MenuAction::AddInstanceToSymbol => self.in_symbol() && self.has_selection(),
            MenuAction::BakeTransform => self.selection_has_instance(),
            MenuAction::Publish => !self.document().scenes().is_empty(),
            MenuAction::DocumentSettings | MenuAction::Preferences => true,
            MenuAction::EditSymbol => self.can_edit_selected_symbol(),
            MenuAction::ExitSymbol => self.in_symbol(),
            MenuAction::ToolSelect
            | MenuAction::ToolSubselection
            | MenuAction::ToolFreeTransform
            | MenuAction::ToolRectangle
            | MenuAction::ToolEllipse
            | MenuAction::ToolLine
            | MenuAction::ToolPen
            | MenuAction::ToolText
            | MenuAction::ToolHand
            | MenuAction::ToolRotateView
            | MenuAction::ToolZoom
            | MenuAction::ToolEyedropper
            | MenuAction::ToolBucket
            | MenuAction::ToolInkBottle
            | MenuAction::ToolEraser
            | MenuAction::ToolPolyStar
            | MenuAction::ToolLasso
            | MenuAction::ToolPencil
            | MenuAction::ToolBrush
            | MenuAction::ToolRuler
            | MenuAction::ToolMirror
            | MenuAction::PlayPause
            | MenuAction::ZoomIn
            | MenuAction::ZoomOut => true,
            MenuAction::ZoomFit => !self.stage_view.is_fit(),
            MenuAction::InsertFrame
            | MenuAction::RemoveFrame
            | MenuAction::InsertKeyframe
            | MenuAction::InsertBlankKeyframe
            | MenuAction::ClearKeyframe
            | MenuAction::ConvertToKeyframes => editable_layer,
            MenuAction::ToggleMotionTween => self.selection_holds_instance(),
            MenuAction::ToggleShapeTween => self.selection_holds_shape(),
            MenuAction::ToggleLayerVisible
            | MenuAction::ToggleLayerLocked
            | MenuAction::ToggleLayerMask
            | MenuAction::ToggleLayerGuide => layer.is_some(),
            MenuAction::MoveLayerUp => position.is_some_and(|p| p > 0),
            MenuAction::MoveLayerDown => position.is_some_and(|p| p + 1 < layer_count),
            MenuAction::DeleteLayer => layer.is_some() && layer_count > 1,
            MenuAction::ResetLayout
            | MenuAction::WorkspaceEssentials
            | MenuAction::WorkspaceAnimator
            | MenuAction::WorkspaceDesigner
            | MenuAction::WorkspaceClassic
            | MenuAction::WorkspaceSmallScreen => true,
            MenuAction::TextBold
            | MenuAction::TextItalic
            | MenuAction::TextLarger
            | MenuAction::TextSmaller => true,
            MenuAction::ToggleGuides => true,
            MenuAction::ClearGuides => !self.display.guides.is_empty(),
            MenuAction::NewSymbol => !self.in_symbol(),
            MenuAction::About | MenuAction::KeyboardShortcuts => true,
            MenuAction::ShowProperties
            | MenuAction::ShowLibrary
            | MenuAction::ShowColor
            | MenuAction::ShowAlign
            | MenuAction::ShowTransform
            | MenuAction::ShowHistory
            | MenuAction::ShowTimeline
            | MenuAction::ShowOutput
            | MenuAction::ShowInfo
            | MenuAction::ShowHome => true,
            MenuAction::ToggleOnionSkin
            | MenuAction::ToggleOutlineMode
            | MenuAction::TogglePasteboard
            | MenuAction::ToggleRulers
            | MenuAction::ToggleGrid
            | MenuAction::ToggleSnap
            | MenuAction::ToggleLoop
            | MenuAction::ToggleMute => true,
            MenuAction::TestMovie => true,
        }
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
            // As in Animate, File > New asks what kind of project, in the New Document dialog.
            MenuAction::New => self.show_new_document(),
            MenuAction::Close => self.close_tab(self.position),
            MenuAction::NewLayer => self.add_layer(),
            MenuAction::NewLayerFolder => self.add_layer_folder(),
            MenuAction::SaveAll => self.save(),
            MenuAction::OpenRecent => self.windows.recent = true,
            MenuAction::PublishSettings => self.windows.publish_settings = true,
            MenuAction::Revert => self.revert(),
            MenuAction::ExportImage => self.dialog = Some(DialogRequest::ExportImage),
            MenuAction::ExportMovie => self.dialog = Some(DialogRequest::ExportMovie),
            MenuAction::ExportFla => self.dialog = Some(DialogRequest::ExportFla),
            MenuAction::Save => self.save(),
            MenuAction::SaveAs => self.dialog = Some(DialogRequest::SaveAs),
            MenuAction::Quit => self.quit_requested = true,
            MenuAction::Undo => self.undo(),
            MenuAction::Redo => self.redo(),
            MenuAction::Delete => {
                if !self.wand_clear() {
                    self.delete_selection();
                }
            }
            MenuAction::Duplicate => self.duplicate_selection(),
            MenuAction::Copy => self.copy_selection(),
            MenuAction::Cut => self.cut_selection(),
            MenuAction::SelectAll => self.select_all(),
            MenuAction::DeselectAll => {
                self.clear_selection();
                self.wand_drop();
            }
            MenuAction::Rewind => self.set_frame(0),
            MenuAction::GoToEnd => self.set_frame(self.scene_frames() - 1),
            MenuAction::StepForward => self.set_frame(self.frame.saturating_add(1)),
            MenuAction::StepBackward => self.set_frame(self.frame.saturating_sub(1)),
            MenuAction::ActualSize => self.set_stage_percent(100.0),
            MenuAction::CopyFrames => self.copy_frames(),
            MenuAction::PasteFrames => self.paste_frames(),
            MenuAction::CutFrames => self.cut_frames(),
            MenuAction::SelectAllFrames => self.select_all_frames(),
            MenuAction::ReverseFrames => self.reverse_frames(),
            MenuAction::Paste => self.paste(),
            MenuAction::PasteInPlace => self.paste_in_place(),
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
            MenuAction::BringToFront => self.stack_selection(Stack::Front),
            MenuAction::BringForward => self.stack_selection(Stack::Forward),
            MenuAction::SendBackward => self.stack_selection(Stack::Backward),
            MenuAction::SendToBack => self.stack_selection(Stack::Back),
            MenuAction::Group => self.group_selection(),
            MenuAction::Ungroup | MenuAction::BreakApart => self.break_apart_selection(),
            MenuAction::IncrementInstances => self.show_increment_instances(),
            MenuAction::AddInstanceToSymbol => self.add_selection_to_symbol(),
            MenuAction::BakeTransform => self.bake_selection_transform(),
            MenuAction::Publish => self.windows.publish = true,
            MenuAction::DocumentSettings => {
                self.show_props_context(PropsContext::Document);
                self.show_pane(Pane::Properties);
            }
            MenuAction::ToolSubselection => self.set_tool(Tool::Subselection),
            MenuAction::Preferences => self.windows.preferences = true,
            MenuAction::ToolFreeTransform => self.set_tool(Tool::FreeTransform),
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
            MenuAction::ToolRotateView => self.set_tool(Tool::RotateView),
            MenuAction::ToolZoom => self.set_tool(Tool::Zoom),
            MenuAction::ToolEyedropper => self.set_tool(Tool::Eyedropper),
            MenuAction::ToolBucket => self.set_tool(Tool::Bucket),
            MenuAction::ToolInkBottle => self.set_tool(Tool::InkBottle),
            MenuAction::ToolEraser => self.set_tool(Tool::Eraser),
            MenuAction::ToolPolyStar => self.set_tool(Tool::PolyStar),
            MenuAction::ToolLasso => self.set_tool(Tool::Lasso),
            MenuAction::ToolPencil => self.set_tool(Tool::Pencil),
            MenuAction::ToolBrush => self.set_tool(Tool::Brush),
            MenuAction::ToolRuler => self.set_tool(Tool::Ruler),
            MenuAction::ToolMirror => self.set_tool(Tool::Mirror),
            MenuAction::MirrorCopyHorizontal => self.mirror_copy_selection(true),
            MenuAction::MirrorCopyVertical => self.mirror_copy_selection(false),
            MenuAction::InsertFrame => self.frame_edit(FrameOp::InsertFrame),
            MenuAction::RemoveFrame => self.frame_edit(FrameOp::RemoveFrame),
            MenuAction::InsertKeyframe => self.frame_edit(FrameOp::InsertKeyframe),
            MenuAction::InsertBlankKeyframe => self.frame_edit(FrameOp::InsertBlankKeyframe),
            MenuAction::ClearKeyframe => self.frame_edit(FrameOp::ClearKeyframe),
            MenuAction::ConvertToKeyframes => self.frame_edit(FrameOp::ConvertToKeyframes),
            MenuAction::ToggleMotionTween => {
                let next = if self.selection_tween() == Some(Tween::Linear) {
                    Tween::None
                } else {
                    Tween::Linear
                };
                self.set_selection_tween(next);
            }
            MenuAction::ToggleShapeTween => {
                let next = if self.selection_tween() == Some(Tween::Shape) {
                    Tween::None
                } else {
                    Tween::Shape
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
            MenuAction::ToggleLayerGuide => {
                if let Some(id) = layer {
                    self.toggle_layer_guide(id);
                }
            }
            MenuAction::MoveLayerUp => self.move_active_layer(-1),
            MenuAction::MoveLayerDown => self.move_active_layer(1),
            MenuAction::DeleteLayer => self.delete_active_layer(),
            MenuAction::ResetLayout => self.reset_layout(),
            MenuAction::WorkspaceEssentials => self.switch_preset(Preset::Essentials),
            MenuAction::WorkspaceAnimator => self.switch_preset(Preset::Animator),
            MenuAction::WorkspaceDesigner => self.switch_preset(Preset::Designer),
            MenuAction::WorkspaceClassic => self.switch_preset(Preset::Classic),
            MenuAction::WorkspaceSmallScreen => self.switch_preset(Preset::SmallScreen),
            MenuAction::NewSymbol => self.new_symbol(),
            MenuAction::TextBold => self.text.bold = !self.text.bold,
            MenuAction::TextItalic => self.text.italic = !self.text.italic,
            MenuAction::TextLarger => {
                self.text.size = (self.text.size * 1.25).min(flare_text::MAX_SIZE)
            }
            MenuAction::TextSmaller => {
                self.text.size = (self.text.size / 1.25).max(flare_text::MIN_SIZE)
            }
            MenuAction::ToggleGuides => self.display.show_guides = !self.display.show_guides,
            MenuAction::ClearGuides => {
                self.display.guides.clear();
                self.display.guide_drag = None;
            }
            MenuAction::About => self.windows.about = true,
            MenuAction::KeyboardShortcuts => self.windows.shortcuts = true,
            MenuAction::ShowProperties => self.show_pane(Pane::Properties),
            MenuAction::ShowLibrary => self.show_pane(Pane::Library),
            MenuAction::ShowColor => self.show_pane(Pane::Color),
            MenuAction::ShowAlign => self.show_pane(Pane::Align),
            MenuAction::ShowTransform => self.show_pane(Pane::Transform),
            MenuAction::ShowHistory => self.show_pane(Pane::History),
            MenuAction::ShowTimeline => self.show_pane(Pane::Timeline),
            MenuAction::ShowOutput => self.show_pane(Pane::Output),
            MenuAction::ShowInfo => self.show_pane(Pane::Info),
            MenuAction::ShowHome => self.windows.home = !self.windows.home,
            MenuAction::ToggleOnionSkin => self.set_onion_skin(!self.onion.enabled),
            MenuAction::ToggleOutlineMode => self.display.outline_mode = !self.display.outline_mode,
            MenuAction::TogglePasteboard => {
                self.display.show_pasteboard = !self.display.show_pasteboard;
            }
            MenuAction::ToggleRulers => self.display.show_rulers = !self.display.show_rulers,
            MenuAction::ToggleGrid => self.display.show_grid = !self.display.show_grid,
            MenuAction::ToggleSnap => self.display.snap_to_grid = !self.display.snap_to_grid,
            MenuAction::ToggleLoop => self.loop_playback = !self.loop_playback,
            MenuAction::ToggleMute => {
                self.muted = !self.muted;
                // Sounds already started stop now; unmuting waits for the next start.
                self.audio.events.push(AudioEvent::Stop);
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
