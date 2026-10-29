//! Where Turu keeps its files (see docs/turu/ARCHITECTURE.md, "Data directory").

use crate::options::BotOptions;
use std::fs;
use std::io;
use std::path::PathBuf;

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

    pub fn options_file(&self) -> PathBuf {
        self.root.join("options.json")
    }

    pub fn advanced_skills_file(&self) -> PathBuf {
        self.root.join("AdvancedSkills.json")
    }

    /// The user's skill sets, seeded from VibeSkua's shipped file on first use.
    pub fn load_advanced_skills(&self) -> String {
        fs::read_to_string(self.advanced_skills_file())
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
