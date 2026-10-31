//! A script's options (VibeSkua `ScriptOption`, `ScriptOptionContainer`).
//!
//! A script declares them with `export const scriptOptions = defineScriptOptions(storage, [...])`
//! (bot/js/turu.ts). On load the engine reads that export with [`declaration`], without running
//! `main`, so the Script Options window can show them before the script starts. The values are
//! saved in `options/<storage>.cfg` in VibeSkua's format (`Options:Name=Value` lines, and
//! `Group:Name=Value` for the options of a group), so a file from VibeSkua's options folder can be
//! copied over.

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

/// The category of the options that aren't in a group (VibeSkua's `Options` list): the prefix of
/// their lines in the options file.
pub const MAIN: &str = "Options";

/// The kinds of option: a checkbox, a whole number, text, or one of a list of names (VibeSkua's
/// `bool`, `int`, `string` and enum options).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Bool,
    Int,
    Text,
    Choice,
}

/// One option as the script declares it (`option.bool(...)`, `option.int(...)`, `option.text(...)`,
/// `option.choice(...)`).
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
    /// The names a `Choice` can take (VibeSkua: the enum's members). Shown with `_` as spaces.
    #[serde(default)]
    pub choices: Vec<String>,
}

impl OptionDecl {
    fn accepts(&self, value: &Value) -> bool {
        match self.kind {
            Kind::Bool => value.is_boolean(),
            Kind::Int => value.is_i64(),
            Kind::Text => value.is_string(),
            Kind::Choice => value
                .as_str()
                .is_some_and(|v| self.choices.iter().any(|c| c == v)),
        }
    }

    fn describe(&self) -> String {
        match self.kind {
            Kind::Bool => "true or false".into(),
            Kind::Int => "a whole number".into(),
            Kind::Text => "text".into(),
            Kind::Choice => format!("one of {}", self.choices.join(", ")),
        }
    }
}

/// A named list of options (VibeSkua `MultiOptions`: each name there is a field holding a list).
/// The window shows it under its own title, the name with `_` as spaces.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OptionGroup {
    pub name: String,
    pub options: Vec<OptionDecl>,
}

/// A script's options: the file they're saved in (VibeSkua `OptionsStorage`) and their list.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Declaration {
    pub storage: String,
    pub options: Vec<OptionDecl>,
    /// The option groups (VibeSkua `MultiOptions`), after `options`.
    #[serde(default)]
    pub groups: Vec<OptionGroup>,
    /// Don't show the Script Options window before each run (VibeSkua `DontPreconfigure`):
    /// the script opens it itself (CoreBots' `SkipOption`).
    #[serde(default)]
    pub dont_preconfigure: bool,
}

impl Declaration {
    /// Checks what the script declared: the storage must be a plain file name, group names must be
    /// unique, option names must be unique within their group and not empty, and each default
    /// must be of its option's kind.
    pub fn check(&self) -> Result<(), String> {
        let storage = self.storage.as_str();
        let plain_name = !storage.is_empty()
            && !storage.starts_with('.')
            && storage.chars().all(|c| {
                c.is_alphanumeric()
                    || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')' | '[' | ']' | '%')
            });
        if !plain_name {
            return Err(format!(
                "the storage \"{storage}\" must be a plain file name"
            ));
        }
        let mut groups = HashSet::new();
        for group in &self.groups {
            let plain = !group.name.is_empty() && !group.name.contains([':', '=', '\n', '\r']);
            if !plain || group.name == MAIN || !groups.insert(group.name.as_str()) {
                return Err(format!(
                    "the group name \"{}\" is empty, has ':' or '=', or is used twice",
                    group.name
                ));
            }
        }
        for (_, options) in self.categories() {
            let mut names = HashSet::new();
            for option in options {
                if option.name.is_empty()
                    || option.name.contains(['=', '\n', '\r'])
                    || !names.insert(option.name.as_str())
                {
                    return Err(format!(
                        "the option name \"{}\" is empty, has '=', or is used twice",
                        option.name
                    ));
                }
                if option.kind == Kind::Choice && option.choices.is_empty() {
                    return Err(format!("\"{}\" has no choices", option.name));
                }
                if !option.accepts(&option.default_value) {
                    return Err(format!(
                        "the default of \"{}\" must be {}",
                        option.name,
                        option.describe()
                    ));
                }
            }
        }
        Ok(())
    }

    /// Every category with its options: [`MAIN`] first, then the groups in order.
    pub fn categories(&self) -> impl Iterator<Item = (&str, &[OptionDecl])> {
        std::iter::once((MAIN, self.options.as_slice())).chain(
            self.groups
                .iter()
                .map(|g| (g.name.as_str(), g.options.as_slice())),
        )
    }

    /// The options of a category ([`MAIN`] or a group's name).
    fn category(&self, category: &str) -> Option<&[OptionDecl]> {
        self.categories()
            .find(|(name, _)| *name == category)
            .map(|(_, options)| options)
    }

    fn option(&self, category: &str, name: &str) -> Option<&OptionDecl> {
        self.category(category)?.iter().find(|o| o.name == name)
    }
}

/// Where an option's value is kept: its category and its name.
type Key = (String, String);

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
    values: BTreeMap<Key, Value>,
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

    /// The value of an option outside the groups, or `None` when the script doesn't declare it.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.get_in(MAIN, name)
    }

    /// The value of an option of a category ([`MAIN`] or a group's name; VibeSkua
    /// `Config.Get<T>(category, name)`), or `None` when the script doesn't declare it.
    pub fn get_in(&self, category: &str, name: &str) -> Option<&Value> {
        self.values.get(&(category.to_owned(), name.to_owned()))
    }

    /// Sets the value of an option outside the groups (VibeSkua `Config.Set`).
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        self.set_in(MAIN, name, value)
    }

    /// Sets an option's value. Fails for an unknown name or a value of the wrong kind. Text can't
    /// have line breaks, since the file has one option per line.
    pub fn set_in(&mut self, category: &str, name: &str, value: Value) -> Result<(), String> {
        let decl = self.decl.option(category, name).ok_or_else(|| {
            if category == MAIN {
                format!("this script has no option named \"{name}\"")
            } else {
                format!("this script has no option named \"{name}\" in \"{category}\"")
            }
        })?;
        if !decl.accepts(&value) {
            return Err(format!("\"{name}\" must be {}", decl.describe()));
        }
        if value
            .as_str()
            .is_some_and(|text| text.contains(['\n', '\r']))
        {
            return Err(format!("\"{name}\" can't have line breaks"));
        }
        self.values
            .insert((category.to_owned(), name.to_owned()), value);
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
        let mut text = String::new();
        for (category, options) in self.decl.categories() {
            for option in options {
                let value = self
                    .get_in(category, &option.name)
                    .map(saved_text)
                    .unwrap_or_default();
                text.push_str(&format!("{category}:{}={value}\n", option.name));
            }
        }
        fs::write(&self.path, text)
    }

    fn apply_saved(&mut self, text: &str) {
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            // VibeSkua's lines are "Options:Name=Value" or "Group:Name=Value"; "Name=Value" is
            // read too.
            let parts: Vec<&str> = line.splitn(3, [':', '=']).collect();
            let (category, name, value) = match parts.as_slice() {
                [category, name, value] => (*category, *name, *value),
                [name, value] => (MAIN, *name, *value),
                _ => continue,
            };
            let Some(decl) = self
                .decl
                .category(category)
                .and_then(|options| options.iter().find(|o| o.name.eq_ignore_ascii_case(name)))
            else {
                continue;
            };
            if let Some(value) = parse_saved(decl, value) {
                self.values
                    .insert((category.to_owned(), decl.name.clone()), value);
            }
        }
    }
}

fn defaults(decl: &Declaration) -> BTreeMap<Key, Value> {
    decl.categories()
        .flat_map(|(category, options)| {
            options.iter().map(move |o| {
                (
                    (category.to_owned(), o.name.clone()),
                    o.default_value.clone(),
                )
            })
        })
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

/// Reads a saved value of `option`; `None` when the text doesn't fit. A choice is saved by its
/// name (VibeSkua writes the enum member, and reads it back with spaces as `_`).
fn parse_saved(option: &OptionDecl, text: &str) -> Option<Value> {
    match option.kind {
        Kind::Bool if text.eq_ignore_ascii_case("true") => Some(Value::Bool(true)),
        Kind::Bool if text.eq_ignore_ascii_case("false") => Some(Value::Bool(false)),
        Kind::Bool => None,
        Kind::Int => text.trim().parse::<i64>().ok().map(Value::from),
        Kind::Text => Some(Value::String(text.to_owned())),
        Kind::Choice => {
            let name = text.trim().replace(' ', "_");
            option
                .choices
                .iter()
                .find(|c| **c == name)
                .or_else(|| {
                    option
                        .choices
                        .iter()
                        .find(|c| c.eq_ignore_ascii_case(&name))
                })
                .map(|c| Value::String(c.clone()))
        }
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
        // VibeSkua's names keep their brackets and percent signs (`HardCoreMetals[mem](Daily)`, `1%Capes`).
        decl.storage = "HardCoreMetals[mem](Daily)".into();
        assert!(decl.check().is_ok());
        decl.storage = "1%Capes".into();
        assert!(decl.check().is_ok());
        decl.storage = "Sample".into();
        decl.options[1].default_value = json!("zero");
        assert!(decl.check().unwrap_err().contains("PotionQuant"));
        decl.options[1].default_value = json!(0);
        decl.options[2].name = "BuyReagents".into();
        assert!(decl.check().unwrap_err().contains("used twice"));
    }

    const GROUPED: &str = r#"{
        "storage": "Merge",
        "options": [
            {"name": "Pet", "displayName": "Pet", "description": "", "kind": "choice", "defaultValue": "None", "choices": ["None", "Dragon_Pet", "Cat"]}
        ],
        "groups": [
            {"name": "Generic", "options": [
                {"name": "mode", "displayName": "Mode", "description": "", "kind": "choice", "defaultValue": "all", "choices": ["all", "acOnly", "mergeMats", "select"]}
            ]},
            {"name": "Select", "options": [
                {"name": "1234", "displayName": "Item", "description": "", "kind": "bool", "defaultValue": false},
                {"name": "mode", "displayName": "Same name, other group", "description": "", "kind": "int", "defaultValue": 3}
            ]}
        ]
    }"#;

    #[test]
    fn groups_and_choices_save_like_vibeskua() {
        let path = temp_file("groups");
        let mut options = ScriptOptions::load(declaration_of(GROUPED), path.clone()).unwrap();
        assert_eq!(options.get("Pet"), Some(&json!("None")));
        assert_eq!(options.get_in("Generic", "mode"), Some(&json!("all")));
        assert_eq!(options.get_in("Select", "mode"), Some(&json!(3)));
        assert_eq!(options.get("mode"), None);
        options.set_in("Generic", "mode", json!("select")).unwrap();
        options.set_in("Select", "1234", json!(true)).unwrap();
        options.set("Pet", json!("Dragon_Pet")).unwrap();
        options.save().unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "Options:Pet=Dragon_Pet\nGeneric:mode=select\nSelect:1234=True\nSelect:mode=3\n"
        );
        let back = ScriptOptions::load(declaration_of(GROUPED), path).unwrap();
        assert_eq!(back.get_in("Generic", "mode"), Some(&json!("select")));
        assert_eq!(back.get_in("Select", "1234"), Some(&json!(true)));
        assert_eq!(back.get("Pet"), Some(&json!("Dragon_Pet")));
    }

    #[test]
    fn choices_read_spaces_and_reject_unknown_names() {
        let path = temp_file("choices");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "Options:Pet=Dragon Pet\nGeneric:mode=nope\nUnknown:mode=x\n",
        )
        .unwrap();
        let mut options = ScriptOptions::load(declaration_of(GROUPED), path).unwrap();
        assert_eq!(options.get("Pet"), Some(&json!("Dragon_Pet")));
        assert_eq!(options.get_in("Generic", "mode"), Some(&json!("all")));
        assert!(options.set("Pet", json!("Dog")).is_err());
        assert!(options.set_in("Nope", "mode", json!("all")).is_err());
    }

    #[test]
    fn check_rejects_bad_groups_and_choices() {
        let mut decl = declaration_of(GROUPED);
        assert!(decl.check().is_ok());
        decl.groups[1].name = "Generic".into();
        assert!(decl.check().unwrap_err().contains("Generic"));
        decl.groups[1].name = "Options".into();
        assert!(decl.check().is_err());
        decl.groups[1].name = "Select".into();
        decl.options[0].default_value = json!("Dog");
        assert!(decl.check().unwrap_err().contains("Pet"));
        decl.options[0].choices.clear();
        assert!(decl.check().unwrap_err().contains("no choices"));
    }
}
