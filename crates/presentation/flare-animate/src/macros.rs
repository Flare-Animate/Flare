//! `tr!`: a message in the language being spoken, with its key checked when the program is
//! built. A key typed wrong would otherwise show on screen as the key itself, and only a test
//! that happens to draw that screen would notice.

/// `tr_n!(workspace, "key", count)`, or with `&[("name", value)]` after the count: a message for
/// `count` things, which it names `{ $n }`, in the plural form its language gives that count.
macro_rules! tr_n {
    ($workspace:expr, $key:literal, $count:expr $(,)?) => {{
        const _: () = assert!(flare_i18n::has_key($key), "no message has this key");
        $workspace.tr_count($key, $count as u64)
    }};
    ($workspace:expr, $key:literal, $count:expr, $args:expr $(,)?) => {{
        const _: () = assert!(flare_i18n::has_key($key), "no message has this key");
        $workspace.tr_count_with($key, $count as u64, $args)
    }};
}

/// `tr!(workspace, "key")`, or `tr!(workspace, "key", &[("name", value)])` for a message with
/// `{ $name }` placeholders. The key must be a literal that English has; a key built at run time
/// (`format!`) goes through `workspace.tr(&key)` and is checked by the tests instead.
macro_rules! tr {
    ($workspace:expr, $key:literal $(,)?) => {{
        const _: () = assert!(flare_i18n::has_key($key), "no message has this key");
        $workspace.tr($key)
    }};
    ($workspace:expr, $key:literal, $args:expr $(,)?) => {{
        const _: () = assert!(flare_i18n::has_key($key), "no message has this key");
        $workspace.tr_with($key, $args)
    }};
}
