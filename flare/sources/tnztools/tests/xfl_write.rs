//! Saving: documents load back the same, XFL the model doesn't read is written back
//! as it was, and `save_preserving` keeps the source's other files.

use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use minnamate_core::doc::{Document, Element, Rotate, TextAlign, TextKind, Tween};
use minnamate_core::{render::render, text, xfl, xfl_write};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tween")
}

/// An empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn round_trip() {
    let dir = scratch("round_trip");
    for (name, doc) in [("tween", xfl::load(&fixture()).unwrap()), ("new", Document::new(1280.0, 720.0, 30.0))] {
        for path in [dir.join(name), dir.join(format!("{name}.fla"))] {
            xfl_write::save(&doc, &path).unwrap();
            let back = xfl::load(&path).unwrap();
            assert_eq!(back, doc, "{}", path.display());
            for f in 0..doc.scenes[0].frame_count() {
                let ((a, a_skipped), (b, b_skipped)) = (render(&doc, f, 1.0).unwrap(), render(&back, f, 1.0).unwrap());
                assert!(a.data() == b.data() && a_skipped == b_skipped, "{} frame {f}", path.display());
            }
        }
    }
    assert_eq!(fs::read_to_string(dir.join("tween/tween.xfl")).unwrap(), "PROXY-CS5");
}

const XMLNS: &str = r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/""#;
const MEDIA: &str = r#"<media><DOMBitmapItem name="photo.png" href="photo.png" bitmapDataHRef="M 1 1.dat"/></media>"#;
const SCRIPT: &str = "<Actionscript><script><![CDATA[if (a < b) stop();]]></script></Actionscript>";
const BITMAP: &str = r#"<DOMBitmapInstance libraryItemName="photo.png">
                  <matrix><Matrix tx="12" ty="3"/></matrix>
                </DOMBitmapInstance>"#;
// Gradients are read now (gradients.rs); bitmap fills are still kept as they were.
const FILL: &str = r#"<BitmapFill bitmapPath="photo.png"><matrix><Matrix a="20" d="20"/></matrix></BitmapFill>"#;

#[test]
fn unread_xml_survives() {
    let source = format!(
        r##"<DOMDocument {XMLNS} xmlns:a="urn:a" a:x="1" xflVersion="2.97" width="320">
  {MEDIA}
  <timelines>
    <DOMTimeline name="Scene 1">
      <layers>
        <DOMLayer name="Fish &amp; &quot;chips&quot;&#10;2" color="#4FFF4F" xmlns:b="urn:b" b:y="2">
          <frames>
            <DOMFrame index="0" duration="5" name="intro" tweenType="shape" acceleration="-50">
              {SCRIPT}
              <elements>
                {BITMAP}
                <DOMShape isDrawingObject="true">
                  <fills><FillStyle index="1">{FILL}</FillStyle></fills>
                  <edges><Edge fillStyle1="1" edges="!0 0|200 0|200 200|0 0"/></edges>
                </DOMShape>
              </elements>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timelines>
  <persistentData><PD n="x" v="1"/></persistentData>
</DOMDocument>"##
    );
    let dir = scratch("unread_xml");
    fs::create_dir_all(dir.join("source")).unwrap();
    fs::write(dir.join("source/DOMDocument.xml"), source).unwrap();
    let doc = xfl::load(&dir.join("source")).unwrap();
    let layer = &doc.scenes[0].layers[0];
    assert_eq!(layer.name, "Fish & \"chips\"\n2");
    let el = &layer.frames[0].elements[0];
    assert!(matches!(el, Element::Unsupported { name, xml } if name == "DOMBitmapInstance" && xml == BITMAP), "{el:?}");

    xfl_write::save(&doc, &dir.join("saved")).unwrap();
    let saved = fs::read_to_string(dir.join("saved/DOMDocument.xml")).unwrap();
    let kept = [MEDIA, SCRIPT, BITMAP, FILL, r#"xflVersion="2.97""#, r#"name="intro""#, r#"acceleration="-50""#];
    for xml in kept {
        assert!(saved.contains(xml), "{xml} missing from\n{saved}");
    }
    let at = |xml: &str| saved.find(xml);
    assert!(at(MEDIA) < at("<timelines>") && at("<timelines>") < at("<persistentData>"), "children reordered");
    assert_eq!(xfl::load(&dir.join("saved")).unwrap(), doc);

    // The model wins over kept XML: made a classic tween, the frame drops the shape tween's ease.
    let mut edited = doc.clone();
    let classic = Tween::Classic { ease: 0.0, rotate: Rotate::Auto };
    edited.scenes[0].layers[0].frames[0].tween = classic.clone();
    xfl_write::save(&edited, &dir.join("edited")).unwrap();
    assert_eq!(xfl::load(&dir.join("edited")).unwrap().scenes[0].layers[0].frames[0].tween, classic);
}

#[test]
fn text_round_trip() {
    let source = format!(
        r##"<DOMDocument {XMLNS}>
  <timelines>
    <DOMTimeline name="Scene 1">
      <layers>
        <DOMLayer name="Text">
          <frames>
            <DOMFrame index="0">
              <elements>
                <DOMStaticText fontRenderingMode="standard" left="0.5" width="185.45" height="29.6" autoExpand="true" isSelectable="false">
                  <matrix><Matrix tx="62.5" ty="96.05"/></matrix>
                  <textRuns>
                    <DOMTextRun>
                      <characters>Fish &amp; &lt;chips&gt;&#xD; "two"</characters>
                      <textAttrs><DOMTextAttrs aliasText="false" autoKern="false" lineSpacing="2" size="24" bitmapSize="480" face="TimesNewRomanPSMT" fillColor="#336699"/></textAttrs>
                    </DOMTextRun>
                    <DOMTextRun>
                      <characters> and more </characters>
                      <textAttrs><DOMTextAttrs lineSpacing="2" letterSpacing="1.5" size="12.5" face="Arial-BoldMT" fillColor="#FF0000" alpha="0.5" italic="true"/></textAttrs>
                    </DOMTextRun>
                  </textRuns>
                  <filters><DropShadowFilter/></filters>
                </DOMStaticText>
                <DOMDynamicText name="score" width="100" height="20" lineType="multiline">
                  <matrix><Matrix a="0.5" b="0.25" tx="12.5" ty="-3.75"/></matrix>
                  <textRuns><DOMTextRun><characters>0</characters><textAttrs><DOMTextAttrs alignment="justify" size="18" face="Arial"/></textAttrs></DOMTextRun></textRuns>
                </DOMDynamicText>
                <DOMInputText width="80" height="20" lineType="password"><textRuns/></DOMInputText>
              </elements>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timelines>
</DOMDocument>"##
    );
    let dir = scratch("text");
    fs::create_dir_all(dir.join("source")).unwrap();
    fs::write(dir.join("source/DOMDocument.xml"), source).unwrap();
    let doc = xfl::load(&dir.join("source")).unwrap();
    let elements = &doc.scenes[0].layers[0].frames[0].elements;
    let [Element::Text(fish), Element::Text(score), Element::Text(password)] = &elements[..] else { panic!("{elements:?}") };
    assert_eq!(text::string(fish), "Fish & <chips>\n \"two\" and more ");
    assert_eq!((fish.width, fish.matrix.tx, fish.runs[1].size, fish.runs[1].italic), (None, 63.0, 12.5, true));
    assert_eq!((score.kind, score.width, score.align), (TextKind::Dynamic, Some(100.0), TextAlign::Justify));
    assert_eq!((password.kind, password.width), (TextKind::Input, Some(80.0)), "a single-line field has a width too");

    for path in [dir.join("saved"), dir.join("saved.fla")] {
        xfl_write::save(&doc, &path).unwrap();
        assert_eq!(xfl::load(&path).unwrap(), doc, "{}", path.display());
    }
    let saved = fs::read_to_string(dir.join("saved/DOMDocument.xml")).unwrap();
    // Dynamic and input fields keep the box they were given, height included.
    let boxes = [r#"width="80" height="20" lineType="password""#, r#"height="20" lineType="multiline""#];
    for kept in [r#"isSelectable="false""#, r#"autoKern="false""#, "<DropShadowFilter/>", r#"name="score""#].into_iter().chain(boxes) {
        assert!(saved.contains(kept), "{kept} missing from\n{saved}");
    }
    assert!(!saved.contains("left="), "the box's offset went into the matrix");
}

#[test]
fn symbols_differing_only_in_case_both_survive() {
    let mut doc = xfl::load(&fixture()).unwrap();
    let ball = doc.library["Ball"].clone();
    doc.library.insert("ball".into(), ball);
    let dir = scratch("case");
    for path in [dir.join("folder"), dir.join("doc.fla")] {
        xfl_write::save(&doc, &path).unwrap();
        assert_eq!(xfl::load(&path).unwrap(), doc, "{}", path.display());
    }
    // Distinct ignoring case, or Windows and macOS would keep one file for both.
    let files: HashSet<String> = fs::read_dir(dir.join("folder/LIBRARY"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_lowercase())
        .collect();
    assert_eq!(files.len(), doc.library.len(), "{files:?}");
}

#[test]
fn existing_folders_save_as_load_reads_them() {
    let doc = Document::new(100.0, 100.0, 24.0);
    let dir = scratch("folder_names");
    for name in ["movie.xfl", "movie.fla"] {
        let folder = dir.join(name);
        fs::create_dir_all(&folder).unwrap();
        xfl_write::save(&doc, &folder).unwrap();
        assert!(folder.join("DOMDocument.xml").is_file(), "{name}");
        assert_eq!(xfl::load(&folder).unwrap(), doc, "{name}");
    }
    assert!(!dir.join("DOMDocument.xml").exists(), "wrote beside the folder");
}

#[test]
fn zip_entries_stay_inside_the_folder() {
    let dir = scratch("zip_slip");
    let fla = dir.join("evil.fla");
    xfl_write::save(&Document::new(100.0, 100.0, 24.0), &fla).unwrap();
    // An absolute entry name, which joined to the folder's path would replace it.
    let outside = dir.join("outside.txt");
    let file = fs::OpenOptions::new().read(true).write(true).open(&fla).unwrap();
    let mut zip = zip::ZipWriter::new_append(file).unwrap();
    zip.start_file(outside.to_string_lossy().replace('\\', "/"), zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"x").unwrap();
    zip.finish().unwrap();

    let doc = xfl::load(&fla).unwrap();
    xfl_write::save_preserving(&doc, Some(&fla), &dir.join("out")).unwrap();
    assert!(!outside.exists(), "wrote {}", outside.display());
}

#[test]
fn save_preserving_copies_other_files() {
    let doc = xfl::load(&fixture()).unwrap();
    let dir = scratch("preserving");
    let source = dir.join("source");
    xfl_write::save(&doc, &source).unwrap();
    let others: [(&str, &[u8]); 5] = [
        ("mimetype", b"application/x-test"),
        ("bin/M 1 1.dat", &[0, 1, 2, 255]),
        ("LIBRARY/photo.png", b"PNG"),
        ("META-INF/metadata.xml", b"<x/>"),
        ("PublishSettings.xml", b"<p/>"),
    ];
    // Not copied: a deleted symbol's file, and hidden files like version control's.
    let dropped: [(&str, &[u8]); 2] = [("LIBRARY/Old.xml", b"<old/>"), (".git/config", b"")];
    for (name, data) in others.into_iter().chain(dropped) {
        let path = source.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }

    let fla = dir.join("out.fla");
    let folder = dir.join("out");
    xfl_write::save_preserving(&doc, Some(&source), &fla).unwrap();
    xfl_write::save_preserving(&doc, Some(&fla), &folder).unwrap();
    xfl_write::save_preserving(&doc, Some(&fla), &fla).unwrap();
    xfl_write::save_preserving(&doc, Some(&folder), &folder).unwrap();

    let mut zip = zip::ZipArchive::new(fs::File::open(&fla).unwrap()).unwrap();
    let first = zip.by_index(0).unwrap();
    assert_eq!((first.name().unwrap().as_ref(), first.compression()), ("mimetype", zip::CompressionMethod::Stored));
    drop(first);
    for (name, data) in others {
        let mut copied = Vec::new();
        zip.by_name(name).unwrap().read_to_end(&mut copied).unwrap();
        assert_eq!(copied, data, "{name} in the .fla");
        assert_eq!(fs::read(folder.join(name)).unwrap(), data, "{name} in the folder");
    }
    for (name, _) in dropped {
        assert!(zip.by_name(name).is_err() && !folder.join(name).exists(), "{name} was copied");
    }
    let names: Vec<String> = zip.file_names().map(|n| n.unwrap().into_owned()).collect();
    assert_eq!(names.iter().filter(|n| n.ends_with(".xfl")).collect::<Vec<_>>(), ["out.xfl"]);
    assert!(!dir.join("out.fla.tmp").exists());
    assert_eq!(xfl::load(&fla).unwrap(), doc);
    assert_eq!(xfl::load(&folder).unwrap(), doc);
}

#[test]
fn frame_labels_survive_saving() {
    let dir = scratch("frame_labels");
    let mut doc = xfl::load(&fixture()).unwrap();
    doc.scenes[0].layers[1].frames[0].set_label("intro", "name");
    doc.scenes[0].layers[1].frames[1].set_label("tidy up later", "comment");
    let path = dir.join("labels.fla");
    xfl_write::save(&doc, &path).unwrap();
    let back = xfl::load(&path).unwrap();
    let frames = &back.scenes[0].layers[1].frames;
    assert_eq!(frames[0].label(), Some(("intro", "name")));
    assert_eq!(frames[1].label(), Some(("tidy up later", "comment")));

    let mut cleared = back.clone();
    cleared.scenes[0].layers[1].frames[0].set_label("", "name");
    assert_eq!(cleared.scenes[0].layers[1].frames[0].label(), None, "an empty name removes the label");
}
