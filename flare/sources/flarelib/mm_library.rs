//! Library folders, and the Library's commands on its items: New Folder, rename and move,
//! Duplicate, and a symbol's linkage. An item (symbol, bitmap or sound) "Folder/Name" sits in
//! Folder; folders that hold nothing are kept as Animate keeps every folder, as `<DOMFolderItem>`s
//! in DOMDocument's `<folders>`, which the model leaves in the document's `extra`.

use crate::Result;
use crate::doc::{Document, Element, Symbol};
use crate::xfl_write::escape;

/// Every library folder, outermost first: those `<folders>` lists, empty ones included, and those
/// the items' names make.
pub fn folders(doc: &Document) -> Vec<String> {
    let listed = listed(doc).into_iter().map(|(name, _)| name);
    let mut all: Vec<String> = listed.flat_map(|f| [parents(&f), vec![f]].concat()).collect();
    all.extend(items(doc).flat_map(|name| parents(name)));
    all.sort();
    all.dedup();
    all
}

/// The folders holding `path`, outermost first: "a" and "a/b" for "a/b/c".
fn parents(path: &str) -> Vec<String> {
    path.match_indices('/').map(|(i, _)| path[..i].to_string()).collect()
}

/// An item's own name, without its folders.
pub fn leaf(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, leaf)| leaf)
}

/// The paths of the library's items: symbols, bitmaps and sounds.
fn items(doc: &Document) -> impl Iterator<Item = &String> {
    doc.library.keys().chain(doc.bitmaps().map(|b| &b.name)).chain(doc.sounds.keys())
}

/// Whether an item or folder of the library is at `path`, or inside it.
pub fn in_use(doc: &Document, path: &str) -> bool {
    let inside = format!("{path}/");
    items(doc).any(|n| n == path || n.starts_with(&inside)) || folders(doc).iter().any(|f| f == path)
}

/// New Folder: "untitled folder N" in folder `parent` ("" for the library's root), as Animate
/// names them. Returns its path.
pub fn new_folder(doc: &mut Document, parent: &str) -> String {
    let path = |n: u32| {
        let leaf = format!("untitled folder {n}");
        if parent.is_empty() { leaf } else { format!("{parent}/{leaf}") }
    };
    let path = (1..).map(path).find(|p| !in_use(doc, p)).unwrap_or_default();
    let mut all = with_attrs(doc);
    all.push((path.clone(), String::new()));
    set_listed(doc, &all);
    path
}

/// Renames or moves the item at `from`, a symbol or a folder with the symbols it holds, to the
/// path `to`, and every instance of what moved. Folders stay where they were, emptied or not.
/// Bitmaps and sounds, and what plays them, the editor moves along (its bitmaps.rs, sound.rs).
pub fn rename(doc: &mut Document, from: &str, to: &str) -> Result<()> {
    if to == from {
        return Ok(());
    }
    if to.split('/').any(|part| part.trim().is_empty()) {
        return Err("A library item needs a name.".into());
    }
    let inside = format!("{from}/");
    if to.starts_with(&inside) {
        return Err("A folder can't go inside itself.".into());
    }
    if in_use(doc, to) {
        return Err(format!("The name \"{}\" is already in use in the library.", leaf(to)).into());
    }
    let under = |name: &str| name == from || name.starts_with(&inside);
    let moved = |name: &str| format!("{to}{}", &name[from.len()..]);
    let mut all = with_attrs(doc);
    for (name, _) in &mut all {
        if under(name) {
            *name = moved(name);
        }
    }
    // Moved into a folder that isn't one yet, it becomes one.
    all.extend(parents(to).into_iter().map(|p| (p, String::new())));
    let names: Vec<String> = doc.library.keys().filter(|n| under(n)).cloned().collect();
    for name in names {
        if let Some(mut symbol) = doc.library.remove(&name) {
            let new = moved(&name);
            symbol.timeline.name = leaf(&new).to_string();
            doc.library.insert(new, symbol);
        }
    }
    all.sort();
    all.dedup_by(|a, b| a.0 == b.0);
    set_listed(doc, &all);
    for_instances(doc, &mut |symbol| {
        if under(symbol) {
            *symbol = moved(symbol);
        }
    });
    Ok(())
}

/// Takes folder `path`, and the folders in it, off the library's list; deleting a folder deletes
/// its symbols besides.
pub fn forget_folder(doc: &mut Document, path: &str) {
    let inside = format!("{path}/");
    let mut all = with_attrs(doc);
    all.retain(|(f, _)| f != path && !f.starts_with(&inside));
    set_listed(doc, &all);
}

/// The name Duplicate suggests for a copy of `name`: "<name> copy", then "<name> copy 2"...
pub fn copy_name(doc: &Document, name: &str) -> String {
    let base = format!("{name} copy");
    (1..).map(|n| if n == 1 { base.clone() } else { format!("{base} {n}") }).find(|n| !in_use(doc, n)).unwrap_or(base)
}

/// Duplicate Symbol: a copy of symbol `name` at `copy`, its own symbol from now on.
pub fn duplicate(doc: &mut Document, name: &str, copy: &str) -> Result<()> {
    let mut symbol = doc.library.get(name).cloned().ok_or("That symbol is no longer in the library.")?;
    if copy.split('/').any(|part| part.trim().is_empty()) {
        return Err("A symbol needs a name.".into());
    }
    if in_use(doc, copy) {
        return Err(format!("The name \"{}\" is already in use in the library.", leaf(copy)).into());
    }
    symbol.timeline.name = leaf(copy).to_string();
    // Animate's id for a library item is the original's; the copy goes without, as new symbols do.
    symbol.extra.set("itemID", None);
    doc.library.insert(copy.to_string(), symbol);
    Ok(())
}

/// The class a symbol is exported for ActionScript as (Symbol Properties' Linkage), if it is.
pub fn linkage(symbol: &Symbol) -> Option<&str> {
    let class = symbol.extra.get("linkageClassName");
    class.filter(|_| symbol.extra.get("linkageExportForAS") == Some("true"))
}

/// Exports the symbol for ActionScript as `class`, or (None) not at all, in XFL's linkage attributes.
pub fn set_linkage(symbol: &mut Symbol, class: Option<&str>) {
    match class {
        Some(class) => {
            symbol.extra.set("linkageExportForAS", Some("true"));
            symbol.extra.set("linkageClassName", Some(class));
        }
        None => symbol.extra.attrs.retain(|(k, _)| !k.starts_with("linkage")),
    }
}

/// Runs `f` on the symbol name of every instance in the document.
fn for_instances(doc: &mut Document, f: &mut dyn FnMut(&mut String)) {
    fn walk(elements: &mut [Element], f: &mut dyn FnMut(&mut String)) {
        for el in elements {
            match el {
                Element::Instance(i) => f(&mut i.symbol),
                Element::Group(g) => walk(g, f),
                _ => {}
            }
        }
    }
    let timelines = doc.scenes.iter_mut().chain(doc.library.values_mut().map(|s| &mut s.timeline));
    for key in timelines.flat_map(|tl| &mut tl.layers).flat_map(|l| &mut l.frames) {
        walk(&mut key.elements, f);
    }
}

/// Where the document's `<folders>` element is kept: in `extra.before` (true) or `after`, and at which index.
fn folders_slot(doc: &Document) -> Option<(bool, usize)> {
    let is = |xml: &String| xml.trim_start().starts_with("<folders");
    let before = doc.extra.before.iter().position(is).map(|i| (true, i));
    before.or_else(|| doc.extra.after.iter().position(is).map(|i| (false, i)))
}

/// The folders `<folders>` lists, each with the rest of its DOMFolderItem's attributes as written.
fn listed(doc: &Document) -> Vec<(String, String)> {
    let Some((before, i)) = folders_slot(doc) else { return Vec::new() };
    let xml = if before { &doc.extra.before[i] } else { &doc.extra.after[i] };
    let Ok(parsed) = roxmltree::Document::parse(xml) else { return Vec::new() };
    let items = parsed.root_element().children().filter(|n| n.has_tag_name("DOMFolderItem"));
    items
        .filter_map(|n| {
            let rest = n.attributes().filter(|a| a.name() != "name").map(|a| format!(" {}=\"{}\"", a.name(), escape(a.value())));
            Some((n.attribute("name")?.to_string(), rest.collect()))
        })
        .collect()
}

/// Every folder, with its attributes where `<folders>` lists it.
fn with_attrs(doc: &Document) -> Vec<(String, String)> {
    let listed = listed(doc);
    let attrs = |f: &str| listed.iter().find(|(name, _)| name == f).map(|(_, rest)| rest.clone()).unwrap_or_default();
    folders(doc).into_iter().map(|f| { let rest = attrs(&f); (f, rest) }).collect()
}

/// Writes `folders` as the document's `<folders>`, where it was or first, where Animate writes it.
// ponytail: new folders get no itemID, as symbols' <Include>s get none; add one if Animate wants it.
fn set_listed(doc: &mut Document, folders: &[(String, String)]) {
    let items: String = folders.iter().map(|(name, rest)| format!("\n    <DOMFolderItem name=\"{}\"{rest}/>", escape(name))).collect();
    let xml = format!("<folders>{items}\n  </folders>");
    match (folders_slot(doc), folders.is_empty()) {
        (Some((true, i)), true) => drop(doc.extra.before.remove(i)),
        (Some((false, i)), true) => drop(doc.extra.after.remove(i)),
        (Some((true, i)), false) => doc.extra.before[i] = xml,
        (Some((false, i)), false) => doc.extra.after[i] = xml,
        (None, true) => {}
        (None, false) => doc.extra.before.insert(0, xml),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::*;

    fn symbol(name: &str) -> Symbol {
        Symbol { kind: SymbolKind::MovieClip, timeline: Timeline::new(name), extra: Extra::default() }
    }

    fn instance(symbol: &str) -> Element {
        let (matrix, color) = (Matrix::IDENTITY, ColorTransform::IDENTITY);
        let kind = SymbolKind::MovieClip;
        Element::Instance(Instance { symbol: symbol.into(), kind, matrix, pivot: (0.0, 0.0), color, looping: Loop::Loop, first_frame: 0, extra: Extra::default() })
    }

    #[test]
    fn empty_folders_stay_and_round_trip() {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        doc.library.insert("Props/Ball".into(), symbol("Ball"));
        doc.scenes[0].layers[0].frames[0].elements.push(instance("Props/Ball"));
        assert_eq!(folders(&doc), ["Props"]);
        assert_eq!(new_folder(&mut doc, ""), "untitled folder 1");
        assert_eq!(new_folder(&mut doc, "Props"), "Props/untitled folder 1");
        assert_eq!(folders(&doc), ["Props", "Props/untitled folder 1", "untitled folder 1"]);

        // Into the empty folder: the instance follows, and Props stays, though empty now.
        rename(&mut doc, "Props/Ball", "untitled folder 1/Ball").unwrap();
        let Element::Instance(i) = &doc.scenes[0].layers[0].frames[0].elements[0] else { panic!() };
        assert_eq!(i.symbol, "untitled folder 1/Ball");
        assert_eq!(doc.library["untitled folder 1/Ball"].timeline.name, "Ball");
        assert!(folders(&doc).contains(&"Props".to_string()));
        // A folder moves with all it holds; it can't go inside itself, nor onto a taken name.
        rename(&mut doc, "untitled folder 1", "Props/Toys").unwrap();
        assert!(doc.library.contains_key("Props/Toys/Ball"));
        assert!(rename(&mut doc, "Props", "Props/Toys/Props").is_err());
        assert!(rename(&mut doc, "Props/Toys/Ball", "Props/untitled folder 1").is_err());
        assert!(rename(&mut doc, "Props/Toys/Ball", "Props/ /Ball").is_err());

        // Saved and read back, the empty folder is still there.
        let dir = std::env::temp_dir().join(format!("minnamate-folders-{}", std::process::id()));
        crate::xfl_write::save(&doc, &dir).unwrap();
        let back = crate::xfl::load(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(folders(&back), ["Props", "Props/Toys", "Props/untitled folder 1"]);
        forget_folder(&mut doc, "Props");
        assert!(listed(&doc).is_empty() && folders(&doc) == ["Props", "Props/Toys"], "{:?}", folders(&doc));
    }

    #[test]
    fn animates_folder_items_keep_their_ids() {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        doc.extra.before.push(r#"<folders><DOMFolderItem name="Art" itemID="5f1-0001" isExpanded="true"/></folders>"#.into());
        rename(&mut doc, "Art", "Artwork").unwrap();
        assert_eq!(listed(&doc), [("Artwork".to_string(), r#" itemID="5f1-0001" isExpanded="true""#.to_string())]);
    }

    #[test]
    fn duplicates_and_linkage() {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        let mut ball = symbol("Ball");
        ball.extra.set("itemID", Some("5f1-0002"));
        doc.library.insert("Props/Ball".into(), ball);
        assert_eq!(copy_name(&doc, "Props/Ball"), "Props/Ball copy");
        duplicate(&mut doc, "Props/Ball", "Props/Ball copy").unwrap();
        assert_eq!(copy_name(&doc, "Props/Ball"), "Props/Ball copy 2");
        assert_eq!(doc.library["Props/Ball copy"].timeline.name, "Ball copy");
        assert_eq!(doc.library["Props/Ball copy"].extra.get("itemID"), None, "the original's id stays the original's");
        assert!(duplicate(&mut doc, "Props/Ball", "Props/Ball copy").is_err());

        let ball = doc.library.get_mut("Props/Ball").unwrap();
        ball.extra.set("linkageBaseClass", Some("MovieClip"));
        set_linkage(ball, Some("Ball"));
        assert_eq!(linkage(ball), Some("Ball"));
        set_linkage(ball, None);
        assert!(linkage(ball).is_none() && ball.extra.attrs == [("itemID".to_string(), "5f1-0002".to_string())]);
    }
}
