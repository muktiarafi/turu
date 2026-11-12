//! Builds `bot/assets/turu.swf`, Turu's host SWF, from `host/as3/` (docs/HOST_SWF_PROPOSAL.md):
//! `cargo run -p turu_host` writes it, `cargo run -p turu_host -- --check` fails if the committed
//! one differs (CI), `--out <path>` writes elsewhere. Needs Java (asc.jar), as Ruffle's
//! playerglobals do.
//!
//! Ruffle's `asc.jar` (ASC 1.0) against Ruffle's own playerglobal, not Adobe's Flex, so:
//!
//! - Each `-in` file is compiled on its own and can't see the others, so every `.as` file goes
//!   into one compile unit, a wrapper of `include`s, with each class after the class it extends
//!   (asc rejects a forward reference to a base class).
//! - That unit is one script, so every class's static initializer runs when it loads, in the
//!   wrapper's order (Flex gives each class a script of its own, run when first used). A static
//!   initializer must not use another class.
//! - asc's `-swf` writes no End tag and a `DoABC`, so only its ABC is used, and the SWF around it
//!   is written here: version 43, zlib, 958x550 at 24 fps (the game's own rate), with the tags a
//!   Flex build has (less its `Metadata` and `ProductInfo`).
//! - Where Flex emits `callpropvoid`, asc emits `callproperty` then `pop`, and `findpropstrict x;
//!   getproperty x` for `getlex x`: bytecode rules written against Flex's output miss them.

use anyhow::{Context, anyhow, bail, ensure};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use swf::avm2::read::Reader;
use swf::{
    Color, Compression, DoAbc2, DoAbc2Flag, FileAttributes, Fixed8, FrameLabel, Header, Rectangle,
    SwfStr, SymbolClassLink, Tag, Twips,
};

/// The document class.
const MAIN_CLASS: &str = "turu.host.Main";
/// Flash Player 28's, as a Flex build for `target-player` 28 has it.
const SWF_VERSION: u8 = 43;
const WIDTH: f64 = 958.0;
const HEIGHT: f64 = 550.0;
/// The game's own frame rate: what Turu sets when the client loads anyway.
const FRAME_RATE: u8 = 24;

fn main() -> anyhow::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut check = false;
    let repo = root.parent().context("host/ has no parent")?;
    let mut out = repo.join("bot/assets/turu.swf");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--out" => out = args.next().context("--out needs a path")?.into(),
            _ => bail!("usage: turu_host [--check] [--out <path>]"),
        }
    }

    let swf = build(root)?;
    if check {
        let committed =
            std::fs::read(&out).with_context(|| format!("couldn't read {}", out.display()))?;
        ensure!(
            committed == swf,
            "{} isn't what host/as3 builds: run `cargo run -p turu_host` and commit it",
            out.display()
        );
        println!("{} is up to date ({} bytes)", out.display(), swf.len());
    } else {
        std::fs::write(&out, &swf).with_context(|| format!("couldn't write {}", out.display()))?;
        println!("wrote {} ({} bytes)", out.display(), swf.len());
    }
    Ok(())
}

/// turu.swf, built from `root/as3`.
fn build(root: &Path) -> anyhow::Result<Vec<u8>> {
    let as3 = root.join("as3");
    let ruffle = root.join("../ruffle");
    let tmp = tempfile::tempdir()?;

    // `playerglobal_import.abc`: Ruffle's playerglobal as an asc import library.
    build_playerglobal::build_avm2_playerglobal(&ruffle, tmp.path(), false)
        .map_err(|e| anyhow!("couldn't build Ruffle's playerglobal: {e}"))?;
    let playerglobal = tmp.path().join("playerglobal_import.abc");

    let files = source_files(&as3)?;
    let wrapper = tmp.path().join("turu.as");
    let includes: String = compile_order(&files)?
        .iter()
        .map(|file| format!("include \"{}\";\n", file.path.display()))
        .collect();
    std::fs::write(&wrapper, includes)?;

    let abc_path = wrapper.with_extension("abc");
    asc::run_asc(asc::AscConfig::new([
        "-AS3".to_string(),
        "-strict".to_string(),
        "-optimize".to_string(),
        "-import".to_string(),
        playerglobal.to_string_lossy().into_owned(),
        wrapper.to_string_lossy().into_owned(),
    ]))?;
    // asc reports errors on its output; it writes no ABC when there were any.
    let abc = std::fs::read(&abc_path).context("asc.jar wrote no ABC (errors above)")?;
    check_abc(&abc)?;

    swf_around(&abc)
}

/// A class in `as3/`.
struct Source {
    path: PathBuf,
    /// `turu.host.module.Module`.
    class: String,
    /// The class it extends, as written (`Module`, `MovieClip`).
    extends: Option<String>,
    /// The classes it imports (`turu.host.module.Module`).
    imports: Vec<String>,
}

/// Every `.as` file under `dir`, sorted by path.
fn source_files(dir: &Path) -> anyhow::Result<Vec<Source>> {
    let mut paths = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "as") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    ensure!(!paths.is_empty(), "no .as files in {}", dir.display());
    paths.into_iter().map(parse).collect()
}

/// A file's package, class, base class and imports, from its tokens.
fn parse(path: PathBuf) -> anyhow::Result<Source> {
    let text = without_comments(&std::fs::read_to_string(&path)?);
    let tokens: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || matches!(c, '{' | '}' | ';'))
        .filter(|t| !t.is_empty())
        .collect();
    let after = |word: &str| {
        tokens
            .iter()
            .position(|t| *t == word)
            .and_then(|i| tokens.get(i + 1).copied())
    };
    let package = after("package").filter(|p| *p != "class" && *p != "public");
    let name = after("class").with_context(|| format!("no class in {}", path.display()))?;
    let class = match package {
        Some(package) => format!("{package}.{name}"),
        None => name.to_string(),
    };
    let extends = after("extends").map(str::to_string);
    let imports = tokens
        .windows(2)
        .filter(|w| w[0] == "import")
        .map(|w| w[1].to_string())
        .collect();
    Ok(Source {
        path,
        class,
        extends,
        imports,
    })
}

/// `text` with its `/* */` and `//` comments blanked, so a word in a doc comment isn't taken for
/// a keyword. Strings aren't parsed: a `//` in one blanks the rest of its line, which holds no
/// declaration.
fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("/*") {
            rest = after.split_once("*/").map_or("", |(_, after)| after);
            out.push(' ');
        } else if rest.starts_with("//") {
            rest = rest.find('\n').map_or("", |i| &rest[i..]);
        } else {
            let c = rest.chars().next().expect("not empty");
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// `files` with each class after the class of ours it extends; otherwise in path order.
fn compile_order(files: &[Source]) -> anyhow::Result<Vec<&Source>> {
    let by_class: HashMap<&str, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.class.as_str(), i))
        .collect();
    // The class of ours each file extends: one in its own package, or one it imports.
    let base = |file: &Source| -> Option<usize> {
        let name = file.extends.as_deref()?;
        let package = file.class.rsplit_once('.').map_or("", |(p, _)| p);
        let own = if package.is_empty() {
            name.to_string()
        } else {
            format!("{package}.{name}")
        };
        by_class.get(own.as_str()).copied().or_else(|| {
            file.imports
                .iter()
                .find(|i| i.rsplit('.').next() == Some(name))
                .and_then(|i| by_class.get(i.as_str()).copied())
        })
    };

    // 0: not yet, 1: being placed, 2: placed.
    let mut state = vec![0u8; files.len()];
    let mut order = Vec::with_capacity(files.len());
    for start in 0..files.len() {
        let mut chain = Vec::new();
        let mut at = Some(start);
        while let Some(i) = at {
            match state[i] {
                2 => break,
                1 => bail!("{} extends itself through its base classes", files[i].class),
                _ => {}
            }
            state[i] = 1;
            chain.push(i);
            at = base(&files[i]);
        }
        for &i in chain.iter().rev() {
            state[i] = 2;
            order.push(&files[i]);
        }
    }
    Ok(order)
}

/// Fails unless `abc` reads, and has no API version marks (asc adds them only with
/// `-apiversioning`, which would leave names Flash Player and AIR don't know).
fn check_abc(abc: &[u8]) -> anyhow::Result<()> {
    let file = Reader::new(abc)
        .read()
        .context("asc.jar's ABC doesn't read")?;
    let pool = &file.constant_pool;
    let marked = pool.strings.iter().find(|s| {
        String::from_utf8_lossy(s)
            .chars()
            .last()
            .is_some_and(|c| ('\u{E000}'..='\u{F8FF}').contains(&c))
    });
    if let Some(s) = marked {
        bail!(
            "asc.jar's ABC has a version-marked name: {:?}",
            String::from_utf8_lossy(s)
        );
    }
    Ok(())
}

/// The SWF around `abc`.
fn swf_around(abc: &[u8]) -> anyhow::Result<Vec<u8>> {
    let header = Header {
        compression: Compression::Zlib,
        version: SWF_VERSION,
        stage_size: Rectangle {
            x_min: Twips::ZERO,
            x_max: Twips::from_pixels(WIDTH),
            y_min: Twips::ZERO,
            y_max: Twips::from_pixels(HEIGHT),
        },
        frame_rate: Fixed8::from_f32(FRAME_RATE.into()),
        num_frames: 1,
    };
    let label = MAIN_CLASS.replace('.', "_");
    let tags = [
        Tag::FileAttributes(
            FileAttributes::IS_ACTION_SCRIPT_3 | FileAttributes::USE_NETWORK_SANDBOX,
        ),
        Tag::ScriptLimits {
            max_recursion_depth: 1000,
            timeout_in_seconds: 60,
        },
        Tag::SetBackgroundColor(Color::BLACK),
        Tag::FrameLabel(FrameLabel {
            label: SwfStr::from_utf8_str(&label),
            is_anchor: false,
        }),
        Tag::DoAbc2(DoAbc2 {
            flags: DoAbc2Flag::LAZY_INITIALIZE,
            name: SwfStr::from_utf8_str(""),
            data: abc,
        }),
        Tag::SymbolClass(vec![SymbolClassLink {
            id: 0,
            class_name: SwfStr::from_utf8_str(MAIN_CLASS),
        }]),
        Tag::ShowFrame,
    ];
    let mut out = Vec::new();
    swf::write_swf(&header, &tags, &mut out)?;
    // What's written reads back as one frame of those tags.
    let read = swf::decompress_swf(&out[..])?;
    let parsed = swf::parse_swf(&read)?;
    ensure!(
        parsed.tags.len() == tags.len(),
        "the written SWF reads back as {} tags, not {}",
        parsed.tags.len(),
        tags.len()
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(class: &str, extends: Option<&str>, imports: &[&str]) -> Source {
        Source {
            path: PathBuf::from(class),
            class: class.to_string(),
            extends: extends.map(str::to_string),
            imports: imports.iter().map(|i| i.to_string()).collect(),
        }
    }

    fn classes<'a>(order: &[&'a Source]) -> Vec<&'a str> {
        order.iter().map(|s| s.class.as_str()).collect()
    }

    #[test]
    fn base_classes_come_first() {
        let files = [
            source(
                "turu.host.Main",
                Some("MovieClip"),
                &["turu.host.module.Modules"],
            ),
            source("turu.host.module.DisableFX", Some("Module"), &[]),
            source("turu.host.module.Module", None, &[]),
            source(
                "turu.host.other.Hide",
                Some("Module"),
                &["turu.host.module.Module"],
            ),
        ];
        assert_eq!(
            classes(&compile_order(&files).unwrap()),
            [
                "turu.host.Main",
                "turu.host.module.Module",
                "turu.host.module.DisableFX",
                "turu.host.other.Hide"
            ]
        );
    }

    #[test]
    fn a_flash_base_class_of_the_same_name_isnt_ours() {
        // `extends Event` without importing our Event: flash.events.Event.
        let files = [
            source(
                "turu.host.net.PacketEvent",
                Some("Event"),
                &["flash.events.Event"],
            ),
            source("turu.host.api.Event", None, &[]),
        ];
        assert_eq!(
            classes(&compile_order(&files).unwrap()),
            ["turu.host.net.PacketEvent", "turu.host.api.Event"]
        );
    }

    #[test]
    fn a_cycle_fails() {
        let files = [source("A", Some("B"), &[]), source("B", Some("A"), &[])];
        assert!(compile_order(&files).is_err());
    }

    #[test]
    fn words_in_comments_arent_keywords() {
        let text = "/** Loads the class of the game. */\npackage a { // extends nothing\n\
                    public class B extends C {} }";
        let tokens: Vec<String> = without_comments(text)
            .split_whitespace()
            .map(str::to_string)
            .collect();
        assert_eq!(
            tokens,
            [
                "package", "a", "{", "public", "class", "B", "extends", "C", "{}", "}"
            ]
        );
    }

    #[test]
    fn parses_a_class() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Hide.as");
        std::fs::write(
            &path,
            "package turu.host.module {\nimport flash.display.MovieClip;\nimport turu.host.Main;\n\
             public class HidePlayers extends Module {\n}\n}\n",
        )
        .unwrap();
        let s = parse(path).unwrap();
        assert_eq!(s.class, "turu.host.module.HidePlayers");
        assert_eq!(s.extends.as_deref(), Some("Module"));
        assert_eq!(s.imports, ["flash.display.MovieClip", "turu.host.Main"]);
    }
}
