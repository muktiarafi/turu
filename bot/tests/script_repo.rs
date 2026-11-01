//! Search Scripts (`turu_bot::repo`) against a fake repository.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use turu_bot::paths::Paths;
use turu_bot::repo::{
    self, Fetched, Index, Remote, Scope, ScriptInfo, ScriptRepo, Status, parse_header, search_rank,
    sha256,
};

#[derive(Default)]
struct Fake {
    index: Mutex<Index>,
    files: Mutex<HashMap<String, Vec<u8>>>,
    index_calls: AtomicUsize,
    file_calls: AtomicUsize,
    archive_calls: AtomicUsize,
}

impl Fake {
    /// Publishes `files` (path, source, deps) as a new commit.
    fn publish(&self, commit: &str, files: &[(&str, &str, &[&str])]) {
        let mut index = Index {
            version: repo::INDEX_VERSION,
            commit: commit.into(),
            ..Default::default()
        };
        let mut stored = self.files.lock();
        stored.clear();
        for (path, source, deps) in files {
            let header = parse_header(source);
            index.scripts.push(ScriptInfo {
                path: (*path).into(),
                name: header.name,
                description: header.description,
                tags: header.tags,
                size: source.len() as u64,
                sha256: sha256(source.as_bytes()),
                deps: deps.iter().map(|d| (*d).into()).collect(),
                ..Default::default()
            });
            stored.insert((*path).into(), source.as_bytes().to_vec());
        }
        *self.index.lock() = index;
    }
}

/// The repo's handle on the fake (the test keeps another).
struct Shared(Arc<Fake>);

impl Remote for Shared {
    fn index(&self, etag: Option<&str>) -> Result<Fetched, String> {
        self.0.index_calls.fetch_add(1, Ordering::Relaxed);
        let index = self.0.index.lock().clone();
        if etag == Some(index.commit.as_str()) {
            return Ok(Fetched::Unchanged);
        }
        Ok(Fetched::Index {
            body: serde_json::to_vec(&index).unwrap(),
            etag: Some(index.commit),
        })
    }

    fn file(&self, _commit: &str, path: &str) -> Result<Vec<u8>, String> {
        self.0.file_calls.fetch_add(1, Ordering::Relaxed);
        self.0
            .files
            .lock()
            .get(path)
            .cloned()
            .ok_or_else(|| "404".into())
    }

    fn archive(&self, commit: &str) -> Result<Vec<u8>, String> {
        self.0.archive_calls.fetch_add(1, Ordering::Relaxed);
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (path, bytes) in self.0.files.lock().iter() {
            zip.start_file(format!("turu-scripts-{commit}/{path}"), options)
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        Ok(zip.finish().unwrap().into_inner())
    }
}

const CORE: &str =
    "/*\nname: CoreBots\ndescription: the shared helpers\ntags: core\n*/\nexport const a = 1;\n";
const FARM_MAIN: &str = "import { bot } from \"turu\";\nexport default function main(): void {\n    bot.log(\"hi\");\n}\n";
const FARM: &str = "/*\nname: Gold Farm\ndescription: honor hall for members,\n    BGE for f2p\ntags: gold, farm\n*/\nimport { a } from \"../core/CoreBots\";\n";

fn setup() -> (tempfile::TempDir, Arc<Fake>, Arc<ScriptRepo>) {
    let dir = tempfile::tempdir().unwrap();
    let fake = Arc::new(Fake::default());
    fake.publish(
        "c1",
        &[
            ("core/CoreBots.ts", CORE, &[]),
            ("farm/GoldFarm.ts", FARM, &["core/CoreBots.ts"]),
        ],
    );
    let paths = Paths {
        root: dir.path().to_path_buf(),
    };
    let repo = ScriptRepo::new(paths, Box::new(Shared(fake.clone())));
    (dir, fake, repo)
}

fn status(repo: &ScriptRepo, path: &str) -> Option<Status> {
    repo.snapshot()
        .entries
        .iter()
        .find(|e| e.info.path == path)
        .map(|e| e.status)
}

#[test]
fn header_reads_continued_lines_and_tags() {
    let header = parse_header(FARM);
    assert_eq!(header.name, "Gold Farm");
    assert_eq!(header.description, "honor hall for members, BGE for f2p");
    assert_eq!(header.tags, ["gold", "farm"]);
    assert_eq!(parse_header("// no header\n"), Default::default());
}

/// A file without `export default` is a library; the index generator says the same.
#[test]
fn libraries_have_no_default_export() {
    assert!(repo::is_library(CORE));
    assert!(!repo::is_library(FARM_MAIN));
    assert!(!repo::is_library("export   default function main() {}\n"));
    // Not at the start of a line, or another name.
    assert!(repo::is_library(
        "// export default function main\nconst a = 1;\n"
    ));
    assert!(repo::is_library("export defaultValue = 1;\n"));
}

#[test]
fn local_libraries_are_marked() {
    let (dir, _fake, repo) = setup();
    let scripts = dir.path().join("scripts/mine");
    fs::create_dir_all(&scripts).unwrap();
    fs::write(scripts.join("CoreMine.ts"), "export const x = 1;\n").unwrap();
    fs::write(scripts.join("Mine.ts"), FARM_MAIN).unwrap();
    repo.rescan();
    let library = |path: &str| {
        repo.snapshot()
            .entries
            .iter()
            .find(|e| e.info.path == path)
            .map(|e| e.info.library)
    };
    assert_eq!(library("mine/CoreMine.ts"), Some(true));
    assert_eq!(library("mine/Mine.ts"), Some(false));
}

#[test]
fn search_ranks_like_vibeskua() {
    let info = ScriptInfo {
        path: "farm/GoldFarm.ts".into(),
        name: "Gold Farm".into(),
        description: "honor hall".into(),
        tags: vec!["gold".into(), "bge".into()],
        ..Default::default()
    };
    assert_eq!(search_rank(&info, "gold", Scope::All), Some(0));
    assert_eq!(search_rank(&info, "honor", Scope::All), Some(1));
    assert_eq!(search_rank(&info, "farm/", Scope::All), Some(2));
    assert_eq!(search_rank(&info, "bge", Scope::All), Some(3));
    assert_eq!(search_rank(&info, "nope", Scope::All), None);
    assert_eq!(search_rank(&info, "gold farm", Scope::Name), Some(0));
    assert_eq!(search_rank(&info, "Gold", Scope::Name), Some(1));
    assert_eq!(search_rank(&info, "farm", Scope::Name), Some(2));
    assert_eq!(search_rank(&info, "goldfarm.ts", Scope::File), Some(0));
    assert_eq!(search_rank(&info, "gold", Scope::Description), None);
    assert!(repo::in_category(&info, "Farm"));
    assert!(!repo::in_category(&info, "Gold"));
}

#[test]
fn download_brings_the_imports_and_update_keeps_edits() {
    let (dir, fake, repo) = setup();
    repo.refresh().unwrap();
    assert_eq!(
        status(&repo, "farm/GoldFarm.ts"),
        Some(Status::NotDownloaded)
    );

    let path = repo.ensure("farm/GoldFarm.ts").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), FARM);
    assert_eq!(status(&repo, "core/CoreBots.ts"), Some(Status::UpToDate));
    assert_eq!(fake.file_calls.load(Ordering::Relaxed), 2);

    // A new commit changes both; CoreBots was edited here.
    let core = dir.path().join("scripts/core/CoreBots.ts");
    fs::write(&core, "// mine\n").unwrap();
    let farm2 = format!("{FARM}// v2\n");
    let core2 = format!("{CORE}// v2\n");
    fake.publish(
        "c2",
        &[
            ("core/CoreBots.ts", &core2, &[]),
            ("farm/GoldFarm.ts", &farm2, &["core/CoreBots.ts"]),
        ],
    );
    repo.refresh().unwrap();
    assert_eq!(status(&repo, "farm/GoldFarm.ts"), Some(Status::Outdated));
    assert_eq!(status(&repo, "core/CoreBots.ts"), Some(Status::Modified));

    let message = repo.update_all().unwrap();
    assert!(message.contains("Kept 1 modified"), "{message}");
    assert_eq!(fs::read_to_string(&path).unwrap(), farm2);
    assert_eq!(fs::read_to_string(&core).unwrap(), "// mine\n");

    // Download on the script itself replaces it.
    repo.download(&["core/CoreBots.ts".into()], true).unwrap();
    assert_eq!(fs::read_to_string(&core).unwrap(), core2);
    assert_eq!(status(&repo, "core/CoreBots.ts"), Some(Status::UpToDate));
}

#[test]
fn refresh_is_a_304_when_nothing_changed_and_survives_offline() {
    let (dir, fake, repo) = setup();
    repo.refresh().unwrap();
    assert!(repo.refresh().unwrap().contains("no changes"));
    assert_eq!(fake.index_calls.load(Ordering::Relaxed), 2);

    // A new client reads the cached list without the network.
    let again = ScriptRepo::new(
        Paths {
            root: dir.path().to_path_buf(),
        },
        Box::new(Shared(fake.clone())),
    );
    assert_eq!(again.snapshot().commit, "c1");
    assert_eq!(again.snapshot().entries.len(), 2);
}

#[test]
fn many_files_come_as_one_zip_and_reset_moves_files_aside() {
    let (dir, fake, repo) = setup();
    let names: Vec<(String, String)> = (0..30)
        .map(|i| (format!("story/S{i}.ts"), format!("/*\nname: S{i}\n*/\n")))
        .collect();
    let files: Vec<(&str, &str, &[&str])> = names
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str(), &[][..]))
        .collect();
    fake.publish("c3", &files);

    // My own script stays through a reset.
    let mine = dir.path().join("scripts/mine/Mine.ts");
    fs::create_dir_all(mine.parent().unwrap()).unwrap();
    fs::write(&mine, "/*\nname: Mine\n*/\n").unwrap();

    repo.download_all().unwrap();
    assert_eq!(fake.archive_calls.load(Ordering::Relaxed), 1);
    assert_eq!(fake.file_calls.load(Ordering::Relaxed), 0);
    assert_eq!(status(&repo, "story/S7.ts"), Some(Status::UpToDate));
    assert_eq!(status(&repo, "mine/Mine.ts"), Some(Status::Local));

    fs::write(dir.path().join("scripts/story/S1.ts"), "edited").unwrap();
    let message = repo.reset().unwrap();
    assert!(message.starts_with("Moved 30 files"), "{message}");
    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/story/S1.ts")).unwrap(),
        "/*\nname: S1\n*/\n"
    );
    assert!(mine.is_file());
    let backups: Vec<_> = fs::read_dir(dir.path().join("scripts-backup"))
        .unwrap()
        .collect();
    assert_eq!(backups.len(), 1);
    let backup = backups[0].as_ref().unwrap().path();
    assert_eq!(
        fs::read_to_string(backup.join("story/S1.ts")).unwrap(),
        "edited"
    );

    // Gone from the repository: still here, marked removed.
    fake.publish("c4", &files[1..]);
    repo.refresh().unwrap();
    assert_eq!(status(&repo, "story/S0.ts"), Some(Status::Removed));
    repo.delete("story/S0.ts").unwrap();
    assert_eq!(status(&repo, "story/S0.ts"), None);
}

#[test]
fn bad_paths_and_bad_downloads_are_refused() {
    let (dir, fake, repo) = setup();
    {
        let mut index = fake.index.lock();
        let mut evil = index.scripts[0].clone();
        evil.path = "../evil.ts".into();
        index.scripts.push(evil);
    }
    repo.refresh().unwrap();
    assert_eq!(repo.snapshot().entries.len(), 2);

    fake.files
        .lock()
        .insert("farm/GoldFarm.ts".into(), b"tampered".to_vec());
    let error = repo.ensure("farm/GoldFarm.ts").unwrap_err();
    assert!(error.contains("sha256"), "{error}");
    assert!(!dir.path().join("scripts/farm/GoldFarm.ts").exists());
    assert!(!dir.path().join("evil.ts").exists());
}

/// The scripts repository type-checks against its copy of the script API: keep it the same.
#[test]
fn scripts_repo_types_match_the_api() {
    let copy = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/types/turu.ts");
    // Not there before the submodule has the copy (or without the submodule).
    let Ok(copy) = fs::read_to_string(&copy) else {
        return;
    };
    assert!(
        copy == include_str!("../js/turu.ts"),
        "scripts/types/turu.ts differs from bot/js/turu.ts: copy bot/js/turu.ts over it"
    );
}

#[test]
fn scripts_folder_gets_editor_types_but_keeps_its_tsconfig() {
    let (dir, _fake, repo) = setup();
    let scripts = dir.path().join("scripts");
    assert_eq!(
        fs::read_to_string(scripts.join("types/turu.ts")).unwrap(),
        turu_bot::TURU_TS
    );
    assert!(
        fs::read_to_string(scripts.join("tsconfig.json"))
            .unwrap()
            .contains(r#""turu": ["./types/turu.ts"]"#)
    );
    assert!(
        repo.snapshot()
            .entries
            .iter()
            .all(|e| !e.info.path.starts_with("types/"))
    );

    fs::write(scripts.join("tsconfig.json"), "{}").unwrap();
    fs::write(scripts.join("types/turu.ts"), "old").unwrap();
    Paths {
        root: dir.path().to_path_buf(),
    }
    .write_script_types()
    .unwrap();
    assert_eq!(
        fs::read_to_string(scripts.join("tsconfig.json")).unwrap(),
        "{}"
    );
    assert_eq!(
        fs::read_to_string(scripts.join("types/turu.ts")).unwrap(),
        turu_bot::TURU_TS
    );
}
