//! The CoreBots Options file (VibeSkua `CBO_Storage(<user>).txt`).

use turu_bot::cbo;

fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("turu-cbo-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn missing_file_is_none() {
    let path = dir("missing").join("CBO_Storage(nobody).txt");
    assert!(cbo::read_lines(&path, |_, _| {}).unwrap().is_none());
}

#[test]
fn writes_and_reads_key_value_lines() {
    let path = dir("roundtrip").join("options").join("CBO_Storage(me).txt");
    let entries = vec![
        ("PrivateRooms".to_owned(), "True".to_owned()),
        ("SoloClassSelect".to_owned(), "Void Highlord".to_owned()),
        ("Helm1Select".to_owned(), String::new()),
    ];
    cbo::write(&path, &entries).unwrap();
    let lines = cbo::read_lines(&path, |_, _| {}).unwrap().unwrap();
    assert_eq!(
        lines,
        [
            "PrivateRooms: True",
            "SoloClassSelect: Void Highlord",
            "Helm1Select: "
        ]
    );
    // Like VibeSkua's ReadValues, an empty value isn't a value.
    assert_eq!(
        cbo::parse(&lines),
        [
            ("PrivateRooms".to_owned(), "True".to_owned()),
            ("SoloClassSelect".to_owned(), "Void Highlord".to_owned()),
        ]
    );
}

#[test]
fn renames_old_dodge_and_boss_keys() {
    let path = dir("migrate").join("CBO_Storage(me).txt");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "HelmDodgeSelect: Hat\nPetBossSelect: Dog\nAntiLag: True\n",
    )
    .unwrap();
    let mut renamed = Vec::new();
    let lines = cbo::read_lines(&path, |old, new| {
        renamed.push((old.to_owned(), new.to_owned()))
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        lines,
        ["Helm3Select: Hat", "Pet4Select: Dog", "AntiLag: True"]
    );
    assert_eq!(renamed.len(), 2);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "Helm3Select: Hat\nPet4Select: Dog\nAntiLag: True\n"
    );
}
