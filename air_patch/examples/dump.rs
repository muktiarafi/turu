//! Prints the AS3 bytecode of a SWF's methods whose name contains a filter, with names resolved:
//! `cargo run -p turu_air_patch --example dump -- Game.swf AirLoader/`.

use swf::avm2::read::Reader;
use swf::avm2::types::{AbcFile, Multiname, Namespace, Op, TraitKind};
use swf::extensions::ReadSwfExt;

fn string(abc: &AbcFile, i: u32) -> String {
    abc.constant_pool
        .strings
        .get((i as usize).wrapping_sub(1))
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .unwrap_or_default()
}

fn ns(abc: &AbcFile, i: u32) -> String {
    match abc.constant_pool.namespaces.get((i as usize).wrapping_sub(1)) {
        Some(Namespace::Package(s)) => format!("pkg({})", string(abc, s.0)),
        Some(Namespace::Private(_)) => "private".into(),
        Some(Namespace::Protected(_)) => "protected".into(),
        Some(Namespace::PackageInternal(s)) => format!("internal({})", string(abc, s.0)),
        Some(other) => format!("{other:?}"),
        None => "*".into(),
    }
}

fn mn(abc: &AbcFile, i: u32) -> String {
    match abc.constant_pool.multinames.get((i as usize).wrapping_sub(1)) {
        Some(Multiname::QName { namespace, name } | Multiname::QNameA { namespace, name }) => {
            format!("{}::{}", ns(abc, namespace.0), string(abc, name.0))
        }
        Some(Multiname::Multiname { name, .. } | Multiname::MultinameA { name, .. }) => {
            format!("[set]::{}", string(abc, name.0))
        }
        Some(Multiname::RTQName { name } | Multiname::RTQNameA { name }) => format!("rt::{}", string(abc, name.0)),
        Some(Multiname::TypeName { base_type, parameters }) => {
            format!(
                "{}<{}>",
                mn(abc, base_type.0),
                parameters.iter().map(|p| mn(abc, p.0)).collect::<Vec<_>>().join(",")
            )
        }
        Some(other) => format!("{other:?}"),
        None => "*".into(),
    }
}

fn op_name(op: &Op) -> String {
    format!("{op:?}")
        .split([' ', '('])
        .next()
        .unwrap_or_default()
        .to_string()
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, filter] = &args[..] else {
        anyhow::bail!("usage: dump <in.swf> <filter>")
    };
    let bytes = std::fs::read(input)?;
    let buf = swf::decompress_swf(&bytes[..])?;
    let parsed = swf::parse_swf(&buf)?;
    for tag in &parsed.tags {
        let swf::Tag::DoAbc2(do_abc) = tag else { continue };
        let abc = Reader::new(do_abc.data).read()?;
        let mut names = std::collections::HashMap::new();
        for (inst, class) in abc.instances.iter().zip(&abc.classes) {
            let c = string(
                &abc,
                match abc.constant_pool.multinames[inst.name.0 as usize - 1] {
                    Multiname::QName { name, .. } => name.0,
                    _ => 0,
                },
            );
            names.insert(inst.init_method.0, format!("{c}/{c}"));
            names.insert(class.init_method.0, format!("{c}/cinit"));
            for t in inst.traits.iter().chain(&class.traits) {
                if let TraitKind::Method { method, .. }
                | TraitKind::Getter { method, .. }
                | TraitKind::Setter { method, .. } = t.kind
                {
                    names.insert(
                        method.0,
                        format!("{c}/{}", mn(&abc, t.name.0).rsplit("::").next().unwrap()),
                    );
                }
            }
        }
        for body in &abc.method_bodies {
            let name = names
                .get(&body.method.0)
                .cloned()
                .unwrap_or(format!("function#{}", body.method.0));
            if !name.contains(filter.as_str()) {
                continue;
            }
            println!("== {name} (max_stack {}, locals {})", body.max_stack, body.num_locals);
            for e in &body.exceptions {
                println!(
                    "  try {}..{} -> catch {} ({})",
                    e.from_offset,
                    e.to_offset,
                    e.target_offset,
                    mn(&abc, e.type_name.0)
                );
            }
            let mut reader = Reader::new(&body.code);
            while !reader.as_slice().is_empty() {
                let pos = body.code.len() - reader.as_slice().len();
                let op = reader.read_op()?;
                let text = match &op {
                    Op::GetLex { index }
                    | Op::GetProperty { index }
                    | Op::SetProperty { index }
                    | Op::InitProperty { index }
                    | Op::FindProperty { index }
                    | Op::FindPropStrict { index }
                    | Op::Coerce { index }
                    | Op::AsType { type_name: index }
                    | Op::IsType { index } => format!("{} {}", op_name(&op), mn(&abc, index.0)),
                    Op::CallProperty { index, num_args }
                    | Op::CallPropVoid { index, num_args }
                    | Op::ConstructProp { index, num_args }
                    | Op::CallPropLex { index, num_args }
                    | Op::CallSuper { index, num_args }
                    | Op::CallSuperVoid { index, num_args } => {
                        format!("{} {} ({num_args})", op_name(&op), mn(&abc, index.0))
                    }
                    Op::PushString { value } => format!("PushString {:?}", string(&abc, value.0)),
                    other => format!("{other:?}"),
                };
                println!("  {pos:5}  {text}");
            }
        }
    }
    Ok(())
}
