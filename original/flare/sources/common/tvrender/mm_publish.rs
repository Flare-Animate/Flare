//! Publish settings and File > Publish. A document's settings live in its XFL's
//! PublishSettings.xml: a `<flash_profile>` per profile, made of sections (`<PublishFormatProperties>`,
//! `<PublishFlashProperties>`...) of one-value elements. The model reads the settings minnamate
//! publishes with and writes them back into the sections as read, everything else as it was.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use roxmltree::Node;

use crate::Result;
use crate::doc::{Document, Tween};
use crate::export::{self, Format};
use crate::xfl_write::{escape, hex};

/// The settings' file in the XFL.
pub const FILE: &str = "PublishSettings.xml";

/// The players the SWF can target, newest last: (player version, SWF version).
pub fn players() -> Vec<(String, u8)> {
    let mut list = vec![("10.3".to_string(), 12), ("11.0".to_string(), 13)];
    list.extend((1..=9).map(|minor| (format!("11.{minor}"), 13 + minor)));
    list.extend((12..=32).map(|major| (format!("{major}.0"), major as u8 + 11)));
    list
}

/// A document's publish profiles, and which one Publish uses.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub profiles: Vec<Profile>,
    pub current: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { profiles: vec![Profile::new("Default")], current: 0 }
    }
}

/// A file a profile publishes: whether it does, and its name, relative to the document's folder
/// (empty for the document's own name).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Target {
    pub on: bool,
    pub file: String,
}

/// A still or animated image a profile publishes, of the first frame or (an animated GIF) all.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub target: Target,
    /// Width and height in pixels; None matches the stage.
    // ponytail: drawn at the width's scale, so a height out of proportion is ignored.
    pub size: Option<(u32, u32)>,
    /// JPEG's quality, 1..=100.
    pub quality: u8,
    /// PNG: the stage colour left out.
    pub transparent: bool,
    /// GIF: every frame rather than the first.
    pub animated: bool,
    /// An animated GIF plays this many more times; None loops for ever.
    pub repeat: Option<u16>,
}

impl Image {
    fn new(on: bool) -> Image {
        Image { target: Target { on, file: String::new() }, size: None, quality: 80, transparent: false, animated: false, repeat: None }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub name: String,
    pub swf: Target,
    /// The SWF version its player reads.
    pub version: u8,
    /// Deflate (LZMA isn't built).
    pub compress: bool,
    /// For bitmaps in the SWF.
    // ponytail: only kept: a JPEG goes into the SWF as imported, the rest lossless (bitmap.rs).
    pub jpeg_quality: u8,
    pub include_hidden: bool,
    /// A web page that plays the SWF through Ruffle.
    pub html: Target,
    pub gif: Image,
    pub jpeg: Image,
    pub png: Image,
    /// The profile's sections as read: tag, attributes, and each element as (tag, its value if it
    /// holds text only, its XML).
    sections: Vec<(String, String, Vec<(String, Option<String>, String)>)>,
}

// ponytail: the section and element names, ExternalPlayer's "FlashPlayer32.0" form and PNG's BitDepth
// values are as recalled from PublishSettings.xml files in public projects; check them against a file
// Animate 2024 saves. Whatever else a file holds is written back untouched either way.
const FORMAT: &str = "PublishFormatProperties";
const SWF: &str = "PublishFlashProperties";
const GIF: &str = "PublishGifProperties";
const JPEG: &str = "PublishJpegProperties";
const PNG: &str = "PublishPNGProperties";
/// PNG's bit depth for an image with alpha; the other is "24-bit".
const WITH_ALPHA: &str = "24-bit with Alpha";
/// What the player is named after, in ExternalPlayer: "FlashPlayer32.0".
const PLAYER: &str = "FlashPlayer";
/// The newest player's SWF version, a new document's.
const NEWEST: u8 = 43;

/// The SWF version an ExternalPlayer value targets, if it's a player `players` lists.
fn listed(external: &str) -> Option<u8> {
    let player = external.strip_prefix(PLAYER)?;
    players().into_iter().find(|(v, _)| v == player).map(|(_, version)| version)
}

impl Profile {
    /// The settings Animate gives a new ActionScript document: SWF and its HTML page, for the newest player.
    pub fn new(name: &str) -> Profile {
        Profile {
            name: name.to_string(),
            swf: Target { on: true, file: String::new() },
            version: NEWEST,
            compress: true,
            jpeg_quality: 80,
            include_hidden: true,
            html: Target { on: true, file: String::new() },
            gif: Image::new(false),
            jpeg: Image::new(false),
            png: Image::new(false),
            sections: Vec::new(),
        }
    }

    /// The player version the SWF version stands for, as Publish Settings' Target lists it.
    pub fn player(&self) -> String {
        players().into_iter().find(|p| p.1 == self.version).map_or_else(|| "32.0".into(), |p| p.0)
    }

    fn get(&self, section: &str, tag: &str) -> Option<&str> {
        let (_, _, items) = self.sections.iter().find(|s| s.0 == section)?;
        items.iter().find(|i| i.0 == tag)?.1.as_deref()
    }

    fn set(&mut self, section: &str, tag: &str, value: &str) {
        if !self.sections.iter().any(|s| s.0 == section) {
            self.sections.push((section.into(), " enabled=\"true\"".into(), Vec::new()));
        }
        let Some((_, _, items)) = self.sections.iter_mut().find(|s| s.0 == section) else { return };
        let item = (tag.to_string(), Some(value.to_string()), format!("<{tag}>{}</{tag}>", escape(value)));
        match items.iter_mut().find(|i| i.0 == tag) {
            Some(slot) if slot.1.as_deref() == Some(value) => {}
            Some(slot) => *slot = item,
            None => items.push(item),
        }
    }

    /// Reads one `<flash_profile>`.
    fn read(n: Node) -> Profile {
        let mut p = Profile::new(n.attribute("name").unwrap_or("Default"));
        for s in n.children().filter(Node::is_element) {
            let attrs: String = s.attributes().map(|a| format!(" {}=\"{}\"", a.name(), escape(a.value()))).collect();
            let items = s.children().filter(Node::is_element).map(|c| {
                let text = (!c.children().any(|g| g.is_element())).then(|| c.text().unwrap_or_default().to_string());
                (c.tag_name().name().to_string(), text, n.document().input_text()[c.range()].to_string())
            });
            p.sections.push((s.tag_name().name().to_string(), attrs, items.collect()));
        }
        fn flag(p: &Profile, section: &str, tag: &str, default: bool) -> bool {
            p.get(section, tag).map_or(default, |v| v.trim() == "1")
        }
        fn number(p: &Profile, section: &str, tag: &str) -> Option<u32> {
            p.get(section, tag).and_then(|v| v.trim().parse().ok())
        }
        fn target(p: &Profile, name: &str, default: bool) -> Target {
            // A default name follows the document; a name of its own is kept.
            let own = !flag(p, FORMAT, &format!("{name}DefaultName"), true);
            let file = if own { p.get(FORMAT, &format!("{name}FileName")).unwrap_or_default().to_string() } else { String::new() };
            Target { on: flag(p, FORMAT, name, default), file }
        }
        p.swf = target(&p, "flash", true);
        p.html = target(&p, "html", true);
        if let Some(version) = p.get(SWF, "ExternalPlayer").and_then(listed) {
            p.version = version;
        }
        p.compress = flag(&p, SWF, "CompressMovie", true);
        p.jpeg_quality = number(&p, SWF, "Quality").map_or(80, |q| q.clamp(0, 100) as u8);
        p.include_hidden = flag(&p, SWF, "InvisibleLayer", true);
        for (name, section) in [("gif", GIF), ("jpeg", JPEG), ("png", PNG)] {
            let size = match (flag(&p, section, "MatchMovieDim", true), number(&p, section, "Width"), number(&p, section, "Height")) {
                (false, Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
                _ => None,
            };
            let image = Image {
                target: target(&p, name, false),
                size,
                quality: number(&p, section, "Quality").map_or(80, |q| q.clamp(1, 100) as u8),
                transparent: p.get(section, "BitDepth") == Some(WITH_ALPHA),
                animated: flag(&p, section, "Animated", false),
                repeat: if flag(&p, section, "Loop", true) { None } else { number(&p, section, "LoopCount").map(|n| n.min(u16::MAX as u32) as u16) },
            };
            match name {
                "gif" => p.gif = image,
                "jpeg" => p.jpeg = image,
                _ => p.png = image,
            }
        }
        p
    }

    /// Puts the settings into the sections, which `Settings::to_xml` writes. `stem` names default files.
    fn store(&mut self, stem: &str) {
        let bit = |on: bool| if on { "1" } else { "0" };
        let targets = [("flash", self.swf.clone(), "swf"), ("html", self.html.clone(), "html"), ("gif", self.gif.target.clone(), "gif"), ("jpeg", self.jpeg.target.clone(), "jpg"), ("png", self.png.target.clone(), "png")];
        for (name, target, ext) in targets {
            self.set(FORMAT, name, bit(target.on));
            self.set(FORMAT, &format!("{name}DefaultName"), bit(target.file.is_empty()));
            self.set(FORMAT, &format!("{name}FileName"), &output(&target, stem, ext));
        }
        // ponytail: <Version> is left alone: whether it holds the player's version or the SWF's is unconfirmed.
        // A target the list lacks (an AIR player...) reads as the newest player, and stays as it
        // was until another version is picked.
        let unlisted = self.get(SWF, "ExternalPlayer").is_some_and(|v| listed(v).is_none());
        if !unlisted || self.version != NEWEST {
            self.set(SWF, "ExternalPlayer", &format!("{PLAYER}{}", self.player()));
        }
        self.set(SWF, "CompressMovie", bit(self.compress));
        self.set(SWF, "Quality", &self.jpeg_quality.to_string());
        self.set(SWF, "InvisibleLayer", bit(self.include_hidden));
        for (section, image) in [(GIF, self.gif.clone()), (JPEG, self.jpeg.clone()), (PNG, self.png.clone())] {
            self.set(section, "MatchMovieDim", bit(image.size.is_none()));
            if let Some((w, h)) = image.size {
                self.set(section, "Width", &w.to_string());
                self.set(section, "Height", &h.to_string());
            }
            match section {
                GIF => {
                    self.set(section, "Animated", bit(image.animated));
                    self.set(section, "Loop", bit(image.repeat.is_none()));
                    // Looping for ever, the count typed for Repeat stays as it was.
                    if let Some(n) = image.repeat {
                        self.set(section, "LoopCount", &n.to_string());
                    }
                }
                JPEG => self.set(section, "Quality", &image.quality.to_string()),
                // Another depth (8-bit...) stays while the transparency it reads as does.
                _ if self.get(section, "BitDepth").map(|d| d == WITH_ALPHA) == Some(image.transparent) => {}
                _ => self.set(section, "BitDepth", if image.transparent { WITH_ALPHA } else { "24-bit" }),
            }
        }
    }
}

impl Settings {
    pub fn current(&self) -> &Profile {
        &self.profiles[self.current.min(self.profiles.len() - 1)]
    }

    pub fn current_mut(&mut self) -> &mut Profile {
        let i = self.current.min(self.profiles.len() - 1);
        &mut self.profiles[i]
    }

    /// The settings of the document at `path` (a .fla or an XFL folder): its PublishSettings.xml,
    /// or a new document's when it has none. One that can't be read is an error.
    pub fn read(path: &Path) -> Result<Settings> {
        let mut text = String::new();
        if let Some(dir) = crate::xfl::folder(path) {
            let file = dir.join(FILE);
            if !file.exists() {
                return Ok(Settings::default());
            }
            text = fs::read_to_string(file)?;
        } else {
            let mut zip = zip::ZipArchive::new(fs::File::open(path)?)?;
            let Some(i) = zip.index_for_name(FILE) else { return Ok(Settings::default()) };
            zip.by_index(i)?.read_to_string(&mut text)?;
        }
        Settings::parse(&text)
    }

    pub fn parse(xml: &str) -> Result<Settings> {
        let doc = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}'))?;
        let nodes: Vec<Node> = doc.root_element().children().filter(|n| n.has_tag_name("flash_profile")).collect();
        if nodes.is_empty() {
            return Err("PublishSettings.xml holds no profile".into());
        }
        let current = nodes.iter().position(|n| n.attribute("current") == Some("true")).unwrap_or(0);
        Ok(Settings { profiles: nodes.into_iter().map(Profile::read).collect(), current })
    }

    /// PublishSettings.xml, for a document named `stem`.
    pub fn to_xml(&self, stem: &str) -> String {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<flash_profiles>\n");
        for (i, profile) in self.profiles.iter().enumerate() {
            let mut p = profile.clone();
            p.store(stem);
            out += &format!("<flash_profile version=\"1.0\" name=\"{}\" current=\"{}\">\n", escape(&p.name), i == self.current);
            for (tag, attrs, items) in &p.sections {
                out += &format!("  <{tag}{attrs}>\n");
                for (_, _, xml) in items {
                    out += &format!("    {xml}\n");
                }
                out += &format!("  </{tag}>\n");
            }
            out += "</flash_profile>\n";
        }
        out + "</flash_profiles>\n"
    }
}

/// The file `target` writes, for a document named `stem`.
pub fn output(target: &Target, stem: &str, ext: &str) -> String {
    if target.file.is_empty() { format!("{stem}.{ext}") } else { target.file.clone() }
}

/// `file` as a path below the publish folder: an error for an absolute path, a drive or root
/// prefix, or any `..` (a document's Publish Settings are untrusted input).
pub fn confine(file: &str) -> Result<PathBuf> {
    let bad = || format!("Publish Settings file name \"{file}\" must stay inside the document's folder.");
    let mut out = PathBuf::new();
    for (i, part) in file.split(['/', '\\']).enumerate() {
        if part == ".." || part.contains(':') || part.contains('\0') || (i == 0 && part.is_empty() && !file.is_empty()) {
            return Err(bad().into());
        }
        if !part.is_empty() && part != "." {
            out.push(part);
        }
    }
    if out.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return Err(bad().into());
    }
    Ok(out)
}

/// The document's name, as published files take it: a .fla's, or its XFL folder's.
pub fn stem(doc_path: &Path) -> String {
    let name = match crate::xfl::folder(doc_path) {
        Some(dir) => dir.file_name(),
        None => doc_path.file_stem(),
    };
    name.map_or_else(|| "Untitled".into(), |n| n.to_string_lossy().into_owned())
}

/// The folder published files go in: the document's.
pub fn folder(doc_path: &Path) -> PathBuf {
    match crate::xfl::folder(doc_path) {
        Some(dir) => dir.to_path_buf(),
        None => doc_path.parent().map(Path::to_path_buf).unwrap_or_default(),
    }
}

/// File > Publish: writes each file `profile` has on for `doc`, saved at `doc_path`, into its
/// folder. Returns the files written, each with the features it couldn't show.
pub fn publish(doc: &Document, profile: &Profile, doc_path: &Path) -> Result<Vec<(PathBuf, BTreeSet<String>)>> {
    let (dir, stem) = (folder(doc_path), stem(doc_path));
    let path = |target: &Target, ext: &str| -> Result<PathBuf> { Ok(dir.join(confine(&output(target, &stem, ext))?)) };
    let write = |path: &Path, bytes: &[u8]| -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(fs::write(path, bytes)?)
    };
    let mut written = Vec::new();
    // An HTML5 Canvas document publishes its own page and script (html5.rs) as the HTML file,
    // and no SWF.
    if doc.is_canvas() {
        if profile.html.on {
            let html = path(&profile.html, "html")?;
            let name = html.file_stem().map_or_else(|| stem.clone(), |s| s.to_string_lossy().into_owned());
            let skipped = crate::html5::publish(doc, html.parent().unwrap_or(&dir), &name)?;
            written.push((html, skipped));
        }
    } else if profile.swf.on {
        let shown;
        let doc = if profile.include_hidden { doc } else { shown = without_hidden_layers(doc); &shown };
        let compression = if profile.compress { swf::Compression::Zlib } else { swf::Compression::None };
        let (bytes, skipped) = crate::swf_export::export_with(doc, profile.version, compression)?;
        let swf = path(&profile.swf, "swf")?;
        write(&swf, &bytes)?;
        written.push((swf, skipped));
    }
    if profile.html.on && !doc.is_canvas() {
        let html = path(&profile.html, "html")?;
        let swf = path(&profile.swf, "swf")?;
        // Relative to the page where it can be, so the two go onto a web server together.
        // ponytail: a SWF outside the page's folder gets a file URL; "../" paths would keep those relative too.
        let src = match html.parent().and_then(|page| swf.strip_prefix(page).ok()) {
            Some(relative) => relative.to_string_lossy().replace('\\', "/"),
            None => format!("file:///{}", swf.to_string_lossy().replace('\\', "/")),
        };
        write(&html, page(&stem, &src, doc).as_bytes())?;
        written.push((html, BTreeSet::new()));
    }
    let scale = |image: &Image| image.size.map_or(1.0, |(w, _)| w as f32 / doc.width);
    if profile.gif.target.on {
        let frames = if profile.gif.animated { 0..export::frame_count(doc) } else { 0..1 };
        let (bytes, skipped) = export::gif(doc, frames, scale(&profile.gif), false, profile.gif.repeat)?;
        let gif = path(&profile.gif.target, "gif")?;
        write(&gif, &bytes)?;
        written.push((gif, skipped));
    }
    for (image, format, ext) in [(&profile.jpeg, Format::Jpeg(profile.jpeg.quality), "jpg"), (&profile.png, Format::Png, "png")] {
        if image.target.on {
            let (bytes, skipped) = export::image(doc, 0, scale(image), image.transparent, format)?;
            let file = path(&image.target, ext)?;
            write(&file, &bytes)?;
            written.push((file, skipped));
        }
    }
    if written.is_empty() {
        return Err("Nothing to publish: turn a format on in Publish Settings.".into());
    }
    Ok(written)
}

/// `doc` without what its hidden layers hold, in every timeline: Include Hidden Layers off.
// ponytail: a hidden mask layer then masks everything out; check what Animate publishes there.
pub fn without_hidden_layers(doc: &Document) -> Document {
    let mut doc = doc.clone();
    let timelines = doc.scenes.iter_mut().chain(doc.library.values_mut().map(|s| &mut s.timeline));
    for layer in timelines.flat_map(|tl| &mut tl.layers).filter(|l| !l.visible()) {
        for key in &mut layer.frames {
            key.elements.clear();
            key.tween = Tween::None;
        }
    }
    doc
}

/// The HTML wrapper: a page the size of the stage playing `swf` through Ruffle, the open-source
/// player (https://ruffle.rs), from its CDN.
fn page(title: &str, swf: &str, doc: &Document) -> String {
    let [r, g, b, _] = doc.background;
    let (w, h, bg) = (doc.width, doc.height, hex([r, g, b]));
    let (title, swf) = (escape(title), escape(swf));
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n\
         <script src=\"https://unpkg.com/@ruffle-rs/ruffle\"></script>\n</head>\n\
         <body style=\"margin:0; background:{bg}\">\n<embed src=\"{swf}\" width=\"{w}\" height=\"{h}\">\n</body>\n</html>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trimmed PublishSettings.xml as Animate writes them, with a setting minnamate doesn't read.
    const ANIMATES: &str = r#"<?xml version="1.0" encoding="UTF-8" ?>
<flash_profiles>
<flash_profile version="1.0" name="Default" current="false">
  <PublishFormatProperties enabled="true">
    <defaultNames>1</defaultNames>
    <flash>1</flash>
    <html>0</html>
    <png>1</png>
    <flashDefaultName>0</flashDefaultName>
    <flashFileName>bin/movie.swf</flashFileName>
  </PublishFormatProperties>
  <PublishFlashProperties enabled="true">
    <Quality>65</Quality>
    <ExternalPlayer>FlashPlayer11.2</ExternalPlayer>
    <AS3PackagePaths>.</AS3PackagePaths>
    <InvisibleLayer>0</InvisibleLayer>
    <LibraryPath><library-path-entry><swc-path>lib</swc-path></library-path-entry></LibraryPath>
  </PublishFlashProperties>
  <PublishPNGProperties enabled="true">
    <MatchMovieDim>0</MatchMovieDim>
    <Width>275</Width>
    <Height>200</Height>
    <BitDepth>24-bit with Alpha</BitDepth>
  </PublishPNGProperties>
</flash_profile>
<flash_profile version="1.0" name="Web" current="true">
  <PublishFormatProperties enabled="true"><flash>0</flash><gif>1</gif></PublishFormatProperties>
</flash_profile>
</flash_profiles>"#;

    #[test]
    fn animates_settings_read_and_write_back() {
        let s = Settings::parse(ANIMATES).unwrap();
        assert_eq!((s.profiles.len(), s.current, s.current().name.as_str()), (2, 1, "Web"));
        let p = &s.profiles[0];
        assert_eq!((p.swf.on, p.swf.file.as_str(), p.html.on), (true, "bin/movie.swf", false));
        assert_eq!((p.version, p.player(), p.jpeg_quality, p.include_hidden, p.compress), (15, "11.2".to_string(), 65, false, true));
        assert_eq!((p.png.target.on, p.png.size, p.png.transparent), (true, Some((275, 200)), true));
        assert!(s.profiles[1].gif.target.on && !s.profiles[1].swf.on);

        // Written and read back: the same settings, and what minnamate doesn't read as it was.
        let mut s = s;
        s.profiles[0].png.size = None;
        let xml = s.to_xml("movie");
        assert!(xml.contains("<LibraryPath><library-path-entry><swc-path>lib</swc-path></library-path-entry></LibraryPath>"), "{xml}");
        assert!(xml.contains("<AS3PackagePaths>.</AS3PackagePaths>") && xml.contains("<pngFileName>movie.png</pngFileName>"));
        let back = Settings::parse(&xml).unwrap();
        assert_eq!(back.profiles.iter().map(|p| (&p.name, &p.swf, p.version, &p.png, &p.gif)).collect::<Vec<_>>(), s.profiles.iter().map(|p| (&p.name, &p.swf, p.version, &p.png, &p.gif)).collect::<Vec<_>>());
        assert_eq!(back.current, 1);
        assert_eq!(players().last(), Some(&("32.0".to_string(), 43)));
    }

    #[test]
    fn values_the_dialog_doesnt_offer_stay_until_changed() {
        let mut s = Settings::parse(&ANIMATES.replace("FlashPlayer11.2", "AdobeAIR32.0")).unwrap();
        assert!(s.to_xml("movie").contains("<ExternalPlayer>AdobeAIR32.0</ExternalPlayer>"), "an AIR document stays one");
        s.profiles[0].version = 15;
        assert!(s.to_xml("movie").contains("<ExternalPlayer>FlashPlayer11.2</ExternalPlayer>"));
        // So does a bit depth other than the two Publish Settings offers, until transparency changes.
        let mut s = Settings::parse(&ANIMATES.replace("24-bit with Alpha", "8-bit")).unwrap();
        assert!(s.to_xml("movie").contains("<BitDepth>8-bit</BitDepth>"));
        s.profiles[0].png.transparent = true;
        assert!(s.to_xml("movie").contains("<BitDepth>24-bit with Alpha</BitDepth>"));
        // And a GIF's repeat count while it loops for ever.
        let gif = r#"<flash_profiles><flash_profile name="G"><PublishGifProperties><Loop>1</Loop><LoopCount>5</LoopCount></PublishGifProperties></flash_profile></flash_profiles>"#;
        assert!(Settings::parse(gif).unwrap().to_xml("movie").contains("<LoopCount>5</LoopCount>"));
    }

    #[test]
    fn file_names_cannot_leave_the_folder() {
        for bad in ["../x", "/abs", "C:\\x", "a/../../b", "a\\..\\b", "\\\\srv\\share", "c:x"] {
            assert!(confine(bad).is_err(), "{bad}");
        }
        assert_eq!(confine("out/movie.swf").unwrap(), PathBuf::from("out").join("movie.swf"));
        let dir = std::env::temp_dir().join(format!("minnamate-publish-bad-{}", std::process::id()));
        let mut p = Profile::new("Default");
        p.swf.file = "../escape.swf".into();
        assert!(publish(&Document::new(10.0, 10.0, 12.0), &p, &dir.join("m.fla")).is_err());
        assert!(!dir.parent().unwrap().join("escape.swf").exists());
    }

    #[test]
    fn publish_writes_each_format_beside_the_document() {
        let dir = std::env::temp_dir().join(format!("minnamate-publish-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let fla = dir.join("movie.fla");
        let mut doc = Document::new(100.0, 50.0, 12.0);
        doc.scenes[0].layers[0].set_visible(false);
        let mut p = Profile::new("Default");
        (p.swf.file, p.version, p.compress, p.include_hidden) = ("out/movie.swf".into(), 15, false, false);
        (p.png.target.on, p.png.size, p.jpeg.target.on, p.gif.target.on, p.gif.animated) = (true, Some((50, 25)), true, true, true);
        let written = publish(&doc, &p, &fla).unwrap();
        let names: Vec<_> = written.iter().map(|(f, _)| f.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/")).collect();
        assert_eq!(names, ["out/movie.swf", "movie.html", "movie.gif", "movie.jpg", "movie.png"]);
        let swf = fs::read(dir.join("out/movie.swf")).unwrap();
        assert_eq!((&swf[..3], swf[3]), (&b"FWS"[..], 15), "uncompressed, for the player asked for");
        let page = fs::read_to_string(dir.join("movie.html")).unwrap();
        assert!(page.contains("src=\"out/movie.swf\""), "the SWF in a folder below, relative to the page: {page}");
        assert_eq!(tiny_skia::Pixmap::load_png(dir.join("movie.png")).unwrap().width(), 50);

        // An HTML5 Canvas document's page is its own (html5.rs), and there's no SWF.
        doc.set_canvas(true);
        let p = Profile::new("Default");
        let canvas = dir.join("canvas");
        let written = publish(&doc, &p, &canvas.join("anim.fla")).unwrap();
        assert_eq!(written.iter().map(|(f, _)| f.clone()).collect::<Vec<_>>(), [canvas.join("anim.html")]);
        assert!(canvas.join("anim.js").exists() && canvas.join("runtime.js").exists() && !canvas.join("anim.swf").exists());
        let _ = fs::remove_dir_all(dir);
    }
}
