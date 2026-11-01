//! ES module resolution for scripts: relative imports (`./CoreBots`, `../core/CoreFarms.ts`)
//! resolve against the importing file; `"turu"` is the built-in runtime (bot/js/turu.ts).

use super::transpile::transpile;
use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::{Ctx, Error, Module, Result};
use std::path::{Path, PathBuf};

pub const TURU_MODULE: &str = "turu";
const TURU_SOURCE: &str = crate::TURU_TS;

pub struct ScriptResolver;

impl Resolver for ScriptResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> Result<String> {
        if name == TURU_MODULE {
            return Ok(TURU_MODULE.to_owned());
        }
        let requested = Path::new(name);
        let path = if requested.is_absolute() {
            requested.to_path_buf()
        } else if name.starts_with('.') {
            Path::new(base)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(requested)
        } else {
            return Err(Error::new_resolving_message(
                base,
                name,
                "only relative imports and \"turu\" are supported",
            ));
        };
        let found = candidates(&path)
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(|| Error::new_resolving_message(base, name, "file not found"))?;
        Ok(normalize(&found).to_string_lossy().into_owned())
    }
}

/// `./x` may mean `./x.ts`, `./x.js` or `./x/index.ts`.
fn candidates(path: &Path) -> Vec<PathBuf> {
    let mut out = vec![path.to_path_buf()];
    for ext in ["ts", "js"] {
        let mut with_ext = path.as_os_str().to_owned();
        with_ext.push(format!(".{ext}"));
        out.push(PathBuf::from(with_ext));
    }
    out.push(path.join("index.ts"));
    out
}

/// Removes `.` and `..` so one file always gets one module name.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

pub struct ScriptLoader;

impl Loader for ScriptLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> Result<Module<'js, Declared>> {
        let (path, source) = if name == TURU_MODULE {
            (PathBuf::from("turu.ts"), TURU_SOURCE.to_owned())
        } else {
            let path = PathBuf::from(name);
            let source = std::fs::read_to_string(&path)
                .map_err(|e| Error::new_loading_message(name, e.to_string()))?;
            (path, source)
        };
        let js = if path.extension().is_some_and(|e| e == "js") {
            source
        } else {
            transpile(&source, &path).map_err(|e| Error::new_loading_message(name, e))?
        };
        Module::declare(ctx.clone(), name, js)
    }
}
