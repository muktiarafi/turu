//! Turu: reads a map's cells, their pads and the doors between them from the map's SWF, for
//! the Jump popup.
//!
//! A map SWF's root timeline is the game's `world.map`: each cell is a frame label, and the
//! pads are the named instances of `Pad_<n>` symbols on that timeline (VibeSkua's
//! `jumpCorrectRoom` finds them in the running game the same way, by class name). This walks the
//! root timeline's display list frame by frame and gives each cell the pads shown in any frame
//! from its label up to the next one.
//!
//! The doors are instances of the game's exit component: walking into one moves you to its
//! `tCell` / `tPad` (or `altCell` / `altPad`, when its quest condition isn't met). Flash sets
//! those component parameters in generated `__setProp_...` functions (`__id4_.tCell = "Side1"`),
//! which this reads out of the AS3 bytecode; the door's cell is the cell its instance is
//! shown in.

use crate::map_items::{decode, name};
use std::collections::{HashMap, VecDeque};
use swf::avm2::read::Reader;
use swf::avm2::types::Op;

/// What a map SWF says about its cells.
#[derive(Debug, Default, PartialEq)]
pub struct MapCells {
    /// Each cell (frame label) and its pads, in timeline order.
    pub cells: Vec<(String, Vec<String>)>,
    /// The doors between cells.
    pub doors: Vec<Door>,
}

/// A door in `cell` that leads to `to_cell`, landing on `to_pad`.
#[derive(Debug, PartialEq)]
pub struct Door {
    pub cell: String,
    pub to_cell: String,
    pub to_pad: String,
}

impl MapCells {
    /// The pads the SWF shows in `cell`, when it has some there.
    pub fn pads(&self, cell: &str) -> Option<&[String]> {
        self.cells
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(cell))
            .map(|(_, pads)| pads.as_slice())
            .filter(|pads| !pads.is_empty())
    }

    /// The pad you'd arrive on walking from `from` to `to`: the pad of the door into `to` at the
    /// end of the shortest walk through the doors, when there is one.
    pub fn arrival_pad(&self, from: &str, to: &str) -> Option<&str> {
        if from.eq_ignore_ascii_case(to) {
            return None;
        }
        let mut reached: Vec<(&str, &str)> = vec![(from, "")];
        let mut queue = VecDeque::from([from]);
        while let Some(cell) = queue.pop_front() {
            for door in self.doors.iter().filter(|d| d.cell.eq_ignore_ascii_case(cell)) {
                if reached
                    .iter()
                    .any(|(known, _)| known.eq_ignore_ascii_case(&door.to_cell))
                {
                    continue;
                }
                if door.to_cell.eq_ignore_ascii_case(to) {
                    return Some(door.to_pad.as_str()).filter(|pad| !pad.is_empty());
                }
                reached.push((&door.to_cell, &door.to_pad));
                queue.push_back(&door.to_cell);
            }
        }
        None
    }

    /// The pad to jump to in `cell`, coming from the cell `from`: where the walk from there
    /// arrives, when the cell has that pad; else [`pick`]'s choice. `None` when the SWF shows no
    /// pads in `cell`.
    pub fn jump_pad(&self, cell: &str, from: Option<&str>, wanted: &str) -> Option<String> {
        let pads = self.pads(cell)?;
        let arrival = from
            .and_then(|from| self.arrival_pad(from, cell))
            .filter(|pad| pads.iter().any(|p| p == pad));
        Some(arrival.map_or_else(|| pick(pads, cell, wanted), str::to_owned))
    }
}

/// Where a door instance leads (its component parameters).
#[derive(Default)]
struct Exit {
    cell: Option<String>,
    pad: Option<String>,
    alt_cell: Option<String>,
    alt_pad: Option<String>,
}

/// The map's [`MapCells`].
pub fn scan(swf_data: &[u8]) -> Result<MapCells, String> {
    let buf = swf::decompress_swf(swf_data).map_err(|e| format!("not a SWF: {e}"))?;
    let parsed = swf::parse_swf(&buf).map_err(|e| format!("not a SWF: {e}"))?;
    let mut pad_symbols = Vec::new();
    let mut exits: HashMap<String, Exit> = HashMap::new();
    for tag in &parsed.tags {
        match tag {
            swf::Tag::SymbolClass(links) => pad_symbols.extend(
                links
                    .iter()
                    .filter(|link| is_pad_class(&link.class_name.to_string_lossy(swf::UTF_8)))
                    .map(|link| link.id),
            ),
            swf::Tag::DoAbc(data) => read_exits(data, &mut exits),
            swf::Tag::DoAbc2(do_abc) => read_exits(do_abc.data, &mut exits),
            _ => {}
        }
    }
    // Depth -> (symbol, instance name) on the root timeline.
    let mut display: HashMap<u16, (u16, Option<String>)> = HashMap::new();
    let mut cells: Vec<(String, Vec<String>)> = Vec::new();
    // Each named instance's cells (by lower-case name, as the bytecode reader gives them).
    let mut shown_in: HashMap<String, Vec<usize>> = HashMap::new();
    for tag in &parsed.tags {
        match tag {
            swf::Tag::FrameLabel(label) => {
                cells.push((label.label.to_string_lossy(swf::UTF_8), Vec::new()));
            }
            swf::Tag::PlaceObject(place) => {
                let name = place.name.map(|n| n.to_string_lossy(swf::UTF_8));
                match place.action {
                    swf::PlaceObjectAction::Place(id) | swf::PlaceObjectAction::Replace(id) => {
                        let name = name.or_else(|| display.get(&place.depth).and_then(|(_, n)| n.clone()));
                        display.insert(place.depth, (id, name));
                    }
                    swf::PlaceObjectAction::Modify => {
                        if let (Some(name), Some(entry)) = (name, display.get_mut(&place.depth)) {
                            entry.1 = Some(name);
                        }
                    }
                }
            }
            swf::Tag::RemoveObject(remove) => {
                display.remove(&remove.depth);
            }
            swf::Tag::ShowFrame => {
                let Some(cell) = cells.len().checked_sub(1) else {
                    continue;
                };
                for name in display.values().filter_map(|(_, name)| name.as_ref()) {
                    let seen = shown_in.entry(name.to_lowercase()).or_default();
                    if !seen.contains(&cell) {
                        seen.push(cell);
                    }
                }
                let pads = &mut cells[cell].1;
                let mut shown: Vec<(u16, &String)> = display
                    .iter()
                    .filter(|(_, (id, _))| pad_symbols.contains(id))
                    .filter_map(|(depth, (_, name))| Some((*depth, name.as_ref().filter(|n| !n.is_empty())?)))
                    .collect();
                shown.sort();
                for (_, name) in shown {
                    if !pads.contains(name) {
                        pads.push(name.clone());
                    }
                }
            }
            _ => {}
        }
    }
    let mut doors = Vec::new();
    let mut names: Vec<&String> = exits.keys().collect();
    names.sort();
    for name in names {
        let exit = &exits[name];
        for &cell in shown_in.get(name).into_iter().flatten() {
            let targets = [(&exit.cell, &exit.pad), (&exit.alt_cell, &exit.alt_pad)];
            for (to_cell, to_pad) in targets {
                let Some(to_cell) = to_cell.clone().filter(|c| !c.is_empty()) else {
                    continue;
                };
                let door = Door {
                    cell: cells[cell].0.clone(),
                    to_cell,
                    to_pad: to_pad.clone().unwrap_or_default(),
                };
                if !doors.contains(&door) {
                    doors.push(door);
                }
            }
        }
    }
    doors.sort_by_key(|door| cells.iter().position(|(name, _)| *name == door.cell));
    Ok(MapCells { cells, doors })
}

/// Reads the doors' component parameters out of one ABC block: `<instance>.tCell = "<cell>"`
/// and the like, a string constant set on a named instance.
fn read_exits(abc: &[u8], exits: &mut HashMap<String, Exit>) {
    let Ok(file) = Reader::new(abc).read() else {
        return;
    };
    let pool = &file.constant_pool;
    let string = |index: u32| {
        pool.strings
            .get((index as usize).wrapping_sub(1))
            .map(|s| String::from_utf8_lossy(s).into_owned())
    };
    for body in &file.method_bodies {
        let ops = decode(&body.code);
        for window in ops.windows(3) {
            let [
                Op::GetLex { index: object } | Op::GetProperty { index: object },
                Op::PushString { value },
                Op::SetProperty { index: property },
            ] = window
            else {
                continue;
            };
            let property = name(pool, property.0);
            if !matches!(property.as_str(), "tcell" | "tpad" | "altcell" | "altpad") {
                continue;
            }
            let exit = exits.entry(name(pool, object.0)).or_default();
            let slot = match property.as_str() {
                "tcell" => &mut exit.cell,
                "tpad" => &mut exit.pad,
                "altcell" => &mut exit.alt_cell,
                _ => &mut exit.alt_pad,
            };
            if slot.is_none() {
                *slot = string(value.0);
            }
        }
    }
}

/// An uncompressed SWF file of a movie Ruffle loaded (its header and decompressed tags), for
/// the scanners, which read SWF files.
pub fn swf_file(header: &swf::HeaderExt, tags: &[u8]) -> Option<Vec<u8>> {
    let header = swf::Header {
        compression: swf::Compression::None,
        ..header.swf_header().clone()
    };
    let mut swf = Vec::with_capacity(tags.len() + 32);
    swf::write::write_swf_raw_tags(&header, tags, &mut swf).ok()?;
    Some(swf)
}

/// The pad to jump to in a cell with `pads`: "Spawn" for "Enter", else `wanted` when the cell has
/// it, else "Left", else its first pad (VibeSkua `jumpCorrectRoom`'s choice).
pub fn pick(pads: &[String], cell: &str, wanted: &str) -> String {
    let has = |pad: &str| pads.iter().any(|p| p == pad);
    let pad = if cell.eq_ignore_ascii_case("Enter") && has("Spawn") {
        "Spawn"
    } else if has(wanted) {
        wanted
    } else if has("Left") {
        "Left"
    } else {
        pads.first().map_or("Left", String::as_str)
    };
    pad.to_owned()
}

/// `town_fla.Pad_70` and the like (the game sees `town_fla::Pad_70`; VibeSkua's
/// `/::Pad_\d+$/`), package optional.
fn is_pad_class(class: &str) -> bool {
    let name = class.rsplit([':', '.']).next().unwrap_or(class);
    name.strip_prefix("Pad_")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use swf::{FrameLabel, PlaceObject, PlaceObjectAction, RemoveObject, SwfStr, SymbolClassLink, Tag};

    fn place(depth: u16, action: PlaceObjectAction, name: Option<&'static str>) -> Tag<'static> {
        Tag::PlaceObject(Box::new(PlaceObject {
            version: 2,
            action,
            depth,
            matrix: None,
            color_transform: None,
            ratio: None,
            name: name.map(SwfStr::from_utf8_str),
            clip_depth: None,
            class_name: None,
            filters: None,
            background_color: None,
            blend_mode: None,
            clip_actions: None,
            has_image: false,
            is_bitmap_cached: None,
            is_visible: None,
            amf_data: None,
        }))
    }

    fn label(name: &'static str) -> Tag<'static> {
        Tag::FrameLabel(FrameLabel {
            label: SwfStr::from_utf8_str(name),
            is_anchor: false,
        })
    }

    fn remove(depth: u16) -> Tag<'static> {
        Tag::RemoveObject(RemoveObject {
            depth,
            character_id: None,
        })
    }

    #[test]
    fn finds_each_cells_pads() {
        use PlaceObjectAction::{Modify, Place};
        let tags = [
            label("Enter"),
            place(1, Place(5), Some("Spawn")),
            place(2, Place(6), Some("Left")), // not a pad symbol
            Tag::ShowFrame,
            label("r2"),
            remove(1),
            place(3, Place(5), Some("Left")),
            Tag::ShowFrame,
            // The cell's later frames add pads; moving one keeps its name.
            place(3, Modify, None),
            place(4, Place(5), Some("Tower")),
            Tag::ShowFrame,
            label("r3"),
            remove(3),
            remove(4),
            Tag::ShowFrame,
            Tag::SymbolClass(vec![
                SymbolClassLink {
                    id: 5,
                    class_name: SwfStr::from_utf8_str("town_fla.Pad_70"),
                },
                SymbolClassLink {
                    id: 6,
                    class_name: SwfStr::from_utf8_str("town_fla.Padding_1"),
                },
            ]),
        ];
        let mut compressed = Vec::new();
        let header = swf::Header {
            compression: swf::Compression::Zlib,
            ..swf::Header::default_with_swf_version(10)
        };
        swf::write_swf(&header, &tags, &mut compressed).unwrap();
        // As the game loaded it: Ruffle's decompressed tags, written back out.
        let loaded = swf::decompress_swf(&compressed[..]).unwrap();
        let swf = swf_file(&loaded.header, &loaded.data).unwrap();
        assert_eq!(&swf[..3], b"FWS");
        assert_eq!(scan(&swf).unwrap(), scan(&compressed).unwrap());
        let strings = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            scan(&swf).unwrap().cells,
            [
                ("Enter".to_owned(), strings(&["Spawn"])),
                ("r2".to_owned(), strings(&["Left", "Tower"])),
                ("r3".to_owned(), strings(&[])),
            ]
        );
    }

    /// An ABC block with one function running `ops`, its multinames being `names` (index 1 =
    /// `names[0]`) and its strings `names` too (index 2 = `names[0]`).
    fn abc(names: &[&str], ops: &[Op]) -> Vec<u8> {
        use swf::avm2::types::*;
        let mut strings: Vec<Vec<u8>> = vec![b"".to_vec()];
        strings.extend(names.iter().map(|n| n.as_bytes().to_vec()));
        let mut code = Vec::new();
        let mut writer = swf::avm2::write::Writer::new(&mut code);
        for op in ops {
            writer.write_op(op).unwrap();
        }
        let file = AbcFile {
            major_version: 46,
            minor_version: 16,
            constant_pool: ConstantPool {
                ints: vec![],
                uints: vec![],
                doubles: vec![],
                strings,
                namespaces: vec![Namespace::Package(Index::new(1))],
                namespace_sets: vec![],
                multinames: (0..names.len())
                    .map(|i| Multiname::QName {
                        namespace: Index::new(1),
                        name: Index::new(i as u32 + 2),
                    })
                    .collect(),
            },
            methods: vec![Method {
                name: Index::new(0),
                params: vec![],
                return_type: Index::new(0),
                flags: MethodFlags::empty(),
                body: None,
            }],
            metadata: vec![],
            instances: vec![],
            classes: vec![],
            scripts: vec![Script {
                init_method: Index::new(0),
                traits: vec![],
            }],
            method_bodies: vec![MethodBody {
                method: Index::new(0),
                max_stack: 2,
                num_locals: 1,
                init_scope_depth: 0,
                max_scope_depth: 1,
                code,
                exceptions: vec![],
                traits: vec![],
            }],
        };
        let mut bytes = Vec::new();
        swf::avm2::write::Writer::new(&mut bytes).write(file).unwrap();
        bytes
    }

    #[test]
    fn finds_the_doors_and_their_cells() {
        use PlaceObjectAction::Place;
        use swf::avm2::types::Index;
        // __id0_.tCell = "r2"; __id0_.tPad = "Left"; __id1_.tCell = "Enter"; __id1_.tPad = "Right";
        // __id1_.altCell = "r3" (no pad); __id1_.intValue = "x" (not a door parameter).
        const NAMES: [&str; 12] = [
            "__id0_", "__id1_", "tCell", "tPad", "altCell", "intValue", "r2", "Left", "Enter", "Right", "r3", "x",
        ];
        let set = |object: u32, property: u32, value: u32| {
            [
                Op::GetLex {
                    index: Index::new(object),
                },
                Op::PushString {
                    value: Index::new(value + 1),
                },
                Op::SetProperty {
                    index: Index::new(property),
                },
            ]
        };
        let ops: Vec<Op> = [
            set(1, 3, 7),
            set(1, 4, 8),
            set(2, 3, 9),
            set(2, 4, 10),
            set(2, 5, 11),
            set(2, 6, 12),
        ]
        .into_iter()
        .flatten()
        .chain([Op::ReturnVoid])
        .collect();
        let code = abc(&NAMES, &ops);
        let tags = [
            Tag::DoAbc(&code),
            label("Enter"),
            place(1, Place(5), Some("__id0_")),
            Tag::ShowFrame,
            label("r2"),
            remove(1),
            place(2, Place(5), Some("__id1_")),
            Tag::ShowFrame,
            label("r3"),
            remove(2),
            Tag::ShowFrame,
        ];
        let mut swf = Vec::new();
        swf::write_swf(&swf::Header::default_with_swf_version(10), &tags, &mut swf).unwrap();
        let door = |cell: &str, to_cell: &str, to_pad: &str| Door {
            cell: cell.into(),
            to_cell: to_cell.into(),
            to_pad: to_pad.into(),
        };
        assert_eq!(
            scan(&swf).unwrap().doors,
            [
                door("Enter", "r2", "Left"),
                door("r2", "Enter", "Right"),
                door("r2", "r3", "")
            ]
        );
    }

    #[test]
    fn lands_where_the_walk_from_the_origin_arrives() {
        let strings = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let door = |cell: &str, to_cell: &str, to_pad: &str| Door {
            cell: cell.into(),
            to_cell: to_cell.into(),
            to_pad: to_pad.into(),
        };
        // Enter <-> r2 <-> r3, and r3 -> r4 through a door that names no pad.
        let map = MapCells {
            cells: vec![
                ("Enter".into(), strings(&["Spawn", "Right"])),
                ("r2".into(), strings(&["Left", "Right"])),
                ("r3".into(), strings(&["Left", "Top"])),
                ("r4".into(), strings(&["Left", "Down"])),
            ],
            doors: vec![
                door("Enter", "r2", "Left"),
                door("r2", "Enter", "Right"),
                door("r2", "r3", "Top"),
                door("r3", "r2", "Right"),
                door("r3", "r4", ""),
            ],
        };
        assert_eq!(map.arrival_pad("Enter", "r2"), Some("Left"));
        assert_eq!(map.arrival_pad("r3", "r2"), Some("Right"));
        // Not next door: the last step of the walk.
        assert_eq!(map.arrival_pad("enter", "R3"), Some("Top"));
        assert_eq!(map.arrival_pad("r2", "r4"), None);
        assert_eq!(map.arrival_pad("r4", "r2"), None);
        assert_eq!(map.arrival_pad("r2", "r2"), None);

        assert_eq!(map.jump_pad("r2", Some("r3"), "Left").as_deref(), Some("Right"));
        assert_eq!(map.jump_pad("Enter", Some("r2"), "").as_deref(), Some("Right"));
        // No walk, or no origin: the usual choice.
        assert_eq!(map.jump_pad("r4", Some("r2"), "Down").as_deref(), Some("Down"));
        assert_eq!(map.jump_pad("Enter", None, "").as_deref(), Some("Spawn"));
        assert_eq!(map.jump_pad("r5", Some("r2"), ""), None);
    }

    #[test]
    fn picks_a_pad_the_cell_has() {
        let pads = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(pick(&pads(&["Spawn", "Left"]), "Enter", "Right"), "Spawn");
        assert_eq!(pick(&pads(&["Center", "Right", "Left"]), "r2", "Right"), "Right");
        assert_eq!(pick(&pads(&["Center", "Right", "Left"]), "r2", ""), "Left");
        assert_eq!(pick(&pads(&["Bottom", "Top"]), "Slugfit", "Left"), "Bottom");
    }

    /// A real map, when one is given: `TURU_MAP_SWF=path cargo test ... map_pads -- --nocapture`.
    #[test]
    fn real_map() {
        let Some(path) = std::env::var_os("TURU_MAP_SWF") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let map = scan(&data).unwrap();
        let loaded = swf::decompress_swf(&data[..]).unwrap();
        assert_eq!(scan(&swf_file(&loaded.header, &loaded.data).unwrap()).unwrap(), map);
        for (cell, pads) in &map.cells {
            let doors: Vec<String> = map
                .doors
                .iter()
                .filter(|d| d.cell == *cell)
                .map(|d| format!("{} ({})", d.to_cell, d.to_pad))
                .collect();
            println!("{cell}: {pads:?} -> {doors:?}");
        }
        assert!(!map.cells.is_empty());
    }
}
