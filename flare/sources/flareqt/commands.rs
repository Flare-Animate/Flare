//! The command registry: every scriptable action has a stable dotted id and
//! JSON arguments. JavaScript, Python and any other language reach the editor
//! only through `execute`, so they all get the same capabilities, and adding
//! a command here makes it available everywhere at once.
//!
//! Conventions: frame and layer numbers are **1-based**, as shown in the
//! timeline. Colours are `"#RRGGBB"`, `"#RRGGBBAA"` or `null` for none.
//! Drawing commands draw on the current layer at the current frame.

use crate::editor::{Editor, FrameSel, Selection};
use crate::geom::{pt, Matrix, Path, Pt, Seg};
use crate::model::*;
use serde_json::{json, Value};

pub struct CommandInfo {
    pub id: &'static str,
    pub args: &'static str,
    pub summary: &'static str,
}

macro_rules! cmds {
    ($( $id:literal, $args:literal, $summary:literal; )*) => {
        pub const COMMANDS: &[CommandInfo] = &[ $( CommandInfo { id: $id, args: $args, summary: $summary } ),* ];
    };
}

cmds! {
    "doc.info", "", "Document size, fps, frame count, layers, library and current position";
    "doc.set", "width?, height?, fps?, background?, perspective_angle?, vanishing_x?, vanishing_y?", "Change document settings (incl. 3D perspective)";
    "timeline.goto", "frame", "Move the playhead";
    "timeline.select", "layer?, frame?, to_layer?, to_frame?", "Select frames (defaults: current layer/frame)";
    "timeline.insert_frames", "frames? (1), layer?, frame?", "F5: insert frames";
    "timeline.remove_frames", "layer?, frame?, to_frame?", "Shift+F5: remove frames";
    "timeline.insert_keyframe", "layer?, frame?", "F6: insert keyframe (copies content)";
    "timeline.insert_blank_keyframe", "layer?, frame?", "F7: insert blank keyframe";
    "timeline.clear_keyframe", "layer?, frame?", "Shift+F6: clear keyframe";
    "timeline.clear_frames", "layer?, frame?, to_frame?", "Clear frames (make them empty)";
    "timeline.set_tween", "kind ('classic'|'shape'|'motion'|'none'), ease? (-100..100), rotate?, orient? (motion guide / path), layer?, frame?", "Create or remove a tween on the span at the target";
    "timeline.set_ease", "ease (preset e.g. 'Quad In', 'Bounce Out', 'Back In Out'; 'none' = simple ease; or points [[x,y,in_dx,in_dy,out_dx,out_dy]...]), layer?, frame?", "Custom ease curve or preset for the tween span at the target";
    "timeline.reverse_frames", "layer?, frame?, to_layer?, to_frame?", "Reverse the selected frames (tweens play backwards)";
    "timeline.to_frame_by_frame", "layer?, frame?, to_frame?", "Convert tweens in the range to one keyframe per frame";
    "timeline.set_layer_end", "end (last frame), layer?", "Lengthen/shorten a layer's last span (motion tween keys stretch)";
    "timeline.set_span_end", "end (new last frame of the span), layer?, frame? (any frame in the span)", "Lengthen/shorten any span; later keyframes move with it";
    "shape_hint.add", "x?, y? (start keyframe), to_x?, to_y? (end keyframe)", "Add a shape hint to the shape tween at the playhead; returns its letter";
    "shape_hint.clear", "", "Remove all shape hints from the shape tween at the playhead";
    "motion.info", "layer?, frame?", "The motion tween at the target: span, pivot and property keyframes";
    "motion.set_key", "prop ('x'|'y'|'rotation'|'skew'|'scale_x'|'scale_y'|'alpha'|'brightness'|'rotation_x'|'rotation_y'|'depth'|'tint'|'filterN.blur_x|blur_y|strength|angle|distance|brightness|contrast|saturation|hue'), frame, value, layer?", "Property keyframe in the motion tween containing frame (scale/alpha in 0..1, angles in degrees)";
    "motion.remove_key", "frame, prop? (all), layer?", "Remove property keyframe(s)";
    "motion.set_ease", "prop ('position' or a property), ease (preset name, 'custom', or 'none'), layer?, frame?", "Ease one property of the motion tween (Motion Editor)";
    "motion.set_interp", "prop, key (frame of the key starting the segment, 1-based), type ('auto'|'constant'|'linear'|'ease_in_out'|'ease_in_out_pct'|'exponential'|'expression'|'similar_shape'), ease_in?, ease_out? (frames or %), expression? (OpenToonz syntax, displayed units: degrees, %, px), offset? (similar shape), layer?", "How a property curve runs from a key to the next (OpenToonz keyframe types)";
    "timeline.set_label", "label, type? ('name'|'comment'|'anchor'), layer?, frame?", "Frame label on the keyframe at the target";
    "timeline.set_tween_options", "sync?, snap?, layer?, frame?", "Classic tween Sync (graphics keep counting across keyframes) and Snap (to the motion guide)";
    "timeline.set_property_ease", "property ('position'|'rotation'|'scale'|'color'|'filters'), ease? (preset name; omit = use the span's ease), layer?, frame?", "Classic tween: ease one property group separately";
    "timeline.set_script", "script, layer?, frame?", "Frame script (JavaScript) on the keyframe at the target";
    "timeline.move_frames", "by, layer?, frame?, to_frame?", "Drag frames left/right";
    "timeline.copy_frames", "layer?, frame?, to_layer?, to_frame?", "Copy Frames (Ctrl+Alt+C)";
    "timeline.cut_frames", "layer?, frame?, to_layer?, to_frame?", "Cut Frames (Ctrl+Alt+X)";
    "timeline.paste_frames", "layer?, frame?, overwrite? (false: the copied frames replace the selection and later frames move; true: Paste and Overwrite Frames)", "Paste Frames (Ctrl+Alt+V) at the target";
    "layer.list", "", "All layers with flags and frame counts";
    "layer.add", "name?", "New layer above the current one; returns its number";
    "layer.delete", "layer?", "Delete a layer";
    "layer.copy", "layer?, to_layer? (copies the rows layer..to_layer)", "Copy Layers: the layers (and everything in their folders) with all frames";
    "layer.cut", "layer?, to_layer?", "Cut Layers";
    "layer.paste", "layer? (pasted above it)", "Paste Layers: copied layers keep their folders, masks and parenting";
    "layer.select", "layer", "Make a layer current (number or name)";
    "layer.set", "layer?, name?, visible?, locked?, outline?, kind? ('normal'|'folder'|'mask'|'guide'), parent? (layer or 0 = none)", "Rename, change flags, type, or the folder/mask/guide a layer belongs to";
    "layer.set_parent", "parent (layer or 0 = none), layer?", "Layer parenting: the layer's objects follow the parent layer's first object";
    "scene.list", "", "Scenes in playing order, with frame counts; marks the current one";
    "scene.add", "name?", "Insert a scene after the current one and switch to it";
    "scene.goto", "scene (number or name)", "Edit another scene";
    "scene.rename", "name, scene?", "Rename a scene";
    "scene.delete", "scene?", "Delete a scene (not the last one)";
    "scene.move", "scene, to", "Reorder scenes";
    "layer.wrap", "kind ('folder'|'mask'|'guide'), name?, layer?", "Insert a folder / mask / motion guide above a layer and put the layer in it";
    "draw.rect", "x, y, w, h, radius?, fill?, stroke?, stroke_width?", "Rectangle";
    "draw.oval", "x, y, w, h, fill?, stroke?, stroke_width?", "Oval inside a box";
    "draw.line", "x1, y1, x2, y2, stroke?, stroke_width?", "Straight line";
    "draw.polystar", "cx, cy, r, sides? (5), star? (false), inner? (0.5), fill?, stroke?", "Polygon or star";
    "draw.path", "points [[x,y]...], closed?, smooth?, fill?, stroke?, stroke_width?, assist? (apply drawing assistants: snap + symmetry)", "Path through points";
    "assistant.add", "type ('line'|'vanishing_point'|'ellipse'|'perspective'|'fisheye'|'mirror'|'star'|'grid'|'affine'|'jitter'), x, y, size? (100), count? (star, affine), count_inv? (affine), mirror? (star), parallel? (line), concentric? (ellipse), three_point? (perspective), period?, amplitude?, keep_first?, keep_last? (jitter), magnetism? (0..1)", "Add a drawing assistant (OpenToonz): guides snap freehand strokes, replicators repeat them, jitter shakes them; returns its index (1-based)";
    "assistant.list", "", "Drawing assistants with their points";
    "draw.particles", "x, y, then any emitter settings: source ('dot'|'square'|'star' or {symbol: name} or {bitmap: name}), rate (/s), life (s), life_var (0..1), speed, speed_var, direction (degrees, -90 = up), spread, gravity [x, y], drag (0..1), spin (°/s), spin_var, random_angle, scale_start, scale_end (%), scale_var, color_start, color_end, tint, fade_in, fade_out (fractions of life), area [w, h], prewarm, duration (s, 0 = forever), max, seed", "Place a particle emitter on the current layer (particles start at its keyframe); returns its selection";
    "selection.particles", "any draw.particles setting", "Change the selected particle emitters";
    "assistant.set", "index (1-based), enabled?, magnetism?, points? [[x,y]...] (perspective: vx, vy[, vz]; affine: c0, a0, b0, c1, a1, b1), count?, count_inv?, mirror?, parallel?, concentric?, three_point?, count_a?, count_b?, count_a_inv?, count_b_inv?, period?, amplitude?, keep_first?, keep_last?", "Change a drawing assistant";
    "assistant.remove", "index (1-based) or 'all'", "Delete drawing assistants";
    "draw.text", "x, y, text, size? (24), color?, font? ('sans'|'mono'|system family), align?, weight? (100–900), bold?, italic?", "Static text";
    "select.all", "", "Select everything editable on the current frame";
    "select.none", "", "Clear the selection";
    "select.layer", "layer?", "Select every object on one layer (default: current) at the current frame";
    "select.at", "x, y, add?", "Select the topmost object at a point";
    "select.rect", "x, y, w, h, add?", "Select objects fully inside a box";
    "select.element", "layer, index (0-based, bottom first), add?", "Select one object by layer and stacking index";
    "select.magic_wand", "x, y, threshold? (20), add?", "Magic Wand: select the similar-coloured part of a broken-apart bitmap";
    "frame.info", "layer?, frame?", "The keyframe span at the target: start, duration, label, tween, script, sound and its objects";
    "symbol.new", "name, kind? ('graphic'|'movieclip'|'button')", "Ctrl+F8: new empty symbol, opened for editing; returns its id";
    "selection.info", "", "Selected objects with kind, name and bounds";
    "selection.move", "dx, dy", "Move the selection";
    "selection.set_position", "x, y", "Move the selection's top-left to x, y";
    "selection.scale", "sx, sy?", "Scale around the selection centre";
    "selection.rotate", "degrees", "Rotate around the selection centre";
    "selection.set", "fill?, stroke?, stroke_width?, alpha?, tint?, tint_amount?, name?, text?, size?, weight? (100–900), bold?, italic?", "Change properties of every selected object";
    "selection.delete", "", "Delete the selection";
    "selection.duplicate", "", "Duplicate (offset by 10 px)";
    "selection.to_symbol", "name, kind? ('graphic'|'movieclip'|'button')", "F8: convert to symbol; returns its id";
    "selection.break_apart", "", "Ctrl+B on a symbol instance";
    "selection.skew", "h? (deg), v? (deg)", "Skew horizontally / vertically around the selection centre";
    "selection.set_3d", "rx?, ry?, rz? (deg), z?", "3D rotation and depth (perspective-projected)";
    "selection.distort", "corners [[x,y] x4: TL, TR, BR, BL]", "Distort shapes onto a quadrilateral";
    "selection.set_gradient", "kind ('linear'|'radial'|'none'), colors [\"#RRGGBB\"...], flow? ('extend'|'reflect'|'repeat')", "Gradient fill on selected shapes";
    "bone.add", "x1, y1, x2, y2", "Bone from the object at (x1,y1) to the object at (x2,y2) on the current layer";
    "bone.list", "layer?", "Bones on a layer";
    "bone.clear", "layer?", "Remove all bones on a layer";
    "bone.drag", "x, y, to_x, to_y", "IK: drag the bonded object point at (x,y) to (to_x,to_y), posing the chain";
    "bone.joint", "x, y (an object attached by a bone), rotate? (bool), min?, max? (degrees from the pose when the bone was made; omit both or free:true = no limit), free?, pin? (bool: the joint stays where it is while posing)", "Joint constraints: whether the joint turns, how far, and pinning";
    "camera.add", "", "Add a camera layer to the scene";
    "camera.set", "x?, y?, zoom?, rotation?, frame?", "Camera centre, zoom (1 = 100%) and rotation at a frame (makes a keyframe)";
    "camera.remove", "", "Remove the camera layer";
    "camera.mode", "mode ('stage'|'free')", "Stage: the stage is the camera frame (Animate). Free: a VCam frame you move around the scene";
    "camera.options", "shake? (px), shake_speed? (per second), follow? (instance name; '' = none), follow_lag? (frames), offset_x?, offset_y?, frame?", "Shake and follow for the camera span at the target frame";
    "camera.effect", "brightness? (-100..100), tint?, tint_amount? (0..100), alpha? (0..100), blur? (px), frame?", "Colour effect / blur on everything the camera sees (makes a camera keyframe)";
    "selection.arrange", "front (bool)", "Bring to front / send to back";
    "drawing.mode", "merge (bool)", "Merge Drawing on: draw.* commands cut and merge into the layer's shape instead of adding drawing objects";
    "merge.select", "x, y, connected? (double-click), add?", "Select the fill or line of the merge shape at a point (it floats until deselected)";
    "merge.select_rect", "x, y, w, h, add?", "Marquee over merge shapes: cuts out and selects what is inside";
    "merge.deselect", "", "Merge floating selections back into their shapes";
    "merge.erase", "x, y, w, h, mode? ('normal'|'fills'|'lines'|'selection'|'inside')", "Eraser over a rectangle on the current layer's merge shape";
    "merge.bucket", "x, y, color?, gap? (px)", "Paint Bucket on the merge shape: recolour a fill or fill the area lines enclose";
    "merge.to_object", "", "Turn the floating merge selection into a drawing object";
    "selection.combine", "op ('union'|'intersect'|'punch'|'crop')", "Modify ▸ Combine Objects on the selected drawing objects";
    "selection.shape", "op ('smooth'|'straighten'|'optimize')", "Modify ▸ Shape cleanup on drawing objects or a merge selection";
    "selection.stroke_style", "dash? ('solid'|'dashed'|'dotted'|'ragged'|'stipple'|'hatched'|OpenToonz: 'herringbone'|'chain'|'circlets'|'scratchy'|'rope'|'tulle'|'plait'|'fuzz'|'bubbles'|'jagged'|'zigzag'|'wave'|'curl'), cap? ('none'|'round'|'square'), join? ('miter'|'round'|'bevel'), dash_len?, gap?, params? ({name: value}, e.g. {\"tilt\": 0.8}), colors? ([\"#RRGGBB\"|null, …] extra colours)", "Stroke style of selected drawing objects";
    "paint.brushes", "", "The bundled MyPaint brushes (id, name, group)";
    "paint.stroke", "points [[x,y] or [x,y,pressure]...], brush? (id or name, default 'classic_pencil'), color?, size? (radius px), opacity? (0..1), eraser?", "Raster Brush: paint a stroke with a MyPaint brush into the bitmap under it (or a new one on the current layer)";
    "selection.mesh_warp", "pins [[x, y, to_x, to_y], …] (stage coordinates), reset? (bool)", "Asset Warp on the selected bitmap, symbol or group: each pin takes the point under (x, y) to (to_x, to_y); reset removes the warp";
    "shape.deform", "tool ('magnet'|'pinch'|'pump'|'bender'|'iron'), x, y, to_x, to_y, size? (px, default 40), sharp? (pinch: corner), strength? (iron 0..1); bender: x, y, x2, y2 = the segment across the lines, from_x, from_y → to_x, to_y = the drag", "OpenToonz deformation tools on drawing objects and merge shapes (one drag); returns how many objects changed";
    "selection.bitmap_fill", "bitmap (name or id, null = none)", "Fill selected drawing objects, or the merge-shape selection, with a library bitmap";
    "draw.rect_primitive", "x, y, w, h, radius? or radii [tl,tr,br,bl], fill?, stroke?", "Rectangle Primitive (corner radii stay editable)";
    "draw.oval_primitive", "x, y, w, h, start?, end?, inner? (%), fill?, stroke?", "Oval Primitive (angles and inner radius stay editable)";
    "shape_bone.add", "x1, y1, x2, y2", "Bone inside the drawing object at (x1,y1), from (x1,y1) to (x2,y2); chains onto a bone ending at (x1,y1)";
    "shape_bone.joint", "x, y (near the bone's tail), rotate? (bool), min?, max? (degrees from rest), free?, pin? (bool: the tail stays put while posing)", "Joint constraints of a shape bone";
    "shape_bone.pose", "x, y, to_x, to_y", "Drag the shape bone whose tail is at (x,y) to (to_x,to_y) (IK)";
    "selection.set_looping", "mode? ('loop'|'play_once'|'single_frame'|'play_reverse_once'|'loop_reverse'), first_frame? (1-based)", "Graphic instance looping options";
    "selection.swap_symbol", "symbol (name or id)", "Swap Symbol: selected instances show another symbol, keeping transform and effects";
    "selection.color_effect", "style ('none'|'brightness'|'tint'|'advanced'|'alpha'), amount? (-100..100 or 0..100), color?, mul? [r,g,b,a %], add? [r,g,b,a]", "Color effect of the selected objects";
    "selection.group", "", "Ctrl+G: group the selection (per layer)";
    "selection.ungroup", "", "Ctrl+Shift+G: ungroup";
    "selection.align", "op ('left'|'hcenter'|'right'|'top'|'vcenter'|'bottom'|'dist_left'|'dist_hcenter'|'dist_right'|'dist_top'|'dist_vcenter'|'dist_bottom'|'match_width'|'match_height'|'match_size'|'space_h'|'space_v'), to_stage?", "Align panel";
    "selection.add_filter", "type ('drop_shadow'|'blur'|'glow'|'bevel'|'gradient_glow'|'gradient_bevel'|'adjust_color'|'motion_blur'|'effect'), …filter fields (blur_x, blur_y, strength, angle, distance, color, kind, stops [[pos, '#RRGGBBAA']…], brightness, contrast, saturation, hue, shutter (degrees), samples…); effect: id (EffectCraft id, e.g. 'ec.blur.radial'; `animationcraft-cli effects` lists them), params {id: number | bool | [x, y] (fraction of the object) | '#RRGGBBAA'}", "Add a filter to the selected objects";
    "selection.clear_filters", "", "Remove all filters";
    "bitmap.trace", "color_threshold? (0–500, 100), min_area? (px, 8), curve_fit? ('pixels'|'very tight'|'tight'|'normal'|'smooth'|'very smooth'), corners? ('many'|'normal'|'few'), mode? ('outline'|'centerline'), line_threshold? (centerline ink darkness 1–254, 128), uniform_width? (centerline)", "Trace Bitmap: the selected bitmaps become vector fills (outline) or lines (centerline, OpenToonz) in the layer's shape; returns how many fills/lines";
    "selection.set_blend", "mode ('normal'|'multiply'|'screen'|…)", "Blend mode";
    "import.image", "path (PNG, JPEG, GIF, BMP, TGA, TIFF, WebP, OpenEXR), x?, y?, place? (true)", "Import an image into the library (and place it)";
    "import.xdts", "path (.xdts)", "Import an XDTS timing sheet: tracks named like layers re-time them (drawing n = the layer's n-th drawing), others load numbered images next to the sheet (name_0001.png or name/0001.png) or become labelled blank keyframes; returns a summary";
    "import.sound", "path (WAV/MP3/OGG/FLAC), name?", "Import a sound into the library; returns its id";
    "timeline.set_sound", "sound (name or id; null = none), sync? ('event'|'start'|'stop'|'stream'), repeat? (1), loop? (false), effect? ('none'|'left channel'|'right channel'|'fade to right'|'fade to left'|'fade in'|'fade out'), layer?, frame?", "Properties ▸ Sound on the keyframe at the target";
    "sound.list", "", "Library sounds with length (seconds, when this build can decode them)";
    "sound.delete", "sound (name or id)", "Delete a library sound (and its keyframe uses)";
    "lipsync.apply", "audio_layer, map? {\"Neutral\": frame, \"MBP\": frame, \"Ai\", \"E\", \"O\", \"U\", \"FV\", \"L\", \"WQ\", \"etc\"} (1-based frames of the mouth symbol)", "Auto Lip-Sync the selected mouth instance to the sound on audio_layer; returns keyframes made";
    "library.list", "", "Library items";
    "library.place", "symbol (name or id), x, y", "Place an instance on the current layer";
    "library.rename", "symbol, name", "Rename a library item";
    "library.delete", "symbol", "Delete a library item and its instances";
    "symbol.edit", "symbol", "Enter a symbol's timeline";
    "symbol.exit", "", "Return to the main scene";
    "group.edit", "", "Edit the selected group (its objects open like a symbol; group.exit closes it)";
    "group.exit", "", "Close the innermost group being edited";
    "edit.undo", "", "Undo";
    "edit.redo", "", "Redo";
}

pub fn find(id: &str) -> Option<&'static CommandInfo> {
    COMMANDS.iter().find(|c| c.id == id)
}

// ------------------------------------------------------------------ argument helpers

fn num(a: &Value, k: &str) -> Option<f32> {
    a.get(k).and_then(Value::as_f64).map(|v| v as f32)
}
fn req(a: &Value, k: &str) -> Result<f32, String> {
    num(a, k).ok_or_else(|| format!("missing number '{k}'"))
}
fn string<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}
fn boolean(a: &Value, k: &str) -> Option<bool> {
    a.get(k).and_then(Value::as_bool)
}

/// `"#RRGGBB"`, `"#RRGGBBAA"`, `[r,g,b(,a)]`, or null (= none).
/// `min` / `max` / `free` of the joint commands (degrees).
fn joint_limit(a: &Value, cur: Option<(f32, f32)>) -> Result<Option<(f32, f32)>, String> {
    if boolean(a, "free").unwrap_or(false) {
        return Ok(None);
    }
    match (num(a, "min"), num(a, "max")) {
        (None, None) => Ok(cur),
        (lo, hi) => {
            let (c0, c1) = cur.unwrap_or((-180.0, 180.0));
            let (lo, hi) = (lo.unwrap_or(c0), hi.unwrap_or(c1));
            if lo > 0.0 || hi < 0.0 {
                return Err("the range must include the current pose: min ≤ 0 ≤ max".into());
            }
            Ok(Some((lo, hi)))
        }
    }
}

/// Type-specific options of `assistant.add` / `assistant.set`.
/// The `source` argument of the particle commands, resolved against the library.
fn particle_source(ed: &Editor, a: &Value) -> Result<Option<crate::particles::ParticleSource>, String> {
    use crate::particles::ParticleSource as Src;
    let Some(src) = a.get("source") else { return Ok(None) };
    Ok(Some(match src {
        Value::String(s) => match s.to_ascii_lowercase().as_str() {
            "dot" | "circle" => Src::Dot,
            "square" => Src::Square,
            "star" => Src::Star,
            other => return Err(format!("unknown particle source '{other}' (dot, square, star, {{symbol}}, {{bitmap}})")),
        },
        Value::Object(o) => {
            if let Some(name) = o.get("symbol") {
                let id = match name {
                    Value::Number(n) => n.as_u64().unwrap_or(0) as crate::model::SymbolId,
                    Value::String(n) => ed.doc.library.iter().find(|s| &s.name == n).map(|s| s.id).ok_or_else(|| format!("no symbol '{n}'"))?,
                    _ => return Err("symbol: name or id".into()),
                };
                Src::Symbol { symbol: id }
            } else if let Some(name) = o.get("bitmap") {
                let id = match name {
                    Value::Number(n) => n.as_u64().unwrap_or(0) as crate::bitmap::BitmapId,
                    Value::String(n) => ed.doc.bitmaps.iter().find(|b| &b.name == n).map(|b| b.id).ok_or_else(|| format!("no bitmap '{n}'"))?,
                    _ => return Err("bitmap: name or id".into()),
                };
                Src::Bitmap { bitmap: id }
            } else {
                return Err("source: 'dot', 'square', 'star', {symbol: name} or {bitmap: name}".into());
            }
        }
        _ => return Err("source: 'dot', 'square', 'star', {symbol: name} or {bitmap: name}".into()),
    }))
}

/// Apply `draw.particles` settings to an emitter (unknown keys are ignored).
fn particle_settings(e: &mut crate::particles::Emitter, a: &Value, src: Option<crate::particles::ParticleSource>) -> Result<(), String> {
    if let Some(s) = src {
        e.source = s;
    }
    let f = |k: &str, v: &mut f32| {
        if let Some(x) = num(a, k) {
            *v = x;
        }
    };
    f("rate", &mut e.rate);
    f("life", &mut e.life);
    f("life_var", &mut e.life_var);
    f("speed", &mut e.speed);
    f("speed_var", &mut e.speed_var);
    f("direction", &mut e.direction);
    f("spread", &mut e.spread);
    f("drag", &mut e.drag);
    f("spin", &mut e.spin);
    f("spin_var", &mut e.spin_var);
    f("scale_start", &mut e.scale_start);
    f("scale_end", &mut e.scale_end);
    f("scale_var", &mut e.scale_var);
    f("fade_in", &mut e.fade_in);
    f("fade_out", &mut e.fade_out);
    f("duration", &mut e.duration);
    let pair = |k: &str| a.get(k).and_then(Value::as_array).and_then(|v| Some(pt(v.first()?.as_f64()? as f32, v.get(1)?.as_f64()? as f32)));
    if let Some(g) = pair("gravity") {
        e.gravity = g;
    }
    if let Some(g) = pair("area") {
        e.area = g;
    }
    for (k, v) in [("random_angle", &mut e.random_angle), ("tint", &mut e.tint), ("prewarm", &mut e.prewarm)] {
        if let Some(b) = boolean(a, k) {
            *v = b;
        }
    }
    if let Some(c) = color_arg(a, "color_start")?.flatten() {
        e.color_start = c;
    }
    if let Some(c) = color_arg(a, "color_end")?.flatten() {
        e.color_end = c;
    }
    if let Some(x) = num(a, "max") {
        e.max = x.clamp(1.0, 20000.0) as u32;
    }
    if let Some(x) = num(a, "seed") {
        e.seed = x.max(0.0) as u32;
    }
    Ok(())
}

fn assistant_options(kind: &mut crate::assist::AssistantKind, a: &Value) {
    use crate::assist::AssistantKind as K;
    let int = |k: &str| num(a, k).map(|v| v.clamp(0.0, 64.0) as u32);
    match kind {
        K::Line { parallel, .. } => {
            if let Some(v) = boolean(a, "parallel") {
                *parallel = v;
            }
        }
        K::Ellipse { concentric, .. } => {
            if let Some(v) = boolean(a, "concentric") {
                *concentric = v;
            }
        }
        K::Star { count, mirror, .. } => {
            if let Some(v) = int("count") {
                *count = v.max(1);
            }
            if let Some(v) = boolean(a, "mirror") {
                *mirror = v;
            }
        }
        K::Grid { count_a, count_a_inv, count_b, count_b_inv, .. } => {
            for (k, f) in [("count_a", count_a), ("count_a_inv", count_a_inv), ("count_b", count_b), ("count_b_inv", count_b_inv)] {
                if let Some(v) = int(k) {
                    *f = v;
                }
            }
        }
        K::Perspective { three_point, .. } => {
            if let Some(v) = boolean(a, "three_point") {
                *three_point = v;
            }
        }
        K::Affine { count, count_inv, .. } => {
            for (k, f) in [("count", count), ("count_inv", count_inv)] {
                if let Some(v) = int(k) {
                    *f = v;
                }
            }
        }
        K::Jitter { period, amplitude, keep_first, keep_last, .. } => {
            if let Some(v) = num(a, "period") {
                *period = v.clamp(0.0, 1000.0);
            }
            if let Some(v) = num(a, "amplitude") {
                *amplitude = v.clamp(0.0, 1000.0);
            }
            if let Some(v) = boolean(a, "keep_first") {
                *keep_first = v;
            }
            if let Some(v) = boolean(a, "keep_last") {
                *keep_last = v;
            }
        }
        _ => {}
    }
}

fn color_arg(a: &Value, k: &str) -> Result<Option<Option<Color>>, String> {
    match a.get(k) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(s)) => Color::from_hex(s).map(|c| Some(Some(c))).ok_or_else(|| format!("bad colour '{s}' for '{k}' (use \"#RRGGBB\")")),
        Some(Value::Array(v)) if v.len() >= 3 => {
            let c = |i: usize| v.get(i).and_then(Value::as_f64).unwrap_or(255.0).clamp(0.0, 255.0) as u8;
            Ok(Some(Some(Color { r: c(0), g: c(1), b: c(2), a: c(3) })))
        }
        Some(other) => Err(format!("bad colour {other} for '{k}'")),
    }
}

fn hex(c: Color) -> String {
    if c.a == 255 { c.to_hex() } else { format!("{}{:02X}", c.to_hex(), c.a) }
}

/// Layer by 1-based number or by name.
fn layer_arg(ed: &Editor, a: &Value, k: &str) -> Result<Option<usize>, String> {
    let n = ed.timeline().layers.len();
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(v)) => {
            let i = v.as_u64().unwrap_or(0) as usize;
            if i == 0 || i > n {
                return Err(format!("layer {i} out of range (1..={n})"));
            }
            Ok(Some(i - 1))
        }
        Some(Value::String(s)) => ed.timeline().layers.iter().position(|l| l.name == *s).map(Some).ok_or_else(|| format!("no layer named '{s}'")),
        Some(other) => Err(format!("bad layer {other}")),
    }
}

fn frame_arg(a: &Value, k: &str) -> Result<Option<u32>, String> {
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let f = v.as_u64().ok_or_else(|| format!("'{k}' must be a frame number"))?;
            if f == 0 {
                return Err(format!("'{k}': frames are numbered from 1"));
            }
            Ok(Some(f as u32 - 1))
        }
    }
}

/// Apply optional layer/frame/to_layer/to_frame args as the frame target.
fn target(ed: &mut Editor, a: &Value) -> Result<(), String> {
    let l = layer_arg(ed, a, "layer")?;
    let f = frame_arg(a, "frame")?;
    let tl = layer_arg(ed, a, "to_layer")?;
    let tf = frame_arg(a, "to_frame")?;
    if l.is_none() && f.is_none() && tl.is_none() && tf.is_none() {
        return Ok(());
    }
    let l0 = l.unwrap_or(ed.layer);
    let f0 = f.unwrap_or(ed.frame);
    ed.select_frames(FrameSel::between((l0, f0), (tl.unwrap_or(l0), tf.unwrap_or(f0))));
    Ok(())
}

fn symbol_arg(ed: &Editor, a: &Value) -> Result<SymbolId, String> {
    match a.get("symbol") {
        Some(Value::Number(n)) => {
            let id = n.as_u64().unwrap_or(0) as SymbolId;
            ed.doc.symbol(id).map(|_| id).ok_or_else(|| format!("no symbol with id {id}"))
        }
        Some(Value::String(s)) => ed.doc.library.iter().find(|x| x.name == *s).map(|x| x.id).ok_or_else(|| format!("no library item named '{s}'")),
        _ => Err("missing 'symbol' (name or id)".into()),
    }
}

fn sound_arg(ed: &Editor, v: &Value) -> Result<crate::sound::SoundId, String> {
    match v {
        Value::Number(n) => {
            let id = n.as_u64().unwrap_or(0) as crate::sound::SoundId;
            ed.doc.sound(id).map(|_| id).ok_or_else(|| format!("no sound with id {id}"))
        }
        Value::String(s) => ed.doc.sounds.iter().find(|x| x.name == *s || x.name.rsplit_once('.').map(|(b, _)| b) == Some(s.as_str())).map(|x| x.id).ok_or_else(|| format!("no sound named '{s}'")),
        _ => Err("sound must be a name or id".into()),
    }
}

fn shape_style(a: &Value, default_fill: Option<Color>) -> Result<(Option<Color>, Option<Stroke>), String> {
    let fill = color_arg(a, "fill")?.unwrap_or(default_fill);
    let width = num(a, "stroke_width").unwrap_or(1.0);
    let stroke = color_arg(a, "stroke")?.unwrap_or(None).map(|c| Stroke { color: c, width });
    Ok((fill, stroke))
}

fn add_shape(ed: &mut Editor, name: &str, path: Path, fill: Option<Color>, stroke: Option<Stroke>) -> Result<Value, String> {
    if fill.is_none() && stroke.is_none() {
        return Err("shape needs a fill or a stroke".into());
    }
    if ed.script_merge && name != "Text" {
        if !ed.merge_draw(&path, fill, None, stroke) {
            return Err("current layer is locked or hidden".into());
        }
        return Ok(json!({ "layer": ed.layer + 1, "merged": true }));
    }
    let s = ed.add_element(Element::shape(name, Shape { path, fill, stroke, ..Default::default() })).ok_or("current layer is locked or hidden")?;
    Ok(json!({ "layer": s.layer + 1, "index": s.element }))
}

fn element_json(ed: &Editor, s: Selection) -> Value {
    let Some(el) = ed.element_at(s) else { return Value::Null };
    let (kind, extra) = match &el.kind {
        ElementKind::Shape(sh) => ("shape", json!({ "fill": sh.fill.map(hex), "stroke": sh.stroke.map(|s| hex(s.color)), "stroke_width": sh.stroke.map(|s| s.width) })),
        ElementKind::Instance(i) => ("instance", json!({ "symbol": ed.doc.symbol(i.symbol).map(|s| s.name.clone()) })),
        ElementKind::Text(t) => ("text", json!({ "text": t.text, "size": t.size, "color": hex(t.color), "font": t.family })),
        ElementKind::Group(c) => ("group", json!({ "children": c.len() })),
        ElementKind::Bitmap(b) => ("bitmap", json!({ "bitmap": ed.doc.bitmap(b.bitmap).map(|a| a.name.clone()) })),
        ElementKind::Canvas(c) => ("merge_shape", json!({ "fills": c.fills.len(), "strokes": c.strokes.len() })),
        ElementKind::Particles(e) => ("particles", serde_json::to_value(e.as_ref()).unwrap_or(Value::Null)),
    };
    let b = ed.bounds_of(&[s]);
    json!({
        "layer": s.layer + 1, "index": s.element, "name": el.name, "kind": kind, "alpha": el.alpha,
        "x": b.map(|b| b.0.x), "y": b.map(|b| b.0.y),
        "width": b.map(|b| b.1.x - b.0.x), "height": b.map(|b| b.1.y - b.0.y),
        "props": extra,
    })
}

fn center(ed: &Editor) -> Result<Pt, String> {
    let (lo, hi) = ed.selection_bounds().ok_or("nothing selected")?;
    Ok(pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
}

fn need_selection(ed: &Editor) -> Result<(), String> {
    if ed.selection.is_empty() {
        Err("nothing selected (use select.at, select.rect or select.all first)".into())
    } else {
        Ok(())
    }
}

// ------------------------------------------------------------------ dispatch

pub fn execute(ed: &mut Editor, id: &str, a: &Value) -> Result<Value, String> {
    let a = if a.is_null() { &json!({}) } else { a };
    match id {
        "doc.info" => {
            let tl = ed.timeline();
            Ok(json!({
                "width": ed.doc.width, "height": ed.doc.height, "fps": ed.doc.fps,
                "background": hex(ed.doc.background),
                "frames": tl.frame_count(),
                "movie_frames": ed.doc.total_frames(),
                "scene": ed.scene_tl().name, "scenes": ed.doc.scene_count(),
                "current_frame": ed.frame + 1, "current_layer": ed.layer + 1,
                "editing_symbol": ed.editing.and_then(|i| ed.doc.symbol(i)).map(|s| s.name.clone()),
                "layers": tl.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>(),
                "library": ed.doc.library.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            }))
        }
        "doc.set" => {
            let d = &ed.doc;
            let bg = color_arg(a, "background")?.flatten().unwrap_or(d.background);
            let (w, h, fps) = (num(a, "width").unwrap_or(d.width), num(a, "height").unwrap_or(d.height), num(a, "fps").unwrap_or(d.fps));
            if w < 1.0 || h < 1.0 || !(0.1..=240.0).contains(&fps) {
                return Err("width/height must be ≥ 1 and fps 0.1–240".into());
            }
            let pa = num(a, "perspective_angle");
            let vx = num(a, "vanishing_x");
            let vy = num(a, "vanishing_y");
            ed.set_doc_props(w, h, fps, bg);
            if let Some(v) = pa {
                ed.doc.perspective_angle = v.clamp(1.0, 179.0);
            }
            if vx.is_some() || vy.is_some() {
                let cur = ed.doc.perspective().vanishing;
                ed.doc.vanishing_point = Some(pt(vx.unwrap_or(cur.x), vy.unwrap_or(cur.y)));
            }
            Ok(Value::Null)
        }
        "timeline.goto" => {
            let f = frame_arg(a, "frame")?.ok_or("missing 'frame'")?;
            ed.set_frame(f);
            ed.frame_sel = None;
            Ok(Value::Null)
        }
        "timeline.select" => {
            target(ed, a)?;
            if ed.frame_sel.is_none() {
                ed.select_frames(FrameSel::cell(ed.layer, ed.frame));
            }
            Ok(Value::Null)
        }
        "timeline.insert_frames" => {
            target(ed, a)?;
            let n = num(a, "frames").unwrap_or(1.0).max(1.0) as u32;
            if n > 1 {
                let s = ed.frame_target();
                ed.select_frames(FrameSel { f1: s.f0 + n - 1, ..s });
            }
            ed.insert_frame();
            Ok(Value::Null)
        }
        "timeline.remove_frames" => {
            target(ed, a)?;
            ed.remove_frame();
            Ok(Value::Null)
        }
        "timeline.insert_keyframe" => {
            target(ed, a)?;
            ed.insert_keyframe();
            Ok(Value::Null)
        }
        "timeline.insert_blank_keyframe" => {
            target(ed, a)?;
            ed.insert_blank_keyframe();
            Ok(Value::Null)
        }
        "timeline.clear_keyframe" => {
            target(ed, a)?;
            ed.clear_keyframe();
            Ok(Value::Null)
        }
        "timeline.clear_frames" => {
            target(ed, a)?;
            ed.clear_frames();
            Ok(Value::Null)
        }
        "timeline.set_tween" => {
            target(ed, a)?;
            let ease = num(a, "ease").unwrap_or(0.0).clamp(-100.0, 100.0);
            let tween = match string(a, "kind").unwrap_or("classic") {
                "classic" => Tween::Classic { ease, rotate_turns: num(a, "rotate").unwrap_or(0.0) as i32 },
                "shape" => Tween::Shape { ease },
                "motion" => Tween::Motion { ease },
                "none" => Tween::None,
                k => return Err(format!("unknown tween kind '{k}' (classic, shape, motion, none)")),
            };
            ed.set_tween(tween);
            if let (Tween::Motion { .. }, Some(r)) = (tween, num(a, "rotate")) {
                ed.set_tween_params(ease, r as i32);
            }
            if let Some(o) = boolean(a, "orient") {
                ed.set_orient_to_path(o);
            }
            Ok(Value::Null)
        }
        "timeline.set_ease" => {
            target(ed, a)?;
            ed.set_ease_curve(parse_ease(a.get("ease"))?);
            Ok(Value::Null)
        }
        "layer.set_parent" => {
            let li = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            let p = match a.get("parent") {
                Some(Value::Number(n)) if n.as_u64() == Some(0) => None,
                None | Some(Value::Null) => None,
                _ => Some(layer_arg(ed, a, "parent")?.ok_or("bad parent")?),
            };
            if let Some(p) = p {
                if !ed.parent_candidates(li).contains(&p) {
                    return Err("that layer can't be the parent (same layer, not a normal layer, or it would loop)".into());
                }
            }
            ed.set_anim_parent(li, p);
            Ok(Value::Null)
        }
        "scene.list" => Ok(Value::Array(
            ed.doc.scenes().enumerate().map(|(i, t)| json!({ "scene": i + 1, "name": t.name, "frames": t.frame_count(), "current": i == ed.scene })).collect(),
        )),
        "scene.add" => {
            let i = ed.add_scene();
            if let Some(n) = string(a, "name") {
                ed.rename_scene(i, n);
            }
            Ok(json!(i + 1))
        }
        "scene.goto" => {
            let i = scene_arg(ed, a, "scene")?.ok_or("scene is required")?;
            ed.goto_scene(i);
            Ok(Value::Null)
        }
        "scene.rename" => {
            let i = scene_arg(ed, a, "scene")?.unwrap_or(ed.scene);
            ed.rename_scene(i, string(a, "name").ok_or("name is required")?);
            Ok(Value::Null)
        }
        "scene.delete" => {
            let i = scene_arg(ed, a, "scene")?.unwrap_or(ed.scene);
            if ed.doc.scene_count() < 2 {
                return Err("a document needs at least one scene".into());
            }
            ed.delete_scene(i);
            Ok(Value::Null)
        }
        "scene.move" => {
            let i = scene_arg(ed, a, "scene")?.ok_or("scene is required")?;
            let to = scene_arg(ed, a, "to")?.ok_or("to is required")?;
            ed.move_scene(i, to);
            Ok(Value::Null)
        }
        "timeline.reverse_frames" => {
            target(ed, a)?;
            ed.reverse_frames();
            Ok(Value::Null)
        }
        "timeline.to_frame_by_frame" => {
            target(ed, a)?;
            ed.convert_to_frame_by_frame();
            Ok(Value::Null)
        }
        "timeline.set_layer_end" => {
            let l = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            let e = frame_arg(a, "end")?.ok_or("end is required")?;
            ed.set_layer_end(l, e + 1, Some("Span Length"));
            Ok(Value::Null)
        }
        "timeline.set_span_end" => {
            let l = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            let f = frame_arg(a, "frame")?.unwrap_or(ed.frame);
            let e = frame_arg(a, "end")?.ok_or("end is required")?;
            let start = ed.timeline().layers.get(l).and_then(|x| x.span_at(f)).map(|(s, _, _)| s).ok_or("no span there")?;
            ed.set_span_end(l, start, e + 1);
            Ok(Value::Null)
        }
        "shape_hint.add" => {
            let (s0, s1) = ed.shape_tween_keys().ok_or("no shape tween at the playhead")?;
            let i = ed.add_shape_hint().ok_or("at most 26 shape hints")?;
            if let (Some(x), Some(y)) = (num(a, "x"), num(a, "y")) {
                ed.move_shape_hint(s0, i, pt(x, y), None);
            }
            if let (Some(x), Some(y)) = (num(a, "to_x"), num(a, "to_y")) {
                ed.move_shape_hint(s1, i, pt(x, y), None);
            }
            Ok(json!(((b'a' + i as u8) as char).to_string()))
        }
        "shape_hint.clear" => {
            ed.remove_all_shape_hints();
            Ok(Value::Null)
        }
        "motion.info" => {
            target(ed, a)?;
            let Some((st, len, m, _)) = ed.current_motion() else { return Err("no motion tween at the target".into()) };
            let mut props = serde_json::Map::new();
            for p in crate::motion::Prop::ALL.into_iter().chain(m.extra.iter().map(|(p, _)| *p)) {
                let keys: Vec<Value> = m.keys_of(p).into_iter().map(|(f, v)| json!([st + f + 1, v])).collect();
                if !keys.is_empty() {
                    props.insert(p.id(), Value::Array(keys));
                }
            }
            Ok(json!({ "start": st + 1, "frames": len, "pivot": [m.pivot.x, m.pivot.y], "keys": props, "rotate": m.rotate_turns }))
        }
        "motion.set_key" => {
            if let Some(l) = layer_arg(ed, a, "layer")? {
                ed.layer = l;
            }
            let p = prop_arg(a)?;
            let f = frame_arg(a, "frame")?.ok_or("frame is required")?;
            let v = num(a, "value").ok_or("value is required")?;
            if ed.motion_span(ed.layer, f).is_none() {
                return Err(format!("frame {} is not inside a motion tween", f + 1));
            }
            ed.set_motion_value(p, f, v, Some("Property Keyframe"));
            Ok(Value::Null)
        }
        "motion.remove_key" => {
            if let Some(l) = layer_arg(ed, a, "layer")? {
                ed.layer = l;
            }
            let f = frame_arg(a, "frame")?.ok_or("frame is required")?;
            let Some((st, _)) = ed.motion_span(ed.layer, f) else { return Err(format!("frame {} is not inside a motion tween", f + 1)) };
            let prop = if a.get("prop").is_some() { Some(prop_arg(a)?) } else { None };
            ed.frame = f;
            ed.edit_motion(Some("Remove Property Keyframe"), |m, _| {
                match prop {
                    Some(p) => {
                        m.remove_value(p, f - st);
                    }
                    None => {
                        m.clear_keys_at(f - st);
                    }
                }
            });
            Ok(Value::Null)
        }
        "motion.set_ease" => {
            target(ed, a)?;
            let name = string(a, "prop").unwrap_or("position");
            let p = if name.eq_ignore_ascii_case("position") { crate::motion::Prop::X } else { prop_arg(a)? };
            let ease = parse_ease(a.get("ease"))?;
            if ed.current_motion().is_none() {
                return Err("no motion tween at the target".into());
            }
            ed.edit_motion(Some("Property Ease"), |m, _| m.set_ease(p, ease));
            Ok(Value::Null)
        }
        "motion.set_interp" => {
            let key = num(a, "key").ok_or("missing 'key' (frame of the key, 1-based)")? as u32;
            let mut a2 = a.clone();
            if let Some(m) = a2.as_object_mut() {
                m.insert("frame".into(), json!(key));
            }
            target(ed, &a2)?;
            let p = prop_arg(a)?;
            if matches!(p, crate::motion::Prop::X | crate::motion::Prop::Y) {
                return Err("position follows the motion path; set interpolation on other properties".into());
            }
            let (st, _) = ed.motion_span(ed.layer, ed.frame).ok_or("no motion tween at the target")?;
            let local = key.saturating_sub(1).checked_sub(st).ok_or("key is before the tween")?;
            let ty = string(a, "type").unwrap_or("auto");
            let n = |k: &str, d: f32| num(a, k).unwrap_or(d).max(0.0);
            let text = || -> Result<String, String> {
                let t = string(a, "expression").ok_or("missing 'expression'")?.to_string();
                let props = crate::motion::expr_props();
                let names: Vec<&str> = props.iter().map(|s| s.as_str()).collect();
                crate::expr::Expr::parse(&t, &names)?;
                Ok(t)
            };
            use crate::motion::Interp;
            let interp = match ty {
                "auto" => Interp::Auto,
                "constant" => Interp::Constant,
                "linear" => Interp::Linear,
                "ease_in_out" => Interp::EaseInOut { ease_in: n("ease_in", 3.0), ease_out: n("ease_out", 3.0) },
                "ease_in_out_pct" => Interp::EaseInOutPct { ease_in: n("ease_in", 33.0).min(100.0), ease_out: n("ease_out", 33.0).min(100.0) },
                "exponential" => Interp::Exponential,
                "expression" => Interp::Expression { text: text()? },
                "similar_shape" => Interp::SimilarShape { text: text()?, offset: num(a, "offset").unwrap_or(0.0) },
                t => return Err(format!("unknown interpolation '{t}'")),
            };
            let has = ed.current_motion().and_then(|(_, _, m, _)| m.track(p).map(|t| t.has_key(local))).unwrap_or(false);
            if !has {
                return Err(format!("no {} key at frame {key}", p.id()));
            }
            ed.edit_motion(Some("Interpolation"), |m, _| {
                if let Some(k) = m.track_mut(p).and_then(|t| t.keys.iter_mut().find(|k| k.frame == local)) {
                    k.interp = interp;
                }
            });
            Ok(Value::Null)
        }
        "timeline.copy_frames" | "timeline.cut_frames" => {
            target(ed, a)?;
            if ed.frame_sel.is_none() {
                ed.select_frames(FrameSel::cell(ed.layer, ed.frame));
            }
            if id == "timeline.copy_frames" {
                ed.copy_frames();
            } else {
                ed.cut_frames();
            }
            Ok(Value::Null)
        }
        "timeline.paste_frames" => {
            target(ed, a)?;
            if ed.frame_sel.is_none() {
                ed.select_frames(FrameSel::cell(ed.layer, ed.frame));
            }
            if boolean(a, "overwrite").unwrap_or(false) {
                ed.paste_overwrite_frames();
            } else {
                ed.paste_frames();
            }
            Ok(Value::Null)
        }
        "timeline.set_label" => {
            target(ed, a)?;
            ed.set_frame_label(string(a, "label").unwrap_or(""));
            if let Some(k) = string(a, "type") {
                let kind = crate::model::LabelKind::from_id(k).ok_or_else(|| format!("unknown label type '{k}'"))?;
                ed.set_frame_label_kind(kind);
            }
            Ok(Value::Null)
        }
        "timeline.set_tween_options" => {
            target(ed, a)?;
            let (sync, snap) = (boolean(a, "sync"), boolean(a, "snap"));
            ed.edit_keyframe("Tween Options", |k| {
                if let Some(v) = sync {
                    k.sync = v;
                }
                if let Some(v) = snap {
                    k.snap = v;
                }
            });
            Ok(Value::Null)
        }
        "timeline.set_property_ease" => {
            target(ed, a)?;
            let p = string(a, "property").ok_or("missing 'property'")?;
            let g = crate::motion::PropGroup::from_id(p).ok_or_else(|| format!("unknown property '{p}'"))?;
            let ease = parse_ease(a.get("ease"))?;
            ed.edit_keyframe("Property Ease", |k| {
                let pe = k.prop_eases.get_or_insert_with(Default::default);
                *pe.get_mut(g) = ease;
                if **pe == crate::motion::PropEases::default() {
                    k.prop_eases = None;
                }
            });
            Ok(Value::Null)
        }
        "timeline.set_script" => {
            target(ed, a)?;
            ed.set_frame_script(string(a, "script").unwrap_or(""));
            Ok(Value::Null)
        }
        "timeline.move_frames" => {
            target(ed, a)?;
            if ed.frame_sel.is_none() {
                ed.select_frames(FrameSel::cell(ed.layer, ed.frame));
            }
            ed.move_frames(req(a, "by")? as i64);
            Ok(Value::Null)
        }
        "layer.list" => Ok(Value::Array(
            ed.timeline()
                .layers
                .iter()
                .enumerate()
                .map(|(i, l)| json!({ "layer": i + 1, "name": l.name, "visible": l.visible, "locked": l.locked, "outline": l.outline, "frames": l.end, "keyframes": l.keyframes.keys().map(|k| k + 1).collect::<Vec<_>>(), "kind": format!("{:?}", l.kind).to_lowercase(), "parent": l.parent.map(|p| p + 1), "color": hex(l.color) }))
                .collect(),
        )),
        "layer.add" => {
            ed.add_layer();
            if let Some(n) = string(a, "name") {
                let i = ed.layer;
                ed.rename_layer(i, n);
            }
            Ok(json!(ed.layer + 1))
        }
        "layer.copy" | "layer.cut" => {
            if let Some(l) = layer_arg(ed, a, "layer")? {
                ed.layer = l;
            }
            ed.frame_sel = layer_arg(ed, a, "to_layer")?.map(|to| FrameSel { layer0: ed.layer, layer1: to, f0: 0, f1: 0 });
            if id == "layer.copy" {
                ed.copy_layers();
            } else {
                ed.cut_layers();
            }
            Ok(Value::Null)
        }
        "layer.paste" => {
            if let Some(l) = layer_arg(ed, a, "layer")? {
                ed.layer = l;
            }
            if !ed.can_paste_layers() {
                return Err("nothing copied (layer.copy first)".into());
            }
            ed.paste_layers();
            Ok(Value::Null)
        }
        "layer.delete" => {
            if let Some(l) = layer_arg(ed, a, "layer")? {
                ed.layer = l;
            }
            if ed.timeline().layers.len() <= 1 {
                return Err("can't delete the only layer".into());
            }
            ed.delete_layer();
            Ok(Value::Null)
        }
        "layer.select" => {
            ed.layer = layer_arg(ed, a, "layer")?.ok_or("missing 'layer'")?;
            ed.frame_sel = None;
            Ok(Value::Null)
        }
        "layer.set" => {
            let i = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            if let Some(n) = string(a, "name") {
                ed.rename_layer(i, n);
            }
            let l = &ed.timeline().layers[i];
            let (v, lk, o) = (l.visible, l.locked, l.outline);
            if boolean(a, "visible").map(|x| x != v).unwrap_or(false) {
                ed.toggle_layer_flag(i, crate::editor::LayerFlag::Visible);
            }
            if boolean(a, "locked").map(|x| x != lk).unwrap_or(false) {
                ed.toggle_layer_flag(i, crate::editor::LayerFlag::Locked);
            }
            if boolean(a, "outline").map(|x| x != o).unwrap_or(false) {
                ed.toggle_layer_flag(i, crate::editor::LayerFlag::Outline);
            }
            if let Some(k) = string(a, "kind") {
                let kind = match k {
                    "normal" => LayerKind::Normal,
                    "folder" => LayerKind::Folder,
                    "mask" => LayerKind::Mask,
                    "guide" => LayerKind::Guide,
                    k => return Err(format!("unknown layer kind '{k}'")),
                };
                ed.set_layer_kind(i, kind);
            }
            match a.get("parent") {
                None => {}
                Some(Value::Number(n)) if n.as_u64() == Some(0) => ed.set_layer_parent(i, None),
                Some(Value::Null) => ed.set_layer_parent(i, None),
                Some(_) => {
                    let p = layer_arg(ed, a, "parent")?;
                    ed.set_layer_parent(i, p);
                    if ed.timeline().layers[i].parent != p {
                        return Err("parent must be a folder, mask or guide layer above this layer".into());
                    }
                }
            }
            Ok(Value::Null)
        }
        "draw.rect" => {
            let (fill, stroke) = shape_style(a, Some(Color::rgb(0x2D, 0x8C, 0xEB)))?;
            let p = Path::rounded_rect(req(a, "x")?, req(a, "y")?, req(a, "w")?, req(a, "h")?, num(a, "radius").unwrap_or(0.0));
            add_shape(ed, "Rectangle", p, fill, stroke)
        }
        "draw.oval" => {
            let (fill, stroke) = shape_style(a, Some(Color::rgb(0x2D, 0x8C, 0xEB)))?;
            let (x, y, w, h) = (req(a, "x")?, req(a, "y")?, req(a, "w")?, req(a, "h")?);
            add_shape(ed, "Oval", Path::ellipse(x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0), fill, stroke)
        }
        "draw.line" => {
            let c = color_arg(a, "stroke")?.flatten().unwrap_or(Color::rgb(0, 0, 0));
            let p = Path { segs: vec![Seg::M(pt(req(a, "x1")?, req(a, "y1")?)), Seg::L(pt(req(a, "x2")?, req(a, "y2")?))] };
            add_shape(ed, "Line", p, None, Some(Stroke { color: c, width: num(a, "stroke_width").unwrap_or(1.0) }))
        }
        "draw.polystar" => {
            let (fill, stroke) = shape_style(a, Some(Color::rgb(0xFF, 0xC4, 0x1F)))?;
            let r = req(a, "r")?;
            let p = Path::polystar(req(a, "cx")?, req(a, "cy")?, r, r, num(a, "sides").unwrap_or(5.0) as usize, boolean(a, "star").unwrap_or(false), num(a, "inner").unwrap_or(0.5));
            add_shape(ed, "PolyStar", p, fill, stroke)
        }
        "draw.path" => {
            let pts: Vec<Pt> = a
                .get("points")
                .and_then(Value::as_array)
                .ok_or("missing 'points' [[x,y], ...]")?
                .iter()
                .filter_map(|p| Some(pt(p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32)))
                .collect();
            if pts.len() < 2 {
                return Err("need at least 2 points".into());
            }
            let closed = boolean(a, "closed").unwrap_or(false);
            let mut path = if boolean(a, "smooth").unwrap_or(false) { Path::smooth_polyline(&pts, 0.0) } else { Path::polygon(&pts) };
            if !closed {
                path.segs.retain(|s| !matches!(s, Seg::Z));
            } else if !matches!(path.segs.last(), Some(Seg::Z)) {
                path.segs.push(Seg::Z);
            }
            let (fill, stroke) = shape_style(a, None)?;
            let stroke = if fill.is_none() && stroke.is_none() { Some(Stroke { color: Color::rgb(0, 0, 0), width: num(a, "stroke_width").unwrap_or(1.0) }) } else { stroke };
            if boolean(a, "assist").unwrap_or(false) && !ed.doc.assistants.is_empty() {
                let smooth = boolean(a, "smooth").unwrap_or(false);
                let mut last = Value::Null;
                for poly in crate::assist::apply(&ed.doc.assistants, &pts, 1.0) {
                    let mut p = if smooth { Path::smooth_polyline(&poly, 0.0) } else { Path::polygon(&poly) };
                    if !closed {
                        p.segs.retain(|s| !matches!(s, Seg::Z));
                    }
                    last = add_shape(ed, "Path", p, if closed { fill } else { None }, stroke)?;
                }
                return Ok(last);
            }
            add_shape(ed, "Path", path, if closed { fill } else { None }, stroke)
        }
        "assistant.add" => {
            let ty = string(a, "type").ok_or("missing 'type'")?;
            let c = pt(num(a, "x").unwrap_or(ed.doc.width / 2.0), num(a, "y").unwrap_or(ed.doc.height / 2.0));
            let mut kind = crate::assist::AssistantKind::create(ty, c, num(a, "size").unwrap_or(100.0)).ok_or_else(|| format!("unknown assistant '{ty}' (line, vanishing_point, ellipse, perspective, fisheye, mirror, star, grid, affine, jitter)"))?;
            assistant_options(&mut kind, a);
            let mut asst = crate::assist::Assistant::new(kind);
            if let Some(m) = num(a, "magnetism") {
                asst.magnetism = m.clamp(0.0, 1.0);
            }
            Ok(json!(ed.add_assistant(asst) + 1))
        }
        "draw.particles" => {
            let mut e = crate::particles::Emitter::default();
            let src = particle_source(ed, a)?;
            particle_settings(&mut e, a, src)?;
            let el = Element { matrix: Matrix::translate(num(a, "x").unwrap_or(ed.doc.width / 2.0), num(a, "y").unwrap_or(ed.doc.height / 2.0)), ..Element::particles(e) };
            let s = ed.add_element(el).ok_or("current layer is locked or hidden")?;
            Ok(json!({"layer": s.layer + 1, "index": s.element}))
        }
        "selection.particles" => {
            need_selection(ed)?;
            let any = ed.selection.iter().any(|&s| matches!(ed.element_at(s).map(|e| e.kind), Some(ElementKind::Particles(_))));
            if !any {
                return Err("no particle emitter selected".into());
            }
            let src = particle_source(ed, a)?;
            // validate once (colours etc.)
            particle_settings(&mut crate::particles::Emitter::default(), a, src)?;
            let changes = a.clone();
            ed.modify_selected("Particles", move |el| {
                if let ElementKind::Particles(p) = &mut el.kind {
                    let _ = particle_settings(p, &changes, src);
                }
            });
            Ok(Value::Null)
        }
        "assistant.list" => Ok(Value::Array(
            ed.doc.assistants.iter().enumerate().map(|(i, x)| json!({ "index": i + 1, "type": x.name(), "enabled": x.enabled, "magnetism": x.magnetism, "points": x.points().iter().map(|p| json!([p.x, p.y])).collect::<Vec<_>>() })).collect(),
        )),
        "assistant.set" => {
            let i = (num(a, "index").ok_or("missing 'index'")? as usize).checked_sub(1).filter(|i| *i < ed.doc.assistants.len()).ok_or("no such assistant")?;
            let mut x = ed.doc.assistants[i].clone();
            if let Some(v) = boolean(a, "enabled") {
                x.enabled = v;
            }
            if let Some(m) = num(a, "magnetism") {
                x.magnetism = m.clamp(0.0, 1.0);
            }
            if let Some(ps) = a.get("points").and_then(Value::as_array) {
                for (k, p) in ps.iter().enumerate() {
                    if let (Some(px), Some(py)) = (p.get(0).and_then(Value::as_f64), p.get(1).and_then(Value::as_f64)) {
                        // set exactly (no carrying of the other handles)
                        if let Some(q) = x.kind.points_mut().into_iter().nth(k) {
                            *q = pt(px as f32, py as f32);
                        }
                    }
                }
            }
            assistant_options(&mut x.kind, a);
            ed.set_assistant(i, x, true);
            Ok(Value::Null)
        }
        "assistant.remove" => {
            if string(a, "index") == Some("all") {
                while !ed.doc.assistants.is_empty() {
                    ed.remove_assistant(0);
                }
                return Ok(Value::Null);
            }
            let i = (num(a, "index").ok_or("missing 'index'")? as usize).checked_sub(1).ok_or("no such assistant")?;
            if !ed.remove_assistant(i) {
                return Err("no such assistant".into());
            }
            Ok(Value::Null)
        }
        "layer.wrap" => {
            if let Some(i) = layer_arg(ed, a, "layer")? {
                ed.layer = i;
            }
            let kind = match string(a, "kind").unwrap_or("folder") {
                "folder" => LayerKind::Folder,
                "mask" => LayerKind::Mask,
                "guide" => LayerKind::Guide,
                k => return Err(format!("unknown kind '{k}' (folder, mask, guide)")),
            };
            let name = string(a, "name").unwrap_or(match kind { LayerKind::Folder => "Folder", LayerKind::Mask => "Mask", _ => "Motion Guide" }).to_string();
            Ok(json!(ed.wrap_layer(kind, &name) + 1))
        }
        "selection.group" => Ok(json!(ed.group_selected())),
        "selection.ungroup" => Ok(json!(ed.ungroup_selected())),
        "selection.align" => {
            need_selection(ed)?;
            use crate::editor::AlignOp::*;
            let op = match string(a, "op").ok_or("missing 'op'")? {
                "left" => Left, "hcenter" => HCenter, "right" => Right, "top" => Top, "vcenter" => VCenter, "bottom" => Bottom,
                "dist_left" => DistLeft, "dist_hcenter" => DistHCenter, "dist_right" => DistRight,
                "dist_top" => DistTop, "dist_vcenter" => DistVCenter, "dist_bottom" => DistBottom,
                "match_width" => MatchWidth, "match_height" => MatchHeight, "match_size" => MatchSize,
                "space_h" => SpaceH, "space_v" => SpaceV,
                o => return Err(format!("unknown align op '{o}'")),
            };
            ed.align(op, boolean(a, "to_stage").unwrap_or(false));
            Ok(Value::Null)
        }
        "selection.add_filter" if string(a, "type").is_some_and(|t| t.eq_ignore_ascii_case("effect")) => {
            need_selection(ed)?;
            let id = string(a, "id").ok_or("missing 'id' (an EffectCraft effect id)")?.to_string();
            let mut params = vec![];
            if let Some(obj) = a.get("params").and_then(Value::as_object) {
                for (k, v) in obj {
                    let ev = if let Some(b) = v.as_bool() {
                        crate::fx::EffectValue::Bool(b)
                    } else if let Some(n) = v.as_f64() {
                        crate::fx::EffectValue::Num(n)
                    } else if let Some(c) = v.as_str().and_then(Color::from_hex) {
                        crate::fx::EffectValue::Color(c)
                    } else if let Some([x, y]) = v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>()).and_then(|v| <[f64; 2]>::try_from(v).ok()) {
                        crate::fx::EffectValue::Vec2([x, y])
                    } else {
                        return Err(format!("effect param '{k}': number, bool, [x, y] or '#RRGGBBAA'"));
                    };
                    params.push((k.clone(), ev));
                }
            }
            let f = crate::fx::Filter::Effect { id, params };
            ed.modify_selected("Add Effect", |e| e.filters.push(f.clone()));
            Ok(Value::Null)
        }
        "selection.add_filter" => {
            need_selection(ed)?;
            let ty = string(a, "type").ok_or("missing 'type'")?;
            let mut f = crate::fx::Filter::default_of(ty).ok_or_else(|| format!("unknown filter '{ty}' (drop_shadow, blur, glow, bevel, gradient_glow, gradient_bevel, adjust_color, motion_blur, effect)"))?;
            // overlay given fields onto the defaults
            let mut v = serde_json::to_value(&f).map_err(|e| e.to_string())?;
            if let (Some(obj), Some(args)) = (v.as_object_mut(), a.as_object()) {
                for (k, val) in args {
                    if k == "type" || !obj.contains_key(k) {
                        continue;
                    }
                    let val = match (k.as_str(), val.as_str().and_then(Color::from_hex)) {
                        ("color" | "shadow" | "highlight", Some(c)) => serde_json::to_value(c).map_err(|e| e.to_string())?,
                        // gradient filters: [[position 0..1, "#RRGGBBAA"], …]
                        ("stops", _) => {
                            let mut st: Vec<(f32, Color)> = vec![];
                            for s in val.as_array().ok_or("stops: [[position, \"#RRGGBBAA\"], …]")? {
                                let pos = s.get(0).and_then(|v| v.as_f64()).ok_or("stop position")? as f32;
                                let c = s.get(1).and_then(|v| v.as_str()).and_then(Color::from_hex).ok_or("stop colour")?;
                                st.push((pos.clamp(0.0, 1.0), c));
                            }
                            st.sort_by(|a, b| a.0.total_cmp(&b.0));
                            serde_json::to_value(st).map_err(|e| e.to_string())?
                        }
                        ("kind", _) => match val.as_str().map(|x| x.to_ascii_lowercase()).as_deref() {
                            Some("outer") => json!("Outer"),
                            Some("full") => json!("Full"),
                            _ => json!("Inner"),
                        },
                        _ => val.clone(),
                    };
                    obj.insert(k.clone(), val);
                }
            }
            f = serde_json::from_value(v).map_err(|e| format!("bad filter fields: {e}"))?;
            ed.modify_selected("Add Filter", |e| e.filters.push(f.clone()));
            Ok(Value::Null)
        }
        "bitmap.trace" => {
            need_selection(ed)?;
            let mut o = crate::trace::TraceOpts::default();
            if let Some(v) = num(a, "color_threshold") {
                o.color_threshold = v.clamp(0.0, 500.0) as u32;
            }
            if let Some(v) = num(a, "min_area") {
                o.min_area = v.clamp(1.0, 1000.0) as u32;
            }
            if let Some(v) = string(a, "curve_fit") {
                o.curve_fit = crate::trace::CurveFit::from_name(v).ok_or_else(|| format!("unknown curve_fit '{v}'"))?;
            }
            if let Some(v) = string(a, "corners") {
                o.corners = crate::trace::Corners::from_name(v).ok_or_else(|| format!("unknown corners '{v}'"))?;
            }
            if let Some(v) = string(a, "mode") {
                o.mode = crate::trace::TraceMode::from_name(v).ok_or_else(|| format!("unknown mode '{v}' (outline, centerline)"))?;
            }
            if let Some(v) = num(a, "line_threshold") {
                o.line_threshold = v.clamp(1.0, 254.0) as u8;
            }
            if let Some(v) = boolean(a, "uniform_width") {
                o.uniform_width = v;
            }
            let n = ed.trace_bitmap(&o);
            if n == 0 {
                return Err("select a bitmap to trace".into());
            }
            Ok(json!(n))
        }
        "paint.brushes" => Ok(Value::Array(crate::paint::PRESETS.iter().map(|p| json!({ "id": p.id, "name": p.name, "group": p.group })).collect())),
        "paint.stroke" => {
            let pts: Vec<(Pt, Option<f32>)> = a
                .get("points")
                .and_then(Value::as_array)
                .ok_or("missing 'points' [[x,y], ...]")?
                .iter()
                .filter_map(|p| Some((pt(p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32), p.get(2).and_then(Value::as_f64).map(|v| v as f32))))
                .collect();
            if pts.is_empty() {
                return Err("need at least one point".into());
            }
            let id = string(a, "brush").unwrap_or("classic_pencil");
            let preset = crate::paint::preset(id).ok_or_else(|| format!("unknown brush '{id}' (see paint.brushes)"))?;
            let mut o = crate::paint::PaintOpts::default();
            if let Some(Some(c)) = color_arg(a, "color")? {
                o.color = c;
            }
            o.radius = num(a, "size").map(|v| v.clamp(0.2, 500.0));
            o.opacity = num(a, "opacity").unwrap_or(1.0).clamp(0.0, 1.0);
            o.eraser = boolean(a, "eraser").unwrap_or(false);
            let mut s = ed.paint_begin(pts[0].0, preset, &o)?;
            // the first event seeds the position; resample long segments so speed stays sane
            let mut last = pts[0].0;
            ed.paint_to(&mut s, last, pts[0].1, 0.01);
            for (q, pr) in &pts {
                let n = (last.dist(*q) / 4.0).ceil().max(1.0) as usize;
                for i in 1..=n {
                    ed.paint_to(&mut s, last.lerp(*q, i as f32 / n as f32), *pr, 0.01);
                }
                last = *q;
            }
            let bmp = s.bitmap;
            ed.paint_end(s);
            Ok(json!(bmp))
        }
        "selection.mesh_warp" => {
            need_selection(ed)?;
            let s = ed.selection[0];
            let el = ed.element_at(s).ok_or("no object")?;
            if !matches!(el.kind, ElementKind::Bitmap(_) | ElementKind::Instance(_) | ElementKind::Group(_)) {
                return Err("Asset Warp pins here work on bitmaps, symbols and groups (shapes: drag pins with the Asset Warp tool)".into());
            }
            if boolean(a, "reset").unwrap_or(false) {
                ed.modify_selected("Reset Warp", |e| e.mesh = None);
                return Ok(Value::Null);
            }
            let pins: Vec<[f32; 4]> = a
                .get("pins")
                .and_then(Value::as_array)
                .ok_or("missing 'pins' [[x, y, to_x, to_y], …]")?
                .iter()
                .filter_map(|p| Some([p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32, p.get(2)?.as_f64()? as f32, p.get(3)?.as_f64()? as f32]))
                .collect();
            let mark = ed.undo_depth();
            for [x, y, tx, ty] in pins {
                let i = ed.mesh_pin_add(s, pt(x, y)).ok_or("the layer is locked")?;
                ed.mesh_pin_move(s, i, pt(tx, ty), false);
            }
            ed.squash_undo(mark, "Asset Warp");
            Ok(Value::Null)
        }
        "shape.deform" => {
            let tool = string(a, "tool").unwrap_or("magnet");
            let kind = crate::sculpt::Kind::from_id(tool).ok_or_else(|| format!("unknown deformation tool '{tool}' (magnet, pinch, pump, bender, iron)"))?;
            let need = |k: &str| num(a, k).ok_or_else(|| format!("'{k}' is required"));
            let (x, y, tx, ty) = (need("x")?, need("y")?, need("to_x")?, need("to_y")?);
            let mut o = crate::sculpt::Opts::default();
            if let Some(v) = num(a, "size") {
                o.size = v.clamp(1.0, 2000.0);
            }
            if let Some(v) = a.get("sharp").and_then(Value::as_bool) {
                o.sharp = v;
            }
            if let Some(v) = num(a, "strength") {
                o.strength = v.clamp(0.0, 1.0);
            }
            // Bender: (x, y)–(x2, y2) is the segment; the drag goes from (from_x, from_y) to (to_x, to_y)
            let (seg, from) = if kind == crate::sculpt::Kind::Bender {
                let b = pt(need("x2")?, need("y2")?);
                (Some((pt(x, y), b)), pt(need("from_x")?, need("from_y")?))
            } else {
                (None, pt(x, y))
            };
            let mut s = ed.sculpt_begin(kind, from, o, 6.0, seg).ok_or_else(|| format!("{}: nothing to deform there", kind.name()))?;
            let to = pt(tx, ty);
            if kind == crate::sculpt::Kind::Iron {
                // brush along the drag
                let n = (from.dist(to) / 2.0).ceil().max(1.0) as usize;
                for i in 0..=n {
                    ed.sculpt_drag(&mut s, from.lerp(to, i as f32 / n as f32));
                }
            } else {
                ed.sculpt_drag(&mut s, to);
            }
            ed.sculpt_end(&s);
            Ok(json!(s.targets.len()))
        }
        "selection.clear_filters" => {
            need_selection(ed)?;
            ed.modify_selected("Remove Filters", |e| e.filters.clear());
            Ok(Value::Null)
        }
        "selection.set_blend" => {
            need_selection(ed)?;
            let m = string(a, "mode").ok_or("missing 'mode'")?;
            let b = crate::fx::BlendMode::from_name(m).ok_or_else(|| format!("unknown blend mode '{m}'"))?;
            ed.modify_selected("Blend", |e| e.blend = b);
            Ok(Value::Null)
        }
        "import.image" => {
            let path = string(a, "path").ok_or("missing 'path'")?;
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or("Bitmap".into());
            #[cfg(feature = "codecs")]
            let asset = crate::codecs::bitmap_from_bytes(&name, bytes)?;
            #[cfg(not(feature = "codecs"))]
            let asset = crate::bitmap::BitmapAsset::from_png(&name, bytes)?;
            let id = ed.add_bitmap(asset);
            if boolean(a, "place").unwrap_or(true) {
                let at = pt(num(a, "x").unwrap_or(0.0), num(a, "y").unwrap_or(0.0));
                ed.place_bitmap(id, at).ok_or("current layer is locked or hidden")?;
            }
            Ok(json!(id))
        }
        "import.xdts" => {
            let path = string(a, "path").ok_or("missing 'path'")?;
            #[cfg(feature = "codecs")]
            {
                let rep = crate::xdts::import_file(ed, std::path::Path::new(path))?;
                Ok(json!({"summary": rep.summary(), "retimed": rep.retimed, "from_images": rep.from_images, "timing_only": rep.timing_only, "missing": rep.missing}))
            }
            #[cfg(not(feature = "codecs"))]
            {
                let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                let rep = crate::xdts::apply(ed, &crate::xdts::parse(&text)?, &[]);
                Ok(json!({"summary": rep.summary()}))
            }
        }
        "draw.text" => {
            let text = string(a, "text").ok_or("missing 'text'")?;
            let color = color_arg(a, "color")?.flatten().unwrap_or(Color::rgb(0, 0, 0));
            let mut el = Element::text(text, num(a, "size").unwrap_or(24.0).max(1.0), color, pt(req(a, "x")?, req(a, "y")?));
            if let ElementKind::Text(t) = &mut el.kind {
                match string(a, "font") {
                    Some("mono") => t.font = crate::text::FontFace::Mono,
                    Some("sans") | None => {}
                    Some(fam) => t.family = Some(fam.to_string()),
                }
                t.align = match string(a, "align") {
                    Some("center") => crate::text::Align::Center,
                    Some("right") => crate::text::Align::Right,
                    _ => crate::text::Align::Left,
                };
                if let Some(w) = text_weight(a) {
                    t.weight = w;
                }
                t.italic = boolean(a, "italic").unwrap_or(false);
            }
            let s = ed.add_element(el).ok_or("current layer is locked or hidden")?;
            Ok(json!({ "layer": s.layer + 1, "index": s.element }))
        }
        "select.all" => {
            ed.select_all();
            Ok(json!(ed.selection.len()))
        }
        "select.layer" => {
            let li = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            ed.select_all();
            ed.selection.retain(|s| s.layer == li);
            ed.layer = li;
            Ok(json!(ed.selection.len()))
        }
        "select.none" => {
            ed.selection.clear();
            Ok(Value::Null)
        }
        "select.at" => {
            let hit = ed.hit_test(pt(req(a, "x")?, req(a, "y")?));
            ed.select(hit, boolean(a, "add").unwrap_or(false));
            Ok(json!(hit.is_some()))
        }
        "select.rect" => {
            let (x, y) = (req(a, "x")?, req(a, "y")?);
            ed.select_in_rect(pt(x, y), pt(x + req(a, "w")?, y + req(a, "h")?), boolean(a, "add").unwrap_or(false));
            Ok(json!(ed.selection.len()))
        }
        "select.magic_wand" => Ok(json!(ed.magic_wand(pt(req(a, "x")?, req(a, "y")?), num(a, "threshold").unwrap_or(20.0), boolean(a, "add").unwrap_or(false)))),
        "select.element" => {
            let li = layer_arg(ed, a, "layer")?.ok_or("missing 'layer'")?;
            let idx = num(a, "index").ok_or("missing 'index'")? as usize;
            let count = ed.timeline().layers[li].span_at(ed.frame).map(|(_, k, _)| k.elements.len()).unwrap_or(0);
            if idx >= count {
                return Err(format!("layer {} has {count} object(s) here", li + 1));
            }
            let s = Selection { layer: li, element: idx };
            if boolean(a, "add").unwrap_or(false) {
                if !ed.selection.contains(&s) {
                    ed.select(Some(s), true);
                }
            } else {
                ed.select(Some(s), false);
            }
            Ok(json!(ed.selection.len()))
        }
        "frame.info" => {
            let li = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            let f = frame_arg(a, "frame")?.unwrap_or(ed.frame);
            let l = &ed.timeline().layers[li];
            let Some((start, kf, _)) = l.span_at(f) else { return Ok(Value::Null) };
            let tween = match kf.tween {
                Tween::None => "none",
                Tween::Classic { .. } => "classic",
                Tween::Shape { .. } => "shape",
                Tween::Motion { .. } => "motion",
            };
            let n = kf.elements.len();
            let saved = ed.frame;
            ed.frame = f;
            let els: Vec<Value> = (0..n).map(|i| element_json(ed, Selection { layer: li, element: i })).collect();
            ed.frame = saved;
            let l = &ed.timeline().layers[li];
            let kf = &l.keyframes[&start];
            Ok(json!({
                "start": start + 1, "duration": l.span_end(start) - start, "label": kf.label, "label_type": kf.label_kind.id(), "sync": kf.sync, "snap": kf.snap, "tween": tween, "ease": kf.tween.ease(),
                "script": kf.script, "sound": kf.sound.as_ref().and_then(|s| ed.doc.sound(s.sound)).map(|s| s.name.clone()),
                "elements": els,
            }))
        }
        "symbol.new" => {
            let kind = match string(a, "kind").unwrap_or("movieclip") {
                "graphic" => SymbolKind::Graphic,
                "movieclip" | "movie_clip" | "movie clip" => SymbolKind::MovieClip,
                "button" => SymbolKind::Button,
                k => return Err(format!("unknown symbol kind '{k}'")),
            };
            Ok(json!(ed.new_symbol(string(a, "name").ok_or("missing 'name'")?, kind)))
        }
        "selection.info" => Ok(Value::Array(ed.selection.clone().into_iter().map(|s| element_json(ed, s)).collect())),
        "selection.move" => {
            need_selection(ed)?;
            let (dx, dy) = (num(a, "dx").unwrap_or(0.0), num(a, "dy").unwrap_or(0.0));
            ed.modify_selected("Move", |e| {
                e.matrix.tx += dx;
                e.matrix.ty += dy;
            });
            Ok(Value::Null)
        }
        "selection.set_position" => {
            need_selection(ed)?;
            let (lo, _) = ed.selection_bounds().ok_or("selection has no size")?;
            let (dx, dy) = (req(a, "x")? - lo.x, req(a, "y")? - lo.y);
            ed.modify_selected("Move", |e| {
                e.matrix.tx += dx;
                e.matrix.ty += dy;
            });
            Ok(Value::Null)
        }
        "selection.scale" => {
            need_selection(ed)?;
            let c = center(ed)?;
            let sx = req(a, "sx")?;
            let sy = num(a, "sy").unwrap_or(sx);
            let m = Matrix::translate(c.x, c.y).concat(&Matrix::scale(sx, sy)).concat(&Matrix::translate(-c.x, -c.y));
            ed.modify_selected("Scale", |e| e.matrix = m.concat(&e.matrix));
            Ok(Value::Null)
        }
        "selection.rotate" => {
            need_selection(ed)?;
            let c = center(ed)?;
            let m = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(req(a, "degrees")?.to_radians())).concat(&Matrix::translate(-c.x, -c.y));
            ed.modify_selected("Rotate", |e| e.matrix = m.concat(&e.matrix));
            Ok(Value::Null)
        }
        "selection.set" => {
            need_selection(ed)?;
            let fill = color_arg(a, "fill")?;
            let stroke = color_arg(a, "stroke")?;
            let sw = num(a, "stroke_width");
            let alpha = num(a, "alpha");
            let tint = color_arg(a, "tint")?;
            let amt = num(a, "tint_amount");
            let name = string(a, "name").map(String::from);
            let text = string(a, "text").map(String::from);
            let size = num(a, "size");
            ed.modify_selected("Set Properties", |e| {
                if let Some(v) = alpha {
                    e.alpha = v.clamp(0.0, 1.0);
                }
                if let Some(n) = &name {
                    e.name = n.clone();
                }
                if let Some(t) = tint {
                    e.tint = t.map(|c| (c, amt.unwrap_or(0.6)));
                } else if let (Some(v), Some((c, _))) = (amt, e.tint) {
                    e.tint = Some((c, v));
                }
                match &mut e.kind {
                    ElementKind::Shape(s) => {
                        if let Some(f) = fill {
                            s.fill = f;
                        }
                        if let Some(st) = stroke {
                            s.stroke = st.map(|c| Stroke { color: c, width: sw.or(s.stroke.map(|x| x.width)).unwrap_or(1.0) });
                        } else if let (Some(w), Some(st)) = (sw, s.stroke.as_mut()) {
                            st.width = w;
                        }
                    }
                    ElementKind::Canvas(cv) => {
                        if let Some(f) = fill {
                            let all: Vec<crate::merge::Part> = (0..cv.fills.len()).map(crate::merge::Part::Fill).collect();
                            cv.set_fill(&all, f, None);
                        }
                        if let Some(Some(c)) = stroke {
                            let all: Vec<crate::merge::Part> = (0..cv.strokes.len()).map(crate::merge::Part::Stroke).collect();
                            cv.set_stroke(&all, Stroke { color: c, width: sw.unwrap_or(1.0) });
                        }
                    }
                    ElementKind::Particles(pe) => {
                        if let Some(Some(c)) = fill {
                            pe.color_start = c;
                        }
                    }
                    ElementKind::Text(t) => {
                        if let Some(Some(c)) = fill {
                            t.color = c;
                        }
                        if let Some(x) = &text {
                            t.text = x.clone();
                        }
                        if let Some(v) = size {
                            t.size = v.max(1.0);
                        }
                        if let Some(w) = text_weight(a) {
                            t.weight = w;
                        }
                        if let Some(i) = boolean(a, "italic") {
                            t.italic = i;
                        }
                    }
                    ElementKind::Instance(_) | ElementKind::Group(_) | ElementKind::Bitmap(_) => {}
                }
            });
            Ok(Value::Null)
        }
        "selection.delete" => Ok(json!(ed.delete_selected())),
        "selection.duplicate" => {
            need_selection(ed)?;
            ed.duplicate();
            Ok(Value::Null)
        }
        "selection.to_symbol" => {
            need_selection(ed)?;
            let kind = match string(a, "kind").unwrap_or("graphic") {
                "graphic" => SymbolKind::Graphic,
                "movieclip" | "movie_clip" => SymbolKind::MovieClip,
                "button" => SymbolKind::Button,
                k => return Err(format!("unknown symbol kind '{k}'")),
            };
            let name = string(a, "name").ok_or("missing 'name'")?;
            Ok(json!(ed.convert_to_symbol(name, kind).ok_or("could not convert (locked layer?)")?))
        }
        "selection.break_apart" => {
            need_selection(ed)?;
            ed.break_apart();
            Ok(Value::Null)
        }
        "selection.skew" => {
            need_selection(ed)?;
            let c = center(ed)?;
            let (h, v) = (num(a, "h").unwrap_or(0.0).to_radians(), num(a, "v").unwrap_or(0.0).to_radians());
            // x' = x + tan(h)·y ; y' = y + tan(v)·x
            let sk = Matrix { a: 1.0, b: v.tan(), c: h.tan(), d: 1.0, tx: 0.0, ty: 0.0 };
            let m = Matrix::translate(c.x, c.y).concat(&sk).concat(&Matrix::translate(-c.x, -c.y));
            ed.modify_selected("Skew", |e| e.matrix = m.concat(&e.matrix));
            Ok(Value::Null)
        }
        "selection.set_3d" => {
            need_selection(ed)?;
            let (rx, ry, rz, z) = (num(a, "rx"), num(a, "ry"), num(a, "rz"), num(a, "z"));
            ed.modify_selected("3D", |e| {
                let mut t = e.three_d.unwrap_or_default();
                if let Some(v) = rx { t.rx = v; }
                if let Some(v) = ry { t.ry = v; }
                if let Some(v) = rz { t.rz = v; }
                if let Some(v) = z { t.z = v; }
                e.three_d = if t.is_identity() { None } else { Some(t) };
            });
            Ok(Value::Null)
        }
        "selection.distort" => {
            need_selection(ed)?;
            let cs: Vec<Pt> = a.get("corners").and_then(Value::as_array).ok_or("missing 'corners' [[x,y] x4]")?
                .iter().filter_map(|p| Some(pt(p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32))).collect();
            if cs.len() != 4 {
                return Err("'corners' needs 4 points: top-left, top-right, bottom-right, bottom-left".into());
            }
            let (lo, hi) = ed.selection_bounds().ok_or("selection has no size")?;
            let quad = [cs[0], cs[1], cs[2], cs[3]];
            let mut skipped = 0;
            for s in ed.selection.clone() {
                if !matches!(ed.element_at(s).map(|e| e.kind), Some(ElementKind::Shape(_))) { skipped += 1; }
            }
            ed.modify_selected("Distort", |e| {
                let m = e.matrix;
                if let (ElementKind::Shape(sh), Some(inv)) = (&mut e.kind, m.invert()) {
                    let world = sh.path.transformed(&m);
                    sh.path = crate::deform::distort(&world, lo, hi, &quad).transformed(&inv);
                    sh.warp = None;
                }
            });
            Ok(json!({ "skipped_non_shapes": skipped }))
        }
        "selection.set_gradient" => {
            need_selection(ed)?;
            let kind = match string(a, "kind").unwrap_or("linear") {
                "linear" => Some(GradientKind::Linear),
                "radial" => Some(GradientKind::Radial),
                "none" => None,
                k => return Err(format!("unknown gradient kind '{k}'")),
            };
            let colors: Vec<Color> = match a.get("colors").and_then(Value::as_array) {
                Some(v) => v.iter().filter_map(|c| c.as_str().and_then(Color::from_hex)).collect(),
                None => vec![Color::rgb(255, 255, 255), Color::rgb(0, 0, 0)],
            };
            if kind.is_some() && colors.len() < 2 {
                return Err("need at least 2 colors".into());
            }
            let n = colors.len();
            let stops: Vec<(f32, Color)> = colors.into_iter().enumerate().map(|(i, c)| (i as f32 / (n - 1).max(1) as f32, c)).collect();
            let spread = match string(a, "flow").unwrap_or("extend") {
                "extend" | "pad" => Spread::Pad,
                "reflect" => Spread::Reflect,
                "repeat" => Spread::Repeat,
                f => return Err(format!("unknown flow '{f}' (extend, reflect, repeat)")),
            };
            ed.modify_selected("Gradient", |e| {
                if let ElementKind::Shape(sh) = &mut e.kind {
                    sh.gradient = kind.and_then(|k| sh.path.bounds().map(|(lo, hi)| Gradient { spread, ..Gradient::fit(k, stops.clone(), lo, hi) }));
                }
            });
            Ok(Value::Null)
        }
        "bone.add" => {
            let p1 = pt(req(a, "x1")?, req(a, "y1")?);
            let p2 = pt(req(a, "x2")?, req(a, "y2")?);
            let h1 = ed.hit_test(p1).ok_or("no object at (x1, y1)")?;
            let h2 = ed.hit_test(p2).ok_or("no object at (x2, y2)")?;
            if h1.layer != h2.layer {
                return Err("both objects must be on the same layer".into());
            }
            ed.add_bone(h1.layer, h1.element, h2.element, p1, p2)?;
            Ok(Value::Null)
        }
        "bone.list" => {
            let li = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            Ok(Value::Array(ed.bones(li).iter().map(|b| json!({ "parent": b.parent, "child": b.child })).collect()))
        }
        "bone.clear" => {
            let li = layer_arg(ed, a, "layer")?.unwrap_or(ed.layer);
            ed.clear_bones(li);
            Ok(Value::Null)
        }
        "bone.drag" => {
            let p = pt(req(a, "x")?, req(a, "y")?);
            let to = pt(req(a, "to_x")?, req(a, "to_y")?);
            let h = ed.hit_test(p).ok_or("no object at (x, y)")?;
            if ed.bones(h.layer).is_empty() {
                return Err("that object's layer has no bones".into());
            }
            let el = ed.element_at(h).ok_or("no object")?;
            let grab = el.matrix.invert().ok_or("object has no size")?.apply(p);
            ed.begin_edit("Pose Bones");
            ed.ik_drag(h.layer, h.element, grab, to);
            Ok(Value::Null)
        }
        "bone.joint" => {
            let p = pt(req(a, "x")?, req(a, "y")?);
            let h = ed.hit_test(p).ok_or("no object at (x, y)")?;
            let b = *ed.bones(h.layer).iter().find(|b| b.child == h.element).ok_or("that object isn't attached by a bone (the root has no joint)")?;
            let rotate = boolean(a, "rotate").unwrap_or(b.rotate);
            let limit = joint_limit(a, b.limit)?;
            ed.set_joint(h.layer, h.element, rotate, limit);
            if let Some(pin) = boolean(a, "pin") {
                ed.set_joint_pin(h.layer, h.element, pin);
            }
            Ok(Value::Null)
        }
        "camera.add" => Ok(json!(ed.add_camera() + 1)),
        "camera.remove" => {
            ed.remove_camera();
            Ok(Value::Null)
        }
        "camera.mode" => {
            let m = match string(a, "mode").unwrap_or("").to_ascii_lowercase().as_str() {
                "stage" | "fixed" => crate::model::CameraMode::Stage,
                "free" | "vcam" => crate::model::CameraMode::Free,
                m => return Err(format!("unknown camera mode '{m}' (stage, free)")),
            };
            ed.set_camera_mode(m);
            Ok(Value::Null)
        }
        "camera.options" => {
            if ed.camera_layer().is_none() {
                ed.add_camera();
            }
            if let Some(f) = frame_arg(a, "frame")? {
                ed.set_frame(f);
            }
            let mut o = ed.camera_opts().unwrap_or_default();
            if let Some(v) = num(a, "shake") {
                o.shake = v.max(0.0);
            }
            if let Some(v) = num(a, "shake_speed") {
                o.shake_speed = v.max(0.01);
            }
            if let Some(v) = a.get("follow") {
                o.follow = v.as_str().filter(|s| !s.is_empty()).map(str::to_string);
            }
            if let Some(v) = num(a, "follow_lag") {
                o.follow_lag = v.clamp(0.0, 24.0) as u32;
            }
            if let Some(v) = num(a, "offset_x") {
                o.follow_offset.x = v;
            }
            if let Some(v) = num(a, "offset_y") {
                o.follow_offset.y = v;
            }
            ed.set_camera_opts(o);
            Ok(Value::Null)
        }
        "camera.effect" => {
            if ed.camera_layer().is_none() {
                ed.add_camera();
            }
            if let Some(f) = frame_arg(a, "frame")? {
                ed.set_frame(f);
            }
            let tint = color_arg(a, "tint")?.flatten();
            let (b, ta, al, bl) = (num(a, "brightness"), num(a, "tint_amount"), num(a, "alpha"), num(a, "blur"));
            ed.begin_edit("Camera Effect");
            ed.edit_camera(|e| {
                if let Some(b) = b {
                    e.tint = crate::motion::brightness_tint(b / 100.0, None);
                }
                if let Some(c) = tint {
                    e.tint = Some((c, ta.unwrap_or(50.0).clamp(0.0, 100.0) / 100.0));
                }
                if let Some(al) = al {
                    e.alpha = (al / 100.0).clamp(0.0, 1.0);
                }
                if let Some(bl) = bl {
                    e.filters.retain(|f| !matches!(f, crate::fx::Filter::Blur { .. }));
                    if bl > 0.0 {
                        e.filters.push(crate::fx::Filter::Blur { blur_x: bl, blur_y: bl, quality: 2 });
                    }
                }
            });
            Ok(Value::Null)
        }
        "camera.set" => {
            if ed.camera_layer().is_none() {
                ed.add_camera();
            }
            if let Some(f) = frame_arg(a, "frame")? {
                ed.set_frame(f);
            }
            let cur = ed.camera_at(ed.frame).unwrap_or_else(|| default_camera(&ed.doc).matrix);
            let (c, rot, zoom) = Editor::camera_params(&cur);
            let m = Editor::camera_from(pt(num(a, "x").unwrap_or(c.x), num(a, "y").unwrap_or(c.y)), num(a, "rotation").unwrap_or(rot), num(a, "zoom").unwrap_or(zoom));
            ed.begin_edit("Camera");
            ed.set_camera(m);
            Ok(Value::Null)
        }
        "selection.set_looping" => {
            need_selection(ed)?;
            let mode = match string(a, "mode") {
                Some(m) => Some(crate::model::LoopMode::from_id(m).ok_or_else(|| format!("unknown looping mode '{m}'"))?),
                None => None,
            };
            let first = num(a, "first_frame").map(|f| (f.max(1.0) as u32) - 1);
            ed.modify_selected("Looping", |e| {
                if let ElementKind::Instance(i) = &mut e.kind {
                    if let Some(m) = mode {
                        i.loop_mode = m;
                    }
                    if let Some(f) = first {
                        i.first_frame = f;
                    }
                }
            });
            Ok(Value::Null)
        }
        "selection.swap_symbol" => {
            let id = symbol_arg(ed, a)?;
            if ed.selection.is_empty() {
                return Err("nothing selected".into());
            }
            ed.swap_symbol(id);
            Ok(Value::Null)
        }
        "selection.color_effect" => {
            let style = string(a, "style").unwrap_or("none").to_ascii_lowercase();
            let amount = num(a, "amount");
            let color = color_arg(a, "color")?.flatten();
            let arr = |k: &str| -> Option<Vec<f32>> { a.get(k)?.as_array().map(|v| v.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()) };
            let (mul, add) = (arr("mul"), arr("add"));
            if !["none", "brightness", "tint", "advanced", "alpha"].contains(&style.as_str()) {
                return Err(format!("unknown style '{style}' (none, brightness, tint, advanced, alpha)"));
            }
            ed.modify_selected("Color Effect", move |e| {
                e.tint = None;
                e.color_fx = None;
                match style.as_str() {
                    "none" => e.alpha = 1.0,
                    "brightness" => e.tint = crate::motion::brightness_tint(amount.unwrap_or(0.0) / 100.0, None),
                    "tint" => e.tint = Some((color.unwrap_or(Color::rgb(255, 0, 0)), amount.unwrap_or(50.0) / 100.0)),
                    "alpha" => e.alpha = amount.unwrap_or(100.0).clamp(0.0, 100.0) / 100.0,
                    _ => {
                        let mut x = ColorXform::IDENTITY;
                        for i in 0..4 {
                            if let Some(m) = mul.as_ref().and_then(|m| m.get(i)) {
                                x.mul[i] = m / 100.0;
                            }
                            if let Some(v) = add.as_ref().and_then(|m| m.get(i)) {
                                x.add[i] = *v;
                            }
                        }
                        e.color_fx = Some(x);
                    }
                }
            });
            Ok(Value::Null)
        }
        "drawing.mode" => {
            ed.script_merge = boolean(a, "merge").unwrap_or(true);
            Ok(Value::Null)
        }
        "merge.select" => {
            let p0 = pt(req(a, "x")?, req(a, "y")?);
            let Some((li, Some(part), _)) = ed.merge_part_at(p0, 1.0) else { return Err("no merge-shape fill or line there".into()) };
            ed.merge_select(li, part, boolean(a, "connected").unwrap_or(false), boolean(a, "add").unwrap_or(false));
            Ok(json!(ed.merge_float().map(|(_, f)| json!({ "fills": f.fills.len(), "lines": f.strokes.len() }))))
        }
        "merge.select_rect" => {
            let (x, y, w, h) = (req(a, "x")?, req(a, "y")?, req(a, "w")?, req(a, "h")?);
            Ok(json!(ed.merge_select_area(&[vec![pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)]], boolean(a, "add").unwrap_or(false))))
        }
        "merge.deselect" => {
            ed.deselect();
            Ok(Value::Null)
        }
        "merge.erase" => {
            use crate::merge::PaintMode;
            let (x, y, w, h) = (req(a, "x")?, req(a, "y")?, req(a, "w")?, req(a, "h")?);
            let mode = match string(a, "mode").unwrap_or("normal") {
                "normal" => PaintMode::Normal,
                "fills" => PaintMode::Fills,
                "lines" => PaintMode::Lines,
                "selection" => PaintMode::Selection,
                "inside" => PaintMode::Inside,
                m => return Err(format!("unknown eraser mode '{m}'")),
            };
            Ok(json!(ed.merge_erase(&[vec![pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)]], mode, pt(x + w / 2.0, y + h / 2.0), Some("Erase"))))
        }
        "merge.bucket" => {
            let c = color_arg(a, "color")?.flatten().unwrap_or(Color::rgb(0x2D, 0x8C, 0xEB));
            Ok(json!(ed.merge_bucket(pt(req(a, "x")?, req(a, "y")?), c, num(a, "gap").unwrap_or(3.0))))
        }
        "merge.to_object" => Ok(json!(ed.merge_to_object())),
        "selection.combine" => {
            use crate::merge::BoolOp;
            let op = match string(a, "op").unwrap_or("union") {
                "union" => BoolOp::Union,
                "intersect" => BoolOp::Intersect,
                "punch" => BoolOp::Punch,
                "crop" => BoolOp::Crop,
                o => return Err(format!("unknown op '{o}'")),
            };
            ed.combine_objects(op)?;
            Ok(Value::Null)
        }
        "selection.shape" => {
            ed.shape_cleanup(string(a, "op").unwrap_or("optimize"));
            Ok(Value::Null)
        }
        "selection.stroke_style" => {
            let (cap, join) = (string(a, "cap").map(str::to_string), string(a, "join").map(str::to_string));
            let dash = match string(a, "dash") {
                Some(d) => Some(Dash::from_id(d).ok_or_else(|| format!("unknown stroke style '{d}'"))?),
                None => None,
            };
            let (dl, gap) = (num(a, "dash_len"), num(a, "gap"));
            let params: Vec<(String, f32)> = match a.get("params") {
                Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.to_ascii_lowercase().replace('_', " "), v.as_f64().unwrap_or(0.0) as f32)).collect(),
                _ => vec![],
            };
            let colors: Option<Vec<Option<Color>>> = match a.get("colors") {
                Some(Value::Array(v)) => Some(v.iter().map(|c| color_arg(&serde_json::json!({ "c": c }), "c").map(|c| c.flatten())).collect::<Result<_, _>>()?),
                _ => None,
            };
            if let Some(d) = dash.or(ed.selected_shape_dash()) {
                for (k, _) in &params {
                    if !crate::strokes::params(d).iter().any(|p| p.name.to_ascii_lowercase() == *k) {
                        let names: Vec<&str> = crate::strokes::params(d).iter().map(|p| p.name).collect();
                        return Err(format!("{} has no parameter '{k}' (has: {})", d.name(), if names.is_empty() { "none".into() } else { names.join(", ") }));
                    }
                }
            }
            ed.modify_selected("Stroke Style", move |e| {
                if let ElementKind::Shape(sh) = &mut e.kind {
                    let st = &mut sh.stroke_style;
                    if let Some(d) = dash {
                        if d != st.dash {
                            st.art = Default::default();
                        }
                        st.dash = d;
                    }
                    match cap.as_deref() {
                        Some("none") => st.cap = Cap::None,
                        Some("round") => st.cap = Cap::Round,
                        Some("square") => st.cap = Cap::Square,
                        _ => {}
                    }
                    match join.as_deref() {
                        Some("miter") => st.join = Join::Miter,
                        Some("round") => st.join = Join::Round,
                        Some("bevel") => st.join = Join::Bevel,
                        _ => {}
                    }
                    if let Some(v) = dl {
                        st.dash_len = v;
                    }
                    if let Some(v) = gap {
                        st.gap = v;
                    }
                    let ps = crate::strokes::params(st.dash);
                    for (k, v) in &params {
                        if let Some(i) = ps.iter().position(|p| p.name.to_ascii_lowercase() == *k) {
                            st.art.p[i] = Some(v.clamp(ps[i].min, ps[i].max));
                        }
                    }
                    if let Some(cs) = &colors {
                        for (i, c) in cs.iter().take(2).enumerate() {
                            st.art.colors[i] = *c;
                        }
                    }
                }
            });
            Ok(Value::Null)
        }
        "selection.bitmap_fill" => {
            let id = match a.get("bitmap") {
                None | Some(Value::Null) => None,
                Some(Value::Number(n)) => Some(n.as_u64().unwrap_or(0) as crate::bitmap::BitmapId),
                Some(Value::String(s)) => Some(ed.doc.bitmaps.iter().find(|b| b.name == *s).map(|b| b.id).ok_or_else(|| format!("no bitmap named '{s}'"))?),
                _ => return Err("bitmap must be a name, id or null".into()),
            };
            if ed.merge_bitmap_fill(id) {
                return Ok(Value::Null);
            }
            ed.modify_selected("Bitmap Fill", move |e| {
                if let ElementKind::Shape(sh) = &mut e.kind {
                    let o = sh.path.bounds().map(|(lo, _)| lo).unwrap_or_default();
                    sh.bitmap_fill = id.map(|bitmap| BitmapFill { bitmap, matrix: Matrix::translate(o.x, o.y) });
                }
            });
            Ok(Value::Null)
        }
        "draw.rect_primitive" | "draw.oval_primitive" => {
            let (fill, stroke) = shape_style(a, Some(Color::rgb(0x2D, 0x8C, 0xEB)))?;
            let (x, y, w, h) = (req(a, "x")?, req(a, "y")?, req(a, "w")?, req(a, "h")?);
            let prim = if id == "draw.rect_primitive" {
                let r = num(a, "radius").unwrap_or(0.0);
                let mut radius = [r; 4];
                if let Some(v) = a.get("radii").and_then(|v| v.as_array()) {
                    for (i, x) in v.iter().take(4).enumerate() {
                        radius[i] = x.as_f64().unwrap_or(0.0) as f32;
                    }
                }
                Primitive::Rect { x, y, w, h, radius }
            } else {
                Primitive::Oval { cx: x + w / 2.0, cy: y + h / 2.0, rx: w / 2.0, ry: h / 2.0, start: num(a, "start").unwrap_or(0.0), end: num(a, "end").unwrap_or(0.0), inner: num(a, "inner").unwrap_or(0.0), close: true }
            };
            let s = ed.add_element(Element::shape(if id == "draw.rect_primitive" { "Rectangle Primitive" } else { "Oval Primitive" }, Shape { path: prim.path(), fill, stroke, primitive: Some(prim), ..Default::default() })).ok_or("current layer is locked or hidden")?;
            Ok(json!({ "layer": s.layer + 1, "index": s.element }))
        }
        "shape_bone.add" => {
            let (h, t) = (pt(req(a, "x1")?, req(a, "y1")?), pt(req(a, "x2")?, req(a, "y2")?));
            // chain from a bone ending at h, else the shape under h
            let s = match ed.rig_bones().into_iter().find(|(_, _, _, tail)| tail.dist(h) < 1.0) {
                Some((s, ..)) => s,
                None => ed.hit_test(h).ok_or("no drawing object at x1, y1")?,
            };
            let b = ed.rig_add_bone(s, h, t).ok_or("shape bones go inside a drawing object")?;
            Ok(json!(b + 1))
        }
        "shape_bone.joint" => {
            let from = pt(req(a, "x")?, req(a, "y")?);
            let (sel, b, _, _) = ed.rig_bones().into_iter().min_by(|x, y| x.3.dist(from).partial_cmp(&y.3.dist(from)).unwrap()).ok_or("no shape bones")?;
            let ElementKind::Shape(Shape { rig: Some(r), .. }) = ed.element_at(sel).ok_or("no object")?.kind else { return Err("no shape bones".into()) };
            let cur = r.bones[b];
            let rotate = boolean(a, "rotate").unwrap_or(cur.rotate);
            let limit = joint_limit(a, cur.limit)?;
            let pin = boolean(a, "pin");
            let tail = r.posed()[b].1;
            ed.selection = vec![sel];
            ed.modify_selected("Joint", move |e| {
                if let ElementKind::Shape(Shape { rig: Some(r), .. }) = &mut e.kind {
                    if let Some(bone) = r.bones.get_mut(b) {
                        bone.rotate = rotate;
                        bone.limit = limit;
                        if let Some(p) = pin {
                            bone.pin = p.then_some(tail);
                        }
                    }
                }
            });
            Ok(json!(b + 1))
        }
        "shape_bone.pose" => {
            let from = pt(req(a, "x")?, req(a, "y")?);
            let (s, b, _, _) = ed.rig_bones().into_iter().min_by(|x, y| x.3.dist(from).partial_cmp(&y.3.dist(from)).unwrap()).ok_or("no shape bones")?;
            ed.begin_edit("Pose Shape Bones");
            ed.rig_ik(s, b, pt(req(a, "to_x")?, req(a, "to_y")?));
            Ok(Value::Null)
        }
        "selection.arrange" => {
            need_selection(ed)?;
            ed.arrange(boolean(a, "front").unwrap_or(true));
            Ok(Value::Null)
        }
        "import.sound" => {
            let path = string(a, "path").ok_or("missing 'path'")?;
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let name = string(a, "name").map(|s| s.to_string()).unwrap_or_else(|| std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or("Sound".into()));
            let probe = crate::sound::SoundAsset { id: 0, name: name.clone(), data: bytes };
            if crate::sound::has_decoder() && crate::sound::seconds(&probe).is_none() {
                return Err(format!("{path}: not a sound this build can read (WAV, MP3, OGG Vorbis, FLAC, AIFF)"));
            }
            Ok(json!(ed.add_sound(&name, probe.data)))
        }
        "timeline.set_sound" => {
            target(ed, a)?;
            let id = match a.get("sound") {
                None | Some(Value::Null) => None,
                Some(v) => Some(sound_arg(ed, v)?),
            };
            let sound = match id {
                None => None,
                Some(id) => {
                    let mut fs = crate::sound::FrameSound::new(id);
                    if let Some(s) = string(a, "sync") {
                        fs.sync = crate::sound::SoundSync::from_name(s).ok_or_else(|| format!("unknown sync '{s}'"))?;
                    }
                    fs.repeat = num(a, "repeat").map(|v| v.max(1.0) as u32).unwrap_or(1);
                    fs.forever = boolean(a, "loop").unwrap_or(false);
                    if let Some(e) = string(a, "effect") {
                        fs.effect = crate::sound::SoundEffect::from_name(e).ok_or_else(|| format!("unknown effect '{e}'"))?;
                    }
                    Some(fs)
                }
            };
            if ed.current_keyframe().is_none() {
                return Err("no frame at the target".into());
            }
            ed.set_frame_sound(sound);
            Ok(Value::Null)
        }
        "sound.list" => Ok(Value::Array(ed.doc.sounds.iter().map(|s| json!({ "id": s.id, "name": s.name, "bytes": s.data.len(), "seconds": crate::sound::seconds(s) })).collect())),
        "sound.delete" => {
            let id = sound_arg(ed, a.get("sound").ok_or("missing 'sound'")?)?;
            ed.delete_sound(id);
            Ok(Value::Null)
        }
        "lipsync.apply" => {
            let al = layer_arg(ed, a, "audio_layer")?.ok_or("missing 'audio_layer'")?;
            let mut map = vec![];
            if let Some(Value::Object(m)) = a.get("map") {
                for (k, v) in m {
                    let vis = crate::sound::Viseme::from_name(k).ok_or_else(|| format!("unknown mouth shape '{k}'"))?;
                    let f = v.as_u64().ok_or("map values are frame numbers")? as u32;
                    map.push((vis, f.saturating_sub(1)));
                }
            }
            Ok(json!(ed.lip_sync(al, &map)?))
        }
        "library.list" => Ok(Value::Array(
            ed.doc
                .library
                .iter()
                .map(|s| json!({ "id": s.id, "name": s.name, "kind": format!("{:?}", s.kind), "frames": s.timeline.frame_count(), "layers": s.timeline.layers.len() }))
                .collect(),
        )),
        "library.place" => {
            let id = symbol_arg(ed, a)?;
            if ed.layer_locked() {
                return Err("current layer is locked or hidden".into());
            }
            ed.place_symbol(id, pt(req(a, "x")?, req(a, "y")?));
            Ok(Value::Null)
        }
        "library.rename" => {
            let id = symbol_arg(ed, a)?;
            ed.rename_symbol(id, string(a, "name").ok_or("missing 'name'")?);
            Ok(Value::Null)
        }
        "library.delete" => {
            let id = symbol_arg(ed, a)?;
            ed.delete_symbol(id);
            Ok(Value::Null)
        }
        "symbol.edit" => {
            let id = symbol_arg(ed, a)?;
            ed.enter_symbol(id);
            Ok(Value::Null)
        }
        "group.edit" => {
            let s = *ed.selection.first().ok_or("select a group first")?;
            if !ed.enter_group(s) {
                return Err("the selection is not an editable group".into());
            }
            Ok(Value::Null)
        }
        "group.exit" => {
            if !ed.exit_group() {
                return Err("no group is being edited".into());
            }
            Ok(Value::Null)
        }
        "symbol.exit" => {
            ed.exit_to_scene();
            Ok(Value::Null)
        }
        "edit.undo" => Ok(json!(ed.undo())),
        "edit.redo" => Ok(json!(ed.redo())),
        other => Err(format!("unknown command '{other}'")),
    }
}

/// Reference text for help panes and generated helper modules.
pub fn reference() -> String {
    let mut s = String::new();
    for c in COMMANDS {
        s += &format!("{:<32} {:<60} {}\n", c.id, c.args, c.summary);
    }
    s
}

/// `weight: 100…900` or `bold: true/false` (700 / 400).
fn text_weight(a: &Value) -> Option<u16> {
    num(a, "weight").map(|w| (w.clamp(100.0, 900.0) / 100.0).round() as u16 * 100).or_else(|| boolean(a, "bold").map(|b| if b { 700 } else { 400 }))
}

fn prop_arg(a: &Value) -> Result<crate::motion::Prop, String> {
    let s = string(a, "prop").ok_or("prop is required")?;
    crate::motion::Prop::from_id(s).ok_or_else(|| format!("unknown property '{s}' (x, y, rotation, skew, scale_x, scale_y, alpha, brightness, rotation_x, rotation_y, depth, tint, filter1.blur_x …)"))
}

/// `"Quad In"`, `"none"`/null, `"custom"` or `[[x,y,in_dx,in_dy,out_dx,out_dy]...]`.
fn parse_ease(v: Option<&Value>) -> Result<Option<crate::motion::Ease>, String> {
    use crate::motion::{CurvePt, Ease};
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("simple") => Ok(None),
        Some(Value::String(s)) => Ease::from_name(s).map(Some).ok_or_else(|| format!("unknown ease '{s}' (e.g. 'Quad In', 'Cubic Out', 'Sine In Out', 'Back', 'Bounce', 'Elastic', 'Stop and Start', 'custom')")),
        Some(Value::Array(pts)) => {
            let mut points = vec![];
            for p in pts {
                let n: Vec<f32> = p.as_array().ok_or("each point is [x, y, ...]")?.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
                if n.len() < 2 {
                    return Err("each point needs at least x and y".into());
                }
                let g = |i: usize| n.get(i).copied().unwrap_or(0.0);
                points.push(CurvePt { x: g(0).clamp(0.0, 1.0), y: g(1), h_in: pt(g(2), g(3)), h_out: pt(g(4), g(5)) });
            }
            points.sort_by(|a, b| a.x.total_cmp(&b.x));
            if points.len() < 2 {
                return Err("a custom ease needs at least two points".into());
            }
            Ok(Some(Ease::Custom { points }))
        }
        _ => Err("ease must be a name, null or a list of points".into()),
    }
}

/// Scene by 1-based number or name.
fn scene_arg(ed: &Editor, a: &Value, k: &str) -> Result<Option<usize>, String> {
    let n = ed.doc.scene_count();
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(v)) => {
            let i = v.as_u64().unwrap_or(0) as usize;
            if i == 0 || i > n {
                return Err(format!("scene {i} out of range (1..={n})"));
            }
            Ok(Some(i - 1))
        }
        Some(Value::String(s)) => ed.doc.scenes().position(|t| t.name == *s).map(Some).ok_or_else(|| format!("no scene named '{s}'")),
        Some(o) => Err(format!("bad scene {o}")),
    }
}
