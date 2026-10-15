use super::*;
use ac_core::model::*;
use ac_core::sound::{FrameSound, SoundAsset};

fn tone(freq: f32, secs: f32, amp: f32) -> Vec<f32> {
    let rate = 22050.0;
    (0..(secs * rate) as usize).flat_map(|i| {
        let v = (i as f32 / rate * freq * std::f32::consts::TAU).sin() * amp;
        [v, v]
    }).collect()
}

fn doc_with(sounds: Vec<(u32, FrameSound)>, frames: u32) -> Document {
    let mut d = Document { fps: 10.0, ..Default::default() };
    d.sounds.push(SoundAsset { id: 1, name: "beep.wav".into(), data: wav(&tone(440.0, 1.0, 0.5), 22050) });
    let l = &mut d.scene.layers[0];
    for (f, s) in sounds {
        let mut k = Keyframe::blank();
        k.sound = Some(s);
        l.keyframes.insert(f, k);
    }
    l.end = frames;
    d
}

#[test]
fn decodes_wav_and_reports_length() {
    let p = decode(&wav(&tone(440.0, 0.5, 0.5), 22050)).unwrap();
    assert_eq!(p.rate, 22050);
    assert!((p.seconds() - 0.5).abs() < 0.01);
    let pk = peaks(&p, 0.0, 0.5, 10);
    assert!(pk.iter().all(|(lo, hi)| *hi > 0.45 && *lo < -0.45));
    assert!(decode(b"not audio").is_err());
}

#[test]
fn event_start_stop_and_stream_follow_animate() {
    let len = |_| Some(1.0);
    // Event at 0 and 5: both play their full second (the movie is 3 s)
    let d = doc_with(vec![(0, FrameSound::new(1)), (5, FrameSound::new(1))], 30);
    let v = schedule(&d, &len);
    assert_eq!(v.len(), 2);
    assert!((v[1].start - 0.5).abs() < 1e-9 && (v[1].end - 1.5).abs() < 1e-9);
    // Start does not restart a sound already playing
    let d = doc_with(vec![(0, FrameSound::new(1)), (5, FrameSound { sync: SoundSync::Start, ..FrameSound::new(1) })], 30);
    assert_eq!(schedule(&d, &len).len(), 1);
    // Stop cuts it; repeat 3 would otherwise run 3 s
    let d = doc_with(vec![(0, FrameSound { repeat: 3, ..FrameSound::new(1) }), (12, FrameSound { sync: SoundSync::Stop, ..FrameSound::new(1) })], 30);
    let v = schedule(&d, &len);
    assert!((v[0].end - 1.2).abs() < 1e-9, "{v:?}");
    // Stream ends with its span (frames 0-3 → 0.4 s)
    let d = doc_with(vec![(0, FrameSound { sync: SoundSync::Stream, forever: true, ..FrameSound::new(1) }), (4, FrameSound { sync: SoundSync::Stop, sound: 2, ..FrameSound::new(2) })], 30);
    let v = schedule(&d, &len);
    assert!((v[0].end - 0.4).abs() < 1e-9, "{v:?}");
}

#[test]
fn mixes_the_timeline_and_joins_streams_midway() {
    let d = doc_with(vec![(0, FrameSound { sync: SoundSync::Stream, ..FrameSound::new(1) })], 20);
    let all = mix(&d, 0, 22050);
    assert_eq!(all.len(), 2 * 22050 * 2);
    let rms = |s: &[f32]| (s.iter().map(|v| v * v).sum::<f32>() / s.len().max(1) as f32).sqrt();
    assert!(rms(&all[..22050 * 2]) > 0.3, "the first second plays the tone");
    assert!(rms(&all[22050 * 2..]) < 1e-3, "then silence");
    // from frame 5 the stream is joined half way
    let mid = mix(&d, 5, 22050);
    assert!(rms(&mid[..1000]) > 0.3);
    // an Event started before the play head is not heard
    let d = doc_with(vec![(0, FrameSound::new(1))], 20);
    assert!(rms(&mix(&d, 5, 22050)) < 1e-3);
    assert!(has_sound(&d));
}

#[test]
fn lip_sync_tells_silence_vowels_and_hiss_apart() {
    let mut s = tone(0.0, 0.5, 0.0);
    s.extend(tone(300.0, 0.5, 0.8));
    // noise for a fricative
    let mut x: u32 = 1;
    s.extend((0..11025).flat_map(|_| {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        let v = (x as f32 / u32::MAX as f32 - 0.5) * 0.6;
        [v, v]
    }));
    let p = Pcm { rate: 22050, samples: s };
    let v = visemes(&p, 0.0, 10.0, 15);
    assert!(v[..4].iter().all(|x| *x == Viseme::Rest), "{v:?}");
    assert!(v[6..9].iter().all(|x| *x == Viseme::O), "{v:?}");
    assert!(v[11..14].iter().all(|x| *x == Viseme::Fv), "{v:?}");
}

#[test]
fn lip_sync_and_sound_commands_drive_the_editor() {
    use ac_core::commands::execute;
    use serde_json::json;
    register();
    let dir = std::env::temp_dir().join(format!("ac-audio-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // 0.5 s silence, 0.5 s "oh", 0.5 s hiss at 10 fps
    let mut s = tone(0.0, 0.5, 0.0);
    s.extend(tone(300.0, 0.5, 0.8));
    let mut x: u32 = 7;
    s.extend((0..11025).flat_map(|_| {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        let v = (x as f32 / u32::MAX as f32 - 0.5) * 0.6;
        [v, v]
    }));
    let path = dir.join("voice.wav");
    std::fs::write(&path, wav(&s, 22050)).unwrap();
    std::fs::write(dir.join("junk.wav"), b"nope").unwrap();

    let mut d = Document { fps: 10.0, ..Default::default() };
    // a mouth symbol: 10 frames, one pose each
    let mut mouth = Timeline::new("Mouth");
    mouth.layers[0].end = 10;
    d.library.push(Symbol { id: 1, name: "Mouth".into(), kind: SymbolKind::Graphic, timeline: mouth });
    d.next_symbol_id = 2;
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements.push(Element::instance("", 1, ac_core::geom::Matrix::IDENTITY));
    d.scene.layers.push(Layer::new("Voice", 1));
    let mut ed = ac_core::Editor::new(d);
    assert!(execute(&mut ed, "import.sound", &json!({ "path": dir.join("junk.wav").to_str().unwrap() })).is_err());
    let id = execute(&mut ed, "import.sound", &json!({ "path": path.to_str().unwrap() })).unwrap();
    assert_eq!(id, json!(1));
    let list = execute(&mut ed, "sound.list", &json!({})).unwrap();
    assert!((list[0]["seconds"].as_f64().unwrap() - 1.5).abs() < 0.01);
    execute(&mut ed, "timeline.set_sound", &json!({ "sound": "voice", "sync": "stream", "layer": 2, "frame": 1 })).unwrap();
    execute(&mut ed, "timeline.set_layer_end", &json!({ "end": 15, "layer": 2 })).unwrap();
    assert_eq!(ed.doc.scene.layers[1].keyframes[&0].sound.as_ref().unwrap().sync, SoundSync::Stream);
    // select the mouth and lip-sync it to layer 2
    ed.layer = 0;
    ed.frame = 0;
    ed.selection = vec![ac_core::Selection { layer: 0, element: 0 }];
    let made = execute(&mut ed, "lipsync.apply", &json!({ "audio_layer": 2, "map": { "O": 5 } })).unwrap().as_u64().unwrap();
    assert!(made >= 3, "{made}");
    let l = &ed.doc.scene.layers[0];
    assert!(l.end >= 15);
    let shown = |f: u32| match &ac_core::frames::content_at(l, f).0[0].kind {
        ElementKind::Instance(i) => (i.loop_mode, i.first_frame),
        _ => panic!(),
    };
    assert_eq!(shown(1), (LoopMode::SingleFrame, 0), "neutral while silent");
    assert_eq!(shown(7).1, 4, "the mapped O frame (5, 1-based)");
    assert_eq!(shown(12).1, 6, "FV = default frame 7");
    // the sound is in the mix and survives an XFL-free JSON round trip
    let back = Document::from_json(&ed.doc.to_json()).unwrap();
    assert_eq!(back.sounds.len(), 1);
    assert!(has_sound(&back));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn sounds_inside_symbols_play_and_effects_shape_them() {
    use ac_core::geom::Matrix;
    // a 5-frame graphic with a sound on its first frame, looping for 12 frames
    let mut d = doc_with(vec![], 12);
    let mut tl = Timeline::new("Beeper");
    let mut k = Keyframe::blank();
    k.sound = Some(FrameSound::new(1));
    tl.layers[0].keyframes.insert(0, k);
    tl.layers[0].end = 5;
    d.library.push(Symbol { id: 7, name: "Beeper".into(), kind: SymbolKind::Graphic, timeline: tl });
    d.next_symbol_id = 8;
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements.push(Element::instance("", 7, Matrix::IDENTITY));
    let v = schedule(&d, &|_| Some(0.2));
    let starts: Vec<f64> = v.iter().map(|v| (v.start * 10.0).round()).collect();
    assert_eq!(starts, [0.0, 5.0, 10.0], "plays each time the graphic loops round");
    // a single-frame graphic doesn't retrigger every frame
    if let ElementKind::Instance(i) = &mut d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0].kind {
        i.loop_mode = LoopMode::SingleFrame;
    }
    assert_eq!(schedule(&d, &|_| Some(0.2)).len(), 1);
    // effects: left channel only, fade in
    let mut d = doc_with(vec![(0, FrameSound { effect: ac_core::sound::SoundEffect::LeftChannel, ..FrameSound::new(1) })], 10);
    let m = mix(&d, 0, 22050);
    let (l, r): (f32, f32) = m[..20000].chunks(2).fold((0.0, 0.0), |(a, b), s| (a + s[0].abs(), b + s[1].abs()));
    assert!(l > 100.0 && r < 1e-3, "{l} {r}");
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().sound.as_mut().unwrap().effect = ac_core::sound::SoundEffect::FadeIn;
    let m = mix(&d, 0, 22050);
    let early: f32 = m[..2000].iter().map(|x| x.abs()).sum();
    let late: f32 = m[40000..42000].iter().map(|x| x.abs()).sum();
    assert!(early * 5.0 < late, "fades in: {early} vs {late}");
}

#[test]
fn decodes_aiff() {
    // 16-bit stereo AIFF, 100 frames of a ramp
    let frames = 100u32;
    let mut ssnd = vec![0u8; 8];
    for i in 0..frames {
        let v = (i as i16 * 300).to_be_bytes();
        ssnd.extend_from_slice(&v);
        ssnd.extend_from_slice(&v);
    }
    let mut comm = vec![];
    comm.extend_from_slice(&2u16.to_be_bytes());
    comm.extend_from_slice(&frames.to_be_bytes());
    comm.extend_from_slice(&16u16.to_be_bytes());
    // 22050 as 80-bit extended
    comm.extend_from_slice(&[0x40, 0x0D, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]);
    let mut body = b"AIFF".to_vec();
    body.extend_from_slice(b"COMM");
    body.extend_from_slice(&(comm.len() as u32).to_be_bytes());
    body.extend_from_slice(&comm);
    body.extend_from_slice(b"SSND");
    body.extend_from_slice(&(ssnd.len() as u32).to_be_bytes());
    body.extend_from_slice(&ssnd);
    let mut file = b"FORM".to_vec();
    file.extend_from_slice(&(body.len() as u32).to_be_bytes());
    file.extend_from_slice(&body);
    let p = decode(&file).unwrap();
    assert_eq!((p.rate, p.frames()), (22050, 100));
}
