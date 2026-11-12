use super::*;
use flare_document::MAX_STAGE_SIDE;

#[test]
fn every_kind_has_sizes_a_stage_can_have_and_an_icon_that_exists() {
    for kind in CATEGORIES {
        assert!(!kind.presets.is_empty(), "{}", kind.name);
        assert!(flare_ui::icons::exists(kind.icon), "{}", kind.icon);
        for p in kind.presets {
            assert!(
                (1..=MAX_STAGE_SIDE).contains(&p.width) && (1..=MAX_STAGE_SIDE).contains(&p.height),
                "{}: {}x{}",
                p.name,
                p.width,
                p.height
            );
            assert!((1..=240).contains(&p.fps), "{}", p.name);
        }
    }
}

#[test]
fn a_preset_is_found_by_its_numbers_within_its_kind_only() {
    let character = category(0);
    assert_eq!(matching(character, 1920, 1080, 24), Some(0));
    assert_eq!(matching(character, 1920, 1080, 25), None, "another rate");
    let game = category(2);
    assert_eq!(
        matching(game, 1920, 1080, 24),
        None,
        "a game's Full HD is at 60"
    );
    assert_eq!(matching(game, 1920, 1080, 60), Some(1));
    assert_eq!(
        category(99).name,
        CATEGORIES[0].name,
        "out of range is the first"
    );
}
