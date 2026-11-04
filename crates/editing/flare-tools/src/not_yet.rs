//! Features of the previous C++ Flare that are not yet re-implemented in Rust.
//! Every entry returns [`NotYetImplemented`]; none ever reports success.
//! The list is mirrored in PORTING_TRACKER.md ("Stubbed re-additions").

use std::fmt;

/// A legacy feature with the original source path it comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyFeature {
    pub name: &'static str,
    pub original: &'static str,
}

/// Error returned by every stubbed feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotYetImplemented(pub LegacyFeature);

impl fmt::Display for NotYetImplemented {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is not yet re-implemented (see {})",
            self.0.name, self.0.original
        )
    }
}

impl std::error::Error for NotYetImplemented {}

macro_rules! stub {
    ($(#[$m:meta])* $fn:ident, $konst:ident, $name:literal, $path:literal) => {
        $(#[$m])*
        pub const $konst: LegacyFeature = LegacyFeature { name: $name, original: $path };
        $(#[$m])*
        pub fn $fn() -> Result<(), NotYetImplemented> {
            Err(NotYetImplemented($konst))
        }
    };
}

stub!(/** Tracker tool. Source: `original/flare/sources/tnztools/trackertool.cpp`. */
    tracker_tool, TRACKER, "Tracker tool", "original/flare/sources/tnztools/trackertool.cpp");
stub!(/** Plastic (mesh deformation) tool. Source: `original/flare/sources/tnztools/plastictool.cpp`. */
    plastic_tool, PLASTIC, "Plastic tool", "original/flare/sources/tnztools/plastictool.cpp");
stub!(/** Hook tool. Source: `original/flare/sources/tnztools/hooktool.cpp`. */
    hook_tool, HOOK, "Hook tool", "original/flare/sources/tnztools/hooktool.cpp");
stub!(/** Cutter tool. Source: `original/flare/sources/tnztools/cuttertool.cpp`. */
    cutter_tool, CUTTER, "Cutter tool", "original/flare/sources/tnztools/cuttertool.cpp");
stub!(/** Stop motion capture. Source: `original/flare/sources/stopmotion/stopmotion.cpp`. */
    stop_motion, STOP_MOTION, "Stop motion", "original/flare/sources/stopmotion/stopmotion.cpp");
stub!(/** Scan cleanup. Source: `original/flare/sources/tcleanupper/tcleanupper.cpp`. */
    cleanup, CLEANUP, "Scan cleanup", "original/flare/sources/tcleanupper/tcleanupper.cpp");
stub!(/** Render farm task. Source: `original/flare/sources/flarefarm/tfarm/tfarmtask.cpp`. */
    render_farm, RENDER_FARM, "Render farm", "original/flare/sources/flarefarm/tfarm/tfarmtask.cpp");
stub!(/** Magnet deformation tool. Source: `original/flare/sources/tnztools/magnettool.cpp`. */
    magnet_tool, MAGNET, "Magnet tool", "original/flare/sources/tnztools/magnettool.cpp");
stub!(/** Pinch deformation tool. Source: `original/flare/sources/tnztools/pinchtool.cpp`. */
    pinch_tool, PINCH, "Pinch tool", "original/flare/sources/tnztools/pinchtool.cpp");
stub!(/** Pump deformation tool. Source: `original/flare/sources/tnztools/pumptool.cpp`. */
    pump_tool, PUMP, "Pump tool", "original/flare/sources/tnztools/pumptool.cpp");
stub!(/** Bender deformation tool. Source: `original/flare/sources/tnztools/bendertool.cpp`. */
    bender_tool, BENDER, "Bender tool", "original/flare/sources/tnztools/bendertool.cpp");
stub!(/** Iron smoothing tool. Source: `original/flare/sources/tnztools/irontool.cpp`. */
    iron_tool, IRON, "Iron tool", "original/flare/sources/tnztools/irontool.cpp");
stub!(/** Style picker tool. Source: `original/flare/sources/tnztools/stylepickertool.cpp`. */
    style_picker_tool, STYLE_PICKER, "Style picker tool", "original/flare/sources/tnztools/stylepickertool.cpp");
stub!(/** Type tool. Source: `original/flare/sources/tnztools/typetool.cpp`. */
    type_tool, TYPE_TOOL, "Type tool", "original/flare/sources/tnztools/typetool.cpp");
stub!(/** Shift and trace tool. Source: `original/flare/sources/tnztools/shifttracetool.cpp`. */
    shift_trace_tool, SHIFT_TRACE, "Shift and trace tool", "original/flare/sources/tnztools/shifttracetool.cpp");
stub!(/** Ruler tool. Source: `original/flare/sources/tnztools/rulertool.cpp`. */
    ruler_tool, RULER_TOOL, "Ruler tool", "original/flare/sources/tnztools/rulertool.cpp");
stub!(/** Fingertool. Source: `original/flare/sources/tnztools/fingertool.cpp`. */
    finger_tool, FINGER, "Finger tool", "original/flare/sources/tnztools/fingertool.cpp");
stub!(/** Full color brush tool. Source: `original/flare/sources/tnztools/fullcolorbrushtool.cpp`. */
    full_color_brush_tool, FULL_COLOR_BRUSH, "Full color brush tool", "original/flare/sources/tnztools/fullcolorbrushtool.cpp");
stub!(/** Full color eraser tool. Source: `original/flare/sources/tnztools/fullcolorerasertool.cpp`. */
    full_color_eraser_tool, FULL_COLOR_ERASER, "Full color eraser tool", "original/flare/sources/tnztools/fullcolorerasertool.cpp");
stub!(/** Full color fill tool. Source: `original/flare/sources/tnztools/fullcolorfilltool.cpp`. */
    full_color_fill_tool, FULL_COLOR_FILL, "Full color fill tool", "original/flare/sources/tnztools/fullcolorfilltool.cpp");
stub!(/** Vector eraser tool. Source: `original/flare/sources/tnztools/vectorerasertool.cpp`. */
    vector_eraser_tool, VECTOR_ERASER, "Vector eraser tool", "original/flare/sources/tnztools/vectorerasertool.cpp");
stub!(/** Vector selection tool. Source: `original/flare/sources/tnztools/vectorselectiontool.cpp`. */
    vector_selection_tool, VECTOR_SELECTION, "Vector selection tool", "original/flare/sources/tnztools/vectorselectiontool.cpp");
stub!(/** Raster selection tool. Source: `original/flare/sources/tnztools/rasterselectiontool.cpp`. */
    raster_selection_tool, RASTER_SELECTION, "Raster selection tool", "original/flare/sources/tnztools/rasterselectiontool.cpp");
stub!(/** Geometric tool. Source: `original/flare/sources/tnztools/geometrictool.cpp`. */
    geometric_tool, GEOMETRIC, "Geometric tool", "original/flare/sources/tnztools/geometrictool.cpp");
stub!(/** Morph tool. Source: `original/flare/sources/tnztools/morphtool.cpp`. */
    morph_tool, MORPH, "Morph tool", "original/flare/sources/tnztools/morphtool.cpp");
stub!(/** Edit assistant tool. Source: `original/flare/sources/tnztools/editassistantstool.cpp`. */
    edit_assistant_tool, EDIT_ASSISTANT, "Edit assistant tool", "original/flare/sources/tnztools/editassistantstool.cpp");
stub!(/** Control point editor tool. Source: `original/flare/sources/tnztools/controlpointeditortool.cpp`. */
    control_point_editor_tool, CONTROL_POINT_EDITOR, "Control point editor tool", "original/flare/sources/tnztools/controlpointeditortool.cpp");
stub!(/** Toonz raster brush tool. Source: `original/flare/sources/tnztools/toonzrasterbrushtool.cpp`. */
    toonz_raster_brush_tool, TOONZ_RASTER_BRUSH, "Toonz raster brush tool", "original/flare/sources/tnztools/toonzrasterbrushtool.cpp");
stub!(/** Toonz vector brush tool. Source: `original/flare/sources/tnztools/toonzvectorbrushtool.cpp`. */
    toonz_vector_brush_tool, TOONZ_VECTOR_BRUSH, "Toonz vector brush tool", "original/flare/sources/tnztools/toonzvectorbrushtool.cpp");
stub!(/** MyPaint Toonz brush tool. Source: `original/flare/sources/tnztools/mypainttoonzbrush.cpp`. */
    mypaint_toonz_brush_tool, MYPAINT_TOONZ_BRUSH, "MyPaint Toonz brush tool", "original/flare/sources/tnztools/mypainttoonzbrush.cpp");
stub!(/** Blured brush tool. Source: `original/flare/sources/tnztools/bluredbrush.cpp`. */
    blured_brush_tool, BLURED_BRUSH, "Blured brush tool", "original/flare/sources/tnztools/bluredbrush.cpp");
stub!(/** Paint brush tool. Source: `original/flare/sources/tnztools/paintbrushtool.cpp`. */
    paint_brush_tool, PAINT_BRUSH, "Paint brush tool", "original/flare/sources/tnztools/paintbrushtool.cpp");
stub!(/** Raster eraser tool. Source: `original/flare/sources/tnztools/rastererasertool.cpp`. */
    raster_eraser_tool, RASTER_ERASER, "Raster eraser tool", "original/flare/sources/tnztools/rastererasertool.cpp");
stub!(/** Vector tape tool. Source: `original/flare/sources/tnztools/vectortapetool.cpp`. */
    vector_tape_tool, VECTOR_TAPE, "Vector tape tool", "original/flare/sources/tnztools/vectortapetool.cpp");
stub!(/** Raster tape tool. Source: `original/flare/sources/tnztools/rastertapetool.cpp`. */
    raster_tape_tool, RASTER_TAPE, "Raster tape tool", "original/flare/sources/tnztools/rastertapetool.cpp");
stub!(/** Fill tool. Source: `original/flare/sources/tnztools/filltool.cpp`. */
    fill_tool, FILL, "Fill tool", "original/flare/sources/tnztools/filltool.cpp");
stub!(/** Autofill PLI. Source: `original/flare/sources/tnztools/autofillpli.cpp`. */
    autofill_pli, AUTOFILL_PLI, "Autofill PLI", "original/flare/sources/tnztools/autofillpli.cpp");
stub!(/** Autofill TLV. Source: `original/flare/sources/tnztools/autofilltlv.cpp`. */
    autofill_tlv, AUTOFILL_TLV, "Autofill TLV", "original/flare/sources/tnztools/autofilltlv.cpp");
stub!(/** Set save box tool. Source: `original/flare/sources/tnztools/setsaveboxtool.cpp`. */
    set_save_box_tool, SET_SAVE_BOX, "Set save box tool", "original/flare/sources/tnztools/setsaveboxtool.cpp");
stub!(/** Replicator. Source: `original/flare/sources/tnztools/replicator.cpp`. */
    replicator, REPLICATOR, "Replicator", "original/flare/sources/tnztools/replicator.cpp");
stub!(/** Skeleton tool. Source: `original/flare/sources/tnztools/skeletontool.cpp`. */
    skeleton_tool, SKELETON, "Skeleton tool", "original/flare/sources/tnztools/skeletontool.cpp");
stub!(/** Assistant tool. Source: `original/flare/sources/tnztools/assistant.cpp`. */
    assistant_tool, ASSISTANT, "Assistant tool", "original/flare/sources/tnztools/assistant.cpp");
stub!(/** Skeleton subtools. Source: `original/flare/sources/tnztools/skeletonsubtools.cpp`. */
    skeleton_subtools, SKELETON_SUBTOOLS, "Skeleton subtools", "original/flare/sources/tnztools/skeletonsubtools.cpp");
stub!(/** RGB picker tool. Source: `original/flare/sources/tnztools/rgbpickertool.cpp`. */
    rgb_picker_tool, RGB_PICKER, "RGB picker tool", "original/flare/sources/tnztools/rgbpickertool.cpp");
stub!(/** Style picker tool. Source: `original/flare/sources/tnztools/stylepickertool.cpp`. */
    style_picker_tool2, STYLE_PICKER2, "Style picker tool 2", "original/flare/sources/tnztools/stylepickertool.cpp");
stub!(/** Control point selection. Source: `original/flare/sources/tnztools/controlpointselection.cpp`. */
    control_point_selection, CONTROL_POINT_SELECTION, "Control point selection", "original/flare/sources/tnztools/controlpointselection.cpp");
stub!(/** Stroke selection. Source: `original/flare/sources/tnztools/strokeselection.cpp`. */
    stroke_selection, STROKE_SELECTION, "Stroke selection", "original/flare/sources/tnztools/strokeselection.cpp");
stub!(/** Raster selection. Source: `original/flare/sources/tnztools/rasterselection.cpp`. */
    raster_selection, RASTER_SELECTION2, "Raster selection", "original/flare/sources/tnztools/rasterselection.cpp");
stub!(/** Hook selection. Source: `original/flare/sources/tnztools/hookselection.cpp`. */
    hook_selection, HOOK_SELECTION, "Hook selection", "original/flare/sources/tnztools/hookselection.cpp");
stub!(/** Image grouping. Source: `original/flare/sources/tnztools/imagegrouping.cpp`. */
    image_grouping, IMAGE_GROUPING, "Image grouping", "original/flare/sources/tnztools/imagegrouping.cpp");
stub!(/** Raster tap tool. Source: `original/flare/sources/tnztools/rastertapetool.cpp`. */
    raster_tape_tool2, RASTER_TAPE2, "Raster tape tool 2", "original/flare/sources/tnztools/rastertapetool.cpp");
stub!(/** Vector tap tool. Source: `original/flare/sources/tnztools/vectortapetool.cpp`. */
    vector_tape_tool2, VECTOR_TAPE2, "Vector tape tool 2", "original/flare/sources/tnztools/vectortapetool.cpp");

/// All stubbed features.
pub const ALL: [LegacyFeature; 52] = [
    ASSISTANT,
    AUTOFILL_PLI,
    AUTOFILL_TLV,
    BENDER,
    BLURED_BRUSH,
    CLEANUP,
    CONTROL_POINT_EDITOR,
    CONTROL_POINT_SELECTION,
    CUTTER,
    EDIT_ASSISTANT,
    FILL,
    FINGER,
    FULL_COLOR_BRUSH,
    FULL_COLOR_ERASER,
    FULL_COLOR_FILL,
    GEOMETRIC,
    HOOK,
    HOOK_SELECTION,
    IMAGE_GROUPING,
    IRON,
    MAGNET,
    MORPH,
    MYPAINT_TOONZ_BRUSH,
    PAINT_BRUSH,
    PINCH,
    PLASTIC,
    PUMP,
    RASTER_ERASER,
    RASTER_SELECTION,
    RASTER_SELECTION2,
    RASTER_TAPE,
    RASTER_TAPE2,
    RENDER_FARM,
    REPLICATOR,
    RGB_PICKER,
    RULER_TOOL,
    SET_SAVE_BOX,
    SHIFT_TRACE,
    SKELETON,
    SKELETON_SUBTOOLS,
    STOP_MOTION,
    STROKE_SELECTION,
    STYLE_PICKER,
    STYLE_PICKER2,
    TOONZ_RASTER_BRUSH,
    TOONZ_VECTOR_BRUSH,
    TRACKER,
    TYPE_TOOL,
    VECTOR_ERASER,
    VECTOR_SELECTION,
    VECTOR_TAPE,
    VECTOR_TAPE2,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_stubs_error_and_cite_existing_path() {
        assert!(tracker_tool().is_err() && render_farm().is_err() && cleanup().is_err());
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for f in ALL {
            assert!(root.join(f.original).exists(), "{}", f.original);
            assert!(
                NotYetImplemented(f)
                    .to_string()
                    .contains("not yet re-implemented")
            );
        }
    }
}
