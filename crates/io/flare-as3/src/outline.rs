//! The classes a movie's scripts define, with their members, as data.

use crate::movie::{BlockInfo, Movie};
use crate::names::Names;
use serde::Serialize;
use swf::avm2::types::{AbcFile, MethodFlags, Trait, TraitKind};

/// Which classes an outline lists.
#[derive(Debug, Clone, Default)]
pub struct OutlineQuery {
    /// Only classes whose full name contains this, ignoring case.
    pub find: Option<String>,
    /// The most classes listed; the rest are counted. `None` lists every one.
    pub limit: Option<usize>,
}

/// A movie's scripts: their blocks and the classes found in them.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Outline {
    pub blocks: Vec<BlockInfo>,
    /// Blocks that could not be read, with why; their classes are not listed.
    pub unreadable: Vec<String>,
    /// How many classes matched the query, listed or not.
    pub matched: usize,
    pub classes: Vec<ClassOutline>,
}

/// One class (or interface) as its declaration gives it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClassOutline {
    /// The block it is defined in.
    pub block: usize,
    pub package: String,
    pub name: String,
    pub full_name: String,
    /// The base class; `None` for `Object` and for interfaces.
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub interface: bool,
    #[serde(rename = "final")]
    pub is_final: bool,
    pub dynamic: bool,
    pub constructor: Signature,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemberKind {
    Var,
    Const,
    Method,
    Getter,
    Setter,
    Function,
    Class,
}

/// A variable, constant, method or accessor of a class.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Member {
    pub name: String,
    pub kind: MemberKind,
    #[serde(rename = "static")]
    pub is_static: bool,
    /// `public`, `private`, `protected`, `internal`, or the custom namespace it is declared in.
    pub visibility: String,
    #[serde(rename = "override")]
    pub is_override: bool,
    /// A variable's or constant's type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// A variable's or constant's value as the bytecode records it, as a literal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<Signature>,
}

/// A method's parameters and result, and how much code its body has.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Signature {
    /// Its index in the block, which a listing names.
    pub method: u32,
    pub params: Vec<Param>,
    /// Whether it takes `...rest` after its parameters.
    pub rest: bool,
    pub returns: String,
    /// Bytes of bytecode in its body; 0 when it has none (an interface's, or a native one).
    pub code_bytes: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Param {
    /// Its name, when the compiler kept names (debug builds and some release builds do).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Lists the classes of every block of `movie` that matches `query`, in block order.
pub fn outline(movie: &Movie, query: &OutlineQuery) -> Outline {
    let find = query.find.as_ref().map(|f| f.to_lowercase());
    let limit = query.limit.unwrap_or(usize::MAX);
    let mut outline = Outline {
        blocks: movie.blocks(),
        unreadable: Vec::new(),
        matched: 0,
        classes: Vec::new(),
    };
    for block in 0..movie.block_count() {
        let abc = match movie.abc(block) {
            Ok(abc) => abc,
            Err(error) => {
                outline.unreadable.push(error.to_string());
                continue;
            }
        };
        let names = Names::new(&abc);
        for index in 0..abc.instances.len() {
            let (package, name) = names.qname(abc.instances[index].name.0);
            let full = full_name(&package, &name);
            if find
                .as_ref()
                .is_some_and(|f| !full.to_lowercase().contains(f))
            {
                continue;
            }
            outline.matched += 1;
            if outline.classes.len() < limit {
                outline.classes.push(class_outline(&abc, block, index));
            }
        }
    }
    outline
}

pub(crate) fn full_name(package: &str, name: &str) -> String {
    if package.is_empty() {
        name.to_owned()
    } else {
        format!("{package}.{name}")
    }
}

/// The class at `index` of `abc`. `index` must be in range.
pub(crate) fn class_outline(abc: &AbcFile, block: usize, index: usize) -> ClassOutline {
    let names = Names::new(abc);
    let instance = &abc.instances[index];
    let (package, name) = names.qname(instance.name.0);
    let extends = (instance.super_name.0 != 0)
        .then(|| names.multiname(instance.super_name.0))
        .filter(|base| base != "Object");
    let mut members: Vec<Member> = instance
        .traits
        .iter()
        .map(|t| member(&names, t, false))
        .collect();
    if let Some(class) = abc.classes.get(index) {
        members.extend(class.traits.iter().map(|t| member(&names, t, true)));
    }
    ClassOutline {
        block,
        full_name: full_name(&package, &name),
        package,
        name,
        extends,
        implements: instance
            .interfaces
            .iter()
            .map(|i| names.multiname(i.0))
            .collect(),
        interface: instance.is_interface,
        is_final: instance.is_final,
        dynamic: !instance.is_sealed,
        constructor: signature(&names, instance.init_method.0),
        members,
    }
}

fn member(names: &Names<'_>, t: &Trait, is_static: bool) -> Member {
    let (_, name) = names.qname(t.name.0);
    let mut member = Member {
        name,
        kind: MemberKind::Var,
        is_static,
        visibility: names.visibility(names.member_namespace(t.name.0)),
        is_override: t.is_override,
        type_name: None,
        value: None,
        signature: None,
    };
    match &t.kind {
        TraitKind::Slot {
            type_name, value, ..
        }
        | TraitKind::Const {
            type_name, value, ..
        } => {
            if matches!(t.kind, TraitKind::Const { .. }) {
                member.kind = MemberKind::Const;
            }
            member.type_name = Some(names.multiname(type_name.0));
            member.value = value.as_ref().map(|v| names.value(v));
        }
        TraitKind::Method { method, .. } => {
            member.kind = MemberKind::Method;
            member.signature = Some(signature(names, method.0));
        }
        TraitKind::Getter { method, .. } => {
            member.kind = MemberKind::Getter;
            member.signature = Some(signature(names, method.0));
        }
        TraitKind::Setter { method, .. } => {
            member.kind = MemberKind::Setter;
            member.signature = Some(signature(names, method.0));
        }
        TraitKind::Function { function, .. } => {
            member.kind = MemberKind::Function;
            member.signature = Some(signature(names, function.0));
        }
        TraitKind::Class { .. } => member.kind = MemberKind::Class,
    }
    member
}

pub(crate) fn signature(names: &Names<'_>, method: u32) -> Signature {
    let abc = names.abc;
    let Some(info) = abc.methods.get(method as usize) else {
        return Signature {
            method,
            params: Vec::new(),
            rest: false,
            returns: "*".into(),
            code_bytes: 0,
        };
    };
    let code_bytes = info
        .body
        .and_then(|b| abc.method_bodies.get(b.0 as usize))
        .map_or(0, |b| b.code.len());
    Signature {
        method,
        params: info
            .params
            .iter()
            .map(|p| Param {
                name: p
                    .name
                    .and_then(|n| names.string(n.0))
                    .filter(|n| !n.is_empty()),
                type_name: names.multiname(p.kind.0),
                default: p.default_value.as_ref().map(|v| names.value(v)),
            })
            .collect(),
        rest: info.flags.contains(MethodFlags::NEED_REST),
        returns: names.multiname(info.return_type.0),
        code_bytes,
    }
}
