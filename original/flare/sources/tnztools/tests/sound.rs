//! Sounds: library sounds and keyframe sounds load back as saved, Animate's media list
//! is read and kept, and the SWF holds DefineSound, StartSound and a stream.

use std::fs;
use std::path::{Path, PathBuf};

use minnamate_core::doc::{Document, Element, Instance, Symbol, SymbolKind};
use minnamate_core::sound::{Codec, Compression, Effect, FrameSound, Sound, Sync};
use minnamate_core::{swf_export, xfl, xfl_write};
use swf::{AudioCompression, SoundEvent, Tag};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sound").join(name)).unwrap()
}

/// An empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A 16-bit WAV file of a 440 Hz tone.
fn wav(rate: u32, channels: u16, seconds: f32) -> Vec<u8> {
    let n = (rate as f32 * seconds) as usize;
    let samples: Vec<i16> = (0..n * channels as usize).map(|i| ((i / channels as usize) as f32 * 440.0 * 6.283 / rate as f32).sin().mul_add(8000.0, 0.0) as i16).collect();
    let body: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut out = b"RIFF".to_vec();
    out.extend((36 + body.len() as u32).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(channels.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend((rate * channels as u32 * 2).to_le_bytes());
    out.extend((channels * 2).to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

/// Twelve frames: a stream sound on the bottom layer; above it an MP3 starting at
/// frame 0 (twice, fading in) and stopped at frame 6; and a movie clip playing a WAV.
fn doc_with_sounds() -> Document {
    let mut doc = Document::new(550.0, 400.0, 24.0);
    doc.sounds.insert("beep.mp3".into(), Sound::from_file(fixture("beep.mp3"), "beep.mp3").unwrap());
    doc.sounds.insert("Sounds/tone.wav".into(), Sound::from_file(wav(22050, 2, 0.4), "tone.wav").unwrap());
    let tl = &mut doc.scenes[0];
    tl.layers[0].frames[0].duration = 12;
    tl.layers[0].frames[0].set_sound(Some(&FrameSound { sync: Sync::Stream, ..FrameSound::new("Sounds/tone.wav") }));
    tl.insert_layer(0, "Sound");
    let layer = &mut tl.layers[0];
    layer.insert_blank_keyframe(6);
    layer.frames[0].set_sound(Some(&FrameSound { repeat: 2, effect: Effect::FadeIn, ..FrameSound::new("beep.mp3") }));
    layer.frames[1].set_sound(Some(&FrameSound { sync: Sync::Stop, ..FrameSound::new("beep.mp3") }));

    let mut clip = Document::new(1.0, 1.0, 1.0).scenes.remove(0);
    clip.name = "Clip".into();
    clip.layers[0].frames[0].set_sound(Some(&FrameSound { sync: Sync::Start, ..FrameSound::new("Sounds/tone.wav") }));
    doc.library.insert("Clip".into(), Symbol { kind: SymbolKind::MovieClip, timeline: clip, extra: Default::default() });
    let instance = Instance {
        symbol: "Clip".into(),
        kind: SymbolKind::MovieClip,
        matrix: minnamate_core::doc::Matrix::IDENTITY,
        pivot: (0.0, 0.0),
        color: minnamate_core::doc::ColorTransform::IDENTITY,
        looping: minnamate_core::doc::Loop::Loop,
        first_frame: 0,
        extra: Default::default(),
    };
    doc.scenes[0].layers[1].frames[0].elements.push(Element::Instance(instance));
    doc
}

#[test]
fn sounds_round_trip() {
    let doc = doc_with_sounds();
    let dir = scratch("sounds_round_trip");
    for path in [dir.join("folder"), dir.join("movie.fla")] {
        xfl_write::save(&doc, &path).unwrap();
        let back = xfl::load(&path).unwrap();
        assert_eq!(back, doc, "{}", path.display());
        assert_eq!(back.scenes[0].layers[0].frames[1].sound().map(|s| s.sync), Some(Sync::Stop));
    }
    // The files go in LIBRARY under their hrefs, each item in the media list.
    assert_eq!(fs::read(dir.join("folder/LIBRARY/beep.mp3")).unwrap(), fixture("beep.mp3"));
    let xml = fs::read_to_string(dir.join("folder/DOMDocument.xml")).unwrap();
    for part in [r#"<DOMSoundItem name="Sounds/tone.wav" href="tone.wav" format="22kHz 16bit Stereo""#, r#"soundName="beep.mp3" soundSync="stop""#] {
        assert!(xml.contains(part), "{part} missing from\n{xml}");
    }
    assert!(xml.find("<media>") < xml.find("<symbols>"), "media goes before the symbols");

    // Saved over itself, the media list doesn't grow.
    xfl_write::save_preserving(&doc, Some(&dir.join("folder")), &dir.join("folder")).unwrap();
    let again = fs::read_to_string(dir.join("folder/DOMDocument.xml")).unwrap();
    assert_eq!(again.matches("<DOMSoundItem").count(), 2);
    assert_eq!(xfl::load(&dir.join("folder")).unwrap(), doc);
}

const XMLNS: &str = r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/""#;
const BITMAP: &str = r#"<DOMBitmapItem name="photo.png" href="photo.png" bitmapDataHRef="M 1 1.dat"/>"#;
const LOST: &str = r#"<DOMSoundItem name="lost.mp3" href="lost.mp3"/>"#;

#[test]
fn animates_media_list_is_read_and_kept() {
    let source = format!(
        r#"<DOMDocument {XMLNS} width="320">
  <media>
    {BITMAP}
    <DOMSoundItem name="click.wav" itemID="5f2d-1" sourceExternalFilepath="./LIBRARY/click.wav" href="click.wav" soundDataHRef="M 2 1328254268.dat" format="22kHz 16bit Mono" sampleCount="4" exportFormat="1" exportBits="4"/>
    {LOST}
  </media>
  <timelines>
    <DOMTimeline name="Scene 1">
      <layers>
        <DOMLayer name="Layer_1">
          <frames>
            <DOMFrame index="0" duration="5" keyMode="9728" soundName="click.wav" soundSync="stream" soundLoopMode="loop" soundEffect="fade out" inPoint44="100">
              <SoundEnvelope><SoundEnvelopePoint mark44="0" level0="32768" level1="16384"/></SoundEnvelope>
              <elements/>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timelines>
</DOMDocument>"#
    );
    let dir = scratch("animate_media");
    fs::create_dir_all(dir.join("source/bin")).unwrap();
    fs::write(dir.join("source/DOMDocument.xml"), source).unwrap();
    let samples: Vec<u8> = [100i16, -100, 2000, -32768].iter().flat_map(|s| s.to_le_bytes()).collect();
    fs::write(dir.join("source/bin/M 2 1328254268.dat"), &samples).unwrap();

    let doc = xfl::load(&dir.join("source")).unwrap();
    let click = &doc.sounds["click.wav"];
    let info = click.info().unwrap();
    assert_eq!((info.codec, info.rate, info.channels, info.samples), (Codec::Raw, 22050, 1, 4));
    assert_eq!(click.decode().unwrap().samples, [100, -100, 2000, -32768]);
    assert!(!doc.sounds.contains_key("lost.mp3"), "no bytes, so it stays unread");
    let s = doc.scenes[0].layers[0].frames[0].sound().unwrap();
    assert_eq!((s.sync, s.looping, s.effect, s.in_point), (Sync::Stream, true, Effect::FadeOut, 100));
    assert_eq!(s.envelope, [(0, 1.0, 0.5)]);

    xfl_write::save_preserving(&doc, Some(&dir.join("source")), &dir.join("saved")).unwrap();
    let saved = fs::read_to_string(dir.join("saved/DOMDocument.xml")).unwrap();
    // The bitmap is bitmap.rs's to write (its compression made explicit); the sounds keep their place after it.
    let bitmap = r#"<DOMBitmapItem name="photo.png" compressionType="lossless" href="photo.png" bitmapDataHRef="M 1 1.dat"/>"#;
    for part in [bitmap, LOST, r#"<DOMSoundItem name="click.wav" itemID="5f2d-1""#, r#"soundDataHRef="M 2 1328254268.dat" format="22kHz 16bit Mono""#, r#"exportBits="4""#] {
        assert!(saved.contains(part), "{part} missing from\n{saved}");
    }
    assert_eq!(fs::read(dir.join("saved/bin/M 2 1328254268.dat")).unwrap(), samples, "Animate's copy goes back as it was");
    assert_eq!(xfl::load(&dir.join("saved")).unwrap(), doc);
}

#[test]
fn swf_defines_starts_and_streams_sounds() {
    let mut doc = doc_with_sounds();
    let (bytes, skipped) = swf_export::export(&doc).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();

    let defines: Vec<_> = movie.tags.iter().filter_map(|t| if let Tag::DefineSound(s) = t { Some(s) } else { None }).collect();
    assert_eq!(defines.len(), 2, "the MP3, and the WAV the clip starts; the stream needs none");
    let mp3 = defines.iter().find(|s| s.format.compression == AudioCompression::Mp3).expect("the MP3 goes as it is");
    assert_eq!((mp3.format.sample_rate, mp3.format.is_stereo), (22050, false));
    assert_eq!(&mp3.data[..2], [0, 0], "no seek");
    assert_eq!((mp3.data[2], mp3.data[3] & 0xE0), (0xFF, 0xE0), "then its frames, the Info frame left out");
    let wav = defines.iter().find(|s| s.format.compression == AudioCompression::Adpcm).expect("a WAV goes as ADPCM");
    assert_eq!((wav.format.sample_rate, wav.format.is_stereo, wav.num_samples), (22050, true, 8820));

    // The main timeline: the MP3 starts on frame 0 and stops on frame 6; the stream runs all 12 frames.
    let mut frame = 0;
    let (mut starts, mut blocks) = (Vec::new(), 0);
    for tag in &movie.tags {
        match tag {
            Tag::ShowFrame => frame += 1,
            Tag::StartSound(s) => starts.push((frame, s.id, s.sound_info.event, s.sound_info.num_loops, s.sound_info.envelope.as_ref().map(Vec::len))),
            Tag::SoundStreamBlock(_) => blocks += 1,
            _ => {}
        }
    }
    assert_eq!(starts, [(0, mp3.id, SoundEvent::Event, 2, Some(2)), (6, mp3.id, SoundEvent::Stop, 1, None)]);
    assert_eq!(blocks, 12);
    let head = movie.tags.iter().find_map(|t| if let Tag::SoundStreamHead2(h) = t { Some(h) } else { None }).expect("a stream");
    assert_eq!((head.stream_format.compression, head.stream_format.sample_rate, head.num_samples_per_block), (AudioCompression::Adpcm, 22050, 919));

    // The movie clip starts its sound in its own timeline.
    let sprite = movie.tags.iter().find_map(|t| if let Tag::DefineSprite(s) = t { Some(s) } else { None }).unwrap();
    let start = sprite.tags.iter().find_map(|t| if let Tag::StartSound(s) = t { Some(s) } else { None }).expect("the clip's sound");
    assert_eq!((start.id, start.sound_info.event), (wav.id, SoundEvent::Start));

    // Raw compression sends plain samples; a sound that's gone is reported, not exported.
    doc.sounds.get_mut("Sounds/tone.wav").unwrap().set_compression(Compression::Raw);
    doc.sounds.remove("beep.mp3");
    let (bytes, skipped) = swf_export::export(&doc).unwrap();
    assert_eq!(skipped.into_iter().collect::<Vec<_>>(), [r#"missing sound "beep.mp3""#]);
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();
    let raw = movie.tags.iter().find_map(|t| if let Tag::DefineSound(s) = t { Some(s) } else { None }).unwrap();
    assert_eq!((raw.format.compression, raw.data.len()), (AudioCompression::Uncompressed, 8820 * 4));
}

/// An XFL folder at `dir` whose library holds `items` (DOMSoundItem XML), and `files` by path.
fn animate_folder(dir: &Path, items: &str, files: &[(&str, &[u8])]) {
    let xml = format!(
        r#"<DOMDocument {XMLNS}><media>{items}</media><timelines><DOMTimeline name="Scene 1"><layers><DOMLayer name="Layer_1"><frames><DOMFrame index="0"><elements/></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"#
    );
    for (path, data) in [("DOMDocument.xml", xml.as_bytes())].iter().chain(files) {
        fs::create_dir_all(dir.join(path).parent().unwrap()).unwrap();
        fs::write(dir.join(path), data).unwrap();
    }
}

#[test]
fn file_names_with_a_percent_sign_round_trip() {
    // Imported, saved and opened again.
    let mut doc = Document::new(550.0, 400.0, 24.0);
    doc.sounds.insert("100% loud.wav".into(), Sound::from_file(wav(22050, 1, 0.1), "100% loud.wav").unwrap());
    let dir = scratch("percent_names");
    xfl_write::save(&doc, &dir.join("saved")).unwrap();
    assert_eq!(xfl::load(&dir.join("saved")).unwrap(), doc);

    // Saved by Animate, its file named as the item says.
    let file = wav(22050, 1, 0.1);
    animate_folder(&dir.join("animate"), r#"<DOMSoundItem name="100% loud.wav" href="100% loud.wav"/>"#, &[("LIBRARY/100% loud.wav", &file)]);
    let doc = xfl::load(&dir.join("animate")).unwrap();
    assert!(doc.sounds.contains_key("100% loud.wav"), "read from LIBRARY/100% loud.wav");
    xfl_write::save_preserving(&doc, Some(&dir.join("animate")), &dir.join("resaved")).unwrap();
    assert_eq!(fs::read(dir.join("resaved/LIBRARY/100% loud.wav")).unwrap(), file);
    assert_eq!(xfl::load(&dir.join("resaved")).unwrap(), doc);
}

#[test]
fn the_imported_file_is_read_before_animates_copy() {
    // Animate keeps the file it imported in LIBRARY/ beside its own copy in bin/, whose layout
    // isn't known: the file is what plays, and both go back as they were.
    let (file, copy) = (wav(22050, 1, 0.1), [0x55u8; 64]);
    let dir = scratch("imported_file_first");
    let item = r#"<DOMSoundItem name="click.wav" href="click.wav" soundDataHRef="M 2 1.dat" format="22kHz 16bit Mono"/>"#;
    animate_folder(&dir.join("source"), item, &[("LIBRARY/click.wav", &file), ("bin/M 2 1.dat", &copy)]);
    let doc = xfl::load(&dir.join("source")).unwrap();
    assert_eq!(doc.sounds["click.wav"].info().map(|i| i.codec), Some(Codec::Wav));
    xfl_write::save_preserving(&doc, Some(&dir.join("source")), &dir.join("saved")).unwrap();
    assert_eq!(fs::read(dir.join("saved/LIBRARY/click.wav")).unwrap(), file);
    assert_eq!(fs::read(dir.join("saved/bin/M 2 1.dat")).unwrap(), copy);
    let saved = fs::read_to_string(dir.join("saved/DOMDocument.xml")).unwrap();
    assert!(saved.contains(item.trim_end_matches("/>")), "{saved}");
    assert_eq!(xfl::load(&dir.join("saved")).unwrap(), doc);
}
