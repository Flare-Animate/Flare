//! Port of TEnv::getSystemVarPathSetValue split logic (flare/sources/common/tapptools/tenv.cpp).
//! Split on ';'; a trailing ';' adds no empty entry; empty value -> empty list.

pub fn path_set(value: &str) -> Vec<String> {
    if value.is_empty() {
        return vec![];
    }
    let mut parts: Vec<String> = value.split(';').map(String::from).collect();
    if value.ends_with(';') {
        parts.pop();
    }
    parts
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn split() {
        assert_eq!(path_set(""), Vec::<String>::new());
        assert_eq!(path_set("a"), vec!["a"]);
        assert_eq!(path_set("a;b"), vec!["a", "b"]);
        assert_eq!(path_set("a;"), vec!["a"]);
        assert_eq!(path_set("a;;"), vec!["a", ""]);
        assert_eq!(path_set(";"), vec![""]);
        assert_eq!(path_set(";a"), vec!["", "a"]);
    }
}
