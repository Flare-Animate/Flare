//! An ABC block's names as text: strings, namespaces and multinames looked up by pool index.
//! Index 0 means "none" (or "any" for a type), and an index past the pool is shown, not trusted.

use swf::avm2::types::{AbcFile, DefaultValue, Multiname, Namespace};

/// How deep a parameterised type (`Vector.<Vector.<int>>`) is followed. A crafted block can make
/// one refer to itself.
const MAX_TYPE_DEPTH: usize = 8;

/// The longest string shown whole in a listing; longer ones are cut with a mark.
const MAX_SHOWN_CHARS: usize = 400;

pub(crate) struct Names<'a> {
    pub abc: &'a AbcFile,
}

impl<'a> Names<'a> {
    pub fn new(abc: &'a AbcFile) -> Self {
        Self { abc }
    }

    /// The string at pool `index`, or `None` for index 0 or one past the pool.
    pub fn string(&self, index: u32) -> Option<String> {
        let at = index.checked_sub(1)? as usize;
        let bytes = self.abc.constant_pool.strings.get(at)?;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }

    fn namespace(&self, index: u32) -> Option<&Namespace> {
        let at = index.checked_sub(1)? as usize;
        self.abc.constant_pool.namespaces.get(at)
    }

    /// The URI of the namespace at `index`: a package's name, or a custom namespace's URI.
    pub fn namespace_name(&self, index: u32) -> String {
        self.namespace(index)
            .and_then(|ns| self.string(namespace_string(ns)))
            .unwrap_or_default()
    }

    /// What a member's namespace makes it in ActionScript: public, private, protected, internal,
    /// or the custom namespace it is declared in (its URI's last part, as code names it).
    pub fn visibility(&self, index: u32) -> String {
        match self.namespace(index) {
            Some(Namespace::Package(_)) => "public".into(),
            Some(Namespace::PackageInternal(_)) => "internal".into(),
            Some(Namespace::Private(_)) => "private".into(),
            Some(Namespace::Protected(_) | Namespace::StaticProtected(_)) => "protected".into(),
            Some(ns @ (Namespace::Namespace(_) | Namespace::Explicit(_))) => {
                let uri = self.string(namespace_string(ns)).unwrap_or_default();
                let last = uri.rsplit(['/', '#', ':']).next().unwrap_or_default();
                if is_identifier(last) {
                    last.to_owned()
                } else {
                    "internal".into()
                }
            }
            None => "public".into(),
        }
    }

    /// A multiname as code would write it: `flash.display.Sprite`, `Vector.<int>`, or `*` for
    /// none. Names a running program works out are shown as `[runtime]`.
    pub fn multiname(&self, index: u32) -> String {
        self.multiname_at(index, 0)
    }

    fn multiname_at(&self, index: u32, depth: usize) -> String {
        let Some(name) = index
            .checked_sub(1)
            .and_then(|at| self.abc.constant_pool.multinames.get(at as usize))
        else {
            return "*".into();
        };
        match name {
            Multiname::QName { namespace, name } | Multiname::QNameA { namespace, name } => {
                let short = self.string(name.0).unwrap_or_else(|| "*".into());
                match self.namespace(namespace.0) {
                    Some(Namespace::Package(ns)) => match self.string(ns.0) {
                        Some(package) if !package.is_empty() => format!("{package}.{short}"),
                        _ => short,
                    },
                    _ => short,
                }
            }
            Multiname::RTQName { name }
            | Multiname::RTQNameA { name }
            | Multiname::Multiname { name, .. }
            | Multiname::MultinameA { name, .. } => {
                self.string(name.0).unwrap_or_else(|| "*".into())
            }
            Multiname::RTQNameL
            | Multiname::RTQNameLA
            | Multiname::MultinameL { .. }
            | Multiname::MultinameLA { .. } => "[runtime]".into(),
            Multiname::TypeName {
                base_type,
                parameters,
            } => {
                if depth >= MAX_TYPE_DEPTH {
                    return "*".into();
                }
                let base = self.multiname_at(base_type.0, depth + 1);
                let params: Vec<String> = parameters
                    .iter()
                    .map(|p| self.multiname_at(p.0, depth + 1))
                    .collect();
                format!("{base}.<{}>", params.join(", "))
            }
        }
    }

    /// The package and short name of a class's name (a QName); the package is empty at the top.
    pub fn qname(&self, index: u32) -> (String, String) {
        let found = index
            .checked_sub(1)
            .and_then(|at| self.abc.constant_pool.multinames.get(at as usize));
        match found {
            Some(Multiname::QName { namespace, name } | Multiname::QNameA { namespace, name }) => (
                self.namespace_name(namespace.0),
                self.string(name.0).unwrap_or_else(|| "*".into()),
            ),
            _ => (String::new(), self.multiname(index)),
        }
    }

    /// The namespace a member's name is declared in, for its visibility.
    pub fn member_namespace(&self, index: u32) -> u32 {
        match index
            .checked_sub(1)
            .and_then(|at| self.abc.constant_pool.multinames.get(at as usize))
        {
            Some(Multiname::QName { namespace, .. } | Multiname::QNameA { namespace, .. }) => {
                namespace.0
            }
            _ => 0,
        }
    }

    /// A default value as an ActionScript literal.
    pub fn value(&self, value: &DefaultValue) -> String {
        let pool = &self.abc.constant_pool;
        let at = |i: u32| i.checked_sub(1).map(|i| i as usize);
        match value {
            DefaultValue::Int(i) => at(i.0)
                .and_then(|i| pool.ints.get(i))
                .map_or("0".into(), i32::to_string),
            DefaultValue::Uint(i) => at(i.0)
                .and_then(|i| pool.uints.get(i))
                .map_or("0".into(), u32::to_string),
            DefaultValue::Double(i) => at(i.0)
                .and_then(|i| pool.doubles.get(i))
                .map_or("NaN".into(), |d| number(*d)),
            DefaultValue::String(i) => quoted(&self.string(i.0).unwrap_or_default()),
            DefaultValue::True => "true".into(),
            DefaultValue::False => "false".into(),
            DefaultValue::Null => "null".into(),
            DefaultValue::Undefined => "undefined".into(),
            DefaultValue::Namespace(i)
            | DefaultValue::Package(i)
            | DefaultValue::PackageInternal(i)
            | DefaultValue::Protected(i)
            | DefaultValue::Explicit(i)
            | DefaultValue::StaticProtected(i)
            | DefaultValue::Private(i) => quoted(&self.namespace_name(i.0)),
        }
    }
}

/// The string index a namespace carries, of whatever kind.
pub(crate) fn namespace_string(ns: &Namespace) -> u32 {
    match ns {
        Namespace::Namespace(s)
        | Namespace::Package(s)
        | Namespace::PackageInternal(s)
        | Namespace::Protected(s)
        | Namespace::Explicit(s)
        | Namespace::StaticProtected(s)
        | Namespace::Private(s) => s.0,
    }
}

/// A number as ActionScript writes it.
pub(crate) fn number(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value.is_infinite() {
        if value > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        format!("{value}")
    }
}

/// A string as an ActionScript literal, cut when very long so one string cannot swamp a listing.
pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(MAX_SHOWN_CHARS) + 2);
    out.push('"');
    for (count, c) in text.chars().enumerate() {
        if count == MAX_SHOWN_CHARS {
            out.push_str("...");
            break;
        }
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}
