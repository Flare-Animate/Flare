//! A method body as a list of instructions, its operands shown by name rather than by index.

use crate::names::{Names, number, quoted};
use serde::Serialize;
use swf::avm2::types::Op;
use swf::extensions::ReadSwfExt;

/// One instruction: where it is in the body and what it does.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Instruction {
    /// Its byte offset in the body; branches name their targets as `L<offset>`.
    pub offset: u32,
    pub text: String,
}

/// The body of method `method` as instructions. `None` when it has no body (an interface's
/// method or a native one). Bytes that are not an instruction end the listing with a line saying
/// so rather than failing: what came before is still worth reading.
pub(crate) fn listing(names: &Names<'_>, method: u32) -> Option<Vec<Instruction>> {
    let abc = names.abc;
    let body = abc
        .methods
        .get(method as usize)?
        .body
        .and_then(|b| abc.method_bodies.get(b.0 as usize))?;
    let code = body.code.as_slice();
    let mut reader = swf::avm2::read::Reader::new(code);
    let mut out = Vec::new();
    loop {
        let at = reader.pos(code);
        if at >= code.len() {
            break;
        }
        let offset = at as u32;
        match reader.read_op() {
            Ok(op) => {
                let next = reader.pos(code) as i64;
                out.push(Instruction {
                    offset,
                    text: describe(names, &op, i64::from(offset), next),
                });
            }
            Err(error) => {
                out.push(Instruction {
                    offset,
                    text: format!("; the rest cannot be read: {error}"),
                });
                break;
            }
        }
    }
    Some(out)
}

/// The instruction's name, as the AVM2 overview spells it (`callpropvoid`).
fn mnemonic(op: &Op) -> String {
    let debug = format!("{op:?}");
    debug
        .split(|c: char| !c.is_alphanumeric())
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

fn label(target: i64) -> String {
    format!("L{target}")
}

fn describe(names: &Names<'_>, op: &Op, at: i64, next: i64) -> String {
    let pool = &names.abc.constant_pool;
    let get = |values: usize, index: u32| index.checked_sub(1).filter(|i| (*i as usize) < values);
    let operand = match op {
        Op::PushString { value } => quoted(&names.string(value.0).unwrap_or_default()),
        Op::PushInt { value } => {
            get(pool.ints.len(), value.0).map_or("?".into(), |i| pool.ints[i as usize].to_string())
        }
        Op::PushUint { value } => get(pool.uints.len(), value.0)
            .map_or("?".into(), |i| pool.uints[i as usize].to_string()),
        Op::PushDouble { value } => get(pool.doubles.len(), value.0)
            .map_or("?".into(), |i| number(pool.doubles[i as usize])),
        // pushbyte's operand is signed.
        Op::PushByte { value } => (*value as i8).to_string(),
        Op::PushShort { value } => value.to_string(),
        Op::PushNamespace { value } => quoted(&names.namespace_name(value.0)),
        Op::GetLocal { index }
        | Op::SetLocal { index }
        | Op::Kill { index }
        | Op::IncLocal { index }
        | Op::IncLocalI { index }
        | Op::DecLocal { index }
        | Op::DecLocalI { index }
        | Op::GetSlot { index }
        | Op::SetSlot { index }
        | Op::GetGlobalSlot { index }
        | Op::SetGlobalSlot { index }
        | Op::GetOuterScope { index } => index.to_string(),
        Op::GetScopeObject { index } => index.to_string(),
        Op::AsType { type_name: index }
        | Op::Coerce { index }
        | Op::DeleteProperty { index }
        | Op::FindDef { index }
        | Op::FindProperty { index }
        | Op::FindPropStrict { index }
        | Op::GetDescendants { index }
        | Op::GetLex { index }
        | Op::GetProperty { index }
        | Op::GetSuper { index }
        | Op::InitProperty { index }
        | Op::IsType { index }
        | Op::SetProperty { index }
        | Op::SetSuper { index } => names.multiname(index.0),
        Op::CallProperty { index, num_args }
        | Op::CallPropLex { index, num_args }
        | Op::CallPropVoid { index, num_args }
        | Op::CallSuper { index, num_args }
        | Op::CallSuperVoid { index, num_args }
        | Op::ConstructProp { index, num_args } => {
            format!("{}, {num_args}", names.multiname(index.0))
        }
        Op::CallMethod { index, num_args } => format!("method {index}, {num_args}"),
        Op::CallStatic { index, num_args } => format!("method {}, {num_args}", index.0),
        Op::Call { num_args }
        | Op::Construct { num_args }
        | Op::ConstructSuper { num_args }
        | Op::NewArray { num_args }
        | Op::NewObject { num_args } => num_args.to_string(),
        Op::ApplyType { num_types } => num_types.to_string(),
        Op::IfEq { offset }
        | Op::IfFalse { offset }
        | Op::IfGe { offset }
        | Op::IfGt { offset }
        | Op::IfLe { offset }
        | Op::IfLt { offset }
        | Op::IfNe { offset }
        | Op::IfNge { offset }
        | Op::IfNgt { offset }
        | Op::IfNle { offset }
        | Op::IfNlt { offset }
        | Op::IfStrictEq { offset }
        | Op::IfStrictNe { offset }
        | Op::IfTrue { offset }
        | Op::Jump { offset } => label(next + i64::from(*offset)),
        // A switch's offsets count from the switch itself, not from the next instruction.
        Op::LookupSwitch(switch) => {
            let cases: Vec<String> = switch
                .case_offsets
                .iter()
                .map(|o| label(at + i64::from(*o)))
                .collect();
            format!(
                "default {}, [{}]",
                label(at + i64::from(switch.default_offset)),
                cases.join(", ")
            )
        }
        Op::NewClass { index } => names
            .abc
            .instances
            .get(index.0 as usize)
            .map(|i| names.multiname(i.name.0))
            .unwrap_or_else(|| "?".into()),
        Op::NewFunction { index } => format!("method {}", index.0),
        Op::NewCatch { index } => format!("exception {}", index.0),
        Op::HasNext2 {
            object_register,
            index_register,
        } => format!("{object_register}, {index_register}"),
        Op::Debug {
            register_name,
            register,
            ..
        } => format!(
            "{}, {register}",
            quoted(&names.string(register_name.0).unwrap_or_default())
        ),
        Op::DebugFile { file_name } => quoted(&names.string(file_name.0).unwrap_or_default()),
        Op::Dxns { index } => quoted(&names.string(index.0).unwrap_or_default()),
        Op::DebugLine { line_num } | Op::BkptLine { line_num } => line_num.to_string(),
        _ => String::new(),
    };
    let name = mnemonic(op);
    if operand.is_empty() {
        name
    } else {
        format!("{name} {operand}")
    }
}
