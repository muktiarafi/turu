//! Logging in and starting a script from the command line (VibeSkua `SkuaStartupHandler`):
//! once the game has loaded, log in on a server and start (or just load) a script, or a
//! Scheduler playlist (`--run-playlist`, Turu's).

use crate::Bot;
use crate::api::{player, servers};
use crate::events::Work;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// What to do at startup. VibeSkua's flags: `--user`, `--password`, `--server`, `--run-script`,
/// `--load-script`; Turu's `--run-playlist`, `--load-playlist`.
#[derive(Debug, Clone, Default)]
pub struct Startup {
    pub username: Option<String>,
    pub password: Option<String>,
    /// The server to log in on; "Twilly" when not given.
    pub server: Option<String>,
    /// The script to load.
    pub script: Option<PathBuf>,
    /// Start the script once logged in (`--run-script`), not only load it (`--load-script`).
    pub run_script: bool,
    /// The Scheduler playlist to load (VibeSkua's playlist JSON).
    pub playlist: Option<PathBuf>,
    /// Start the playlist once logged in (`--run-playlist`), instead of the script.
    pub run_playlist: bool,
}

impl Startup {
    /// Logs in at startup: a username and a password were given.
    pub fn logs_in(&self) -> bool {
        self.username
            .as_deref()
            .is_some_and(|u| !u.trim().is_empty())
            && self.password.as_deref().is_some_and(|p| !p.is_empty())
    }
}

/// Loads the script and the playlist and remembers the login; the login itself waits for the
/// game (`loaded`). Scripts are found as [`crate::script::resolve`] does.
pub fn configure(bot: &Bot, mut startup: Startup) {
    let scripts = bot.paths.scripts_dir();
    if let Some(script) = startup.script.take() {
        match crate::script::resolve(&scripts, &script) {
            Some(found) => {
                bot.scripts.load(&found);
                startup.script = Some(found);
            }
            None => bot.log(format!("No script at {}.", script.display())),
        }
    }
    if let Some(playlist) = &startup.playlist {
        match bot.scheduler.load(playlist, &scripts) {
            Ok(count) => bot.log(format!(
                "Loaded {count} script(s) from the playlist {}.",
                playlist.display()
            )),
            Err(e) => {
                bot.log(format!(
                    "Couldn't load the playlist {}: {e}",
                    playlist.display()
                ));
                startup.playlist = None;
            }
        }
    }
    // VibeSkua also keeps a username without a password; relogins would then send an empty
    // password instead of the one the game remembers from your own login.
    if startup.logs_in() {
        let (username, password) = (startup.username.as_deref(), startup.password.as_deref());
        servers::set_login_info(
            bot,
            username.unwrap_or_default(),
            password.unwrap_or_default(),
        );
        *bot.startup.lock() = Some(startup);
    }
}

/// `loaded`: the first time, log in on another thread.
pub(crate) fn on_game_loaded(bot: &Bot) {
    if bot.startup.lock().is_some() {
        bot.events.post(Work::AutoLogin);
    }
}

/// VibeSkua `SkuaStartupHandler.Login`: `EnsureRelogin(server)`, up to 20 s for the player,
/// then after 1 s starts the script.
pub(crate) fn auto_login(bot: &Arc<Bot>) {
    let Some(startup) = bot.startup.lock().take() else {
        return;
    };
    let server = startup
        .server
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Twilly");
    bot.log(format!("Logging in on {server}."));
    let ctx = servers::Ctx::new(bot);
    if let Err(e) = servers::ensure_relogin(&ctx, Some(server)) {
        bot.log(format!("Login failed: {e}"));
        return;
    }
    let mut waited = 0;
    while !player::playing(bot).unwrap_or(false) && waited < 80 {
        std::thread::sleep(Duration::from_millis(250));
        waited += 1;
    }
    if !player::playing(bot).unwrap_or(false) {
        bot.log("Couldn't log in.");
        return;
    }
    std::thread::sleep(Duration::from_secs(1));
    if startup.run_playlist && startup.playlist.is_some() {
        if let Err(e) = bot.scheduler.start(bot) {
            bot.log(format!("Couldn't start the playlist: {e}"));
        }
    } else if startup.run_script
        && startup.script.is_some()
        && let Err(e) = crate::script::start_silent(bot)
    {
        bot.log(format!("Couldn't start the script: {e}"));
    }
}
