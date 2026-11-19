//! One class written out as an ActionScript declaration: its package, its members with their
//! types and values, and, when asked, each method body listed as instructions in comments.
//! The declarations are what the bytecode records; the bodies are not rebuilt into statements.

use crate::ScriptError;
use crate::disasm::listing;
use crate::movie::Movie;
use crate::names::Names;
use crate::outline::{ClassOutline, Member, MemberKind, Signature, class_outline, full_name};
use serde::Serialize;
use std::fmt::Write;

/// The longest text one class is written to; past it the text stops with a note. A class with
/// very large methods, listed whole, could otherwise grow without bound.
const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// What a class is written out with.
#[derive(Debug, Clone, Default)]
pub struct SourceOptions {
    /// List each method body as instructions; otherwise a body says only how large it is.
    pub code: bool,
    /// Write only this member (and the constructor when it is the class's own name).
    pub member: Option<String>,
}

/// A class written out.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClassSource {
    pub class: ClassOutline,
    pub text: String,
    /// The text reached its limit and stops early.
    pub truncated: bool,
}

/// Writes out the class called `name`: its full name (`alternativa.tanks.Main`), or its short
/// name when only one class has it. The first block that defines it is used.
pub fn class_source(
    movie: &Movie,
    name: &str,
    options: &SourceOptions,
) -> Result<ClassSource, ScriptError> {
    let mut by_short = Vec::new();
    for block in 0..movie.block_count() {
        let Ok(abc) = movie.abc(block) else { continue };
        let names = Names::new(&abc);
        for (index, instance) in abc.instances.iter().enumerate() {
            let (package, short) = names.qname(instance.name.0);
            if full_name(&package, &short) == name {
                return write_class(&abc, block, index, options);
            }
            if short == name {
                by_short.push((block, index));
            }
        }
    }
    match by_short.as_slice() {
        [(block, index)] => {
            let abc = movie.abc(*block)?;
            write_class(&abc, *block, *index, options)
        }
        _ => Err(ScriptError::NoClass(name.to_owned())),
    }
}

fn write_class(
    abc: &swf::avm2::types::AbcFile,
    block: usize,
    index: usize,
    options: &SourceOptions,
) -> Result<ClassSource, ScriptError> {
    let names = Names::new(abc);
    let class = class_outline(abc, block, index);
    let wanted = |m: &Member| options.member.as_deref().is_none_or(|w| m.name == w);
    let with_constructor = options.member.as_deref().is_none_or(|w| w == class.name);
    if !with_constructor && !class.members.iter().any(wanted) {
        return Err(ScriptError::NoMember {
            class: class.full_name.clone(),
            member: options.member.clone().unwrap_or_default(),
        });
    }
    let mut out = Text::default();
    out.line(&format!(
        "// Read from script block {block} of the movie. Declarations are as the bytecode \
         records them; method bodies are compiled code{}.",
        if options.code {
            ", listed as instructions, so this does not compile as it is"
        } else {
            ", shown by their size"
        }
    ));
    if class.package.is_empty() {
        out.line("package {");
    } else {
        out.line(&format!("package {} {{", class.package));
    }
    let instance_ns = names.member_namespace(abc.instances[index].name.0);
    let mut head = format!("    {}", names.visibility(instance_ns));
    if class.is_final {
        head.push_str(" final");
    }
    if class.dynamic && !class.interface {
        head.push_str(" dynamic");
    }
    if class.interface {
        let _ = write!(head, " interface {}", class.name);
        if !class.implements.is_empty() {
            let _ = write!(head, " extends {}", class.implements.join(", "));
        }
    } else {
        let _ = write!(head, " class {}", class.name);
        if let Some(base) = &class.extends {
            let _ = write!(head, " extends {base}");
        }
        if !class.implements.is_empty() {
            let _ = write!(head, " implements {}", class.implements.join(", "));
        }
    }
    out.line(&format!("{head} {{"));
    for member in class.members.iter().filter(|m| wanted(m)) {
        if matches!(member.kind, MemberKind::Var | MemberKind::Const) {
            out.line(&format!("        {};", variable(member)));
        }
    }
    if with_constructor && !class.interface {
        out.line("");
        let head = format!(
            "        public function {}({})",
            class.name,
            params(&class.constructor)
        );
        body(&mut out, &names, &head, &class.constructor, options.code);
    }
    for member in class.members.iter().filter(|m| wanted(m)) {
        let Some(signature) = &member.signature else {
            continue;
        };
        out.line("");
        let head = format!(
            "        {}",
            function_head(member, signature, class.interface)
        );
        if class.interface {
            out.line(&format!("{head};"));
        } else {
            body(&mut out, &names, &head, signature, options.code);
        }
    }
    out.line("    }");
    out.line("}");
    Ok(ClassSource {
        class,
        truncated: out.truncated,
        text: out.text,
    })
}

/// Text that stops growing at its limit.
#[derive(Default)]
struct Text {
    text: String,
    truncated: bool,
}

impl Text {
    fn line(&mut self, line: &str) {
        if self.truncated {
            return;
        }
        if self.text.len() + line.len() + 1 > MAX_SOURCE_BYTES {
            self.truncated = true;
            self.text
                .push_str("// ... the rest is left out: the text reached its limit\n");
            return;
        }
        self.text.push_str(line);
        self.text.push('\n');
    }
}

fn qualifiers(member: &Member) -> String {
    let mut out = String::new();
    if member.is_override {
        out.push_str("override ");
    }
    out.push_str(&member.visibility);
    if member.is_static {
        out.push_str(" static");
    }
    out
}

fn variable(member: &Member) -> String {
    let keyword = if member.kind == MemberKind::Const {
        "const"
    } else {
        "var"
    };
    let mut out = format!(
        "{} {keyword} {}:{}",
        qualifiers(member),
        member.name,
        member.type_name.as_deref().unwrap_or("*")
    );
    if let Some(value) = &member.value {
        let _ = write!(out, " = {value}");
    }
    out
}

fn params(signature: &Signature) -> String {
    let mut list: Vec<String> = signature
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let name = p.name.clone().unwrap_or_else(|| format!("param{}", i + 1));
            match &p.default {
                Some(value) => format!("{name}:{} = {value}", p.type_name),
                None => format!("{name}:{}", p.type_name),
            }
        })
        .collect();
    if signature.rest {
        list.push("...rest".into());
    }
    list.join(", ")
}

fn function_head(member: &Member, signature: &Signature, interface: bool) -> String {
    let accessor = match member.kind {
        MemberKind::Getter => "get ",
        MemberKind::Setter => "set ",
        _ => "",
    };
    let qualifiers = if interface {
        String::new()
    } else {
        format!("{} ", qualifiers(member))
    };
    format!(
        "{qualifiers}function {accessor}{}({}):{}",
        member.name,
        params(signature),
        signature.returns
    )
}

fn body(out: &mut Text, names: &Names<'_>, head: &str, signature: &Signature, code: bool) {
    if signature.code_bytes == 0 {
        out.line(&format!("{head}; // no body: native"));
        return;
    }
    if !code {
        out.line(&format!(
            "{head} {{ /* {} bytes of bytecode, method {} */ }}",
            signature.code_bytes, signature.method
        ));
        return;
    }
    out.line(&format!("{head} {{"));
    for instruction in listing(names, signature.method).unwrap_or_default() {
        out.line(&format!(
            "            // L{:<6} {}",
            instruction.offset, instruction.text
        ));
    }
    out.line("        }");
}
