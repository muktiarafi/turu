//! Turu: finds a map's items in its SWF for Grabber > GetMap Item IDs (`turu_bot::map_items`).
//!
//! VibeSkua decompiles the map with FFDec and looks through the ActionScript text for
//! `getMapItem(id)` near `isQuestInProgress(quest)`, and for `mapItem = id` near
//! `questnum` / `intQuest = quest` (the timeline setting a map item button's properties).
//! This reads the same out of the AS3 bytecode, function by function:
//!
//! * a constant pushed right before `setproperty mapItem` (or `itemdrop`) is an item; its quest
//!   is the constant set as `intQuest` (or `questnum`) on the same object in that function;
//! * a constant pushed right before a one-argument `getMapItem` call is an item; its quest is
//!   the constant passed to the nearest `isQuestInProgress` call before it in that function (the
//!   check that guards it), else the nearest after it.
//!
//! An item with no quest gets 0 (VibeSkua leaves those out, or fails on them). An item found
//! twice keeps the first quest found for it.

use swf::avm2::read::Reader;
use swf::avm2::types::{AbcFile, ConstantPool, Multiname, Op};
use swf::extensions::ReadSwfExt;

/// The `turu_bot::map_items::Scanner`: each map item's ID and quest (0 when none), by ID.
pub fn scan(swf_data: &[u8]) -> Result<Vec<(i64, i64)>, String> {
    let buf = swf::decompress_swf(swf_data).map_err(|e| format!("not a SWF: {e}"))?;
    let parsed = swf::parse_swf(&buf).map_err(|e| format!("not a SWF: {e}"))?;
    let mut found: Vec<(i64, i64)> = Vec::new();
    for tag in &parsed.tags {
        let abc = match tag {
            swf::Tag::DoAbc(data) => *data,
            swf::Tag::DoAbc2(do_abc) => do_abc.data,
            _ => continue,
        };
        let Ok(file) = Reader::new(abc).read() else {
            continue;
        };
        scan_abc(&file, &mut found);
    }
    found.sort_by_key(|(id, _)| *id);
    Ok(found)
}

/// Adds an item, keeping the first quest found for it (a later sighting can fill a missing one).
fn add(found: &mut Vec<(i64, i64)>, id: i64, quest: i64) {
    if id <= 0 {
        return;
    }
    match found.iter_mut().find(|(known, _)| *known == id) {
        Some(entry) if entry.1 == 0 => entry.1 = quest,
        Some(_) => {}
        None => found.push((id, quest)),
    }
}

fn scan_abc(file: &AbcFile, found: &mut Vec<(i64, i64)>) {
    let pool = &file.constant_pool;
    for body in &file.method_bodies {
        let ops = decode(&body.code);
        scan_ops(pool, &ops, found);
    }
}

/// The function's instructions, up to the first one that doesn't decode.
pub(crate) fn decode(code: &[u8]) -> Vec<Op> {
    let mut reader = Reader::new(code);
    let mut ops = Vec::new();
    while !reader.as_slice().is_empty() {
        match reader.read_op() {
            Ok(op) => ops.push(op),
            Err(_) => break,
        }
    }
    ops
}

/// A multiname's local name, in lower case ("" for runtime names).
pub(crate) fn name(pool: &ConstantPool, index: u32) -> String {
    let string = match pool.multinames.get((index as usize).wrapping_sub(1)) {
        Some(
            Multiname::QName { name, .. }
            | Multiname::QNameA { name, .. }
            | Multiname::RTQName { name }
            | Multiname::RTQNameA { name }
            | Multiname::Multiname { name, .. }
            | Multiname::MultinameA { name, .. },
        ) => name.0,
        _ => return String::new(),
    };
    pool.strings
        .get((string as usize).wrapping_sub(1))
        .map(|s| String::from_utf8_lossy(s).to_lowercase())
        .unwrap_or_default()
}

/// The integer an instruction pushes, when it pushes a constant one.
fn constant(pool: &ConstantPool, op: &Op) -> Option<i64> {
    let value = match op {
        Op::PushByte { value } => *value as i8 as i64,
        Op::PushShort { value } => *value as i64,
        Op::PushInt { value } => *pool.ints.get((value.0 as usize).wrapping_sub(1))? as i64,
        Op::PushUint { value } => *pool.uints.get((value.0 as usize).wrapping_sub(1))? as i64,
        Op::PushDouble { value } => {
            let v = *pool.doubles.get((value.0 as usize).wrapping_sub(1))?;
            if v.fract() != 0.0 || !v.is_finite() {
                return None;
            }
            v as i64
        }
        _ => return None,
    };
    Some(value)
}

/// What a property is set on: the instruction that pushed the object (its debug text, which
/// names the local, the lexical name or the property read).
fn receiver(ops: &[Op], value_at: usize) -> Option<String> {
    value_at.checked_sub(1).map(|i| format!("{:?}", ops[i]))
}

/// A `setproperty` / `initproperty` of a constant: where, the property name, the value, the
/// object.
struct Set {
    at: usize,
    name: String,
    value: i64,
    object: Option<String>,
}

/// A one-argument call with a constant argument: where, the function name, the argument.
struct Call {
    at: usize,
    name: String,
    arg: i64,
}

fn scan_ops(pool: &ConstantPool, ops: &[Op], found: &mut Vec<(i64, i64)>) {
    let mut sets = Vec::new();
    let mut calls = Vec::new();
    for (at, op) in ops.iter().enumerate() {
        let Some(previous) = at.checked_sub(1) else {
            continue;
        };
        match op {
            Op::SetProperty { index } | Op::InitProperty { index } => {
                if let Some(value) = constant(pool, &ops[previous]) {
                    let object = receiver(ops, previous);
                    sets.push(Set {
                        at,
                        name: name(pool, index.0),
                        value,
                        object,
                    });
                }
            }
            Op::CallProperty { index, num_args: 1 }
            | Op::CallPropVoid { index, num_args: 1 }
            | Op::CallPropLex { index, num_args: 1 } => {
                if let Some(arg) = constant(pool, &ops[previous]) {
                    calls.push(Call {
                        at,
                        name: name(pool, index.0),
                        arg,
                    });
                }
            }
            _ => {}
        }
    }
    let nearest = |at: usize, candidates: &mut dyn Iterator<Item = (usize, i64)>| {
        candidates
            .min_by_key(|(other, _)| (other.abs_diff(at), *other > at))
            .map(|(_, value)| value)
    };
    for item in sets.iter().filter(|s| s.name == "mapitem" || s.name == "itemdrop") {
        let quest = nearest(
            item.at,
            &mut sets
                .iter()
                .filter(|s| (s.name == "intquest" || s.name == "questnum") && s.object == item.object)
                .map(|s| (s.at, s.value)),
        );
        add(found, item.value, quest.unwrap_or(0));
    }
    for item in calls.iter().filter(|c| c.name == "getmapitem") {
        // The check guards the call (`if (isQuestInProgress(q)) getMapItem(i)`): the nearest one
        // before it, else the nearest after it.
        let checks = || calls.iter().filter(|c| c.name == "isquestinprogress");
        let quest = checks()
            .filter(|c| c.at < item.at)
            .max_by_key(|c| c.at)
            .or_else(|| checks().filter(|c| c.at > item.at).min_by_key(|c| c.at))
            .map(|c| c.arg);
        add(found, item.arg, quest.unwrap_or(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swf::avm2::types::{
        AbcFile, ConstantPool, Index, Method, MethodBody, MethodFlags, Multiname, Namespace, Script,
    };
    use swf::avm2::write::Writer;

    /// Builds a SWF whose ABC has one function per `bodies` entry, written with `names` as its
    /// multinames (index 1 = `names[0]`).
    fn swf_with(names: &[&str], bodies: Vec<Vec<Op>>) -> Vec<u8> {
        let mut strings: Vec<Vec<u8>> = vec![b"".to_vec()];
        strings.extend(names.iter().map(|n| n.as_bytes().to_vec()));
        let pool = ConstantPool {
            ints: vec![70000],
            uints: vec![],
            doubles: vec![123.0],
            strings,
            namespaces: vec![Namespace::Package(Index::new(1))],
            namespace_sets: vec![],
            multinames: (0..names.len())
                .map(|i| Multiname::QName {
                    namespace: Index::new(1),
                    name: Index::new(i as u32 + 2),
                })
                .collect(),
        };
        let methods = (0..bodies.len())
            .map(|_| Method {
                name: Index::new(0),
                params: vec![],
                return_type: Index::new(0),
                flags: MethodFlags::empty(),
                body: None,
            })
            .collect();
        let method_bodies = bodies
            .into_iter()
            .enumerate()
            .map(|(i, ops)| {
                let mut code = Vec::new();
                let mut writer = Writer::new(&mut code);
                for op in &ops {
                    writer.write_op(op).unwrap();
                }
                MethodBody {
                    method: Index::new(i as u32),
                    max_stack: 4,
                    num_locals: 1,
                    init_scope_depth: 0,
                    max_scope_depth: 1,
                    code,
                    exceptions: vec![],
                    traits: vec![],
                }
            })
            .collect();
        let abc = AbcFile {
            major_version: 46,
            minor_version: 16,
            constant_pool: pool,
            methods,
            metadata: vec![],
            instances: vec![],
            classes: vec![],
            scripts: vec![Script {
                init_method: Index::new(0),
                traits: vec![],
            }],
            method_bodies,
        };
        let mut abc_bytes = Vec::new();
        Writer::new(&mut abc_bytes).write(abc).unwrap();
        let header = swf::Header::default_with_swf_version(10);
        let mut out = Vec::new();
        swf::write_swf(&header, &[swf::Tag::DoAbc(&abc_bytes)], &mut out).unwrap();
        out
    }

    fn lex(i: u32) -> Op {
        Op::GetLex { index: Index::new(i) }
    }
    fn set(i: u32) -> Op {
        Op::SetProperty { index: Index::new(i) }
    }
    fn short(v: i16) -> Op {
        Op::PushShort { value: v }
    }

    // Multinames: 1 mapItem, 2 intQuest, 3 btnA, 4 btnB, 5 getMapItem, 6 isQuestInProgress,
    // 7 world, 8 itemdrop, 9 questnum, 10 intQuestVal.
    const NAMES: [&str; 10] = [
        "mapItem",
        "intQuest",
        "btnA",
        "btnB",
        "getMapItem",
        "isQuestInProgress",
        "world",
        "itemdrop",
        "questnum",
        "intQuestVal",
    ];

    #[test]
    fn finds_items_set_on_buttons_with_their_quests() {
        // btnA.mapItem = 2209; btnB.mapItem = 2504; btnB.intQuest = 3370; btnA.intQuest = 3190;
        // btnA.intQuestVal = 5 (not a quest); then a button with no quest: btnB.itemdrop = 42.
        let swf = swf_with(
            &NAMES,
            vec![
                vec![
                    lex(3),
                    short(2209),
                    set(1),
                    lex(4),
                    short(2504),
                    set(1),
                    lex(4),
                    short(3370),
                    set(2),
                    lex(3),
                    short(3190),
                    set(2),
                    lex(3),
                    Op::PushByte { value: 5 },
                    set(10),
                    Op::ReturnVoid,
                ],
                vec![lex(4), Op::PushByte { value: 42 }, set(8), Op::ReturnVoid],
            ],
        );
        assert_eq!(scan(&swf).unwrap(), [(42, 0), (2209, 3190), (2504, 3370)]);
    }

    #[test]
    fn finds_get_map_item_calls_and_the_nearest_quest_check() {
        // if (world.isQuestInProgress(70000)) world.getMapItem(48);
        // world.getMapItem(123.0) with world.questnum unrelated; a getMapItem of a variable.
        let get_world = || lex(7);
        let check = || Op::CallProperty {
            index: Index::new(6),
            num_args: 1,
        };
        let get_item = || Op::CallPropVoid {
            index: Index::new(5),
            num_args: 1,
        };
        let swf = swf_with(
            &NAMES,
            vec![
                vec![
                    get_world(),
                    Op::PushInt { value: Index::new(1) },
                    Op::CallProperty {
                        index: Index::new(6),
                        num_args: 1,
                    },
                    Op::IfFalse { offset: 0 },
                    get_world(),
                    Op::PushByte { value: 48 },
                    Op::CallPropVoid {
                        index: Index::new(5),
                        num_args: 1,
                    },
                    Op::ReturnVoid,
                ],
                // if (isQuestInProgress(260)) getMapItem(44); if (isQuestInProgress(267))
                // getMapItem(49): each call takes the check before it, not the nearer one after.
                vec![
                    get_world(),
                    short(260),
                    check(),
                    Op::IfFalse { offset: 0 },
                    get_world(),
                    Op::PushByte { value: 44 },
                    get_item(),
                    get_world(),
                    short(267),
                    check(),
                    Op::IfFalse { offset: 0 },
                    get_world(),
                    Op::PushByte { value: 49 },
                    get_item(),
                    Op::ReturnVoid,
                ],
                vec![
                    get_world(),
                    Op::PushDouble { value: Index::new(1) },
                    Op::CallPropVoid {
                        index: Index::new(5),
                        num_args: 1,
                    },
                    get_world(),
                    Op::GetLocal { index: 1 },
                    Op::CallPropVoid {
                        index: Index::new(5),
                        num_args: 1,
                    },
                    Op::ReturnVoid,
                ],
            ],
        );
        assert_eq!(scan(&swf).unwrap(), [(44, 260), (48, 70000), (49, 267), (123, 0)]);
    }

    #[test]
    fn keeps_the_first_quest_and_fills_a_missing_one() {
        let mut found = Vec::new();
        add(&mut found, 7, 0);
        add(&mut found, 7, 11);
        add(&mut found, 7, 12);
        add(&mut found, 0, 5);
        assert_eq!(found, [(7, 11)]);
    }

    #[test]
    fn rejects_what_isnt_a_swf() {
        assert!(scan(b"<html>404</html>").unwrap_err().starts_with("not a SWF"));
    }

    /// A real map, when one is given: `TURU_MAP_SWF=path cargo test ... real_map -- --nocapture`.
    #[test]
    fn real_map() {
        let Some(path) = std::env::var_os("TURU_MAP_SWF") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let items = scan(&data).unwrap();
        println!("{items:?}");
        assert!(!items.is_empty());
    }
}
