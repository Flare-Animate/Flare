//! Library operations: Convert to Symbol (F8).

use crate::Result;
use crate::doc::*;
use crate::hit::{self, ElementRef};

/// Moves the elements `refs` of scene `scene` into a new library symbol `name`,
/// on one layer, with `registration` (in the scene's coordinates) as the
/// symbol's origin, and puts an instance of it where the first of them was.
/// Returns the instance.
pub fn convert_to_symbol(
    doc: &mut Document,
    scene: usize,
    frame: u32,
    refs: &[ElementRef],
    name: &str,
    kind: SymbolKind,
    registration: Point,
) -> Result<ElementRef> {
    check_name(doc, name)?;
    let timeline = doc.scenes.get(scene).ok_or("no such scene")?;
    let center = hit::bounds(doc, timeline, frame, refs).map(|b| ((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0));
    let pivot = center.map_or((0.0, 0.0), |c| (c.0 - registration.0, c.1 - registration.1));

    // Highest index first, so removing one doesn't shift the next. The instance
    // takes the place of the bottom-most element of the topmost layer involved.
    let mut refs = refs.to_vec();
    refs.sort_by_key(|r| std::cmp::Reverse((r.layer, r.keyframe, r.element)));
    refs.dedup();
    let target = *refs.iter().min_by_key(|r| (r.layer, r.element)).ok_or("nothing selected")?;
    let layers = &mut doc.scenes[scene].layers;
    let mut moved = Vec::new();
    for r in &refs {
        let key = layers.get_mut(r.layer).and_then(|l| l.frames.get_mut(r.keyframe)).ok_or("selection is out of date")?;
        if r.element >= key.elements.len() {
            return Err("selection is out of date".into());
        }
        let mut el = key.elements.remove(r.element);
        hit::translate(&mut el, -registration.0, -registration.1);
        moved.push(el);
    }
    // `moved` runs top-most first; the symbol stacks bottom first.
    moved.reverse();

    let frames = vec![Frame { index: 0, duration: 1, tween: Tween::None, elements: moved, extra: Extra::default() }];
    let layer = Layer { name: "Layer_1".into(), kind: LayerKind::Normal, frames, extra: Extra::default() };
    let timeline = Timeline { name: name.to_string(), layers: vec![layer], extra: Extra::default() };
    doc.library.insert(name.to_string(), Symbol { kind, timeline, extra: Extra::default() });

    let instance = Instance {
        symbol: name.to_string(),
        kind,
        matrix: Matrix { tx: registration.0, ty: registration.1, ..Matrix::IDENTITY },
        pivot,
        color: ColorTransform::IDENTITY,
        looping: Loop::Loop,
        first_frame: 0,
        extra: Extra::default(),
    };
    let key = &mut doc.scenes[scene].layers[target.layer].frames[target.keyframe];
    let at = target.element.min(key.elements.len());
    key.elements.insert(at, Element::Instance(instance));
    Ok(ElementRef { element: at, ..target })
}

/// Insert > New Symbol: an empty symbol `name` in the library.
pub fn new_symbol(doc: &mut Document, name: &str, kind: SymbolKind) -> Result<()> {
    check_name(doc, name)?;
    doc.library.insert(name.to_string(), Symbol { kind, timeline: Timeline::new(name), extra: Extra::default() });
    Ok(())
}

fn check_name(doc: &Document, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err("A symbol needs a name.".into());
    }
    // A folder's name is taken too.
    if crate::library::in_use(doc, name) {
        return Err(format!("The name \"{name}\" is already in use in the library.").into());
    }
    Ok(())
}

/// Whether `elements` draw symbol `name`: one is an instance of it, or of a
/// symbol drawing it at any depth. Inside `name` they'd draw it in itself
/// without end, so pasting there refuses them.
// ponytail: a file already holding a symbol inside itself still draws it 32 deep (display.rs);
// keeping the symbols being drawn on a stack there would stop that.
pub fn draws<'a>(doc: &'a Document, elements: impl IntoIterator<Item = &'a Element>, name: &str) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut todo: Vec<&Element> = elements.into_iter().collect();
    while let Some(el) = todo.pop() {
        match el {
            Element::Instance(i) if i.symbol == name => return true,
            Element::Instance(i) => {
                if seen.insert(&i.symbol)
                    && let Some(s) = doc.library.get(&i.symbol)
                {
                    todo.extend(s.timeline.layers.iter().flat_map(|l| &l.frames).flat_map(|k| &k.elements));
                }
            }
            Element::Group(members) => todo.extend(members),
            _ => {}
        }
    }
    false
}

/// The first "Symbol N" name the library doesn't use yet, as Animate suggests.
pub fn next_symbol_name(doc: &Document) -> String {
    (1..).map(|n| format!("Symbol {n}")).find(|n| !doc.library.contains_key(n)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{draw, render::render};

    fn red() -> Paint {
        Paint::Solid([1.0, 0.0, 0.0, 1.0])
    }

    #[test]
    fn converted_shapes_render_the_same_from_the_symbol() {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        let frame = &mut doc.scenes[0].layers[0].frames[0];
        draw::add(frame, draw::rectangle([(10.0, 10.0), (30.0, 30.0)], 0.0, None, Some(red())));
        draw::add(frame, draw::oval([(50.0, 50.0), (80.0, 70.0)], None, Some(red())));
        let before = render(&doc, 0, 1.0).unwrap().0;

        let refs = [ElementRef { layer: 0, keyframe: 0, element: 0 }, ElementRef { layer: 0, keyframe: 0, element: 1 }];
        let inst = convert_to_symbol(&mut doc, 0, 0, &refs, "Symbol 1", SymbolKind::MovieClip, (10.0, 10.0)).unwrap();

        assert_eq!(inst, ElementRef { layer: 0, keyframe: 0, element: 0 });
        assert_eq!(doc.scenes[0].layers[0].frames[0].elements.len(), 1);
        let Element::Instance(i) = &doc.scenes[0].layers[0].frames[0].elements[0] else { panic!("not an instance") };
        assert_eq!((i.matrix.tx, i.matrix.ty, i.kind), (10.0, 10.0, SymbolKind::MovieClip));
        assert_eq!(doc.library["Symbol 1"].timeline.layers[0].frames[0].elements.len(), 2);
        assert_eq!(render(&doc, 0, 1.0).unwrap().0.data(), before.data(), "looks the same after converting");
    }

    #[test]
    fn names_must_be_new() {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        draw::add(&mut doc.scenes[0].layers[0].frames[0], draw::line((0.0, 0.0), (10.0, 10.0), Stroke {
            width: 1.0,
            paint: red(),
            cap: Cap::Round,
            join: Join::Round,
        }));
        let r = [ElementRef { layer: 0, keyframe: 0, element: 0 }];
        assert_eq!(next_symbol_name(&doc), "Symbol 1");
        convert_to_symbol(&mut doc, 0, 0, &r, "Symbol 1", SymbolKind::Graphic, (0.0, 0.0)).unwrap();
        assert_eq!(next_symbol_name(&doc), "Symbol 2");
        assert!(convert_to_symbol(&mut doc, 0, 0, &r, "Symbol 1", SymbolKind::Graphic, (0.0, 0.0)).is_err());
        assert!(convert_to_symbol(&mut doc, 0, 0, &r, "  ", SymbolKind::Graphic, (0.0, 0.0)).is_err());
        assert!(new_symbol(&mut doc, "Symbol 1", SymbolKind::Graphic).is_err());
    }

    #[test]
    fn what_draws_a_symbol() {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        new_symbol(&mut doc, "A", SymbolKind::Graphic).unwrap();
        new_symbol(&mut doc, "B", SymbolKind::Graphic).unwrap();
        let instance = |symbol: &str| {
            let (matrix, color, looping) = (Matrix::IDENTITY, ColorTransform::IDENTITY, Loop::Loop);
            let kind = SymbolKind::Graphic;
            Element::Instance(Instance { symbol: symbol.into(), kind, matrix, pivot: (0.0, 0.0), color, looping, first_frame: 0, extra: Extra::default() })
        };
        // B holds A in a group; A holds itself already, as a file might.
        for (s, el) in [("B", Element::Group(vec![instance("A")])), ("A", instance("A"))] {
            doc.library.get_mut(s).unwrap().timeline.layers[0].frames[0].elements.push(el);
        }
        assert!(draws(&doc, &[instance("B")], "A"), "through B");
        assert!(!draws(&doc, &[instance("A")], "B"), "A's loop ends");
    }

    #[test]
    fn new_symbols_start_empty() {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        new_symbol(&mut doc, "Walk", SymbolKind::Graphic).unwrap();
        let walk = &doc.library["Walk"];
        assert_eq!((walk.kind, walk.timeline.name.as_str(), walk.timeline.frame_count()), (SymbolKind::Graphic, "Walk", 1));
        assert!(walk.timeline.layers[0].frames[0].elements.is_empty());
        assert_eq!(next_symbol_name(&doc), "Symbol 1");
    }
}
