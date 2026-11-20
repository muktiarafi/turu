//! Writes the view models' Kotlin types (docs/NATIVE_UI_PROPOSAL.md, L4) into a folder: the
//! Android build runs it before compiling its Kotlin (`android/app/build.gradle.kts`,
//! `turuTypes`), so a record changed in Rust can't ship with stale types in the shell. Never
//! committed. `cargo run -p turu_frontend --no-default-features --features typegen --bin
//! turu-typegen -- <out dir>`.

use facet_generate::generation::json::JsonPlugin;
use facet_generate::generation::kotlin;
use facet_generate::reflection::RegistryBuilder;
use turu_frontend::models::scripts::ScriptsView;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).ok_or("give the folder to write into")?;
    let registry = RegistryBuilder::new().add_type::<ScriptsView>()?.build()?;
    kotlin::Installer::new("com.muktiarafi.turu.models", std::path::Path::new(&out))
        .plugin(JsonPlugin)
        .generate(&registry)?;
    Ok(())
}
