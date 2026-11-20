//! Writes the view models' Kotlin types (docs/NATIVE_UI_PROPOSAL.md, L4) into a folder: the
//! Android build runs it before compiling its Kotlin (`android/app/build.gradle.kts`,
//! `turuTypes`), so a record changed in Rust can't ship with stale types in the shell. Never
//! committed. `cargo run -p turu_frontend --no-default-features --features typegen --bin
//! turu-typegen -- <out dir>`.

use facet_generate::generation::json::JsonPlugin;
use facet_generate::generation::kotlin;
use facet_generate::reflection::RegistryBuilder;
use turu_frontend::models::{
    application_options::AppOptionsView, army::ArmyView, auto::AutoView, bank::BankView, butler::ButlerView,
    core_bots::CoreBotsView, drops::DropsView, fast_travel::FastTravelView, game_options::GameOptionsView,
    jump::JumpView, loadouts::LoadoutsView, logs::LogsView, notifications::NotificationsView, rail::RailView,
    repo::RepoView, scheduler::SchedulerView, scripts::ScriptsView, skills::SkillsView, stats::StatsView,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).ok_or("give the folder to write into")?;
    let registry = RegistryBuilder::new()
        .add_type::<ScriptsView>()?
        .add_type::<AutoView>()?
        .add_type::<JumpView>()?
        .add_type::<BankView>()?
        .add_type::<RailView>()?
        .add_type::<LogsView>()?
        .add_type::<RepoView>()?
        .add_type::<GameOptionsView>()?
        .add_type::<AppOptionsView>()?
        .add_type::<CoreBotsView>()?
        .add_type::<SchedulerView>()?
        .add_type::<SkillsView>()?
        .add_type::<LoadoutsView>()?
        .add_type::<FastTravelView>()?
        .add_type::<ButlerView>()?
        .add_type::<ArmyView>()?
        .add_type::<NotificationsView>()?
        .add_type::<StatsView>()?
        .add_type::<DropsView>()?
        .build()?;
    kotlin::Installer::new("com.muktiarafi.turu.models", std::path::Path::new(&out))
        .plugin(JsonPlugin)
        .generate(&registry)?;
    Ok(())
}
