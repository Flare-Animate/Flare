//! The file endings a still picture is named with.
#![allow(clippy::expect_used)]

use super::*;

#[test]
fn every_listed_ending_names_a_format_and_the_format_names_its_own_ending() {
    for ending in StillFormat::EXTENSIONS {
        let format = StillFormat::from_extension(ending)
            .unwrap_or_else(|| panic!("{ending} is listed but names no format"));
        // The canonical ending of each format is one of the listed ones.
        assert!(
            StillFormat::EXTENSIONS.contains(&format.extension()),
            "{ending} gives {}, which is not listed",
            format.extension()
        );
    }
}

#[test]
fn endings_are_read_whatever_their_case_and_the_aliases_name_one_format() {
    assert_eq!(StillFormat::from_extension("PNG"), Some(StillFormat::Png));
    assert_eq!(
        StillFormat::from_extension("Jpeg"),
        Some(StillFormat::Jpeg { quality: 90 })
    );
    assert_eq!(
        StillFormat::from_extension("jpg"),
        StillFormat::from_extension("JPEG")
    );
    assert_eq!(
        StillFormat::from_extension("tiff"),
        StillFormat::from_extension("tif")
    );
    assert_eq!(
        StillFormat::from_extension("pam"),
        StillFormat::from_extension("pnm")
    );
}

#[test]
fn an_ending_no_still_format_has_names_nothing() {
    // Animations and video are not stills; neither is a name with no ending at all.
    for ending in ["gif", "apng", "mp4", "mov", "svg", "", "pn", "png "] {
        assert_eq!(StillFormat::from_extension(ending), None, "{ending:?}");
    }
}
