#![allow(clippy::expect_used)]
//! PLI decoding and scene conversion on files built byte by byte in these tests.

use flare_document::{Content, PathSegment};
use flare_toonz::{
    Severity, ToonzError, decode_pli, parse_palette, parse_scene, to_flare_document,
};

fn tag(out: &mut Vec<u8>, kind: u8, body: &[u8]) -> u32 {
    let at = out.len() as u32;
    out.push(kind | (2 << 6));
    out.extend((body.len() as u16).to_le_bytes());
    out.extend(body);
    at
}

fn signed(v: i16) -> [u8; 2] {
    let raw = if v < 0 {
        0x8000 | (-v) as u16
    } else {
        v as u16
    };
    raw.to_le_bytes()
}

/// A version 6.6 file with one frame (number 1): a red stroke of one quadratic from (0,0) through
/// (1,0) to (2,0) in level units, and the solid colour style 2.
fn pli() -> Vec<u8> {
    let mut f = b"PLIM".to_vec();
    f.extend([6, 6]);
    f.extend(0u16.to_le_bytes()); // empty creator
    f.extend(0u32.to_le_bytes()); // file length
    f.extend(1u16.to_le_bytes()); // frames
    f.extend([2, 0, 0]); // autoclose s, i, d
    let mut style = Vec::new();
    style.extend(2u16.to_le_bytes());
    style.extend(0u16.to_le_bytes());
    style.extend([6, 5, 0]);
    style.extend(b"color");
    style.extend([1, 3, 1, 255, 1, 0, 1, 0, 1, 255]);
    tag(&mut f, 20, &style);
    let mut color = vec![1, 1];
    color.extend(2u16.to_le_bytes());
    let c = tag(&mut f, 17, &color);
    let mut chain = vec![255];
    chain.extend(signed(0));
    chain.extend(signed(0));
    chain.push(10);
    chain.extend(signed(128));
    chain.extend(signed(0));
    chain.push(20);
    chain.extend(signed(128));
    chain.extend(signed(0));
    chain.push(30);
    let s = tag(&mut f, 10, &chain);
    let mut image = 1u16.to_le_bytes().to_vec();
    image.push(0);
    image.extend((c as u16).to_le_bytes());
    image.extend((s as u16).to_le_bytes());
    tag(&mut f, 16, &image);
    f.push(0);
    f
}

#[test]
fn a_hand_built_pli_decodes_strokes_thickness_and_palette() {
    let level = decode_pli(&pli()).expect("decodes");
    assert_eq!(level.version, (6, 6));
    assert_eq!(level.frames.len(), 1);
    let stroke = &level.frames[0].strokes[0];
    assert_eq!(stroke.style, 2);
    let [a, c, b] = stroke.chunks[0].0;
    assert_eq!((a.x, c.x, b.x), (0.0, 1.0, 2.0));
    assert_eq!((a.thick, c.thick, b.thick), (10.0, 20.0, 30.0));
    assert_eq!(level.styles[0].0, 2);
    assert_eq!(level.styles[0].1.color, Some([255, 0, 0, 255]));
}

#[test]
fn damaged_plis_are_refused_not_panicked_on() {
    let good = pli();
    for cut in [5, 12, good.len() - 3] {
        assert!(decode_pli(&good[..cut]).is_err(), "cut at {cut}");
    }
    let mut lying = good.clone();
    lying[18] = 0xFF; // the first tag claims a long body
    lying[19] = 0xFF;
    assert!(matches!(
        decode_pli(&lying),
        Err(ToonzError::Malformed { .. })
    ));
    assert!(matches!(
        decode_pli(b"nope"),
        Err(ToonzError::UnknownFormat(_))
    ));
    let mut old = b"PLIM".to_vec();
    old.extend([4, 0]);
    assert!(matches!(
        decode_pli(&old),
        Err(ToonzError::NotYetImplemented { .. })
    ));
}

const SCENE: &str = r#"<tnz version="71.0">
  <properties>
    <outputs><output name="main"><fps>12</fps></output></outputs>
    <cameras><camera><cameraSize>16 9</cameraSize><cameraRes>1280 720</cameraRes></camera></cameras>
  </properties>
  <levelSet><levels>
    <level id="1" name="A" type="vector"><path>+drawings/A.pli</path></level>
    <level id="2" name="B" type="raster"><path>+drawings/B.tlv</path></level>
    <level id="3" name="S" type="sound"><path>+sounds/S.wav</path></level>
  </levels></levelSet>
  <xsheet><columns>
    <levelColumn><cells><cell>0 2 1 1 0</cell></cells></levelColumn>
    <levelColumn><cells><cell>1 1 2 1 0</cell></cells></levelColumn>
    <soundColumn><SoundCells>0 10 0 3</SoundCells></soundColumn>
  </columns></xsheet>
</tnz>"#;

#[test]
fn a_scene_becomes_a_document_with_level_layers() {
    let scene = parse_scene(SCENE).expect("scene");
    assert_eq!(scene.camera_res, Some((1280, 720)));
    let bytes = pli();
    let out = to_flare_document(&scene, &|l| (l.id == 1).then(|| bytes.clone())).expect("converts");
    let doc = &out.document;
    assert_eq!((doc.stage().width(), doc.stage().height()), (1280, 720));
    assert_eq!(doc.frame_rate().numerator(), 12);
    let layers = doc.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 2);
    // The last column is the top layer.
    assert_eq!(layers[0].name(), "Column 2");
    let Content::Instance(i) = layers[1].spans()[0].content() else {
        panic!("instance")
    };
    assert_eq!(layers[1].spans()[0].duration().get(), 2);
    let symbol = doc.symbol(i.symbol()).expect("symbol");
    let Content::Shape(shape) = symbol.timeline().layers()[0].spans()[0].content() else {
        panic!("shape")
    };
    assert!(matches!(shape.path()[1], PathSegment::CubicTo(..)));
    assert_eq!(shape.stroke().expect("stroke").color().r, 255);
    let unsupported: Vec<_> = out
        .notes
        .iter()
        .filter(|n| n.severity == Severity::Unsupported)
        .collect();
    assert!(unsupported.iter().any(|n| n.location == "B"));
    assert!(
        unsupported
            .iter()
            .any(|n| n.message.contains("sound column"))
    );
}

#[test]
fn a_missing_level_file_is_named_and_the_cells_stay_empty() {
    let scene = parse_scene(SCENE).expect("scene");
    let out = to_flare_document(&scene, &|_| None).expect("converts");
    assert!(
        out.notes
            .iter()
            .any(|n| n.location == "A" && n.message.contains("not found"))
    );
}

#[test]
fn palette_styles_carry_real_colours() {
    let palette = parse_palette(
        r#"<palette><styles><style>_0 color 3 10 20 30 255</style><style>"my red" 3 255 0 0 128</style><style>9 1 2 3 4</style></styles></palette>"#,
    )
    .expect("palette");
    assert_eq!(palette.styles[0].color, Some([10, 20, 30, 255]));
    assert_eq!(palette.styles[1].name, "my red");
    assert_eq!(palette.styles[1].color, Some([255, 0, 0, 128]));
    assert_eq!(palette.styles[2].color, None);
}
