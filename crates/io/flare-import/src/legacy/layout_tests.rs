use super::layout::{Era, LegacyInfo, StreamKind, classify};

#[test]
fn both_naming_styles_are_told_apart() {
    assert_eq!(classify("Page 1"), Some((StreamKind::Page, Era::Numbered)));
    assert_eq!(
        classify("Symbol 12"),
        Some((StreamKind::Symbol, Era::Numbered))
    );
    assert_eq!(
        classify("Media 3"),
        Some((StreamKind::Media, Era::Numbered))
    );
    assert_eq!(
        classify("P 1 1678559673"),
        Some((StreamKind::Page, Era::Stamped))
    );
    assert_eq!(
        classify("S 2 1678560746"),
        Some((StreamKind::Symbol, Era::Stamped))
    );
    assert_eq!(
        classify("M 1 1687180517"),
        Some((StreamKind::Media, Era::Stamped))
    );
    for other in [
        "Contents", "Page", "Page x", "P 1", "Page 1 2", "S 1 x", "Symbol", "",
    ] {
        assert_eq!(classify(other), None, "{other:?}");
    }
}

#[test]
fn the_summary_counts_and_names_the_era() {
    let names = ["Contents", "P 1 5", "S 1 7", "S 2 8", "M 1 9", "Other"];
    let info = LegacyInfo::from_names(names.into_iter(), Some(0x147));
    assert_eq!(info.era, Era::Stamped);
    assert_eq!(
        (info.pages, info.symbols, info.media, info.other_streams),
        (1, 2, 1, 1)
    );
    let text = info.to_string();
    assert!(
        text.contains("0x0147") && text.contains("2 symbol(s)"),
        "{text}"
    );
    let mixed = LegacyInfo::from_names(["Page 1", "S 1 2"].into_iter(), None);
    assert_eq!(mixed.era, Era::Unknown);
}
