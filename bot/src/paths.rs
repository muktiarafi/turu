//! Where Turu keeps its files (see docs/turu/ARCHITECTURE.md, "Data directory").

use crate::options::BotOptions;
use serde_json::Value;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Once;

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
        let user_file = self.user_advanced_skills_file();
        let Ok(user_text) = fs::read_to_string(&user_file) else {
            return fs::write(&user_file, shipped);
        };
        let (Ok(Value::Object(defaults)), Ok(Value::Object(mut user))) = (
            serde_json::from_str::<Value>(shipped),
            serde_json::from_str::<Value>(&user_text),
        ) else {
            return Ok(());
        };
        let mut changed = false;
        for (class, modes) in defaults {
            let Value::Object(modes) = modes else {
                continue;
            };
            match user.get_mut(&class) {
                Some(Value::Object(user_modes)) => {
                    for (mode, set) in modes {
                        if !user_modes.contains_key(&mode) {
                            user_modes.insert(mode, set);
                            changed = true;
                        }
                    }
                }
                _ => {
                    user.insert(class, Value::Object(modes));
                    changed = true;
                }
            }
        }
        if changed {
            let text =
                serde_json::to_string_pretty(&Value::Object(user)).map_err(io::Error::other)?;
            fs::write(&user_file, text)?;
        }
        Ok(())
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
