use super::matches;

#[test]
fn a_query_matches_the_words_it_names_in_order() {
    assert!(matches("key", "Insert Keyframe"));
    assert!(matches("kfrm", "Insert Keyframe"));
    assert!(matches("brsh", "Brush"));
    assert!(matches("frame", "Remove Frame"));
}

#[test]
fn letters_out_of_order_do_not_match() {
    assert!(!matches("yek", "Insert Keyframe"));
    assert!(!matches("xyz", "Insert Keyframe"));
}

#[test]
fn an_empty_query_matches_everything() {
    assert!(matches("", "Insert Keyframe"));
    assert!(matches("", ""));
}

#[test]
fn matching_ignores_case_and_accents_fall_to_their_own_letters() {
    assert!(matches("KEY", "Insert Keyframe"));
    assert!(matches("é", "Répéter"));
    assert!(!matches("r", ""));
}
