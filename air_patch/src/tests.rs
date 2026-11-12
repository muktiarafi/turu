use super::*;
use swf::avm2::types::{Class, Instance, LookupSwitch, Method, MethodBody, MethodFlags};

fn encode(ops: &[Op]) -> Vec<u8> {
    let mut code = Vec::new();
    let mut writer = Writer::new(&mut code);
    for op in ops {
        writer.write_op(op).unwrap();
    }
    code
}

/// An ABC block with one method whose body is `ops`. `names` are QNames as (package, name):
/// multinames 1, 2, ... Classes are declared for each name in `classes` (by multiname index).
fn abc_with(
    names: &[(&str, &str)],
    classes: &[u32],
    ops: &[Op],
    exceptions: Vec<Exception>,
) -> AbcFile {
    let mut strings: Vec<Vec<u8>> = Vec::new();
    let mut namespaces = Vec::new();
    let string_index = |s: &str, strings: &mut Vec<Vec<u8>>| match strings
        .iter()
        .position(|x| x == s.as_bytes())
    {
        Some(i) => i as u32 + 1,
        None => {
            strings.push(s.as_bytes().to_vec());
            strings.len() as u32
        }
    };
    let mut multinames = Vec::new();
    for (package, name) in names {
        let package = string_index(package, &mut strings);
        let ns = Namespace::Package(Index::new(package));
        let ns = match namespaces.iter().position(|n| *n == ns) {
            Some(i) => i as u32 + 1,
            None => {
                namespaces.push(ns);
                namespaces.len() as u32
            }
        };
        let name = string_index(name, &mut strings);
        multinames.push(Multiname::QName {
            namespace: Index::new(ns),
            name: Index::new(name),
        });
    }
    let instances = classes
        .iter()
        .map(|&name| Instance {
            name: Index::new(name),
            super_name: Index::new(0),
            is_sealed: false,
            is_final: false,
            is_interface: false,
            protected_namespace: None,
            interfaces: vec![],
            init_method: Index::new(0),
            traits: vec![],
        })
        .collect();
    let classes = classes
        .iter()
        .map(|_| Class {
            init_method: Index::new(0),
            traits: vec![],
        })
        .collect();
    AbcFile {
        major_version: 46,
        minor_version: 16,
        constant_pool: ConstantPool {
            ints: vec![],
            uints: vec![],
            doubles: vec![],
            strings,
            namespaces,
            namespace_sets: vec![],
            multinames,
        },
        methods: vec![Method {
            name: Index::new(0),
            params: vec![],
            return_type: Index::new(0),
            flags: MethodFlags::empty(),
            body: Some(Index::new(0)),
        }],
        metadata: vec![],
        instances,
        classes,
        scripts: vec![],
        method_bodies: vec![MethodBody {
            method: Index::new(0),
            max_stack: 3,
            num_locals: 2,
            init_scope_depth: 0,
            max_scope_depth: 1,
            code: encode(ops),
            exceptions,
            traits: vec![],
        }],
    }
}

fn ops_of(abc: &AbcFile) -> Vec<Op> {
    decode(&abc.method_bodies[0].code)
        .unwrap()
        .into_iter()
        .map(|d| d.op)
        .collect()
}

/// The instruction a branch at `index` lands on.
fn branch_target(code: &[u8], index: usize) -> usize {
    let ops = decode(code).unwrap();
    let mut op = ops[index].op.clone();
    let offset = *branch_offset(&mut op).unwrap() as i64;
    let target = (ops[index].pos + ops[index].len) as i64 + offset;
    ops.iter()
        .position(|d| d.pos as i64 == target)
        .expect("lands on an instruction")
}

/// Loader (1), AirLoader (2), SwfLoaderPatch (3), with the last two declared as classes.
const LOADERS: [(&str, &str); 3] = [
    ("flash.display", "Loader"),
    ("types", "AirLoader"),
    ("types", "SwfLoaderPatch"),
];

#[test]
fn makes_new_loader_an_air_loader() {
    let ops = [
        Op::GetLocal { index: 0 },
        Op::FindPropStrict {
            index: Index::new(1),
        },
        Op::ConstructProp {
            index: Index::new(1),
            num_args: 0,
        },
        Op::InitProperty {
            index: Index::new(1),
        },
        // Not a construction: a type check stays a Loader's.
        Op::GetLocal { index: 1 },
        Op::GetLex {
            index: Index::new(1),
        },
        Op::IsTypeLate,
        Op::ReturnValue,
    ];
    let mut abc = abc_with(&LOADERS, &[2, 3], &ops, vec![]);
    let mut report = Report::default();
    assert!(patch_abc(&mut abc, Target::Game, &mut report).unwrap());
    assert!(report.has_air_loader);
    assert_eq!(report.swapped.len(), 1);
    assert_eq!(report.problem(), None);
    let mut expected = ops.to_vec();
    expected[1] = Op::FindPropStrict {
        index: Index::new(2),
    };
    expected[2] = Op::ConstructProp {
        index: Index::new(2),
        num_args: 0,
    };
    assert_eq!(ops_of(&abc), expected);
}

/// `AirLoader`'s fetch becomes the host's `TuruURLLoader`; a `URLLoader` elsewhere stays.
#[test]
fn air_loader_fetches_through_the_host() {
    let ops = [
        Op::FindPropStrict {
            index: Index::new(4),
        },
        Op::ConstructProp {
            index: Index::new(4),
            num_args: 0,
        },
        Op::ReturnValue,
    ];
    let names = [
        ("flash.display", "Loader"),
        ("types", "SwfLoaderPatch"),
        ("types", "AirLoader"),
        ("flash.net", "URLLoader"),
    ];
    // The method is the last class's (`abc_with`): AirLoader's.
    let mut abc = abc_with(&names, &[2, 3], &ops, vec![]);
    let mut report = Report::default();
    assert!(patch_abc(&mut abc, Target::Game, &mut report).unwrap());
    assert_eq!(report.problem(), None);
    assert_eq!(report.swapped.len(), 1);
    assert_eq!(report.swapped[0].name, "URLLoader");
    let host = abc
        .constant_pool
        .multinames
        .iter()
        .position(|m| local_name(&abc.constant_pool, m) == Some(HOST_URL_LOADER))
        .expect("TuruURLLoader added") as u32
        + 1;
    assert_eq!(
        ops_of(&abc)[..2],
        [
            Op::FindPropStrict {
                index: Index::new(host),
            },
            Op::ConstructProp {
                index: Index::new(host),
                num_args: 0,
            },
        ]
    );

    // In another class (here SwfLoaderPatch's method), or for skua, it stays a URLLoader.
    let mut abc = abc_with(&names, &[3, 2], &ops, vec![]);
    let mut report = Report::default();
    assert!(!patch_abc(&mut abc, Target::Game, &mut report).unwrap());
    let mut abc = abc_with(&names, &[2, 3], &ops, vec![]);
    let mut report = Report::default();
    assert!(!patch_abc(&mut abc, Target::Skua, &mut report).unwrap());
}

#[test]
fn no_air_loader_no_swap() {
    let ops = [
        Op::FindPropStrict {
            index: Index::new(1),
        },
        Op::ConstructProp {
            index: Index::new(1),
            num_args: 0,
        },
        Op::ReturnValue,
    ];
    let mut abc = abc_with(&LOADERS, &[], &ops, vec![]);
    let mut report = Report::default();
    assert!(!patch_abc(&mut abc, Target::Game, &mut report).unwrap());
    assert!(report.problem().is_some());

    // Another package's Loader isn't flash.display's.
    let names = [
        ("game", "Loader"),
        ("types", "AirLoader"),
        ("types", "SwfLoaderPatch"),
    ];
    let mut abc = abc_with(&names, &[2, 3], &ops, vec![]);
    assert!(!patch_abc(&mut abc, Target::Game, &mut report).unwrap());
}

#[test]
fn drops_allow_domain_and_keeps_branches_on_their_instructions() {
    let ops = [
        Op::GetLocal { index: 1 },  // 0
        Op::IfFalse { offset: 10 }, // 1..5, to ReturnVoid
        Op::GetLex {
            index: Index::new(1),
        }, // 5..7, Security
        Op::PushNull,               // 7
        Op::CallPropVoid {
            index: Index::new(2),
            num_args: 1,
        }, // 8..11, allowDomain
        Op::Jump { offset: -15 },   // 11..15, to GetLocal 1
        Op::ReturnVoid,             // 15
    ];
    let names = [("flash.system", "Security"), ("", "allowDomain")];
    let code = encode(&ops);
    assert_eq!(branch_target(&code, 1), 6);
    assert_eq!(branch_target(&code, 5), 0);

    let mut abc = abc_with(&names, &[], &ops, vec![]);
    let mut report = Report::default();
    assert!(patch_abc(&mut abc, Target::Game, &mut report).unwrap());
    assert_eq!(report.dropped.len(), 1);
    let patched = ops_of(&abc);
    assert_eq!(
        patched,
        [
            Op::GetLocal { index: 1 },
            Op::IfFalse { offset: 0 }, // checked below
            Op::GetLex {
                index: Index::new(1)
            },
            Op::PushNull,
            Op::Pop,
            Op::Pop,
            Op::Jump { offset: 0 },
            Op::ReturnVoid,
        ]
        .iter()
        .zip(&patched)
        .map(
            |(want, got)| if branch_offset(&mut want.clone()).is_some() {
                got.clone()
            } else {
                want.clone()
            }
        )
        .collect::<Vec<_>>()
    );
    let code = &abc.method_bodies[0].code;
    assert_eq!(branch_target(code, 1), 7);
    assert_eq!(branch_target(code, 6), 0);
    assert_eq!(abc.method_bodies[0].max_stack, 4);
}

#[test]
fn a_used_result_becomes_undefined() {
    let ops = [
        Op::GetLex {
            index: Index::new(1),
        },
        Op::PushNull,
        Op::CallProperty {
            index: Index::new(2),
            num_args: 1,
        },
        Op::ReturnValue,
    ];
    let mut abc = abc_with(
        &[("flash.system", "Security"), ("", "allowInsecureDomain")],
        &[],
        &ops,
        vec![],
    );
    let mut report = Report::default();
    assert!(patch_abc(&mut abc, Target::Game, &mut report).unwrap());
    assert_eq!(
        ops_of(&abc),
        [
            Op::GetLex {
                index: Index::new(1)
            },
            Op::PushNull,
            Op::Pop,
            Op::Pop,
            Op::PushUndefined,
            Op::ReturnValue
        ]
    );
}

#[test]
fn moves_exception_ranges_and_switch_targets() {
    let ops = vec![
        Op::GetLex {
            index: Index::new(1),
        }, // 0..2
        Op::PushNull, // 2
        Op::CallPropVoid {
            index: Index::new(2),
            num_args: 1,
        }, // 3..6
        Op::GetLocal { index: 1 }, // 6
        Op::LookupSwitch(Box::new(LookupSwitch {
            // 7..15
            default_offset: 8,            // -> 15
            case_offsets: vec![9].into(), // -> 16
        })),
        Op::ReturnVoid, // 15
        Op::ReturnVoid, // 16
    ];
    let exception = Exception {
        from_offset: 0,
        to_offset: 6,
        target_offset: 16,
        variable_name: Index::new(0),
        type_name: Index::new(0),
    };
    let names = [("flash.system", "Security"), ("", "allowDomain")];
    let mut abc = abc_with(&names, &[], &ops, vec![exception]);
    let mut report = Report::default();
    assert!(patch_abc(&mut abc, Target::Game, &mut report).unwrap());

    let body = &abc.method_bodies[0];
    let decoded = decode(&body.code).unwrap();
    let at = |pos: u32| {
        decoded
            .iter()
            .position(|d| d.pos == pos as usize)
            .expect("an instruction start")
    };
    let get_local_1 = decoded
        .iter()
        .position(|d| d.op == Op::GetLocal { index: 1 })
        .unwrap();
    let switch = get_local_1 + 1;
    let Op::LookupSwitch(table) = &decoded[switch].op else {
        panic!("no switch")
    };
    let base = decoded[switch].pos as i64;
    assert_eq!(at((base + table.default_offset as i64) as u32), switch + 1);
    assert_eq!(at((base + table.case_offsets[0] as i64) as u32), switch + 2);
    let exception = &body.exceptions[0];
    assert_eq!(at(exception.from_offset), 0);
    assert_eq!(at(exception.to_offset), get_local_1);
    assert_eq!(at(exception.target_offset), switch + 2);
}

/// The real client: `TURU_GAME_SWF=/path/to/Game.swf cargo test -p turu_air_patch -- --nocapture`.
#[test]
fn patches_the_real_game() {
    let Ok(path) = std::env::var("TURU_GAME_SWF") else {
        eprintln!("TURU_GAME_SWF not set, skipped");
        return;
    };
    let input = std::fs::read(&path).unwrap();
    let (output, report) = patch_swf(&input, Target::Game).unwrap();
    assert_eq!(report.problem(), None, "{report:#?}");
    assert!(!report.swapped.is_empty());

    // Patching it again finds nothing left to change, and every body decodes.
    let (_, again) = patch_swf(&output, Target::Game).unwrap();
    assert_eq!(again.swapped, vec![]);
    assert_eq!(again.dropped, vec![]);
    let buf = swf::decompress_swf(&output[..]).unwrap();
    let parsed = swf::parse_swf(&buf).unwrap();
    let mut bodies = 0;
    for tag in &parsed.tags {
        if let swf::Tag::DoAbc2(do_abc) = tag {
            let abc = Reader::new(do_abc.data).read().unwrap();
            for body in &abc.method_bodies {
                decode(&body.code).unwrap();
                bodies += 1;
            }
        }
    }
    eprintln!(
        "{} ABC blocks, {bodies} bodies; swapped {}, dropped {}; {} -> {} bytes",
        report.abc_blocks,
        report.swapped.len(),
        report.dropped.len(),
        input.len(),
        output.len()
    );
}

#[test]
fn skua_routes_external_interface_and_loaders_through_the_host() {
    let ops = [
        Op::GetLex {
            index: Index::new(1),
        }, // ExternalInterface
        Op::PushNull,
        Op::PushNull,
        Op::CallPropVoid {
            index: Index::new(2),
            num_args: 2,
        }, // addCallback
        Op::FindPropStrict {
            index: Index::new(3),
        },
        Op::ConstructProp {
            index: Index::new(3),
            num_args: 0,
        }, // new Loader()
        Op::ReturnValue,
    ];
    let names = [
        ("flash.external", "ExternalInterface"),
        ("", "addCallback"),
        ("flash.display", "Loader"),
    ];
    let mut abc = abc_with(&names, &[], &ops, vec![]);
    let mut report = Report {
        target: Target::Skua,
        ..Report::default()
    };
    assert!(patch_abc(&mut abc, Target::Skua, &mut report).unwrap());
    let swapped: Vec<_> = report.swapped.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(swapped, ["ExternalInterface", "Loader"]);
    assert_eq!(report.problem(), None);

    let pool = &abc.constant_pool;
    let name_of = |op: &Op| match op {
        Op::GetLex { index } | Op::FindPropStrict { index } | Op::ConstructProp { index, .. } => {
            let m = &pool.multinames[index.0 as usize - 1];
            assert!(in_package(pool, m, ""), "{m:?} isn't in the host's package");
            local_name(pool, m).unwrap().to_string()
        }
        other => panic!("{other:?}"),
    };
    let patched = ops_of(&abc);
    assert_eq!(name_of(&patched[0]), HOST_EXTERNAL);
    assert_eq!(name_of(&patched[4]), HOST_LOADER);
    assert_eq!(name_of(&patched[5]), HOST_LOADER);
    // The call itself is unchanged: the host's class has the same static methods.
    assert_eq!(patched[3], ops[3]);
}

#[test]
fn the_game_target_leaves_external_interface_alone() {
    let ops = [
        Op::GetLex {
            index: Index::new(1),
        },
        Op::ReturnValue,
    ];
    let mut abc = abc_with(
        &[("flash.external", "ExternalInterface")],
        &[],
        &ops,
        vec![],
    );
    let mut report = Report::default();
    assert!(!patch_abc(&mut abc, Target::Game, &mut report).unwrap());
}

/// VibeSkua's skua.swf, as bundled (bot/assets).
#[test]
fn patches_the_real_skua() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../bot/assets/skua.swf");
    let input = std::fs::read(path).unwrap();
    let (output, report) = patch_swf(&input, Target::Skua).unwrap();
    assert_eq!(report.problem(), None, "{report:#?}");
    let count = |name: &str| report.swapped.iter().filter(|s| s.name == name).count();
    // Externalizer's addCallback and call; Main's loadGame, injectScript and backgrounds.
    assert_eq!(count("ExternalInterface"), 2, "{report:#?}");
    assert_eq!(count("Loader"), 4, "{report:#?}");
    assert_eq!(report.dropped.len(), 1, "{report:#?}");
    let (_, again) = patch_swf(&output, Target::Skua).unwrap();
    assert!(!again.changed(), "{again:#?}");
}

#[test]
fn shared_stage_finds_each_clients_game() {
    let ops = [
        Op::GetLocal { index: 0 },
        Op::GetProperty {
            index: Index::new(1),
        }, // this.stage
        Op::PushByte { value: 0 },
        Op::CallProperty {
            index: Index::new(2),
            num_args: 1,
        }, // getChildAt(0)
        Op::Pop,
        Op::GetLex {
            index: Index::new(1),
        }, // stage, through the scope
        Op::PushByte { value: 0 },
        Op::CallProperty {
            index: Index::new(2),
            num_args: 1,
        },
        Op::ReturnValue,
    ];
    let names = [("", "stage"), ("", "getChildAt")];
    let shared = Options {
        shared_stage: true,
        client: None,
    };
    // Without the option, nothing changes.
    let mut abc = abc_with(&names, &[], &ops, vec![]);
    let mut report = Report::default();
    assert!(!patch_abc(&mut abc, Target::Game, &mut report).unwrap());

    let mut report = Report::default();
    assert!(patch_abc_with(&mut abc, Target::Game, shared, &mut report).unwrap());
    assert_eq!(report.stage_roots.len(), 2);
    assert_eq!(report.stage_missed, vec![]);
    let pool = &abc.constant_pool;
    let name = |index: &Index<Multiname>| {
        local_name(pool, &pool.multinames[index.0 as usize - 1]).unwrap()
    };
    let got: Vec<String> = decode(&abc.method_bodies[0].code)
        .unwrap()
        .into_iter()
        .map(|d| match d.op {
            Op::GetLex { index } => format!("getlex {}", name(&index)),
            Op::GetProperty { index } => format!("getproperty {}", name(&index)),
            Op::FindPropStrict { index } => format!("findpropstrict {}", name(&index)),
            Op::CallProperty { index, num_args } => format!("call {} {num_args}", name(&index)),
            op => format!("{op:?}"),
        })
        .collect();
    assert_eq!(
        got,
        [
            "GetLocal { index: 0 }",
            "getlex TuruStage",
            "Swap",
            "call rootOf 1",
            "Pop",
            "findpropstrict stage",
            "getlex TuruStage",
            "Swap",
            "call rootOf 1",
            "ReturnValue",
        ]
    );
}

#[test]
fn shared_stage_leaves_a_site_a_branch_lands_in() {
    let ops = [
        Op::GetLocal { index: 0 },
        Op::GetProperty {
            index: Index::new(1),
        },
        Op::Jump { offset: 0 }, // lands on the next instruction: the `0`
        Op::PushByte { value: 0 },
        Op::CallProperty {
            index: Index::new(2),
            num_args: 1,
        },
        Op::ReturnValue,
    ];
    // The jump sits between `stage` and `0`, so the pattern isn't there; put it right after.
    let ops = [
        ops[0].clone(),
        ops[2].clone(),
        ops[1].clone(),
        ops[3].clone(),
        ops[4].clone(),
        ops[5].clone(),
    ];
    let names = [("", "stage"), ("", "getChildAt")];
    let mut abc = abc_with(&names, &[], &ops, vec![]);
    // Point the jump at the `0` (after `getproperty stage`, 2 bytes).
    let decoded = decode(&abc.method_bodies[0].code).unwrap();
    let jump_end = decoded[1].pos + decoded[1].len;
    let zero = decoded[3].pos;
    let mut ops = ops;
    ops[1] = Op::Jump {
        offset: (zero - jump_end) as i32,
    };
    abc.method_bodies[0].code = encode(&ops);
    let mut report = Report::default();
    let shared = Options {
        shared_stage: true,
        client: None,
    };
    assert!(!patch_abc_with(&mut abc, Target::Game, shared, &mut report).unwrap());
    assert_eq!(report.stage_missed.len(), 1);
}

/// AQW's SWFs on a shared stage: `TURU_STAGE_SWFS=/dir/with/swfs cargo test -p turu_air_patch
/// shared_stage_real -- --nocapture`. Every `stage.getChildAt(0)` is rewritten, and the result
/// patches to nothing more.
#[test]
fn shared_stage_real_swfs() {
    let Ok(dir) = std::env::var("TURU_STAGE_SWFS") else {
        eprintln!("TURU_STAGE_SWFS not set, skipped");
        return;
    };
    let shared = Options {
        shared_stage: true,
        client: None,
    };
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let input = std::fs::read(&path).unwrap();
        let target = if path.to_string_lossy().contains("skua") {
            Target::Skua
        } else {
            Target::Game
        };
        let started = std::time::Instant::now();
        let (output, report) = patch_swf_with(&input, target, shared).unwrap();
        let took = started.elapsed();
        let (_, again) = patch_swf_with(&output, target, shared).unwrap();
        assert_eq!(again.stage_roots, vec![], "{}", path.display());
        let removes = report
            .stage_roots
            .iter()
            .filter(|s| s.name == "removeChildAt")
            .count();
        eprintln!(
            "{}: {} roots, {removes} removes, {} missed, {} undecodable, {took:?}, {} -> {} bytes",
            path.file_name().unwrap().to_string_lossy(),
            report.stage_roots.len() - removes,
            report.stage_missed.len(),
            report.undecodable.len(),
            input.len(),
            output.len()
        );
    }
}

#[test]
fn a_numbered_client_gets_its_own_host_classes() {
    let ops = [
        Op::GetLex {
            index: Index::new(1),
        }, // ExternalInterface
        Op::Pop,
        Op::FindPropStrict {
            index: Index::new(2),
        },
        Op::ConstructProp {
            index: Index::new(2),
            num_args: 0,
        }, // new Loader()
        Op::ReturnValue,
    ];
    let names = [
        ("flash.external", "ExternalInterface"),
        ("flash.display", "Loader"),
    ];
    let mut abc = abc_with(&names, &[], &ops, vec![]);
    let mut report = Report {
        target: Target::Skua,
        ..Report::default()
    };
    let options = Options {
        shared_stage: false,
        client: Some(2),
    };
    assert!(patch_abc_with(&mut abc, Target::Skua, options, &mut report).unwrap());
    let pool = &abc.constant_pool;
    let named: Vec<&str> = decode(&abc.method_bodies[0].code)
        .unwrap()
        .into_iter()
        .filter_map(|d| match d.op {
            Op::GetLex { index } | Op::FindPropStrict { index } => {
                local_name(pool, &pool.multinames[index.0 as usize - 1])
            }
            _ => None,
        })
        .collect();
    assert_eq!(named, ["TuruExternal2", "TuruLoader2"]);
}

#[test]
fn shared_stage_reads_the_mouse_from_the_game() {
    let ops = [
        Op::GetLocal { index: 0 },
        Op::GetProperty {
            index: Index::new(1),
        }, // this.stage
        Op::GetProperty {
            index: Index::new(2),
        }, // .mouseX
        Op::ReturnValue,
    ];
    let names = [("", "stage"), ("", "mouseX")];
    let mut abc = abc_with(&names, &[], &ops, vec![]);
    let mut report = Report::default();
    let shared = Options {
        shared_stage: true,
        client: None,
    };
    assert!(patch_abc_with(&mut abc, Target::Game, shared, &mut report).unwrap());
    assert_eq!(report.stage_roots.len(), 1);
    let pool = &abc.constant_pool;
    let got: Vec<String> = decode(&abc.method_bodies[0].code)
        .unwrap()
        .into_iter()
        .map(|d| match d.op {
            Op::GetLex { index } | Op::GetProperty { index } | Op::CallProperty { index, .. } => {
                local_name(pool, &pool.multinames[index.0 as usize - 1])
                    .unwrap()
                    .to_owned()
            }
            op => format!("{op:?}"),
        })
        .collect();
    assert_eq!(
        got,
        [
            "GetLocal { index: 0 }",
            "TuruStage",
            "Swap",
            "rootOf",
            "mouseX",
            "ReturnValue"
        ]
    );
}
