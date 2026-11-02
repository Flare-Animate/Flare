//! The language the operating system is set to.

/// The system's locale as a tag such as `de-DE`, or `en` when it cannot be found.
pub fn system_tag() -> String {
    sys_locale::get_locale().unwrap_or_else(|| "en".to_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn there_is_always_a_tag_even_when_the_system_says_nothing() {
        assert!(!super::system_tag().is_empty());
    }
}
