//! Writes the view models' Kotlin types (docs/NATIVE_UI_PROPOSAL.md, L4) into a folder: the
//! Android build runs it before compiling its Kotlin (`android/app/build.gradle.kts`,
//! `turuTypes`), so a record changed in Rust can't ship with stale types in the shell. Never
//! committed. `cargo run -p turu_frontend --no-default-features --features typegen --bin
//! turu-typegen -- <out dir>`.

use facet_generate::generation::json::JsonPlugin;
use facet_generate::generation::kotlin;
use facet_generate::reflection::RegistryBuilder;
use turu_frontend::models::{
    application_options::{AppOptionsAct, AppOptionsView},
    army::{ArmyAct, ArmyView},
    army_client::{ArmyClientAct, ArmyGearView},
    army_travel::{ArmyTravelAct, ArmyTravelView},
    auto::{AutoAct, AutoView},
    bank::{BankAct, BankView},
    butler::{ButlerAct, ButlerView},
    client_update::{ClientUpdateAct, ClientUpdateView},
    console::{ConsoleAct, ConsoleView},
    core_bots::{CoreBotsAct, CoreBotsView},
    cosmetics::{CosmeticsAct, CosmeticsView},
    crashes::{CrashesAct, CrashesView},
    drops::{DropsAct, DropsView},
    fast_travel::{FastTravelAct, FastTravelView},
    game_options::{GameOptionsAct, GameOptionsView},
    grabber::{GrabberAct, GrabberView},
    hotkeys::{HotKeysAct, HotKeysView},
    jump::{JumpAct, JumpView},
    junk::{JunkAct, JunkView},
    loader::{LoaderAct, LoaderView},
    loadouts::{LoadoutsAct, LoadoutsView},
    logs::{LogsAct, LogsView},
    notifications::{NotificationsAct, NotificationsView},
    notify_drop::{NotifyDropAct, NotifyDropView},
    packets::{InterceptorAct, InterceptorView, PacketLoggerAct, PacketLoggerView, SpammerAct, SpammerView},
    prompts::{PromptsAct, PromptsView},
    rail::{RailAct, RailView},
    repo::{RepoAct, RepoView},
    runtime::{RuntimeAct, RuntimeView},
    scheduler::{SchedulerAct, SchedulerView},
    script_options::{ScriptOptionsAct, ScriptOptionsView},
    scripts::{ScriptsAct, ScriptsView},
    skills::{SkillsAct, SkillsView},
    stats::{StatsAct, StatsView},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).ok_or("give the folder to write into")?;
    let registry = RegistryBuilder::new()
        .add_type::<ScriptsView>()?
        .add_type::<AutoView>()?
        .add_type::<JumpView>()?
        .add_type::<JunkView>()?
        .add_type::<ConsoleView>()?
        .add_type::<PromptsView>()?
        .add_type::<HotKeysView>()?
        .add_type::<PacketLoggerView>()?
        .add_type::<SpammerView>()?
        .add_type::<InterceptorView>()?
        .add_type::<ArmyTravelView>()?
        .add_type::<ArmyGearView>()?
        .add_type::<BankView>()?
        .add_type::<RailView>()?
        .add_type::<ClientUpdateView>()?
        .add_type::<LogsView>()?
        .add_type::<CrashesView>()?
        .add_type::<RepoView>()?
        .add_type::<GameOptionsView>()?
        .add_type::<AppOptionsView>()?
        .add_type::<CoreBotsView>()?
        .add_type::<SchedulerView>()?
        .add_type::<SkillsView>()?
        .add_type::<LoadoutsView>()?
        .add_type::<FastTravelView>()?
        .add_type::<ButlerView>()?
        .add_type::<CosmeticsView>()?
        .add_type::<ScriptOptionsView>()?
        .add_type::<RuntimeView>()?
        .add_type::<NotifyDropView>()?
        .add_type::<LoaderView>()?
        .add_type::<GrabberView>()?
        .add_type::<ArmyView>()?
        .add_type::<NotificationsView>()?
        .add_type::<StatsView>()?
        .add_type::<DropsView>()?
        // Each model's actions: what a shell sends to `POST /models/<name>/do`.
        .add_type::<AppOptionsAct>()?
        .add_type::<ArmyAct>()?
        .add_type::<ArmyClientAct>()?
        .add_type::<ArmyTravelAct>()?
        .add_type::<AutoAct>()?
        .add_type::<BankAct>()?
        .add_type::<ButlerAct>()?
        .add_type::<ClientUpdateAct>()?
        .add_type::<ConsoleAct>()?
        .add_type::<CoreBotsAct>()?
        .add_type::<CosmeticsAct>()?
        .add_type::<CrashesAct>()?
        .add_type::<DropsAct>()?
        .add_type::<FastTravelAct>()?
        .add_type::<GameOptionsAct>()?
        .add_type::<GrabberAct>()?
        .add_type::<HotKeysAct>()?
        .add_type::<InterceptorAct>()?
        .add_type::<JumpAct>()?
        .add_type::<JunkAct>()?
        .add_type::<LoaderAct>()?
        .add_type::<LoadoutsAct>()?
        .add_type::<LogsAct>()?
        .add_type::<NotificationsAct>()?
        .add_type::<NotifyDropAct>()?
        .add_type::<PacketLoggerAct>()?
        .add_type::<PromptsAct>()?
        .add_type::<RailAct>()?
        .add_type::<RepoAct>()?
        .add_type::<RuntimeAct>()?
        .add_type::<SchedulerAct>()?
        .add_type::<ScriptOptionsAct>()?
        .add_type::<ScriptsAct>()?
        .add_type::<SkillsAct>()?
        .add_type::<SpammerAct>()?
        .add_type::<StatsAct>()?
        .build()?;
    kotlin::Installer::new("com.muktiarafi.turu.models", std::path::Path::new(&out))
        .plugin(JsonPlugin)
        .generate(&registry)?;
    Ok(())
}
