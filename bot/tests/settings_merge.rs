//! `settings.json` written by several processes (docs/TAB_PROCESSES_PROPOSAL.md, M5): each saves
//! what it changed without undoing the others'.

use turu_bot::paths::Paths;
use turu_bot::settings::AppSettings;

#[test]
fn two_writers_keep_each_others_changes() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths {
        root: dir.path().to_owned(),
    };
    let start = AppSettings::default();
    paths.save_settings(&start).unwrap();
    let loaded = paths.load_settings().unwrap();
    let base = serde_json::to_value(&loaded).unwrap();

    // Two processes read the same file, then each changes a different setting.
    let mut first = loaded.clone();
    first.grandpa_mode = !start.grandpa_mode;
    let mut second = loaded.clone();
    second.animation_frame_rate = start.animation_frame_rate + 7;

    let (after_first, first_base) = paths.merge_settings(&base, &first).unwrap();
    assert_eq!(after_first.grandpa_mode, first.grandpa_mode);
    let (after_second, _) = paths.merge_settings(&base, &second).unwrap();
    // The second kept the first's change, and took it up.
    assert_eq!(after_second.grandpa_mode, first.grandpa_mode);
    assert_eq!(
        after_second.animation_frame_rate,
        second.animation_frame_rate
    );
    let on_disk = paths.load_settings().unwrap();
    assert_eq!(on_disk.grandpa_mode, first.grandpa_mode);
    assert_eq!(on_disk.animation_frame_rate, second.animation_frame_rate);

    // The first saving again with nothing new doesn't undo the second's change.
    let (again, _) = paths.merge_settings(&first_base, &after_first).unwrap();
    assert_eq!(again.animation_frame_rate, second.animation_frame_rate);
}

#[test]
fn settings_a_newer_turu_wrote_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths {
        root: dir.path().to_owned(),
    };
    let mut value = serde_json::to_value(AppSettings::default()).unwrap();
    value["fromANewerTuru"] = serde_json::json!(42);
    std::fs::write(paths.settings_file(), value.to_string()).unwrap();
    let loaded = paths.load_settings().unwrap();
    let base = serde_json::to_value(&loaded).unwrap();
    let mut mine = loaded;
    mine.grandpa_mode = !mine.grandpa_mode;
    paths.merge_settings(&base, &mine).unwrap();
    let text = std::fs::read_to_string(paths.settings_file()).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(saved["fromANewerTuru"], 42);
}
