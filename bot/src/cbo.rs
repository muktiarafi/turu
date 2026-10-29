//! The CoreBots Options file: `options/CBO_Storage(<username>).txt`, one `Key: Value` per
//! line. Options > CoreBots writes it (VibeSkua `CoreBotsViewModel`); CoreBots reads it when a
//! script starts (C# `ReadCBO`, through `bot.coreBotsOptions()`).

use std::fs;
use std::io;
use std::path::Path;

/// The old dodge and boss gear keys and their current names (C# `MigrateCBOKeys`).
const MIGRATIONS: [(&str, &str); 12] = [
    ("HelmDodgeSelect", "Helm3Select"),
    ("ArmorDodgeSelect", "Armor3Select"),
    ("CapeDodgeSelect", "Cape3Select"),
    ("WeaponDodgeSelect", "Weapon3Select"),
    ("PetDodgeSelect", "Pet3Select"),
    ("GroundItemDodgeSelect", "GroundItem3Select"),
    ("HelmBossSelect", "Helm4Select"),
    ("ArmorBossSelect", "Armor4Select"),
    ("CapeBossSelect", "Cape4Select"),
    ("WeaponBossSelect", "Weapon4Select"),
    ("PetBossSelect", "Pet4Select"),
    ("GroundItemBossSelect", "GroundItem4Select"),
];

/// The file's lines, after renaming old keys (the file is rewritten when one was renamed).
/// `None` when there is no file. `renamed` gets each `(old, new)` key renamed.
pub fn read_lines(
    path: &Path,
    mut renamed: impl FnMut(&str, &str),
) -> io::Result<Option<Vec<String>>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut changed = false;
    for line in &mut lines {
        if let Some((old, new)) = MIGRATIONS
            .iter()
            .find(|(old, _)| line.starts_with(&format!("{old}:")))
        {
            *line = line.replacen(old, new, 1);
            renamed(old, new);
            changed = true;
        }
    }
    if changed {
        write_lines(path, &lines)?;
    }
    Ok(Some(lines))
}

/// The window's reading of the file (VibeSkua `CoreBotsViewModel.ReadValues`): lines that split
/// on `:` into exactly two non-empty parts, trimmed.
pub fn parse(lines: &[String]) -> Vec<(String, String)> {
    lines
        .iter()
        .filter_map(|line| {
            let parts: Vec<&str> = line
                .split(':')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .collect();
            match parts[..] {
                [key, value] => Some((key.to_owned(), value.to_owned())),
                _ => None,
            }
        })
        .collect()
}

/// Writes `Key: Value` lines (VibeSkua `SaveInternal`).
pub fn write(path: &Path, entries: &[(String, String)]) -> io::Result<()> {
    let lines: Vec<String> = entries.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    write_lines(path, &lines)
}

fn write_lines(path: &Path, lines: &[String]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut text = lines.join("\n");
    text.push('\n');
    fs::write(path, text)
}
