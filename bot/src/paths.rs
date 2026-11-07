//! Where Turu keeps its files (see docs/ARCHITECTURE.md, "Data directory").

use crate::options::BotOptions;
use crate::settings::AppSettings;
use serde_json::Value;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Once, OnceLock};

static DEFAULT_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// The data folder to use in place of the system's (Android has none: the app passes its files
/// folder). Set it before the first bot is made; `TURU_DATA_DIR` still wins. False when it was
/// already set.
pub fn set_default_root(root: PathBuf) -> bool {
    DEFAULT_ROOT.set(root).is_ok()
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Default for Paths {
    fn default() -> Self {
        Self::new()
    }
}

impl Paths {
    pub fn new() -> Self {
        let root = std::env::var_os("TURU_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| DEFAULT_ROOT.get().cloned())
            .or_else(|| dirs::data_dir().map(|d| d.join("Turu")))
            .unwrap_or_else(|| PathBuf::from("Turu"));
        Self { root }
    }

    pub fn ensure(&self) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(self.scripts_dir())?;
        fs::create_dir_all(self.script_options_dir())?;
        Ok(())
    }

    /// Reports of how earlier runs ended badly (`turu_frontend::crash_reports`): a panic's, and
    /// on Android the system's record of an exit Turu didn't ask for.
    pub fn crash_dir(&self) -> PathBuf {
        self.root.join("crash")
    }

    /// Where client updates are downloaded and unpacked (`crate::updates`), emptied before each.
    pub fn updates_dir(&self) -> PathBuf {
        self.root.join("updates")
    }

    /// The control API's token (`control_api::saved_token`), readable by you only.
    pub fn api_token_file(&self) -> PathBuf {
        self.root.join("api-token")
    }

    /// The web monitor's access key (`monitor::server`), readable by you only.
    pub fn monitor_key_file(&self) -> PathBuf {
        self.root.join("monitor-key")
    }

    /// The key other machines report to the web monitor with, readable by you only.
    pub fn monitor_push_key_file(&self) -> PathBuf {
        self.root.join("monitor-push-key")
    }

    /// The web monitor's bots as last seen (`monitor::Hub`), shown offline after a restart.
    pub fn monitor_file(&self) -> PathBuf {
        self.root.join("monitor.json")
    }

    /// The account list (`accounts::Accounts`); passwords are in the system keychain.
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }

    /// Every account's gear (`gear::GearFile`), written by its client.
    pub fn gear_dir(&self) -> PathBuf {
        self.root.join("gear")
    }

    /// Each account's loadouts (`loadouts::Loadout`), VibeSkua's `Loadouts/<username>.json`.
    pub fn loadouts_dir(&self) -> PathBuf {
        self.root.join("loadouts")
    }

    pub fn skua_swf(&self) -> PathBuf {
        self.root.join("skua.swf")
    }

    /// Writes the embedded `skua.swf` to the data directory and returns its path.
    pub fn write_skua_swf(&self) -> io::Result<PathBuf> {
        self.ensure()?;
        let path = self.skua_swf();
        if fs::read(&path).ok().as_deref() != Some(crate::SKUA_SWF) {
            fs::write(&path, crate::SKUA_SWF)?;
        }
        Ok(path)
    }

    pub fn scripts_dir(&self) -> PathBuf {
        self.root.join("scripts")
    }

    /// Writes `types/turu.ts` into the scripts folder for code editors: the script API this app
    /// runs, rewritten whenever it differs. The repository's `tsconfig.json`, downloaded with
    /// the scripts (`repo::ScriptRepo`), maps `"turu"` to it; without both, editors report
    /// "Cannot find module 'turu'".
    pub fn write_script_types(&self) -> io::Result<()> {
        let dir = self.scripts_dir();
        fs::create_dir_all(dir.join("types"))?;
        let types = dir.join("types").join("turu.ts");
        if fs::read_to_string(&types).ok().as_deref() != Some(crate::TURU_TS) {
            fs::write(&types, crate::TURU_TS)?;
        }
        Ok(())
    }

    pub fn script_options_dir(&self) -> PathBuf {
        self.root.join("options")
    }

    /// The CoreBots Options file of `username` (VibeSkua `CBO_Storage(<user>).txt`).
    pub fn cbo_file(&self, username: &str) -> PathBuf {
        self.script_options_dir()
            .join(format!("CBO_Storage({username}).txt"))
    }

    /// The saved values of a script's options (VibeSkua `<Storage>.cfg`).
    pub fn script_options_file(&self, storage: &str) -> PathBuf {
        self.script_options_dir().join(format!("{storage}.cfg"))
    }

    /// Tools & Helpers > Fast Travel's destinations (VibeSkua keeps them in its settings,
    /// `FastTravels`).
    pub fn fast_travels_file(&self) -> PathBuf {
        self.root.join("FastTravels.json")
    }

    /// Options > HotKeys' bindings (VibeSkua keeps them in its settings, `HotKeys`, as
    /// `"Action|Gesture"` strings; so does this file, as a JSON array).
    pub fn hotkeys_file(&self) -> PathBuf {
        self.root.join("HotKeys.json")
    }

    pub fn options_file(&self) -> PathBuf {
        self.root.join("options.json")
    }

    /// The shipped skill sets (VibeSkua `AdvancedSkills.json`), rewritten from the copy built
    /// into the app whenever it differs.
    pub fn default_advanced_skills_file(&self) -> PathBuf {
        self.root.join("AdvancedSkills.json")
    }

    /// The user's skill sets (VibeSkua `UserAdvancedSkills.json`): what the bot and the
    /// Skills window use.
    pub fn user_advanced_skills_file(&self) -> PathBuf {
        self.root.join("UserAdvancedSkills.json")
    }

    /// VibeSkua `AdvancedSkillContainer`: seeds the user's sets from the shipped ones, and
    /// adds the shipped classes and modes the user's file lacks without changing the ones it
    /// has. An unreadable user file is left alone.
    pub fn sync_advanced_skills(&self) -> io::Result<()> {
        self.ensure()?;
        let shipped = crate::DEFAULT_ADVANCED_SKILLS;
        let default_file = self.default_advanced_skills_file();
        if fs::read_to_string(&default_file).ok().as_deref() != Some(shipped) {
            fs::write(&default_file, shipped)?;
        }
        self.merge_advanced_skills(shipped).map(drop)
    }

    /// VibeSkua `RefreshDefaultSkillsAsync` after `UpdateSkillSetsFile`: `published` (the
    /// community `AdvancedSkills.json`) becomes the default file and its new classes and modes
    /// are added to the user's. Returns how many sets were added. Nothing changes when
    /// `published` isn't a JSON object.
    pub fn update_advanced_skills(&self, published: &str) -> io::Result<usize> {
        if !matches!(
            serde_json::from_str::<Value>(published),
            Ok(Value::Object(_))
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "AdvancedSkills.json did not contain a JSON object",
            ));
        }
        // Seeds the user's file first, so this run's lazy sync doesn't run after it.
        self.load_advanced_skills();
        self.ensure()?;
        write_atomically(&self.default_advanced_skills_file(), published)?;
        self.merge_advanced_skills(published)
    }

    /// Adds the classes and modes of `defaults` that the user's file lacks, keeping the ones it
    /// has; writes `defaults` when there is no user file. Returns how many sets were added
    /// (0 for an unreadable user file, which is left alone).
    fn merge_advanced_skills(&self, defaults: &str) -> io::Result<usize> {
        let user_file = self.user_advanced_skills_file();
        let Ok(user_text) = fs::read_to_string(&user_file) else {
            write_atomically(&user_file, defaults)?;
            return Ok(count_sets(defaults));
        };
        let (Ok(Value::Object(defaults)), Ok(Value::Object(mut user))) = (
            serde_json::from_str::<Value>(defaults),
            serde_json::from_str::<Value>(&user_text),
        ) else {
            return Ok(0);
        };
        let (mut added, mut changed) = (0, false);
        for (class, modes) in defaults {
            let Value::Object(modes) = modes else {
                continue;
            };
            match user.get_mut(&class) {
                Some(Value::Object(user_modes)) => {
                    for (mode, set) in modes {
                        if !user_modes.contains_key(&mode) {
                            user_modes.insert(mode, set);
                            added += 1;
                            changed = true;
                        }
                    }
                }
                _ => {
                    added += modes.len();
                    changed = true;
                    user.insert(class, Value::Object(modes));
                }
            }
        }
        if changed {
            let text =
                serde_json::to_string_pretty(&Value::Object(user)).map_err(io::Error::other)?;
            write_atomically(&user_file, &text)?;
        }
        Ok(added)
    }

    /// VibeSkua `ResetSkillsSets`: replaces the user's sets with the shipped ones.
    pub fn reset_advanced_skills(&self) -> io::Result<()> {
        self.ensure()?;
        let shipped = crate::DEFAULT_ADVANCED_SKILLS;
        fs::write(self.default_advanced_skills_file(), shipped)?;
        fs::write(self.user_advanced_skills_file(), shipped)
    }

    /// The user's skill sets as JSON (synced with the shipped ones once per run).
    pub fn load_advanced_skills(&self) -> String {
        static SYNCED: Once = Once::new();
        SYNCED.call_once(|| {
            if let Err(e) = self.sync_advanced_skills() {
                tracing::warn!(
                    "Couldn't set up the skill sets in {}: {e}",
                    self.root.display()
                );
            }
        });
        fs::read_to_string(self.user_advanced_skills_file())
            .unwrap_or_else(|_| crate::DEFAULT_ADVANCED_SKILLS.to_owned())
    }

    /// Options > Application (`AppSettings`).
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// Combat > Stats' rate snapshots, shared by every client (`stats::load_history`; VibeSkua
    /// `StatsRateHistory` in its settings).
    pub fn stats_rate_history_file(&self) -> PathBuf {
        self.root.join("StatsRateHistory.json")
    }

    /// Tools > Loader's quest cache (VibeSkua `QuestData.json`, `quest_cache`).
    pub fn quest_data_file(&self) -> PathBuf {
        self.root.join(crate::quest_cache::FILE_NAME)
    }

    /// Tools > Junk Items' sell list (VibeSkua `JunkItems.json`, `junk`).
    pub fn junk_items_file(&self) -> PathBuf {
        self.root.join(crate::junk::FILE_NAME)
    }

    /// The map items found in each map's SWF, by map file (`map_items`; VibeSkua
    /// `cache/0SavedMaps.json`).
    pub fn map_items_file(&self) -> PathBuf {
        self.root.join("map-items.json")
    }

    /// Butler's sync files, one per account (`butler::sync`).
    pub fn butler_dir(&self) -> PathBuf {
        self.root.join("butler")
    }

    /// Butler's saved and auto-saved debug logs (VibeSkua `butlerv5_logs`).
    pub fn butler_logs_dir(&self) -> PathBuf {
        self.root.join("butler_logs")
    }

    /// Where the Scheduler's Save and Load dialogs open (its playlists, VibeSkua's JSON).
    pub fn playlists_dir(&self) -> PathBuf {
        self.root.join("playlists")
    }

    /// CoreArmyLite's sync logs (`bot.army.appendLog`), shared by every client.
    pub fn army_dir(&self) -> PathBuf {
        self.root.join("army")
    }

    /// Cosmetics' catalog, favorites and outfits (`cosmetics::Library`), shared by every client.
    pub fn cosmetics_dir(&self) -> PathBuf {
        self.root.join("cosmetics")
    }

    /// Butler's options (`butler::ButlerSettings`), shared by every client.
    pub fn butler_settings_file(&self) -> PathBuf {
        self.root.join("butler.json")
    }

    pub fn load_settings(&self) -> Option<AppSettings> {
        let text = fs::read_to_string(self.settings_file()).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save_settings(&self, settings: &AppSettings) -> io::Result<()> {
        self.ensure()?;
        let text = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
        fs::write(self.settings_file(), text)
    }

    pub fn load_options(&self) -> Option<BotOptions> {
        let text = fs::read_to_string(self.options_file()).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save_options(&self, options: &BotOptions) -> io::Result<()> {
        self.ensure()?;
        let text = serde_json::to_string_pretty(options).map_err(io::Error::other)?;
        fs::write(self.options_file(), text)
    }
}

/// Writes `text` to a temporary file next to `path`, then renames it over `path`, so a reader
/// (or another Turu on the same data folder) never sees half a file.
pub(crate) fn write_atomically(path: &std::path::Path, text: &str) -> io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(format!(".{}.tmp", std::process::id()));
    let temp = PathBuf::from(temp);
    fs::write(&temp, text)?;
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

/// The skill sets in an `AdvancedSkills.json` (classes times modes).
fn count_sets(json: &str) -> usize {
    match serde_json::from_str::<Value>(json) {
        Ok(Value::Object(classes)) => classes
            .values()
            .map(|modes| modes.as_object().map_or(0, |m| m.len()))
            .sum(),
        _ => 0,
    }
}
