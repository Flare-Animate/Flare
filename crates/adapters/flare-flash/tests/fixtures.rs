//! Real small files of the Flash family: what the inspector says of each.
#![allow(clippy::expect_used)]

use std::path::Path;

use flare_flash::{PrefixKind, SwfCompression, inspect_prefix};

fn prefix_of(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let bytes = std::fs::read(path).expect("fixture");
    bytes[..bytes.len().min(4096)].to_vec()
}

#[test]
fn swf_files_are_recognised_by_their_first_bytes_with_version_and_length() {
    let kind = inspect_prefix(&prefix_of("sample.swf")).expect("a SWF header");
    assert_eq!(
        kind,
        PrefixKind::SwfHeader {
            compression: SwfCompression::None,
            version: 5,
            declared_uncompressed_length: 21,
        }
    );
    for name in [
        "ruffle_DefineShape.swf",
        "ruffle_DefineShape3.swf",
        "ruffle_DefineShape4.swf",
    ] {
        let kind = inspect_prefix(&prefix_of(name)).expect(name);
        assert!(
            matches!(
                kind,
                PrefixKind::SwfHeader { version, .. } if version >= 1
            ),
            "{name}: {kind:?}"
        );
    }
}

#[test]
fn video_files_are_not_mistaken_for_flash_containers() {
    for name in ["sample.flv", "sample.f4v"] {
        assert_eq!(
            inspect_prefix(&prefix_of(name)),
            Ok(PrefixKind::Unknown),
            "{name}"
        );
    }
}

#[test]
fn a_zip_is_only_a_candidate_and_every_truncation_of_a_header_is_an_error_not_a_panic() {
    let zip = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../io/flare-import/tests/fixtures/sample.fla"),
    )
    .expect("fla");
    assert_eq!(
        inspect_prefix(&zip[..zip.len().min(4096)]),
        Ok(PrefixKind::ZipContainerCandidate),
        "a zip signature identifies a container, not a Flash document"
    );
    let swf = prefix_of("sample.swf");
    for cut in 0..swf.len() {
        let _ = inspect_prefix(&swf[..cut]);
    }
    assert!(inspect_prefix(&swf[..4]).is_err());
}
