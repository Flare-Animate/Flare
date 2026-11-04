#![allow(clippy::expect_used)]
//! Palette reader and header sniff tests on synthetic inputs.

use flare_toonz::{Format, ToonzError, decode_pli, decode_tlv, parse_palette, sniff};

const PALETTE: &str = r#"<palette>
  <version>71 0</version>
  <styles>
    <style id="1"/>
    <style id="2"/>
  </styles>
  <stylepages>
    <page>
      <name>Main</name>
      <indices>1 2</indices>
    </page>
  </stylepages>
</palette>"#;

#[test]
fn reads_palette_version_styles_and_pages() {
    let palette = parse_palette(PALETTE).expect("palette parses");
    assert_eq!(palette.version, Some((71, 0)));
    assert_eq!(palette.style_count, 2);
    assert_eq!(palette.pages.len(), 1);
    assert_eq!(palette.pages[0].name, "Main");
    assert_eq!(palette.pages[0].style_ids, vec![1, 2]);
}

#[test]
fn refuses_palette_with_a_bad_index_or_wrong_root() {
    let bad = PALETTE.replace("1 2", "1 x");
    assert!(parse_palette(&bad).is_err());
    assert!(matches!(
        parse_palette("<tnz/>"),
        Err(ToonzError::UnknownFormat(_))
    ));
}

#[test]
fn sniffs_scene_and_palette_by_root_element() {
    assert_eq!(
        sniff(b"<?xml version=\"1.0\"?>\n<!-- c -->\n<tnz version=\"71.0\"/>"),
        Format::Scene
    );
    assert_eq!(sniff(PALETTE.as_bytes()), Format::Palette);
    assert_eq!(sniff(b"\xEF\xBB\xBF<tnz>"), Format::Scene);
    assert_eq!(sniff(b"<html>"), Format::Unknown);
    assert_eq!(sniff(b""), Format::Unknown);
}

#[test]
fn sniffs_pli_both_byte_orders() {
    assert_eq!(
        sniff(b"PLIM\x00\x00"),
        Format::Pli {
            legacy_byte_order: false
        }
    );
    assert_eq!(
        sniff(b"MILP\x00\x00"),
        Format::Pli {
            legacy_byte_order: true
        }
    );
}

#[test]
fn sniffs_tlv_versions() {
    assert_eq!(sniff(b"TLV10\0\0\0"), Format::Tlv { version: 10 });
    assert_eq!(sniff(b"TLV11\0\0\0"), Format::Tlv { version: 11 });
    assert_eq!(sniff(b"TLV12\0\0\0"), Format::Tlv { version: 12 });
    assert_eq!(sniff(b"TLV13\0\0\0"), Format::Unknown);
}

#[test]
fn pli_and_tlv_decoding_is_not_yet_implemented_but_refuses_other_files() {
    assert!(matches!(
        decode_pli(b"PLIM\x00"),
        Err(ToonzError::NotYetImplemented { format: "pli", .. })
    ));
    assert!(matches!(
        decode_tlv(b"TLV12\0"),
        Err(ToonzError::NotYetImplemented { format: "tlv", .. })
    ));
    assert!(matches!(
        decode_pli(b"TLV12\0"),
        Err(ToonzError::UnknownFormat(_))
    ));
    assert!(matches!(
        decode_tlv(b"PLIM\x00"),
        Err(ToonzError::UnknownFormat(_))
    ));
}
