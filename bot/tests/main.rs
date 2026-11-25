//! The engine's tests, one module per file: one test binary for all of them, so a change
//! links one binary instead of one per file (and macOS checks one new program at its first
//! launch, not each). cargo-nextest still runs every test in its own process; `cargo test` runs
//! them all in one, so the bots of every module share one data folder (`common::data_dir`).
//!
//! `autotests = false` in Cargo.toml: a new file here needs its `mod` line below
//! (`every_test_file_is_a_module` checks).

mod common;

mod army_queue;
mod auto;
mod batched_reads;
mod boosts;
mod butler;
mod cbo;
mod control_api;
mod core_advanced;
mod core_dailies;
mod core_enhancement;
mod core_story;
mod core_tasks;
mod cosmetics;
mod counter_attack;
mod engine_a_api;
mod engine_a_packets;
mod engine_b_items;
mod engine_b_skills;
mod ensure_complete;
mod event_waits;
mod gear;
mod handlers;
mod host_swf;
mod junk;
mod loader_grabber;
mod monitor;
mod notifications;
mod player_packets;
mod reputation_auras;
mod runtime;
mod scheduler;
mod script_repo;
mod scripts_transpile;
mod servers;
mod settings_merge;
mod skill_sets;
mod spammer_interceptor;
mod stats_rates;
mod target_auras;

#[test]
fn every_test_file_is_a_module() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let main = include_str!("main.rs");
    for entry in std::fs::read_dir(&dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let Some(module) = name.strip_suffix(".rs") else {
            continue;
        };
        if module != "main" {
            assert!(
                main.lines().any(|line| line == format!("mod {module};")),
                "bot/tests/{name} isn't in bot/tests/main.rs: add `mod {module};` there"
            );
        }
    }
}
