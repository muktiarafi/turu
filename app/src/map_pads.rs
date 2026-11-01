//! Turu: reads which pads each cell of a map has from the map's SWF, for the Jump popup.
//!
//! A map SWF's root timeline is the game's `world.map`: each cell is a frame label, and the
//! pads are the named instances of `Pad_<n>` symbols on that timeline (VibeSkua's
//! `jumpCorrectRoom` finds them in the running game the same way, by class name). This walks the
//! root timeline's display list frame by frame and gives each cell the pads shown in any frame
//! from its label up to the next one.

use std::collections::HashMap;

/// Each cell (frame label) of the map and its pads, in timeline order.
pub fn scan(swf_data: &[u8]) -> Result<Vec<(String, Vec<String>)>, String> {
    let buf = swf::decompress_swf(swf_data).map_err(|e| format!("not a SWF: {e}"))?;
    let parsed = swf::parse_swf(&buf).map_err(|e| format!("not a SWF: {e}"))?;
    let mut pad_symbols = Vec::new();
    for tag in &parsed.tags {
        if let swf::Tag::SymbolClass(links) = tag {
            pad_symbols.extend(
                links
                    .iter()
                    .filter(|link| is_pad_class(&link.class_name.to_string_lossy(swf::UTF_8)))
                    .map(|link| link.id),
            );
        }
    }
    // Depth -> (symbol, instance name) on the root timeline.
    let mut display: HashMap<u16, (u16, Option<String>)> = HashMap::new();
    let mut cells: Vec<(String, Vec<String>)> = Vec::new();
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
                let Some((_, pads)) = cells.last_mut() else {
                    continue;
                };
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
    Ok(cells)
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
        let mut swf = Vec::new();
        swf::write_swf(&swf::Header::default_with_swf_version(10), &tags, &mut swf).unwrap();
        let strings = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            scan(&swf).unwrap(),
            [
                ("Enter".to_owned(), strings(&["Spawn"])),
                ("r2".to_owned(), strings(&["Left", "Tower"])),
                ("r3".to_owned(), strings(&[])),
            ]
        );
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
        let cells = scan(&data).unwrap();
        for (cell, pads) in &cells {
            println!("{cell}: {pads:?}");
        }
        assert!(!cells.is_empty());
    }
}
