//! Writes the view models' Kotlin types (docs/NATIVE_UI_PROPOSAL.md, L4) into a folder: the
//! Android build runs it before compiling its Kotlin (`android/app/build.gradle.kts`,
//! `turuTypes`), so a record changed in Rust can't ship with stale types in the shell. Never
//! committed. `cargo run -p turu_frontend --no-default-features --features typegen --bin
//! turu-typegen -- <out dir>`.
//!
//! Beside the types it writes `ModelApi.kt`: each model's name on the control API with its view's
//! and its actions' types, taken from [`ModelView`] and [`ModelActs`], so a page names a model by
//! a value that carries its types (`Api.grabber`) and can't send it another model's action. And
//! what Android's shell activity and its Rust say to each other over JNI (`native_shell`).

use facet::Facet;
use facet_generate::generation::json::JsonPlugin;
use facet_generate::generation::kotlin;
use facet_generate::reflection::RegistryBuilder;
use std::fmt::Write as _;
use turu_frontend::models::{
    CopiedText, Model, ModelActs, ModelView,
    application_options::{AppOptionsAct, AppOptionsView},
    army::{ArmyAct, ArmyView},
    army_client::{ArmyClientAct, ArmyGearView, ClientLog},
    army_travel::{ArmyTravelAct, ArmyTravelView},
    auto::{AutoAct, AutoView},
    bank::{BankAct, BankView},
    butler::{ButlerAct, ButlerView},
    client_update::{ClientUpdateAct, ClientUpdateView},
    console::{ConsoleAct, ConsoleView},
    core_bots::{CoreBotsAct, CoreBotsView},
    cosmetics::{CosmeticsAct, CosmeticsView},
    crashes::{CrashText, CrashesAct, CrashesView},
    drops::{DropsAct, DropsView},
    fast_travel::{FastTravelAct, FastTravelView},
    game_options::{GameOptionsAct, GameOptionsView},
    grabber::{GrabberAct, GrabberView},
    hotkeys::{HotKeysAct, HotKeysView, KeyMoved},
    jump::{JumpAct, JumpView},
    junk::{JunkAct, JunkView},
    loader::{LoaderAct, LoaderView},
    loadouts::{LoadoutsAct, LoadoutsView},
    logs::{LogsAct, LogsView},
    notifications::{NotificationsAct, NotificationsView},
    notify_drop::{NotifyDropAct, NotifyDropView},
    packets::{InterceptorAct, InterceptorView, PacketLoggerAct, PacketLoggerView, SpammerAct, SpammerView},
    prompts::{PromptAnswer, PromptsAct, PromptsView},
    rail::{RailAct, RailView},
    repo::{LaunchScript, RepoAct, RepoView},
    runtime::{RuntimeAct, RuntimeView},
    scheduler::{SchedulerAct, SchedulerView},
    script_options::{ScriptOptionsAct, ScriptOptionsView},
    scripts::{ScriptsAct, ScriptsView},
    skills::{SkillsAct, SkillsView},
    stats::{StatsAct, StatsCsv, StatsView},
};
use turu_frontend::native_shell::{
    AccountsAnswer, AccountsAsk, AccountsView, ServersView, ShellAction, SlotMessage, TabRequest,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// A model's name as a Kotlin field: `game-options` is `gameOptions`.
fn field_name(name: &str) -> String {
    let mut parts = name.split('-');
    let mut field = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let mut chars = part.chars();
        field.extend(chars.next().map(|c| c.to_ascii_uppercase()));
        field.push_str(chars.as_str());
    }
    field
}

/// `--samples <dir>`: each model's view as the control API answers it (`{revision, view}`), read
/// in process from a fresh tab with no game ([`turu_frontend::client`]), as `<name>.json`. The
/// Android build's tests decode each into its generated type, so a view Rust writes that Kotlin
/// can't read fails there. The data folder is a temporary one: nothing of the user's is read.
fn write_samples(dir: &std::path::Path) -> Result<()> {
    let data = std::env::temp_dir().join(format!("turu-typegen-samples-{}", std::process::id()));
    turu_bot::paths::set_default_root(data.clone());
    std::fs::create_dir_all(dir)?;
    let client = turu_frontend::client::client();
    let tab = client.active_tab();
    for model in Model::ALL {
        if model == Model::ArmyClient {
            // Its rows answer actions; it has no view.
            continue;
        }
        let copy = client
            .view(tab, model)
            .map_err(|e| format!("model {}'s view: {e}", model.name()))?;
        let json = serde_json::json!({ "revision": copy.revision, "view": copy.view });
        std::fs::write(
            dir.join(format!("{}.json", model.name())),
            serde_json::to_vec_pretty(&json)?,
        )?;
    }
    let _ = std::fs::remove_dir_all(&data);
    Ok(())
}

/// The registry, and each model's entry for `ModelApi.kt`: its name, its view's and actions' types.
struct Gen {
    registry: RegistryBuilder,
    models: Vec<(&'static str, Option<String>, String)>,
}

/// A type's name as Kotlin has it (the last part of its Rust path).
fn short<T>() -> String {
    std::any::type_name::<T>()
        .rsplit("::")
        .next()
        .unwrap_or_default()
        .to_owned()
}

impl Gen {
    fn add<T: Facet<'static>>(mut self) -> Result<Self> {
        self.registry = self.registry.add_type::<T>()?;
        Ok(self)
    }

    /// A model with a view: both must be the same model's.
    fn model<V: ModelView + Facet<'static>, A: ModelActs + Facet<'static>>(mut self) -> Result<Self> {
        if V::MODEL != A::MODEL {
            return Err(format!("{} and {} aren't one model's", short::<V>(), short::<A>()).into());
        }
        self.models.push((A::MODEL.name(), Some(short::<V>()), short::<A>()));
        self.add::<V>()?.add::<A>()
    }

    /// A model with actions only.
    fn acts<A: ModelActs + Facet<'static>>(mut self) -> Result<Self> {
        self.models.push((A::MODEL.name(), None, short::<A>()));
        self.add::<A>()
    }

    /// `ModelApi.kt`: each model as a value of `Api`, named in camelCase ("fast-travel":
    /// `Api.fastTravel`).
    fn api(&self) -> String {
        let mut kotlin = String::from(
            "// Written by turu-typegen (frontend/src/bin/turu-typegen.rs): each model a shell reads, by its name on\n\
             // the control API, with its view's and its actions' types.\n\n\
             package com.muktiarafi.turu.models\n\n\
             import kotlinx.serialization.DeserializationStrategy\n\
             import kotlinx.serialization.SerializationStrategy\n\
             import kotlinx.serialization.serializer\n\n\
             /** A model: `GET /models/<name>` answers `V`, `POST /models/<name>/do` takes `A`. */\n\
             class ModelApi<V, A>(val name: String, val view: DeserializationStrategy<V>?, val act: SerializationStrategy<A>)\n\n\
             object Api {\n",
        );
        for (name, view, acts) in &self.models {
            let field = field_name(name);
            let (view_type, view) = match view {
                Some(v) => (v.as_str(), format!("serializer<{v}>()")),
                None => ("Nothing", "null".to_owned()),
            };
            let _ = writeln!(
                kotlin,
                "    val {field} = ModelApi<{view_type}, {acts}>(\"{name}\", {view}, serializer<{acts}>())"
            );
        }
        kotlin.push_str("\n    /** Every model, for the tests that decode each one's view. */\n    val all: List<ModelApi<*, *>> = listOf(");
        let fields: Vec<String> = self.models.iter().map(|(name, _, _)| field_name(name)).collect();
        kotlin.push_str(&fields.join(", "));
        kotlin.push_str(")\n}\n");
        kotlin
    }

    /// Every model has its types here: one added to `models::Model` but not to this list would
    /// have no Kotlin types, and a shell couldn't use it.
    fn check_every_model(&self) -> Result<()> {
        let missing: Vec<&str> = Model::ALL
            .iter()
            .map(|model| model.name())
            .filter(|name| !self.models.iter().any(|(listed, _, _)| listed == name))
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "no types generated for model(s) {}: add them in turu-typegen",
                missing.join(", ")
            )
            .into())
        }
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let out = args.next().ok_or("give the folder to write into")?;
    let samples = match (args.next().as_deref(), args.next()) {
        (Some("--samples"), Some(dir)) => Some(dir),
        (None, _) => None,
        _ => return Err("turu-typegen <out> [--samples <dir>]".into()),
    };
    let generated = Gen {
        registry: RegistryBuilder::new(),
        models: Vec::new(),
    }
    .model::<ScriptsView, ScriptsAct>()?
    .model::<AutoView, AutoAct>()?
    .model::<JumpView, JumpAct>()?
    .model::<JunkView, JunkAct>()?
    .model::<ConsoleView, ConsoleAct>()?
    .model::<PromptsView, PromptsAct>()?
    .model::<HotKeysView, HotKeysAct>()?
    .model::<PacketLoggerView, PacketLoggerAct>()?
    .model::<SpammerView, SpammerAct>()?
    .model::<InterceptorView, InterceptorAct>()?
    .model::<ArmyTravelView, ArmyTravelAct>()?
    .acts::<ArmyClientAct>()?
    .model::<BankView, BankAct>()?
    .model::<RailView, RailAct>()?
    .model::<ClientUpdateView, ClientUpdateAct>()?
    .model::<LogsView, LogsAct>()?
    .model::<CrashesView, CrashesAct>()?
    .model::<RepoView, RepoAct>()?
    .model::<GameOptionsView, GameOptionsAct>()?
    .model::<AppOptionsView, AppOptionsAct>()?
    .model::<CoreBotsView, CoreBotsAct>()?
    .model::<SchedulerView, SchedulerAct>()?
    .model::<SkillsView, SkillsAct>()?
    .model::<LoadoutsView, LoadoutsAct>()?
    .model::<FastTravelView, FastTravelAct>()?
    .model::<ButlerView, ButlerAct>()?
    .model::<CosmeticsView, CosmeticsAct>()?
    .model::<ScriptOptionsView, ScriptOptionsAct>()?
    .model::<RuntimeView, RuntimeAct>()?
    .model::<NotifyDropView, NotifyDropAct>()?
    .model::<LoaderView, LoaderAct>()?
    .model::<GrabberView, GrabberAct>()?
    .model::<ArmyView, ArmyAct>()?
    .model::<NotificationsView, NotificationsAct>()?
    .model::<StatsView, StatsAct>()?
    .model::<DropsView, DropsAct>()?
    // What actions answer.
    .add::<ArmyGearView>()?
    .add::<ClientLog>()?
    .add::<CopiedText>()?
    .add::<CrashText>()?
    .add::<StatsCsv>()?
    .add::<LaunchScript>()?
    .add::<KeyMoved>()?
    .add::<PromptAnswer>()?
    // What the shell's activity and its Rust say over JNI (`native_shell`).
    .add::<ShellAction>()?
    .add::<SlotMessage>()?
    .add::<TabRequest>()?
    .add::<AccountsView>()?
    .add::<ServersView>()?
    .add::<AccountsAsk>()?
    .add::<AccountsAnswer>()?;
    generated.check_every_model()?;
    let api = generated.api();
    let registry = generated.registry.build()?;
    let out = std::path::Path::new(&out);
    kotlin::Installer::new("com.muktiarafi.turu.models", out)
        .plugin(JsonPlugin)
        .generate(&registry)?;
    let dir = out.join("com/muktiarafi/turu/models");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("ModelApi.kt"), api)?;
    if let Some(samples) = samples {
        write_samples(std::path::Path::new(&samples))?;
    }
    Ok(())
}
