use super::parse_hex;
use egui::Color32;

#[test]
fn hex_colors_parse_with_or_without_the_hash_and_reject_junk() {
    assert_eq!(
        parse_hex("#E8453C"),
        Some(Color32::from_rgb(0xe8, 0x45, 0x3c))
    );
    assert_eq!(
        parse_hex("e8453c"),
        Some(Color32::from_rgb(0xe8, 0x45, 0x3c))
    );
    assert_eq!(
        parse_hex(" #fa0 "),
        Some(Color32::from_rgb(0xff, 0xaa, 0x00))
    );
    assert_eq!(parse_hex("#12"), None);
    assert_eq!(parse_hex("#GGGGGG"), None);
    assert_eq!(parse_hex("#é1234"), None);
}
