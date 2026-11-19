//! A small movie shaped like an old Flash 3D game's scripts, for tests here and on every surface:
//! a main class in its own package whose code compares a host with the server's name and traces
//! the server's address, and a second block whose helper names the server too. It is written by
//! the swf crate's own writers, so the reader is exercised on well-formed data.

use swf::avm2::types::{
    AbcFile, Class, ConstantPool, DefaultValue, Index, Instance, Method, MethodBody, MethodFlags,
    MethodParam, Multiname, Namespace, Op, Script, Trait, TraitKind,
};
use swf::{Compression, DoAbc2, DoAbc2Flag, Header, Rectangle, SwfStr, Tag, Twips};

/// The address the main class traces, held as a constant.
pub const SERVER_URL: &str = "ws://old.example:5190";
/// The server's name, compared with by the main class and held by the helper.
pub const HOST: &str = "old.example";
/// The main class.
pub const MAIN_CLASS: &str = "alternativa.tanks.Main";

/// The movie, compressed as asked.
pub fn movie(compression: Compression) -> Result<Vec<u8>, swf::error::Error> {
    let main = abc_bytes(main_block()?)?;
    let helper = abc_bytes(helper_block()?)?;
    let header = Header {
        compression,
        version: 13,
        stage_size: Rectangle {
            x_min: Twips::ZERO,
            x_max: Twips::from_pixels(640.0),
            y_min: Twips::ZERO,
            y_max: Twips::from_pixels(480.0),
        },
        frame_rate: swf::Fixed8::from_f32(30.0),
        num_frames: 1,
    };
    let tags = [
        do_abc2("frame1", &main),
        do_abc2("helper", &helper),
        Tag::ShowFrame,
    ];
    let mut out = Vec::new();
    swf::write_swf(&header, &tags, &mut out)?;
    Ok(out)
}

fn do_abc2<'a>(name: &'a str, data: &'a [u8]) -> Tag<'a> {
    Tag::DoAbc2(DoAbc2 {
        flags: DoAbc2Flag::LAZY_INITIALIZE,
        name: SwfStr::from_utf8_str(name),
        data,
    })
}

fn abc_bytes(file: AbcFile) -> Result<Vec<u8>, swf::error::Error> {
    let mut out = Vec::new();
    swf::avm2::write::Writer::new(&mut out).write(file)?;
    Ok(out)
}

fn code(ops: &[Op]) -> Result<Vec<u8>, swf::error::Error> {
    let mut out = Vec::new();
    let mut writer = swf::avm2::write::Writer::new(&mut out);
    for op in ops {
        writer.write_op(op)?;
    }
    Ok(out)
}

fn i<T>(index: u32) -> Index<T> {
    Index::new(index)
}

fn body(method: u32, code: Vec<u8>) -> MethodBody {
    MethodBody {
        method: i(method),
        max_stack: 4,
        num_locals: 3,
        init_scope_depth: 0,
        max_scope_depth: 1,
        code,
        exceptions: vec![],
        traits: vec![],
    }
}

fn method(name: u32, params: Vec<MethodParam>, returns: u32, flags: MethodFlags) -> Method {
    Method {
        name: i(name),
        params,
        return_type: i(returns),
        flags,
        body: None,
    }
}

fn plain(name: u32, kind: TraitKind) -> Trait {
    Trait {
        name: i(name),
        kind,
        metadata: vec![],
        is_final: false,
        is_override: false,
    }
}

fn strings(all: &[&str]) -> Vec<Vec<u8>> {
    all.iter().map(|s| s.as_bytes().to_vec()).collect()
}

fn qname(namespace: u32, name: u32) -> Multiname {
    Multiname::QName {
        namespace: i(namespace),
        name: i(name),
    }
}

/// `alternativa.tanks.Main extends flash.display.Sprite`: a constant with the server's address,
/// a private counter, and `connect(host:String, port:int = 5190):void`.
fn main_block() -> Result<AbcFile, swf::error::Error> {
    let pool = ConstantPool {
        ints: vec![5190, 3],
        uints: vec![],
        doubles: vec![],
        // Pool index = position + 1.
        strings: strings(&[
            "",                  // 1
            "alternativa.tanks", // 2
            "Main",              // 3
            "flash.display",     // 4
            "Sprite",            // 5
            "SERVER",            // 6
            SERVER_URL,          // 7
            "String",            // 8
            "lives",             // 9
            "int",               // 10
            "connect",           // 11
            "host",              // 12
            "port",              // 13
            "void",              // 14
            HOST,                // 15
            "trace",             // 16
        ]),
        namespaces: vec![
            Namespace::Package(i(2)), // 1: alternativa.tanks
            Namespace::Package(i(1)), // 2: public
            Namespace::Package(i(4)), // 3: flash.display
            Namespace::Private(i(1)), // 4: private
        ],
        namespace_sets: vec![],
        multinames: vec![
            qname(1, 3),  // 1: alternativa.tanks.Main
            qname(3, 5),  // 2: flash.display.Sprite
            qname(2, 6),  // 3: SERVER
            qname(2, 8),  // 4: String
            qname(4, 9),  // 5: lives (private)
            qname(2, 10), // 6: int
            qname(2, 11), // 7: connect
            qname(2, 14), // 8: void
            qname(2, 16), // 9: trace
        ],
    };
    // connect: if (host === "old.example") trace(SERVER_URL);
    let then = code(&[
        Op::FindPropStrict { index: i(9) },
        Op::PushString { value: i(7) },
        Op::CallPropVoid {
            index: i(9),
            num_args: 1,
        },
    ])?;
    let mut connect = code(&[
        Op::GetLocal { index: 0 },
        Op::PushScope,
        Op::GetLocal { index: 1 },
        Op::PushString { value: i(15) },
        Op::IfStrictNe {
            offset: then.len() as i32,
        },
    ])?;
    connect.extend_from_slice(&then);
    connect.extend(code(&[Op::ReturnVoid])?);
    let constructor = code(&[
        Op::GetLocal { index: 0 },
        Op::PushScope,
        Op::GetLocal { index: 0 },
        Op::ConstructSuper { num_args: 0 },
        Op::ReturnVoid,
    ])?;
    let init = code(&[Op::GetLocal { index: 0 }, Op::PushScope, Op::ReturnVoid])?;
    let params = vec![
        MethodParam {
            name: Some(i(12)),
            kind: i(4),
            default_value: None,
        },
        MethodParam {
            name: Some(i(13)),
            kind: i(6),
            default_value: Some(DefaultValue::Int(i(1))),
        },
    ];
    Ok(AbcFile {
        major_version: 46,
        minor_version: 16,
        constant_pool: pool,
        methods: vec![
            method(0, vec![], 0, MethodFlags::empty()), // 0: constructor
            method(
                11,
                params,
                8,
                MethodFlags::HAS_OPTIONAL | MethodFlags::HAS_PARAM_NAMES,
            ), // 1: connect
            method(0, vec![], 0, MethodFlags::empty()), // 2: class init
            method(0, vec![], 0, MethodFlags::empty()), // 3: script init
        ],
        metadata: vec![],
        instances: vec![Instance {
            name: i(1),
            super_name: i(2),
            is_sealed: true,
            is_final: false,
            is_interface: false,
            protected_namespace: None,
            interfaces: vec![],
            init_method: i(0),
            traits: vec![
                plain(
                    5,
                    TraitKind::Slot {
                        slot_id: 1,
                        type_name: i(6),
                        value: Some(DefaultValue::Int(i(2))),
                    },
                ),
                plain(
                    7,
                    TraitKind::Method {
                        disp_id: 0,
                        method: i(1),
                    },
                ),
            ],
        }],
        classes: vec![Class {
            init_method: i(2),
            traits: vec![plain(
                3,
                TraitKind::Const {
                    slot_id: 1,
                    type_name: i(4),
                    value: Some(DefaultValue::String(i(7))),
                },
            )],
        }],
        scripts: vec![Script {
            init_method: i(3),
            traits: vec![plain(
                1,
                TraitKind::Class {
                    slot_id: 1,
                    class: i(0),
                },
            )],
        }],
        method_bodies: vec![
            body(0, constructor),
            body(1, connect),
            body(2, init.clone()),
            body(3, init),
        ],
    })
}

/// `Helper`, a final class at the top level, with `static const HOST:String = "old.example"`.
fn helper_block() -> Result<AbcFile, swf::error::Error> {
    let pool = ConstantPool {
        ints: vec![],
        uints: vec![],
        doubles: vec![],
        strings: strings(&["", "Helper", "HOST", "String", HOST]),
        namespaces: vec![Namespace::Package(i(1))],
        namespace_sets: vec![],
        multinames: vec![qname(1, 2), qname(1, 3), qname(1, 4)],
    };
    let init = code(&[Op::GetLocal { index: 0 }, Op::PushScope, Op::ReturnVoid])?;
    Ok(AbcFile {
        major_version: 46,
        minor_version: 16,
        constant_pool: pool,
        methods: vec![
            method(0, vec![], 0, MethodFlags::empty()),
            method(0, vec![], 0, MethodFlags::empty()),
            method(0, vec![], 0, MethodFlags::empty()),
        ],
        metadata: vec![],
        instances: vec![Instance {
            name: i(1),
            super_name: i(0),
            is_sealed: true,
            is_final: true,
            is_interface: false,
            protected_namespace: None,
            interfaces: vec![],
            init_method: i(0),
            traits: vec![],
        }],
        classes: vec![Class {
            init_method: i(1),
            traits: vec![plain(
                2,
                TraitKind::Const {
                    slot_id: 1,
                    type_name: i(3),
                    value: Some(DefaultValue::String(i(5))),
                },
            )],
        }],
        scripts: vec![Script {
            init_method: i(2),
            traits: vec![plain(
                1,
                TraitKind::Class {
                    slot_id: 1,
                    class: i(0),
                },
            )],
        }],
        method_bodies: vec![body(0, init.clone()), body(1, init.clone()), body(2, init)],
    })
}
