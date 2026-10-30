//! A script's options (VibeSkua `ScriptOption`, `ScriptOptionContainer`).
//!
//! A script declares them with `export const scriptOptions = defineScriptOptions(storage, [...])`
//! (bot/js/turu.ts). On load the engine reads that export with [`declaration`], without running
//! `main`, so the Script Options window can show them before the script starts. The values are
//! saved in `options/<storage>.cfg` in VibeSkua's format (`Options:Name=Value` lines), so a file
//! from VibeSkua's options folder can be copied over.

use super::loader::{ScriptLoader, ScriptResolver, normalize};
use rquickjs::{CatchResultExt, Context, Function, Module, Object, Runtime};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long reading a script's declaration may take. A top-level loop is stopped after this.
const READ_LIMIT: Duration = Duration::from_secs(5);

/// The kinds of option: a checkbox, a whole number, or text (VibeSkua's `bool`, `int`, `string`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Bool,
    Int,
    Text,
}

impl Kind {
    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::Bool => value.is_boolean(),
            Self::Int => value.is_i64(),
            Self::Text => value.is_string(),
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Self::Bool => "true or false",
            Self::Int => "a whole number",
            Self::Text => "text",
        }
    }
}

/// One option as the script declares it (`option.bool(...)`, `option.int(...)`, `option.text(...)`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionDecl {
    /// The key `bot.config.get` reads (VibeSkua `Name`).
    pub name: String,
    /// The label in the window (VibeSkua `DisplayName`).
    pub display_name: String,
    /// The text under the label (VibeSkua `Description`).
    pub description: String,
    pub kind: Kind,
    pub default_value: Value,
}

/// A script's options: the file they're saved in (VibeSkua `OptionsStorage`) and their list.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Declaration {
    pub storage: String,
    pub options: Vec<OptionDecl>,
    /// Don't show the Script Options window before each run (VibeSkua `DontPreconfigure`):
    /// the script opens it itself (CoreBots' `SkipOption`).
    #[serde(default)]
    pub dont_preconfigure: bool,
}

impl Declaration {
    /// Checks what the script declared: the storage must be a plain file name, option names must
    /// be unique and not empty, and each default must be of its option's kind.
    pub fn check(&self) -> Result<(), String> {
        let storage = self.storage.as_str();
        let plain_name = !storage.is_empty()
            && !storage.starts_with('.')
            && storage
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')'));
        if !plain_name {
            return Err(format!(
                "the storage \"{storage}\" must be a plain file name"
            ));
        }
        let mut names = HashSet::new();
        for option in &self.options {
            if option.name.is_empty() || !names.insert(option.name.as_str()) {
                return Err(format!(
                    "the option name \"{}\" is empty or used twice",
                    option.name
                ));
            }
            if !option.kind.accepts(&option.default_value) {
                return Err(format!(
                    "the default of \"{}\" must be {}",
                    option.name,
                    option.kind.describe()
                ));
            }
        }
        Ok(())
    }

    fn option(&self, name: &str) -> Option<&OptionDecl> {
        self.options.iter().find(|o| o.name == name)
    }
}

/// Reads the `scriptOptions` export of the script at `path`. `Ok(None)` when it declares none.
///
/// Only the module's top level runs, in a separate runtime. A bot call there fails (the script
/// has to call the bot from `main`), so a script that does it reports an error here.
pub fn declaration(path: &Path) -> Result<Option<Declaration>, String> {
    let failed = |e: rquickjs::Error| e.to_string();
    let runtime = Runtime::new().map_err(failed)?;
    runtime.set_loader(ScriptResolver, ScriptLoader);
    let deadline = Instant::now() + READ_LIMIT;
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let context = Context::full(&runtime).map_err(failed)?;
    let entry = normalize(path).to_string_lossy().into_owned();

    let text = context.with(|ctx| -> Result<Option<String>, String> {
        let turu = Function::new(ctx.clone(), |_method: String, _args: String| -> String {
            json!({ "error": "bot calls can't run while the script's options are read" })
                .to_string()
        })
        .map_err(failed)?;
        ctx.globals().set("__turu", turu).map_err(failed)?;

        let read = || -> rquickjs::Result<Option<String>> {
            let namespace: Object = Module::import(&ctx, entry.as_str())?.finish()?;
            let value: Option<rquickjs::Value> = namespace.get("scriptOptions")?;
            let Some(value) = value.filter(|v| !v.is_null() && !v.is_undefined()) else {
                return Ok(None);
            };
            let json: Object = ctx.globals().get("JSON")?;
            let stringify: Function = json.get("stringify")?;
            Ok(Some(stringify.call::<_, String>((value,))?))
        };
        read().catch(&ctx).map_err(|e| e.to_string())
    })?;

    let Some(text) = text else {
        return Ok(None);
    };
    let declaration: Declaration =
        serde_json::from_str(&text).map_err(|e| format!("scriptOptions is wrong: {e}"))?;
    declaration
        .check()
        .map_err(|e| format!("scriptOptions: {e}"))?;
    Ok(Some(declaration))
}

/// A script's options with their current values: the saved values, or the defaults for the rest.
#[derive(Debug, Clone)]
pub struct ScriptOptions {
    decl: Declaration,
    values: BTreeMap<String, Value>,
    path: PathBuf,
}

impl ScriptOptions {
    /// Reads the saved values from `path` (VibeSkua `Load`). A missing file means the defaults.
    /// Lines that don't name one of the options, or whose value doesn't fit, are skipped.
    pub fn load(decl: Declaration, path: PathBuf) -> io::Result<Self> {
        let mut options = Self {
            values: defaults(&decl),
            decl,
            path,
        };
        match fs::read_to_string(&options.path) {
            Ok(text) => options.apply_saved(&text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        Ok(options)
    }

    pub fn declaration(&self) -> &Declaration {
        &self.decl
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The value of an option, or `None` when the script doesn't declare it.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// Sets an option's value (VibeSkua `Config.Set`). Fails for an unknown name or a value of
    /// the wrong kind. Text can't have line breaks, since the file has one option per line.
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        let decl = self
            .decl
            .option(name)
            .ok_or_else(|| format!("this script has no option named \"{name}\""))?;
        if !decl.kind.accepts(&value) {
            return Err(format!("\"{name}\" must be {}", decl.kind.describe()));
        }
        if value
            .as_str()
            .is_some_and(|text| text.contains(['\n', '\r']))
        {
            return Err(format!("\"{name}\" can't have line breaks"));
        }
        self.values.insert(name.to_owned(), value);
        Ok(())
    }

    /// Puts every option back to its default (the window's Reset Defaults).
    pub fn reset(&mut self) {
        self.values = defaults(&self.decl);
    }

    /// Writes the values to the options file (VibeSkua `Save`).
    pub fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text: String = self
            .decl
            .options
            .iter()
            .map(|option| {
                let value = self
                    .values
                    .get(&option.name)
                    .map(saved_text)
                    .unwrap_or_default();
                format!("Options:{}={value}\n", option.name)
            })
            .collect();
        fs::write(&self.path, text)
    }

    fn apply_saved(&mut self, text: &str) {
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            // VibeSkua's lines are "Options:Name=Value"; "Name=Value" is read too.
            let parts: Vec<&str> = line.splitn(3, [':', '=']).collect();
            let (name, value) = match parts.as_slice() {
                ["Options", name, value] | [name, value] => (*name, *value),
                _ => continue,
            };
            let Some(decl) = self
                .decl
                .options
                .iter()
                .find(|o| o.name.eq_ignore_ascii_case(name))
            else {
                continue;
            };
            if let Some(value) = parse_saved(decl.kind, value) {
                self.values.insert(decl.name.clone(), value);
            }
        }
    }
}

fn defaults(decl: &Declaration) -> BTreeMap<String, Value> {
    decl.options
        .iter()
        .map(|o| (o.name.clone(), o.default_value.clone()))
        .collect()
}

/// A value as VibeSkua writes it: `True`/`False`, the number, or the text.
fn saved_text(value: &Value) -> String {
    match value {
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Reads a saved value of `kind`; `None` when the text doesn't fit.
fn parse_saved(kind: Kind, text: &str) -> Option<Value> {
    match kind {
        Kind::Bool if text.eq_ignore_ascii_case("true") => Some(Value::Bool(true)),
        Kind::Bool if text.eq_ignore_ascii_case("false") => Some(Value::Bool(false)),
        Kind::Bool => None,
        Kind::Int => text.trim().parse::<i64>().ok().map(Value::from),
        Kind::Text => Some(Value::String(text.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration_of(json: &str) -> Declaration {
        serde_json::from_str(json).unwrap()
    }

    const SAMPLE: &str = r#"{
        "storage": "Sample",
        "options": [
            {"name": "BuyReagents", "displayName": "Buy", "description": "", "kind": "bool", "defaultValue": false},
            {"name": "PotionQuant", "displayName": "Quantity", "description": "", "kind": "int", "defaultValue": 0},
            {"name": "Label", "displayName": "Label", "description": "", "kind": "text", "defaultValue": "x"}
        ]
    }"#;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("turu-options-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("Sample.cfg")
    }

    #[test]
    fn missing_file_gives_the_defaults() {
        let options = ScriptOptions::load(declaration_of(SAMPLE), temp_file("missing")).unwrap();
        assert_eq!(options.get("BuyReagents"), Some(&Value::Bool(false)));
        assert_eq!(options.get("PotionQuant"), Some(&json!(0)));
        assert_eq!(options.get("Label"), Some(&json!("x")));
        assert_eq!(options.get("Nope"), None);
    }

    #[test]
    fn reads_vibeskua_lines_and_skips_the_rest() {
        let path = temp_file("read");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "Options:BuyReagents=True\n\
             Options:potionquant=250\n\
             Options:PotionQuant=abc\n\
             Options:Unknown=1\n\
             Options:Label=a=b:c\n\
             \n",
        )
        .unwrap();
        let options = ScriptOptions::load(declaration_of(SAMPLE), path).unwrap();
        assert_eq!(options.get("BuyReagents"), Some(&Value::Bool(true)));
        // The name matches without case, and a value that doesn't parse keeps the earlier value.
        assert_eq!(options.get("PotionQuant"), Some(&json!(250)));
        assert_eq!(options.get("Label"), Some(&json!("a=b:c")));
    }

    #[test]
    fn saves_in_vibeskua_format_and_reads_back() {
        let path = temp_file("save");
        let mut options = ScriptOptions::load(declaration_of(SAMPLE), path.clone()).unwrap();
        options.set("BuyReagents", Value::Bool(true)).unwrap();
        options.set("PotionQuant", json!(300)).unwrap();
        options.save().unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Options:BuyReagents=True\nOptions:PotionQuant=300\nOptions:Label=x\n"
        );
        let back = ScriptOptions::load(declaration_of(SAMPLE), path).unwrap();
        assert_eq!(back.get("BuyReagents"), Some(&Value::Bool(true)));
        assert_eq!(back.get("PotionQuant"), Some(&json!(300)));
    }

    #[test]
    fn set_checks_the_name_and_the_kind() {
        let mut options = ScriptOptions::load(declaration_of(SAMPLE), temp_file("set")).unwrap();
        assert!(options.set("Nope", json!(1)).is_err());
        assert!(options.set("PotionQuant", json!("300")).is_err());
        assert!(options.set("PotionQuant", json!(1.5)).is_err());
        assert!(options.set("Label", json!("two\nlines")).is_err());
        options.set("Label", json!("ok")).unwrap();
        assert_eq!(options.get("Label"), Some(&json!("ok")));
        options.reset();
        assert_eq!(options.get("Label"), Some(&json!("x")));
    }

    #[test]
    fn check_rejects_bad_declarations() {
        let mut decl = declaration_of(SAMPLE);
        assert!(decl.check().is_ok());
        decl.storage = "../escape".into();
        assert!(decl.check().is_err());
        decl.storage = "Sample".into();
        decl.options[1].default_value = json!("zero");
        assert!(decl.check().unwrap_err().contains("PotionQuant"));
        decl.options[1].default_value = json!(0);
        decl.options[2].name = "BuyReagents".into();
        assert!(decl.check().unwrap_err().contains("used twice"));
    }
}
