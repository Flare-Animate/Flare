//! A small but realistic Animate project, as a zipped `.fla` and as an XFL folder: a main
//! document with a bitmap, a font and a script that Flare keeps only as source data, two library
//! symbols, and the binary and settings files Flare never models.

use std::io::Write;
use std::path::Path;

pub const MAIN: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<DOMDocument xmlns="http://ns.adobe.com/xfl/2008/" frameRate="24" width="100" height="50" xflVersion="2.2" creatorInfo="Test" platform="Windows" versionInfo="Saved by Test" majorVersion="9">
<media><DOMBitmapItem name="pic.png" itemID="1" sourceExternalFilepath="./LIBRARY/pic.png" bitmapDataHRef="M 1 1.dat" frameRight="64" frameBottom="64"/></media>
<fonts><DOMFontItem name="Sans"/></fonts>
<symbols><Include href="Box.xml"/><Include href="Dot.xml"/></symbols>
<timelines><DOMTimeline name="Scene 1"><layers>
  <DOMLayer name="Main"><frames>
    <DOMFrame index="0" duration="3">
      <Actionscript><script><![CDATA[if (a < b) { stop(); }]]></script></Actionscript>
      <elements>
        <DOMShape><fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
          <edges><Edge fillStyle1="1" edges="!0 0|200 0|200 200|0 200|0 0"/></edges></DOMShape>
        <DOMSymbolInstance libraryItemName="Box"><matrix><Matrix tx="10" ty="5"/></matrix></DOMSymbolInstance>
        <DOMBitmapInstance libraryItemName="pic.png"/>
      </elements>
    </DOMFrame>
  </frames></DOMLayer>
</layers></DOMTimeline></timelines></DOMDocument>
"##;

pub const BOX: &str = r##"<DOMSymbolItem xmlns="http://ns.adobe.com/xfl/2008/" name="Box" symbolType="graphic" itemID="2"><timeline><DOMTimeline name="Box"><layers><DOMLayer name="L"><frames>
  <DOMFrame index="0"><elements><DOMShape><fills><FillStyle index="1"><SolidColor color="#00FF00"/></FillStyle></fills><edges><Edge fillStyle1="1" edges="!0 0|100 0|100 100|0 100|0 0"/></edges></DOMShape></elements></DOMFrame>
</frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>
"##;

pub const DOT: &str = r##"<DOMSymbolItem xmlns="http://ns.adobe.com/xfl/2008/" name="Dot" itemID="3"><timeline><DOMTimeline name="Dot"><layers><DOMLayer name="D"><frames>
  <DOMFrame index="0"><elements><DOMShape><fills><FillStyle index="1"><SolidColor color="#0000FF"/></FillStyle></fills><edges><Edge fillStyle1="1" edges="!0 0|40 0|40 40|0 40|0 0"/></edges></DOMShape></elements></DOMFrame>
</frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>
"##;

/// Bytes that are not text and not compressible by luck: a PNG-looking header and a ramp.
fn binary(seed: u8, len: usize) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00];
    bytes.extend((0..len).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)));
    bytes
}

/// Every file of the project, in the order Animate writes them.
pub fn files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("mimetype", b"application/vnd.adobe.xfl".to_vec()),
        ("DOMDocument.xml", MAIN.as_bytes().to_vec()),
        (
            "PublishSettings.xml",
            b"<flash_profiles><flash_profile name=\"Default\"/></flash_profiles>\r\n".to_vec(),
        ),
        ("MobileSettings.xml", b"<mobile/>".to_vec()),
        (
            "META-INF/metadata.xml",
            b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>".to_vec(),
        ),
        ("LIBRARY/Box.xml", BOX.as_bytes().to_vec()),
        ("LIBRARY/Dot.xml", DOT.as_bytes().to_vec()),
        ("LIBRARY/pic.png", binary(1, 700)),
        ("bin/M 1 1.dat", binary(2, 900)),
        ("bin/SymDepend.cache", binary(3, 40)),
        ("extras/unknown.bin", binary(4, 5)),
    ]
}

/// The project as a zipped `.fla`. Entries are stored or deflated as Animate does, with a
/// timestamp that is not the writer's default, so a rewrite could not match by luck.
pub fn fla() -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut cursor);
    let time = zip::DateTime::from_date_and_time(2021, 6, 7, 8, 9, 10).expect("a date");
    for (name, body) in files() {
        let method = if name == "mimetype" || name.ends_with(".png") {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        };
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(method)
            .last_modified_time(time);
        zip.start_file(name, options).expect("start");
        zip.write_all(&body).expect("write");
    }
    zip.finish().expect("finish");
    cursor.into_inner()
}

/// The project as an XFL folder in `dir`, with its stub, an empty folder and a hidden file.
pub fn write_folder(dir: &Path) {
    for (name, body) in files() {
        if name == "mimetype" {
            continue;
        }
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, body).expect("file");
    }
    std::fs::write(dir.join("Project.xfl"), "PROXY-CS5").expect("stub");
    std::fs::create_dir_all(dir.join("empty folder")).expect("empty");
    std::fs::write(dir.join(".hidden"), b"x").expect("hidden");
}

/// Every file and empty folder under `dir` by relative name, with its bytes.
pub fn tree(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        let mut any = false;
        for entry in std::fs::read_dir(at).expect("list") {
            any = true;
            let entry = entry.expect("entry");
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let name = path
                    .strip_prefix(root)
                    .expect("inside")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(name, std::fs::read(&path).expect("read"));
            }
        }
        if !any && at != root {
            let name = at
                .strip_prefix(root)
                .expect("inside")
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(format!("{name}/"), Vec::new());
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The entries of a zip by name, in order, with their bytes.
pub fn zip_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a zip");
    (0..archive.len())
        .map(|i| {
            let mut entry = archive.by_index(i).expect("entry");
            let mut body = Vec::new();
            entry.read_to_end(&mut body).expect("read");
            (entry.name().expect("name").into_owned(), body)
        })
        .collect()
}
