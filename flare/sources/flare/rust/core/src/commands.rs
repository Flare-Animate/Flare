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
    "shape_hint.add", "x?, y? (start keyframe), to_x?, to_y? (end keyframe)", "Add a shape hint to the shape tween at the playhead; returns its letter";
    "shape_hint.clear", "", "Remove all shape hints from the shape tween at the playhead";
    "motion.info", "layer?, frame?", "The motion tween at the target: span, pivot and property keyframes";
    "motion.set_key", "prop ('x'|'y'|'rotation'|'skew'|'scale_x'|'scale_y'|'alpha'|'brightness'), frame, value, layer?", "Property keyframe in the motion tween containing frame (scale/alpha in 0..1, angles in degrees)";
    "motion.remove_key", "frame, prop? (all), layer?", "Remove property keyframe(s)";
    "motion.set_ease", "prop ('position' or a property), ease (preset name, 'custom', or 'none'), layer?, frame?", "Ease one property of the motion tween (Motion Editor)";
    "timeline.set_label", "label, layer?, frame?", "Frame label on the keyframe at the target";
    "timeline.set_script", "script, layer?, frame?", "Frame script (JavaScript) on the keyframe at the target";
    "timeline.move_frames", "by, layer?, frame?, to_frame?", "Drag frames left/right";
    "layer.list", "", "All layers with flags and frame counts";
    "layer.add", "name?", "New layer above the current one; returns its number";
    "layer.delete", "layer?", "Delete a layer";
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
    "draw.path", "points [[x,y]...], closed?, smooth?, fill?, stroke?, stroke_width?", "Path through points";
    "draw.text", "x, y, text, size? (24), color?, font? ('sans'|'mono'|system family), align?", "Static text";
    "select.all", "", "Select everything editable on the current frame";
    "select.none", "", "Clear the selection";
    "select.layer", "layer?", "Select every object on one layer (default: current) at the current frame";
    "select.at", "x, y, add?", "Select the topmost object at a point";
    "select.rect", "x, y, w, h, add?", "Select objects fully inside a box";
    "selection.info", "", "Selected objects with kind, name and bounds";
    "selection.move", "dx, dy", "Move the selection";
    "selection.set_position", "x, y", "Move the selection's top-left to x, y";
    "selection.scale", "sx, sy?", "Scale around the selection centre";
    "selection.rotate", "degrees", "Rotate around the selection centre";
    "selection.set", "fill?, stroke?, stroke_width?, alpha?, tint?, tint_amount?, name?, text?, size?", "Change properties of every selected object";
    "selection.delete", "", "Delete the selection";
    "selection.duplicate", "", "Duplicate (offset by 10 px)";
    "selection.to_symbol", "name, kind? ('graphic'|'movieclip'|'button')", "F8: convert to symbol; returns its id";
    "selection.break_apart", "", "Ctrl+B on a symbol instance";
    "selection.skew", "h? (deg), v? (deg)", "Skew horizontally / vertically around the selection centre";
    "selection.set_3d", "rx?, ry?, rz? (deg), z?", "3D rotation and depth (perspective-projected)";
    "selection.distort", "corners [[x,y] x4: TL, TR, BR, BL]", "Distort shapes onto a quadrilateral";
    "selection.set_gradient", "kind ('linear'|'radial'|'none'), colors [\"#RRGGBB\"...]", "Gradient fill on selected shapes";
    "bone.add", "x1, y1, x2, y2", "Bone from the object at (x1,y1) to the object at (x2,y2) on the current layer";
    "bone.list", "layer?", "Bones on a layer";
    "bone.clear", "layer?", "Remove all bones on a layer";
    "bone.drag", "x, y, to_x, to_y", "IK: drag the bonded object point at (x,y) to (to_x,to_y), posing the chain";
    "camera.add", "", "Add a camera layer to the scene";
    "camera.set", "x?, y?, zoom?, rotation?, frame?", "Camera centre, zoom (1 = 100%) and rotation at a frame (makes a keyframe)";
    "camera.remove", "", "Remove the camera layer";
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
    "selection.stroke_style", "dash? ('solid'|'dashed'|'dotted'), cap? ('none'|'round'|'square'), join? ('miter'|'round'|'bevel'), dash_len?, gap?", "Stroke style of selected drawing objects";
    "selection.bitmap_fill", "bitmap (name or id, null = none)", "Fill selected drawing objects with a library bitmap";
    "draw.rect_primitive", "x, y, w, h, radius? or radii [tl,tr,br,bl], fill?, stroke?", "Rectangle Primitive (corner radii stay editable)";
    "draw.oval_primitive", "x, y, w, h, start?, end?, inner? (%), fill?, stroke?", "Oval Primitive (angles and inner radius stay editable)";
    "shape_bone.add", "x1, y1, x2, y2", "Bone inside the drawing object at (x1,y1), from (x1,y1) to (x2,y2); chains onto a bone ending at (x1,y1)";
    "shape_bone.pose", "x, y, to_x, to_y", "Drag the shape bone whose tail is at (x,y) to (to_x,to_y) (IK)";
    "selection.swap_symbol", "symbol (name or id)", "Swap Symbol: selected instances show another symbol, keeping transform and effects";
    "selection.color_effect", "style ('none'|'brightness'|'tint'|'advanced'|'alpha'), amount? (-100..100 or 0..100), color?, mul? [r,g,b,a %], add? [r,g,b,a]", "Color effect of the selected objects";
    "selection.group", "", "Ctrl+G: group the selection (per layer)";
    "selection.ungroup", "", "Ctrl+Shift+G: ungroup";
    "selection.align", "op ('left'|'hcenter'|'right'|'top'|'vcenter'|'bottom'|'dist_left'|'dist_hcenter'|'dist_right'|'dist_top'|'dist_vcenter'|'dist_bottom'|'match_width'|'match_height'|'match_size'|'space_h'|'space_v'), to_stage?", "Align panel";
    "selection.add_filter", "type ('drop_shadow'|'blur'|'glow'|'bevel'|'adjust_color'), …filter fields (blur_x, blur_y, strength, angle, distance, color, brightness, contrast, saturation, hue…)", "Add a filter to the selected objects";
    "selection.clear_filters", "", "Remove all filters";
    "selection.set_blend", "mode ('normal'|'multiply'|'screen'|…)", "Blend mode";
    "import.image", "path (PNG), x?, y?, place? (true)", "Import a PNG into the library (and place it)";
    "library.list", "", "Library items";
    "library.place", "symbol (name or id), x, y", "Place an instance on the current layer";
    "library.rename", "symbol, name", "Rename a library item";
    "library.delete", "symbol", "Delete a library item and its instances";
    "symbol.edit", "symbol", "Enter a symbol's timeline";
    "symbol.exit", "", "Return to the main scene";
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
fn color_arg(a: &Value, k: &str) -> Result<Option<Option<Color>>, String> {
    match a.get(k) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(s)) => Color::from_hex(s)
            .map(|c| Some(Some(c)))
            .ok_or_else(|| format!("bad colour '{s}' for '{k}' (use \"#RRGGBB\")")),
        Some(Value::Array(v)) if v.len() >= 3 => {
            let c = |i: usize| {
                v.get(i)
                    .and_then(Value::as_f64)
                    .unwrap_or(255.0)
                    .clamp(0.0, 255.0) as u8
            };
            Ok(Some(Some(Color {
                r: c(0),
                g: c(1),
                b: c(2),
                a: c(3),
            })))
        }
        Some(other) => Err(format!("bad colour {other} for '{k}'")),
    }
}

fn hex(c: Color) -> String {
    if c.a == 255 {
        c.to_hex()
    } else {
        format!("{}{:02X}", c.to_hex(), c.a)
    }
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
        Some(Value::String(s)) => ed
            .timeline()
            .layers
            .iter()
            .position(|l| l.name == *s)
            .map(Some)
            .ok_or_else(|| format!("no layer named '{s}'")),
        Some(other) => Err(format!("bad layer {other}")),
    }
}

fn frame_arg(a: &Value, k: &str) -> Result<Option<u32>, String> {
    match a.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let f = v
                .as_u64()
                .ok_or_else(|| format!("'{k}' must be a frame number"))?;
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
    ed.select_frames(FrameSel::between(
        (l0, f0),
        (tl.unwrap_or(l0), tf.unwrap_or(f0)),
    ));
    Ok(())
}

fn symbol_arg(ed: &Editor, a: &Value) -> Result<SymbolId, String> {
    match a.get("symbol") {
        Some(Value::Number(n)) => {
            let id = n.as_u64().unwrap_or(0) as SymbolId;
            ed.doc
                .symbol(id)
                .map(|_| id)
                .ok_or_else(|| format!("no symbol with id {id}"))
        }
        Some(Value::String(s)) => ed
            .doc
            .library
            .iter()
            .find(|x| x.name == *s)
            .map(|x| x.id)
            .ok_or_else(|| format!("no library item named '{s}'")),
        _ => Err("missing 'symbol' (name or id)".into()),
    }
}

fn shape_style(
    a: &Value,
    default_fill: Option<Color>,
) -> Result<(Option<Color>, Option<Stroke>), String> {
    let fill = color_arg(a, "fill")?.unwrap_or(default_fill);
    let width = num(a, "stroke_width").unwrap_or(1.0);
    let stroke = color_arg(a, "stroke")?
        .unwrap_or(None)
        .map(|c| Stroke { color: c, width });
    Ok((fill, stroke))
}

fn add_shape(
    ed: &mut Editor,
    name: &str,
    path: Path,
    fill: Option<Color>,
    stroke: Option<Stroke>,
) -> Result<Value, String> {
    if fill.is_none() && stroke.is_none() {
        return Err("shape needs a fill or a stroke".into());
    }
    if ed.script_merge && name != "Text" {
        if !ed.merge_draw(&path, fill, None, stroke) {
            return Err("current layer is locked or hidden".into());
        }
        return Ok(json!({ "layer": ed.layer + 1, "merged": true }));
    }
    let s = ed
        .add_element(Element::shape(
            name,
            Shape {
                path,
                fill,
                stroke,
                ..Default::default()
            },
        ))
        .ok_or("current layer is locked or hidden")?;
    Ok(json!({ "layer": s.layer + 1, "index": s.element }))
}

fn element_json(ed: &Editor, s: Selection) -> Value {
    let Some(el) = ed.element_at(s) else {
        return Value::Null;
    };
    let (kind, extra) = match &el.kind {
        ElementKind::Shape(sh) => (
            "shape",
            json!({ "fill": sh.fill.map(hex), "stroke": sh.stroke.map(|s| hex(s.color)), "stroke_width": sh.stroke.map(|s| s.width) }),
        ),
        ElementKind::Instance(i) => (
            "instance",
            json!({ "symbol": ed.doc.symbol(i.symbol).map(|s| s.name.clone()) }),
        ),
        ElementKind::Text(t) => (
            "text",
            json!({ "text": t.text, "size": t.size, "color": hex(t.color), "font": t.family }),
        ),
        ElementKind::Group(c) => ("group", json!({ "children": c.len() })),
        ElementKind::Bitmap(b) => (
            "bitmap",
            json!({ "bitmap": ed.doc.bitmap(b.bitmap).map(|a| a.name.clone()) }),
        ),
        ElementKind::Canvas(c) => (
            "merge_shape",
            json!({ "fills": c.fills.len(), "strokes": c.strokes.len() }),
        ),
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
            for p in crate::motion::Prop::ALL {
                let keys: Vec<Value> = m.keys_of(p).into_iter().map(|(f, v)| json!([st + f + 1, v])).collect();
                if !keys.is_empty() {
                    props.insert(prop_id(p).into(), Value::Array(keys));
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
        "timeline.set_label" => {
            target(ed, a)?;
            ed.set_frame_label(string(a, "label").unwrap_or(""));
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
                .map(|(i, l)| json!({ "layer": i + 1, "name": l.name, "visible": l.visible, "locked": l.locked, "outline": l.outline, "frames": l.end, "keyframes": l.keyframes.keys().map(|k| k + 1).collect::<Vec<_>>() }))
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
            add_shape(ed, "Path", path, if closed { fill } else { None }, stroke)
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
        "selection.add_filter" => {
            need_selection(ed)?;
            let ty = string(a, "type").ok_or("missing 'type'")?;
            let mut f = crate::fx::Filter::default_of(ty).ok_or_else(|| format!("unknown filter '{ty}' (drop_shadow, blur, glow, bevel, adjust_color)"))?;
            // overlay given fields onto the defaults
            let mut v = serde_json::to_value(&f).map_err(|e| e.to_string())?;
            if let (Some(obj), Some(args)) = (v.as_object_mut(), a.as_object()) {
                for (k, val) in args {
                    if k == "type" || !obj.contains_key(k) {
                        continue;
                    }
                    let val = match (k.as_str(), val.as_str().and_then(Color::from_hex)) {
                        ("color" | "shadow" | "highlight", Some(c)) => serde_json::to_value(c).map_err(|e| e.to_string())?,
                        _ => val.clone(),
                    };
                    obj.insert(k.clone(), val);
                }
            }
            f = serde_json::from_value(v).map_err(|e| format!("bad filter fields: {e}"))?;
            ed.modify_selected("Add Filter", |e| e.filters.push(f.clone()));
            Ok(Value::Null)
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
            let asset = crate::bitmap::BitmapAsset::from_png(&name, bytes)?;
            let id = ed.add_bitmap(asset);
            if boolean(a, "place").unwrap_or(true) {
                let at = pt(num(a, "x").unwrap_or(0.0), num(a, "y").unwrap_or(0.0));
                ed.place_bitmap(id, at).ok_or("current layer is locked or hidden")?;
            }
            Ok(json!(id))
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
            ed.modify_selected("Gradient", |e| {
                if let ElementKind::Shape(sh) = &mut e.kind {
                    sh.gradient = kind.and_then(|k| sh.path.bounds().map(|(lo, hi)| Gradient::fit(k, stops.clone(), lo, hi)));
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
        "camera.add" => Ok(json!(ed.add_camera() + 1)),
        "camera.remove" => {
            ed.remove_camera();
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
            let (dash, cap, join) = (string(a, "dash").map(str::to_string), string(a, "cap").map(str::to_string), string(a, "join").map(str::to_string));
            let (dl, gap) = (num(a, "dash_len"), num(a, "gap"));
            ed.modify_selected("Stroke Style", move |e| {
                if let ElementKind::Shape(sh) = &mut e.kind {
                    let st = &mut sh.stroke_style;
                    match dash.as_deref() {
                        Some("dashed") => st.dash = Dash::Dashed,
                        Some("dotted") => st.dash = Dash::Dotted,
                        Some("solid") => st.dash = Dash::Solid,
                        _ => {}
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

fn prop_id(p: crate::motion::Prop) -> &'static str {
    use crate::motion::Prop::*;
    match p {
        X => "x",
        Y => "y",
        Rotation => "rotation",
        Skew => "skew",
        ScaleX => "scale_x",
        ScaleY => "scale_y",
        Alpha => "alpha",
        Brightness => "brightness",
    }
}

fn prop_arg(a: &Value) -> Result<crate::motion::Prop, String> {
    let s = string(a, "prop").ok_or("prop is required")?;
    crate::motion::Prop::ALL
        .into_iter()
        .find(|p| prop_id(*p) == s.to_ascii_lowercase() || p.name().eq_ignore_ascii_case(s))
        .ok_or_else(|| format!("unknown property '{s}' (x, y, rotation, skew, scale_x, scale_y, alpha, brightness)"))
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
        Some(Value::String(s)) => ed
            .doc
            .scenes()
            .position(|t| t.name == *s)
            .map(Some)
            .ok_or_else(|| format!("no scene named '{s}'")),
        Some(o) => Err(format!("bad scene {o}")),
    }
}
