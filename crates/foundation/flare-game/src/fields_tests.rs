#![allow(clippy::expect_used)]

use super::*;

#[test]
fn splits_each_token_at_its_first_equals() {
    let fields = parse_fields(&["x=1.5", " speed = 4 ", "label=a=b", "empty="]).expect("fields");
    assert_eq!(
        fields,
        vec![
            ("x".to_owned(), "1.5".to_owned()),
            ("speed".to_owned(), "4".to_owned()),
            ("label".to_owned(), "a=b".to_owned()),
            ("empty".to_owned(), String::new()),
        ]
    );
}

#[test]
fn refuses_a_token_without_a_name() {
    for bad in ["x", "=3", "  =3"] {
        assert!(
            matches!(parse_fields(&[bad]), Err(GameError::BadInput(_))),
            "{bad:?} must be refused"
        );
    }
}

#[test]
fn refuses_more_fields_than_a_component_can_hold() {
    let tokens: Vec<String> = (0..=MAX_FIELDS).map(|i| format!("f{i}=1")).collect();
    assert!(matches!(parse_fields(&tokens), Err(GameError::BadInput(_))));
    assert!(parse_fields(&tokens[..MAX_FIELDS]).is_ok());
}
