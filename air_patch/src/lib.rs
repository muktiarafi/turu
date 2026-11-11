//! AQW's client and Turu's host SWF under Adobe AIR (docs/AIR_PROPOSAL.md).
//!
//! AIR runs Turu's host in its application sandbox, which loads the SWFs below with `loadBytes`
//! and `allowCodeImport` (so they share code with it) after this crate has rewritten their AS3
//! bytecode. Two targets:
//!
//! * [`Target::Game`], AQW's client. It has an AIR mode of its own (Artix's mobile app):
//!   `types.AirLoader`, a `Loader` that in AIR's application sandbox fetches a SWF's bytes,
//!   patches them (`SwfLoaderPatch`: each `new Loader()` in them becomes `new AirLoader()`) and
//!   loads them with `loadBytes`, so maps and assets share code with the game. The client builds
//!   most of its loaders as `AirLoader`s; its remaining `new Loader()` (pets, mail images, the map
//!   popup's news, ...) become `new AirLoader()` here, as `SwfLoaderPatch` does to the SWFs it
//!   loads. Its server paths come from the host's `params.sFilePath`. `AirLoader`'s own
//!   `new URLLoader()` becomes the host's `TuruURLLoader` (a `URLLoader` that fetches the game's
//!   files through Turu's disk cache; AIR keeps none).
//! * [`Target::Skua`], VibeSkua's `skua.swf`, unchanged otherwise: `ExternalInterface` (which
//!   AIR on Android doesn't have) becomes the host's `TuruExternal` (same `addCallback` and
//!   `call`, through Turu's native extension), and each `new Loader()` the host's `TuruLoader`
//!   (fetches the bytes, patches the game, `loadBytes`).
//!
//! For both, `Security.allowDomain` and `allowInsecureDomain` calls are dropped (their arguments
//! popped, `undefined` pushed where the result is used): they throw in the application sandbox
//! (#3207), and the ones outside a try/catch would stop the code that makes them.
//!
//! The rules match instructions, not method names or code shapes, so an update that moves code
//! around is still patched. [`Report`] lists what changed, and [`Report::problem`] says whether
//! the result still has what AIR mode needs.

use anyhow::{Context, Result, bail};
use std::collections::{HashMap, HashSet};
use swf::avm2::read::Reader;
use swf::avm2::types::{
    AbcFile, ConstantPool, Exception, Index, Multiname, Namespace, Op, Trait, TraitKind,
};
use swf::avm2::write::Writer;
use swf::extensions::ReadSwfExt;
use swf::{Compression, Header};

pub mod cache;

const TAG_END: u16 = 0;
const TAG_DO_ABC: u16 = 72;
const TAG_DO_ABC2: u16 = 82;

/// The host's classes that skua's `Loader` and `ExternalInterface` become (package `""`).
pub const HOST_LOADER: &str = "TuruLoader";
pub const HOST_EXTERNAL: &str = "TuruExternal";
/// The host's `URLLoader` that `AirLoader`'s fetches become (package `""`).
pub const HOST_URL_LOADER: &str = "TuruURLLoader";
/// The host's class that finds a client's game on a stage several clients share (package `""`):
/// `rootOf(x)` and `removeOwn(main)`.
pub const HOST_STAGE: &str = "TuruStage";

/// What [`patch_swf_with`] does beyond its [`Target`]'s rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Several clients share AIR's one stage. The game, skua and the SWFs the game loads find
    /// the game as `stage.getChildAt(0)`; each `x.stage.getChildAt(0)` (and `stage`, through the
    /// scope, then `getChildAt(0)`) becomes `TuruStage.rootOf(x)`, the game `x` belongs to.
    /// In skua, its `stage.removeChildAt(0)` (itself, before it adds the game) becomes
    /// `TuruStage.removeOwn(this)`.
    pub shared_stage: bool,
    /// skua's `ExternalInterface` becomes this host class instead of [`HOST_EXTERNAL`] (a
    /// client the engine doesn't drive).
    pub external: Option<&'static str>,
}

/// Which SWF is being patched: what its loaders become, and what it must have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    /// AQW's client (or a SWF it loads): `new Loader()` becomes its own `types.AirLoader`.
    #[default]
    Game,
    /// `skua.swf`: `new Loader()` becomes [`HOST_LOADER`], `ExternalInterface`
    /// [`HOST_EXTERNAL`].
    Skua,
}

/// One patched instruction: the method it's in and the property it names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub method: String,
    pub name: String,
}

/// What [`patch_swf`] found and changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub target: Target,
    /// ABC blocks (DoAbc tags) read.
    pub abc_blocks: usize,
    /// Whether the client has its AIR loader (`AirLoader` and `SwfLoaderPatch`).
    pub has_air_loader: bool,
    /// `new Loader()` made the target's loader ("Loader"), `ExternalInterface` made
    /// [`HOST_EXTERNAL`] ("ExternalInterface"), and `AirLoader`'s `new URLLoader()` made
    /// [`HOST_URL_LOADER`] ("URLLoader").
    pub swapped: Vec<Site>,
    /// `allowDomain`/`allowInsecureDomain` calls dropped.
    pub dropped: Vec<Site>,
    /// Method bodies whose bytecode couldn't be decoded, left as they were.
    pub undecodable: Vec<String>,
    /// [`Options::shared_stage`]: `stage.getChildAt(0)` made `TuruStage.rootOf` ("getChildAt"),
    /// skua's `removeChildAt(0)` made `TuruStage.removeOwn` ("removeChildAt").
    pub stage_roots: Vec<Site>,
    /// [`Options::shared_stage`]: `getChildAt(0)` right after a `stage` that wasn't rewritten (a
    /// branch lands inside it), so it still finds stage child 0.
    pub stage_missed: Vec<Site>,
}

impl Report {
    pub fn changed(&self) -> bool {
        !self.swapped.is_empty() || !self.dropped.is_empty() || !self.stage_roots.is_empty()
    }

    /// Why the patched SWF wouldn't run under AIR, or `None` if it should.
    pub fn problem(&self) -> Option<String> {
        if !self.undecodable.is_empty() {
            return Some(format!(
                "{} methods couldn't be read",
                self.undecodable.len()
            ));
        }
        match self.target {
            Target::Game if !self.has_air_loader => {
                Some("the client has no AirLoader/SwfLoaderPatch (its AIR mode)".into())
            }
            Target::Skua if !self.swapped.iter().any(|s| s.name == "ExternalInterface") => {
                Some("no ExternalInterface found to route through the extension".into())
            }
            _ => None,
        }
    }
}

/// Patches a SWF (compressed or not) for `target`. Returns the patched SWF, zlib-compressed, or
/// the input unchanged when nothing needed patching, with what was done.
pub fn patch_swf(input: &[u8], target: Target) -> Result<(Vec<u8>, Report)> {
    patch_swf_with(input, target, Options::default())
}

/// [`patch_swf`] with [`Options`].
pub fn patch_swf_with(input: &[u8], target: Target, options: Options) -> Result<(Vec<u8>, Report)> {
    let buf = swf::decompress_swf(input).context("not a SWF")?;
    let data = &buf.data;
    let mut report = Report {
        target,
        ..Report::default()
    };
    let mut tags = Vec::with_capacity(data.len() + 4096);
    let mut pos = 0;
    while pos < data.len() {
        let (code, body_start, end) = tag_bounds(data, pos)?;
        let body = &data[body_start..end];
        let patched = match code {
            TAG_DO_ABC => patch_abc_bytes(body, target, options, &mut report)?,
            TAG_DO_ABC2 => {
                // Flags (u32), then a null-terminated name, then the ABC.
                let name_end = body
                    .get(4..)
                    .and_then(|rest| rest.iter().position(|&b| b == 0))
                    .context("DoAbc2 tag without a name")?;
                let abc_start = 4 + name_end + 1;
                patch_abc_bytes(&body[abc_start..], target, options, &mut report)?
                    .map(|abc| [&body[..abc_start], &abc[..]].concat())
            }
            _ => None,
        };
        match patched {
            Some(body) => write_tag(&mut tags, code, &body),
            None => tags.extend_from_slice(&data[pos..end]),
        }
        pos = end;
        if code == TAG_END {
            tags.extend_from_slice(&data[pos..]);
            break;
        }
    }
    if !report.changed() {
        return Ok((input.to_vec(), report));
    }
    let header = Header {
        compression: Compression::Zlib,
        ..buf.header.swf_header().clone()
    };
    let mut output = Vec::with_capacity(input.len() + 4096);
    swf::write::write_swf_raw_tags(&header, &tags, &mut output)?;
    Ok((output, report))
}

/// A tag's code, where its body starts and where it ends.
fn tag_bounds(data: &[u8], pos: usize) -> Result<(u16, usize, usize)> {
    let short = data.get(pos..pos + 2).context("truncated tag header")?;
    let code_and_length = u16::from_le_bytes([short[0], short[1]]);
    let code = code_and_length >> 6;
    let mut length = (code_and_length & 0x3f) as usize;
    let mut body_start = pos + 2;
    if length == 0x3f {
        let long = data.get(pos + 2..pos + 6).context("truncated tag header")?;
        length = u32::from_le_bytes([long[0], long[1], long[2], long[3]]) as usize;
        body_start = pos + 6;
    }
    let end = body_start
        .checked_add(length)
        .filter(|&end| end <= data.len());
    let end = end.with_context(|| format!("tag {code} at {pos} runs past the end"))?;
    Ok((code, body_start, end))
}

fn write_tag(out: &mut Vec<u8>, code: u16, body: &[u8]) {
    out.extend_from_slice(&((code << 6) | 0x3f).to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
}

/// Patches one ABC block: `Some(bytes)` if anything changed.
fn patch_abc_bytes(
    bytes: &[u8],
    target: Target,
    options: Options,
    report: &mut Report,
) -> Result<Option<Vec<u8>>> {
    let mut abc = Reader::new(bytes).read().context("reading ABC")?;
    report.abc_blocks += 1;
    if !patch_abc_with(&mut abc, target, options, report)? {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(bytes.len() + 1024);
    Writer::new(&mut out).write(abc).context("writing ABC")?;
    Ok(Some(out))
}

/// Patches an ABC block in place for `target`. True if anything changed.
pub fn patch_abc(abc: &mut AbcFile, target: Target, report: &mut Report) -> Result<bool> {
    patch_abc_with(abc, target, Options::default(), report)
}

/// [`patch_abc`] with [`Options`].
pub fn patch_abc_with(
    abc: &mut AbcFile,
    target: Target,
    options: Options,
    report: &mut Report,
) -> Result<bool> {
    let pool = &abc.constant_pool;
    let class_named = |wanted: &str| {
        abc.instances
            .iter()
            .map(|instance| instance.name)
            .find(|name| {
                pool.multinames
                    .get((name.0 as usize).wrapping_sub(1))
                    .and_then(|m| local_name(pool, m))
                    == Some(wanted)
            })
    };
    let air_loader = class_named("AirLoader");
    if air_loader.is_some() && class_named("SwfLoaderPatch").is_some() {
        report.has_air_loader = true;
    }

    // Multinames for flash.display's Loader, flash.net's URLLoader, flash.external's
    // ExternalInterface, and the calls to drop.
    let mut loaders = HashSet::new();
    let mut url_loaders = HashSet::new();
    let mut externals = HashSet::new();
    let mut drops = HashMap::new();
    // `stage`, `getChildAt` and `removeChildAt`, in any namespace ([`Options::shared_stage`]).
    let mut stage_names = HashSet::new();
    let mut get_child_at = HashSet::new();
    let mut remove_child_at = HashSet::new();
    // skua's `Main.stg`, the stage it keeps.
    let mut skua_stage = HashSet::new();
    for (i, multiname) in pool.multinames.iter().enumerate() {
        let index = i as u32 + 1;
        match local_name(pool, multiname) {
            Some("stage") => {
                stage_names.insert(index);
            }
            Some("stg") if target == Target::Skua => {
                skua_stage.insert(index);
            }
            Some("getChildAt") => {
                get_child_at.insert(index);
            }
            Some("removeChildAt") if target == Target::Skua => {
                remove_child_at.insert(index);
            }
            Some("Loader") if in_package(pool, multiname, "flash.display") => {
                loaders.insert(index);
            }
            Some("URLLoader") if in_package(pool, multiname, "flash.net") => {
                url_loaders.insert(index);
            }
            Some("ExternalInterface") if in_package(pool, multiname, "flash.external") => {
                externals.insert(index);
            }
            Some(name @ ("allowDomain" | "allowInsecureDomain")) => {
                drops.insert(index, name.to_string());
            }
            _ => {}
        }
    }
    // What `new Loader()` and `ExternalInterface` become.
    let (loader, external) = match target {
        Target::Game => (air_loader.filter(|_| !loaders.is_empty()), None),
        Target::Skua => (
            (!loaders.is_empty()).then(|| add_qname(&mut abc.constant_pool, "", HOST_LOADER)),
            (!externals.is_empty()).then(|| {
                add_qname(
                    &mut abc.constant_pool,
                    "",
                    options.external.unwrap_or(HOST_EXTERNAL),
                )
            }),
        ),
    };
    // What `new URLLoader()` in the client's `AirLoader` becomes.
    let url_loader = (target == Target::Game && air_loader.is_some() && !url_loaders.is_empty())
        .then(|| add_qname(&mut abc.constant_pool, "", HOST_URL_LOADER));
    // `TuruStage`, `rootOf` and `removeOwn`, for a shared stage.
    let stage_host = (options.shared_stage
        && !stage_names.is_empty()
        && (!get_child_at.is_empty() || !remove_child_at.is_empty()))
    .then(|| {
        let pool = &mut abc.constant_pool;
        (
            add_qname(pool, "", HOST_STAGE),
            add_qname(pool, "", "rootOf"),
            add_qname(pool, "", "removeOwn"),
        )
    });
    if loader.is_none()
        && external.is_none()
        && url_loader.is_none()
        && drops.is_empty()
        && stage_host.is_none()
    {
        return Ok(false);
    }

    let names = method_names(abc);
    let mut changed = false;
    for body in abc.method_bodies.iter_mut() {
        let method = names
            .get(&body.method.0)
            .cloned()
            .unwrap_or_else(|| format!("function#{}", body.method.0));
        let Ok(ops) = decode(&body.code) else {
            report.undecodable.push(method);
            continue;
        };
        // `findpropstrict Loader; constructprop Loader, 0` pairs (and URLLoader's in AirLoader):
        // both instructions' positions, with what they construct instead and the report's name.
        let mut swaps = HashMap::new();
        let in_air_loader = method.starts_with("AirLoader/");
        let kinds = [
            (&loaders, loader, "Loader"),
            (
                &url_loaders,
                url_loader.filter(|_| in_air_loader),
                "URLLoader",
            ),
        ];
        for (names, to, name) in kinds {
            let Some(to) = to else { continue };
            for (i, pair) in ops.windows(2).enumerate() {
                if let [
                    Decoded {
                        op: Op::FindPropStrict { index: find },
                        ..
                    },
                    Decoded {
                        op:
                            Op::ConstructProp {
                                index: construct,
                                num_args: 0,
                            },
                        ..
                    },
                ] = pair
                    && names.contains(&find.0)
                    && names.contains(&construct.0)
                {
                    swaps.insert(i, (to, name));
                    swaps.insert(i + 1, (to, name));
                }
            }
        }
        // A shared stage: instructions replaced, by index, and what the report calls them.
        let mut stage_edits: HashMap<usize, Vec<Op>> = HashMap::new();
        let mut stage_sites = Vec::new();
        let mut stage_missed = 0;
        if let Some((stage_class, root_of, remove_own)) = stage_host {
            let targets = jump_targets(&ops, &body.exceptions);
            let inside = |i: usize| targets.contains(&ops[i].pos);
            for i in 0..ops.len().saturating_sub(2) {
                let zero = matches!(ops[i + 1].op, Op::PushByte { value: 0 });
                // `x.stage` (or `stage` through the scope), `0`, `getChildAt(1 arg)`.
                if let Op::CallProperty {
                    index: call,
                    num_args: 1,
                } = ops[i + 2].op
                    && zero
                    && get_child_at.contains(&call.0)
                {
                    let receiver = match ops[i].op {
                        Op::GetProperty { index } if stage_names.contains(&index.0) => Some(vec![]),
                        // `getlex stage`: `findpropstrict` leaves the object that has it.
                        Op::GetLex { index } if stage_names.contains(&index.0) => {
                            Some(vec![Op::FindPropStrict { index }])
                        }
                        _ => None,
                    };
                    let Some(mut first) = receiver else { continue };
                    if inside(i + 1) || inside(i + 2) {
                        stage_missed += 1;
                        continue;
                    }
                    // x, TuruStage -> TuruStage, x -> TuruStage.rootOf(x).
                    first.extend([Op::GetLex { index: stage_class }, Op::Swap]);
                    stage_edits.insert(i, first);
                    stage_edits.insert(i + 1, vec![]);
                    stage_edits.insert(
                        i + 2,
                        vec![Op::CallProperty {
                            index: root_of,
                            num_args: 1,
                        }],
                    );
                    stage_sites.push("getChildAt");
                }
                // skua's `this.stg.removeChildAt(0)` (itself, before it adds the game; not the
                // other `removeChildAt(0)`s): `TuruStage.removeOwn(this)`.
                if let Op::CallPropVoid {
                    index: call,
                    num_args: 1,
                } = ops[i + 2].op
                    && zero
                    && remove_child_at.contains(&call.0)
                    && matches!(ops[i].op, Op::GetProperty { index } if skua_stage.contains(&index.0) || stage_names.contains(&index.0))
                    && !inside(i + 1)
                    && !inside(i + 2)
                {
                    stage_edits.insert(
                        i + 1,
                        vec![
                            Op::Pop,
                            Op::GetLex { index: stage_class },
                            Op::GetLocal { index: 0 },
                        ],
                    );
                    stage_edits.insert(
                        i + 2,
                        vec![Op::CallPropVoid {
                            index: remove_own,
                            num_args: 1,
                        }],
                    );
                    stage_sites.push("removeChildAt");
                }
            }
        }
        let mut swapped = Vec::new();
        let mut dropped = Vec::new();
        let code = relink(body.code.len(), &ops, &mut body.exceptions, |i, op| {
            if let Some(replacement) = stage_edits.get(&i) {
                return Some(replacement.clone());
            }
            if let Some(&(index, name)) = swaps.get(&i) {
                return Some(vec![match *op {
                    Op::FindPropStrict { .. } => Op::FindPropStrict { index },
                    _ => {
                        swapped.push(name);
                        Op::ConstructProp { index, num_args: 0 }
                    }
                }]);
            }
            if let Some(index) = external {
                let replaced = match *op {
                    Op::GetLex { index: from } if externals.contains(&from.0) => {
                        Some(Op::GetLex { index })
                    }
                    Op::FindPropStrict { index: from } if externals.contains(&from.0) => {
                        Some(Op::FindPropStrict { index })
                    }
                    Op::FindProperty { index: from } if externals.contains(&from.0) => {
                        Some(Op::FindProperty { index })
                    }
                    _ => None,
                };
                if let Some(op) = replaced {
                    swapped.push("ExternalInterface");
                    return Some(vec![op]);
                }
            }
            let (index, num_args, void) = match *op {
                Op::CallPropVoid { index, num_args } => (index, num_args, true),
                Op::CallProperty { index, num_args } => (index, num_args, false),
                _ => return None,
            };
            let name = drops.get(&index.0)?;
            let mut seq = vec![Op::Pop; num_args as usize + 1];
            if !void {
                seq.push(Op::PushUndefined);
            }
            dropped.push(name.clone());
            Some(seq)
        })
        .with_context(|| format!("relinking {method}"))?;
        for _ in 0..stage_missed {
            report.stage_missed.push(Site {
                method: method.clone(),
                name: "getChildAt".into(),
            });
        }
        let Some(code) = code else {
            continue;
        };
        body.code = code;
        if !stage_sites.is_empty() {
            // The rewrites peak at the stack the originals did; one more in case.
            body.max_stack += 1;
        }
        for name in stage_sites {
            report.stage_roots.push(Site {
                method: method.clone(),
                name: name.into(),
            });
        }
        if !dropped.is_empty() {
            // Room for the `undefined` pushed after a dropped call that had no arguments.
            body.max_stack += 1;
        }
        for name in swapped {
            report.swapped.push(Site {
                method: method.clone(),
                name: name.into(),
            });
        }
        for name in dropped {
            report.dropped.push(Site {
                method: method.clone(),
                name,
            });
        }
        changed = true;
    }
    Ok(changed)
}

/// `QName(PackageNamespace(package), name)` in the pool (added, with its strings and
/// namespace, where missing).
fn add_qname(pool: &mut ConstantPool, package: &str, name: &str) -> Index<Multiname> {
    let package = add_string(pool, package);
    let namespace = Namespace::Package(package);
    let namespace = match pool.namespaces.iter().position(|ns| *ns == namespace) {
        Some(i) => i as u32 + 1,
        None => {
            pool.namespaces.push(namespace);
            pool.namespaces.len() as u32
        }
    };
    let qname = Multiname::QName {
        namespace: Index::new(namespace),
        name: add_string(pool, name),
    };
    match pool.multinames.iter().position(|m| *m == qname) {
        Some(i) => Index::new(i as u32 + 1),
        None => {
            pool.multinames.push(qname);
            Index::new(pool.multinames.len() as u32)
        }
    }
}

fn add_string(pool: &mut ConstantPool, value: &str) -> Index<String> {
    match pool.strings.iter().position(|s| s == value.as_bytes()) {
        Some(i) => Index::new(i as u32 + 1),
        None => {
            pool.strings.push(value.as_bytes().to_vec());
            Index::new(pool.strings.len() as u32)
        }
    }
}

fn string(pool: &ConstantPool, index: Index<String>) -> Option<&str> {
    let bytes = pool.strings.get((index.0 as usize).checked_sub(1)?)?;
    std::str::from_utf8(bytes).ok()
}

fn local_name<'a>(pool: &'a ConstantPool, multiname: &Multiname) -> Option<&'a str> {
    match multiname {
        Multiname::QName { name, .. }
        | Multiname::QNameA { name, .. }
        | Multiname::Multiname { name, .. }
        | Multiname::MultinameA { name, .. } => string(pool, *name),
        _ => None,
    }
}

/// A QName in `package`, or a multiname whose namespace set includes it.
fn in_package(pool: &ConstantPool, multiname: &Multiname, package: &str) -> bool {
    let matches = |ns: &Index<Namespace>| match pool.namespaces.get((ns.0 as usize).wrapping_sub(1))
    {
        Some(Namespace::Package(name)) => string(pool, *name).unwrap_or("") == package,
        _ => false,
    };
    match multiname {
        Multiname::QName { namespace, .. } => matches(namespace),
        Multiname::Multiname { namespace_set, .. } => pool
            .namespace_sets
            .get((namespace_set.0 as usize).wrapping_sub(1))
            .is_some_and(|set| set.iter().any(matches)),
        _ => false,
    }
}

/// Method index to "Class/method", for reports.
fn method_names(abc: &AbcFile) -> HashMap<u32, String> {
    let pool = &abc.constant_pool;
    let mut names = HashMap::new();
    let name_of = |index: Index<Multiname>| {
        pool.multinames
            .get((index.0 as usize).wrapping_sub(1))
            .and_then(|m| local_name(pool, m))
            .unwrap_or("?")
            .to_string()
    };
    let add_traits = |names: &mut HashMap<u32, String>, prefix: &str, traits: &[Trait]| {
        for t in traits {
            let method = match t.kind {
                TraitKind::Method { method, .. } => method,
                TraitKind::Getter { method, .. } => method,
                TraitKind::Setter { method, .. } => method,
                TraitKind::Function { function, .. } => function,
                _ => continue,
            };
            names.insert(method.0, format!("{prefix}/{}", name_of(t.name)));
        }
    };
    for (instance, class) in abc.instances.iter().zip(&abc.classes) {
        let class_name = name_of(instance.name);
        names.insert(instance.init_method.0, format!("{class_name}/{class_name}"));
        names.insert(class.init_method.0, format!("{class_name}/cinit"));
        add_traits(&mut names, &class_name, &instance.traits);
        add_traits(&mut names, &class_name, &class.traits);
    }
    for (i, script) in abc.scripts.iter().enumerate() {
        names.insert(script.init_method.0, format!("script#{i}"));
        add_traits(&mut names, "", &script.traits);
    }
    names
}

/// An instruction and where it was.
struct Decoded {
    pos: usize,
    len: usize,
    op: Op,
}

fn decode(code: &[u8]) -> Result<Vec<Decoded>> {
    let mut reader = Reader::new(code);
    let mut ops = Vec::new();
    while !reader.as_slice().is_empty() {
        let pos = code.len() - reader.as_slice().len();
        let op = reader
            .read_op()
            .with_context(|| format!("instruction at {pos}"))?;
        let len = code.len() - reader.as_slice().len() - pos;
        ops.push(Decoded { pos, len, op });
    }
    Ok(ops)
}

fn encoded_len(op: &Op) -> Result<usize> {
    let mut out = Vec::new();
    Writer::new(&mut out).write_op(op)?;
    Ok(out.len())
}

/// Where branches, switches and exception ranges land: no edit may remove an instruction there.
fn jump_targets(ops: &[Decoded], exceptions: &[Exception]) -> HashSet<usize> {
    let mut targets = HashSet::new();
    for decoded in ops {
        let mut op = decoded.op.clone();
        if let Some(offset) = branch_offset(&mut op) {
            targets.insert((decoded.pos as i64 + decoded.len as i64 + *offset as i64) as usize);
        } else if let Op::LookupSwitch(switch) = &decoded.op {
            let base = decoded.pos as i64;
            targets.insert((base + switch.default_offset as i64) as usize);
            for offset in switch.case_offsets.iter() {
                targets.insert((base + *offset as i64) as usize);
            }
        }
    }
    for exception in exceptions {
        targets.insert(exception.from_offset as usize);
        targets.insert(exception.to_offset as usize);
        targets.insert(exception.target_offset as usize);
    }
    targets
}

fn branch_offset(op: &mut Op) -> Option<&mut i32> {
    match op {
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
        | Op::Jump { offset } => Some(offset),
        _ => None,
    }
}

/// Rebuilds a method's code with `edit`'s replacements (`Some(ops)` for the instruction at an
/// index to replace), moving branches and exception ranges to where their instructions went.
/// `None` if nothing was replaced.
fn relink(
    code_len: usize,
    ops: &[Decoded],
    exceptions: &mut [Exception],
    mut edit: impl FnMut(usize, &Op) -> Option<Vec<Op>>,
) -> Result<Option<Vec<u8>>> {
    // Each emitted instruction, with the original it is (None for inserted ones).
    let mut emitted: Vec<(Op, Option<usize>)> = Vec::with_capacity(ops.len());
    let mut first_emitted = Vec::with_capacity(ops.len());
    let mut changed = false;
    for (i, decoded) in ops.iter().enumerate() {
        first_emitted.push(emitted.len());
        match edit(i, &decoded.op) {
            Some(replacement) => {
                changed = true;
                emitted.extend(replacement.into_iter().map(|op| (op, None)));
            }
            None => emitted.push((decoded.op.clone(), Some(i))),
        }
    }
    if !changed {
        return Ok(None);
    }

    // Branch offsets have a fixed width, so the layout doesn't depend on their values.
    let mut new_pos = Vec::with_capacity(emitted.len() + 1);
    let mut pos = 0;
    for (op, _) in &emitted {
        new_pos.push(pos);
        pos += encoded_len(op)?;
    }
    new_pos.push(pos);
    let new_len = pos;

    let by_pos: HashMap<usize, usize> = ops.iter().enumerate().map(|(i, d)| (d.pos, i)).collect();
    let map = |old: i64| -> Result<i64> {
        if old == code_len as i64 {
            return Ok(new_len as i64);
        }
        match usize::try_from(old).ok().and_then(|old| by_pos.get(&old)) {
            Some(&i) => Ok(new_pos[first_emitted[i]] as i64),
            None => bail!("a branch to {old}, which isn't an instruction's start"),
        }
    };

    for (k, (op, original)) in emitted.iter_mut().enumerate() {
        let Some(i) = *original else {
            continue;
        };
        let old = &ops[i];
        if let Some(offset) = branch_offset(op) {
            // Relative to the end of the instruction.
            let target = map((old.pos + old.len) as i64 + *offset as i64)?;
            *offset = i32::try_from(target - new_pos[k + 1] as i64)?;
        } else if let Op::LookupSwitch(switch) = op {
            // Relative to the start of the instruction.
            let base_old = old.pos as i64;
            let base_new = new_pos[k] as i64;
            switch.default_offset =
                i32::try_from(map(base_old + switch.default_offset as i64)? - base_new)?;
            for offset in switch.case_offsets.iter_mut() {
                *offset = i32::try_from(map(base_old + *offset as i64)? - base_new)?;
            }
        }
    }
    for exception in exceptions.iter_mut() {
        exception.from_offset = u32::try_from(map(exception.from_offset as i64)?)?;
        exception.to_offset = u32::try_from(map(exception.to_offset as i64)?)?;
        exception.target_offset = u32::try_from(map(exception.target_offset as i64)?)?;
    }

    let mut code = Vec::with_capacity(new_len);
    let mut writer = Writer::new(&mut code);
    for (op, _) in &emitted {
        writer.write_op(op)?;
    }
    if code.len() != new_len {
        bail!(
            "relinked code is {} bytes, laid out as {new_len}",
            code.len()
        );
    }
    Ok(Some(code))
}

#[cfg(test)]
mod tests;
