use crate::backends::{
    DesktopExternalInterfaceProvider, DesktopFSCommandProvider, DesktopNavigatorInterface,
    DesktopUiBackend, PathAllowList,
};
use crate::cli::FilesystemAccessMode;
use crate::cli::GameModePreference;
use crate::custom_event::RuffleEvent;
use crate::gui::{FilePicker, MovieView};
use crate::preferences::GlobalPreferences;
use crate::{CALLSTACK, RENDER_INFO, SWF_INFO};
use anyhow::anyhow;
use ruffle_core::backend::navigator::{OwnedFuture, SocketMode};
use ruffle_core::config::Letterbox;
use ruffle_core::events::{GamepadButton, KeyCode};
use ruffle_core::font::DefaultFont;
use ruffle_core::{FloatDuration, LoadBehavior, Player, PlayerBuilder, PlayerEvent, PlayerThreadState};
use ruffle_frontend_utils::backends::audio::CpalAudioBackend;
use ruffle_frontend_utils::backends::navigator::{ExternalNavigatorBackend, FutureSpawner};
use ruffle_frontend_utils::bundle::source::BundleSourceError;
use ruffle_frontend_utils::bundle::{Bundle, BundleError};
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_frontend_utils::player_options::PlayerOptions;
use ruffle_frontend_utils::recents::Recent;
use ruffle_render::backend::RenderBackend;
use ruffle_render::quality::StageQuality;
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use ruffle_render_wgpu::clap::PowerPreference;
use ruffle_render_wgpu::descriptors::Descriptors;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use url::Url;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

/// Options used when creating a Player (& passed through to a PlayerBuilder).
/// These may be primed by command line arguments.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub player: PlayerOptions,
    pub proxy: Option<Url>,
    pub socket_allowed: HashSet<String>,
    pub tcp_connections: Option<SocketMode>,
    pub fullscreen: bool,
    pub save_directory: PathBuf,
    pub cache_directory: PathBuf,
    pub filesystem_access_mode: FilesystemAccessMode,
    pub gamepad_button_mapping: HashMap<GamepadButton, KeyCode>,
    pub avm2_optimizer_enabled: bool,
}

impl From<&GlobalPreferences> for LaunchOptions {
    fn from(value: &GlobalPreferences) -> Self {
        Self {
            player: PlayerOptions {
                parameters: value.cli.parameters().collect(),
                max_execution_duration: value.cli.max_execution_duration,
                base: value.cli.base.clone(),
                quality: value.cli.quality,
                align: value.cli.align,
                force_align: if value.cli.force_align {
                    Some(true)
                } else {
                    None
                },
                scale: value.cli.scale,
                force_scale: if value.cli.force_scale {
                    Some(true)
                } else {
                    None
                },
                upgrade_to_https: if value.cli.upgrade_to_https {
                    Some(true)
                } else {
                    None
                },
                load_behavior: value.cli.load_behavior,
                letterbox: value.cli.letterbox,
                spoof_url: value.cli.spoof_url.clone(),
                referer: value.cli.referer.clone(),
                cookie: value.cli.cookie.clone(),
                player_version: value.cli.player_version,
                player_runtime: value.cli.player_runtime,
                frame_rate: value.cli.frame_rate,
                dummy_external_interface: if value.cli.dummy_external_interface {
                    Some(true)
                } else {
                    None
                },
            },
            proxy: value.cli.proxy.clone(),
            fullscreen: value.cli.fullscreen,
            save_directory: value.cli.save_directory.clone(),
            cache_directory: value.cli.cache_directory.clone(),
            filesystem_access_mode: value.cli.filesystem_access_mode,
            socket_allowed: HashSet::from_iter(value.cli.socket_allow.iter().cloned()),
            tcp_connections: value.cli.tcp_connections,
            gamepad_button_mapping: HashMap::from_iter(value.cli.gamepad_button.iter().cloned()),
            avm2_optimizer_enabled: !value.cli.no_avm2_optimizer,
        }
    }
}

/// Represents a current Player and any associated state with that player,
/// which may be lost when this Player is closed (dropped)
struct ActivePlayer {
    id: PlayerId,
    /// Turu: the tab this player is the game of (when it plays AQW).
    tab: Option<crate::turu::TabId>,
    player: Arc<Mutex<Player>>,
    /// Turu: this player's thread-local state while another player is the active one (see
    /// `ThreadState`); the active player's is the thread's, and this is left default.
    thread_state: RefCell<ThreadState>,
    /// Turu: while it's a hidden tab's player, when it was last ticked and when it's due again
    /// (`PlayerController::tick_hidden`).
    hidden_tick: Cell<Option<(Instant, Instant)>>,
    /// Turu: the tab's bot's stats, which count this player's ticks.
    perf: Option<Arc<turu_bot::perf::PerfStats>>,

    #[cfg(target_os = "linux")]
    _gamemode_session: crate::dbus::GameModeSession,
}

impl ActivePlayer {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        opt: &LaunchOptions,
        event_loop: EventLoopProxy<RuffleEvent>,
        content_descriptor: &ContentDescriptor,
        window: Arc<Window>,
        descriptors: Arc<Descriptors>,
        movie_view: MovieView,
        font_database: Rc<fontdb::Database>,
        preferences: GlobalPreferences,
        file_picker: FilePicker,
        tab: crate::turu::TabId,
    ) -> Self {
        let player_id = PlayerId::new();
        let tab = crate::mugetsu::plays_aqw().then_some(tab);
        let bot = tab.and_then(crate::turu::tab_bot);
        let mut builder = PlayerBuilder::new();

        match CpalAudioBackend::new(preferences.output_device_name().as_deref()) {
            Ok(audio) => {
                builder = builder.with_audio(audio);
            }
            Err(e) => {
                tracing::error!("Unable to create audio device: {}", e);
            }
        };

        let mut content = PlayingContent::DirectFile(content_descriptor.clone());
        if content_descriptor.url.scheme() == "file"
            && let Ok(path) = content_descriptor.url.to_file_path()
        {
            match Bundle::from_path(&path) {
                Ok(bundle) => {
                    if bundle.warnings().is_empty() {
                        tracing::info!("Opening bundle at {path:?}");
                    } else {
                        // TODO: Show warnings to user (toast?)
                        tracing::warn!("Opening bundle at {path:?} with warnings");
                        for warning in bundle.warnings() {
                            tracing::warn!("{warning}");
                        }
                    }
                    content = PlayingContent::Bundle(content_descriptor.clone(), Box::new(bundle));
                }
                Err(
                    BundleError::BundleDoesntExist
                    | BundleError::InvalidSource(BundleSourceError::UnknownSource),
                ) => {
                    // Do nothing and carry on opening it as a swf - this likely isn't a bundle at all
                }
                Err(e) => {
                    // TODO: Visible popup when a bundle (or regular file) fails to open
                    tracing::error!("Couldn't open bundle at {path:?}: {e}");
                }
            }
        }

        let recent_limit = preferences.recent_limit();
        if let Err(e) = preferences.write_recents(|writer| {
            writer.push(
                Recent {
                    content_descriptor: content_descriptor.clone(),
                    name: content.name(),
                },
                recent_limit,
            )
        }) {
            tracing::warn!("Couldn't update recents: {e}");
        }

        let opt = match &content {
            PlayingContent::DirectFile(_) => Cow::Borrowed(opt),
            PlayingContent::Bundle(_, bundle) => {
                let player = opt.player.or(&bundle.information().player);

                Cow::Owned(LaunchOptions {
                    player,
                    proxy: opt.proxy.clone(),
                    socket_allowed: opt.socket_allowed.clone(),
                    tcp_connections: opt.tcp_connections,
                    fullscreen: opt.fullscreen,
                    save_directory: opt.save_directory.clone(),
                    cache_directory: opt.cache_directory.clone(),
                    filesystem_access_mode: opt.filesystem_access_mode,
                    gamepad_button_mapping: opt.gamepad_button_mapping.clone(),
                    avm2_optimizer_enabled: opt.avm2_optimizer_enabled,
                })
            }
        };

        let future_spawner = WinitExecutor {
            event_loop: event_loop.clone(),
            player_id,
        };
        let movie_url = content.initial_swf_url().clone();
        let readable_name = content.name();
        let initial_allow_list = PathAllowList::new(content_descriptor);
        let navigator = ExternalNavigatorBackend::new(
            opt.player
                .base
                .to_owned()
                .unwrap_or_else(|| movie_url.clone()),
            opt.player.referer.clone(),
            opt.player.cookie.clone(),
            future_spawner,
            opt.proxy.clone(),
            opt.player.upgrade_to_https.unwrap_or_default(),
            opt.socket_allowed.clone(),
            opt.tcp_connections.unwrap_or(SocketMode::Ask),
            Rc::new(content),
            DesktopNavigatorInterface::new(
                preferences.clone(),
                event_loop.clone(),
                initial_allow_list,
                opt.filesystem_access_mode,
            ),
        );
        // Turu: the tab's game socket goes through its Interceptor capture.
        let navigator = crate::capture::CaptureNavigator::new(
            navigator,
            bot.as_ref().map(|bot| bot.capture.clone()),
        );

        if cfg!(feature = "external_video") && preferences.openh264_enabled() {
            #[cfg(feature = "external_video")]
            {
                use ruffle_video_external::{
                    backend::ExternalVideoBackend, decoder::openh264::OpenH264Codec,
                };
                let openh264 = tokio::task::block_in_place(|| {
                    OpenH264Codec::load(&opt.cache_directory.join("video"))
                });
                let backend = match openh264 {
                    Ok(codec) => ExternalVideoBackend::new_with_openh264(codec),
                    Err(e) => {
                        tracing::error!("Failed to load OpenH264: {}", e);
                        ExternalVideoBackend::new()
                    }
                };
                builder = builder.with_video(backend);
            }
        } else {
            #[cfg(feature = "software_video")]
            {
                builder =
                    builder.with_video(ruffle_video_software::backend::SoftwareVideoBackend::new());
            }
        }

        #[cfg_attr(not(target_os = "linux"), allow(unused))]
        let gamemode_enable = match preferences.gamemode_preference() {
            GameModePreference::Default => {
                preferences.graphics_power_preference() == PowerPreference::High
            }
            GameModePreference::On => {
                if cfg!(not(target_os = "linux")) {
                    tracing::warn!("Cannot enable GameMode, as it is supported only on Linux");
                }
                true
            }
            GameModePreference::Off => false,
        };

        let renderer = WgpuRenderBackend::new(descriptors, movie_view)
            .map_err(|e| anyhow!(e.to_string()))
            .expect("Couldn't create wgpu rendering backend");
        RENDER_INFO.with(|i| *i.borrow_mut() = Some(renderer.debug_info().to_string()));

        if let Some(bot) = &bot {
            // Turu: skua.swf talks to the tab's bot through ExternalInterface.
            builder = builder.with_external_interface(Box::new(crate::turu::TuruExternalInterface {
                bot: bot.clone(),
            }));
        } else if opt.player.dummy_external_interface.unwrap_or_default() {
            builder = builder.with_external_interface(Box::new(DesktopExternalInterfaceProvider {
                spoof_url: opt.player.spoof_url.clone(),
            }));
        }

        if !opt.gamepad_button_mapping.is_empty() {
            builder = builder.with_gamepad_button_mapping(opt.gamepad_button_mapping.clone());
        }

        let (notification_sender, notification_recv) = async_channel::unbounded();

        let event_loop2 = event_loop.clone();
        tokio::spawn(async move {
            while let Ok(notification) = notification_recv.recv().await {
                let event = RuffleEvent::PlayerNotification(notification);
                match event_loop2.send_event(event) {
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        });

        builder = builder
            .with_navigator(navigator)
            .with_renderer(renderer)
            .with_storage(preferences.storage_backend().create_backend(&opt))
            .with_notification_sender(notification_sender)
            .with_fs_commands(Box::new(DesktopFSCommandProvider {
                event_loop: event_loop.clone(),
            }))
            .with_ui(
                DesktopUiBackend::new(
                    window.clone(),
                    event_loop.clone(),
                    font_database,
                    preferences,
                    file_picker,
                )
                .expect("Couldn't create ui backend"),
            )
            .with_autoplay(true)
            .with_letterbox(opt.player.letterbox.unwrap_or(Letterbox::On))
            .with_max_execution_duration(opt.player.max_execution_duration.unwrap_or(Duration::MAX))
            .with_quality(opt.player.quality.unwrap_or(StageQuality::High))
            .with_align(
                opt.player.align.unwrap_or_default(),
                opt.player.force_align.unwrap_or_default(),
            )
            .with_scale_mode(
                opt.player.scale.unwrap_or_default(),
                opt.player.force_scale.unwrap_or_default(),
            )
            .with_fullscreen(opt.fullscreen)
            .with_load_behavior(opt.player.load_behavior.unwrap_or(LoadBehavior::Streaming))
            .with_spoofed_url(opt.player.spoof_url.clone().map(|url| url.to_string()))
            .with_page_url(opt.player.spoof_url.clone().map(|url| url.to_string()))
            .with_player_version(opt.player.player_version)
            .with_player_runtime(opt.player.player_runtime.unwrap_or_default())
            .with_frame_rate(opt.player.frame_rate)
            .with_avm2_optimizer_enabled(opt.avm2_optimizer_enabled);
        let player = builder.build();
        if let (Some(bot), Some(tab)) = (&bot, tab) {
            bot.set_host(Some(Arc::new(crate::turu::DesktopFlashHost::new(
                event_loop.clone(),
                tab,
            ))));
        }

        window.set_title(&crate::window_title()); // AQW build: name the window after the launcher, not the SWF.

        SWF_INFO.with(|i| *i.borrow_mut() = Some(readable_name));

        let on_metadata = move |swf_header: &ruffle_core::swf::HeaderExt| {
            let _ = event_loop.send_event(RuffleEvent::OnMetadata(swf_header.clone()));
        };

        {
            let mut player_lock = player.lock().expect("Player lock must be available");
            CALLSTACK.with(|callstack| {
                *callstack.borrow_mut() = Some(player_lock.callstack());
            });
            player_lock.fetch_root_movie(
                movie_url.to_string(),
                opt.player.parameters.to_owned(),
                Box::new(on_metadata),
            );

            player_lock.set_default_font(
                DefaultFont::Serif,
                vec![
                    "Times New Roman".into(),
                    "Tinos".into(),
                    "Liberation Serif".into(),
                    "DejaVu Serif".into(),
                ],
            );
            player_lock.set_default_font(
                DefaultFont::Sans,
                vec![
                    "Arial".into(),
                    "Arimo".into(),
                    "Liberation Sans".into(),
                    "DejaVu Sans".into(),
                ],
            );
            player_lock.set_default_font(
                DefaultFont::Typewriter,
                vec![
                    "Courier New".into(),
                    "Cousine".into(),
                    "Liberation Mono".into(),
                    "DejaVu Sans Mono".into(),
                ],
            );
            player_lock.set_default_font(
                DefaultFont::JapaneseGothic,
                vec![
                    "ヒラギノ角ゴ Pro W3".into(), // Mac with Japanese environment
                    "MS UI Gothic".into(),        // Windows
                    "Noto Sans CJK JP".into(),    // Linux
                    "Arial Unicode MS".into(),    // Mac fallback
                ],
            );
            player_lock.set_default_font(
                DefaultFont::JapaneseGothicMono,
                vec![
                    "Osaka－等幅".into(),      // Mac with Japanese environment
                    "MS Gothic".into(),        // Windows
                    "Noto Sans CJK JP".into(), // Linux
                    "Arial Unicode MS".into(), // Mac fallback
                ],
            );
            player_lock.set_default_font(
                DefaultFont::JapaneseMincho,
                vec![
                    "ヒラギノ明朝 Pro W3".into(), // Mac with Japanese environment
                    "MS PMincho".into(),          // Windows
                    "Noto Sans CJK JP".into(),    // Linux
                    "Arial Unicode MS".into(),    // Mac fallback
                ],
            );
        }

        Self {
            id: player_id,
            tab,
            player,
            thread_state: RefCell::default(),
            hidden_tick: Cell::new(None),
            perf: bot.as_ref().map(|bot| bot.flash.perf().clone()),
            #[cfg(target_os = "linux")]
            _gamemode_session: crate::dbus::GameModeSession::new(gamemode_enable),
        }
    }
}

/// Turu: the longest a hidden tab's player goes without a tick, so its sockets (the game's
/// packets) and timers are serviced at least this often.
const HIDDEN_TICK_MAX: Duration = Duration::from_millis(50);

/// Turu: what one player keeps in the thread: the core's (`PlayerThreadState`) and its
/// renderer's command segments not submitted yet (`ruffle_render_wgpu::segments`), which
/// another player's submit must not take.
#[derive(Default)]
struct ThreadState {
    core: PlayerThreadState,
    segments: ruffle_render_wgpu::segments::ThreadState,
}

impl ThreadState {
    /// Swaps this state with the thread's.
    fn swap(&mut self) {
        self.core.swap();
        ruffle_render_wgpu::segments::swap(&mut self.segments);
    }
}

/// Turu: while alive, a player that isn't the active one has its own thread-local state.
struct SwappedIn<'a>(&'a RefCell<ThreadState>);

impl<'a> SwappedIn<'a> {
    fn new(state: &'a RefCell<ThreadState>) -> Self {
        state.borrow_mut().swap();
        Self(state)
    }
}

impl Drop for SwappedIn<'_> {
    fn drop(&mut self) {
        self.0.borrow_mut().swap();
    }
}

/// Turu: a tab's locked player. Unless it's the active tab's, its own thread-local state is
/// swapped in until this drops, as its code may run.
pub struct TabPlayer<'a> {
    player: MutexGuard<'a, Player>,
    _swapped: Option<SwappedIn<'a>>,
}

impl Deref for TabPlayer<'_> {
    type Target = Player;

    fn deref(&self) -> &Player {
        &self.player
    }
}

impl DerefMut for TabPlayer<'_> {
    fn deref_mut(&mut self) -> &mut Player {
        &mut self.player
    }
}

impl ActivePlayer {
    /// Turu: locks the player, swapping its thread-local state in unless it's the active one.
    fn lock(&self, active: bool) -> TabPlayer<'_> {
        TabPlayer {
            _swapped: (!active).then(|| SwappedIn::new(&self.thread_state)),
            player: self
                .player
                .try_lock()
                .expect("Player lock must be available"),
        }
    }

    /// Turu: detaches the tab's bot, which would otherwise take the closed game for a
    /// disconnect.
    fn detach_bot(&self) {
        if let Some(bot) = self.tab.and_then(crate::turu::tab_bot) {
            bot.set_host(None);
        }
    }
}

/// Owner of a Ruffle Player (via ActivePlayer),
/// responsible for either creating, destroying or communicating with that player.
///
/// Turu: one player per tab; `get` and `handle_event` are the active tab's.
pub struct PlayerController {
    players: Vec<ActivePlayer>,
    /// Index of the active player in `players`.
    active: usize,
    event_loop: EventLoopProxy<RuffleEvent>,
    window: Arc<Window>,
    descriptors: Arc<Descriptors>,
    font_database: Rc<fontdb::Database>,
    preferences: GlobalPreferences,
    file_picker: FilePicker,
}

impl PlayerController {
    pub fn new(
        event_loop: EventLoopProxy<RuffleEvent>,
        window: Arc<Window>,
        descriptors: Arc<Descriptors>,
        font_database: fontdb::Database,
        preferences: GlobalPreferences,
        file_picker: FilePicker,
    ) -> Self {
        crate::turu::set_event_loop(event_loop.clone());
        Self {
            players: Vec::new(),
            active: 0,
            event_loop,
            window,
            descriptors,
            font_database: Rc::new(font_database),
            preferences,
            file_picker,
        }
    }

    /// Opens the movie in the active tab (replacing its player, if any).
    pub fn create(
        &mut self,
        opt: &LaunchOptions,
        content_descriptor: &ContentDescriptor,
        movie_view: MovieView,
    ) {
        let tab = crate::turu::active_tab();
        let player = self.new_player(opt, content_descriptor, movie_view, tab);
        if let Some(slot) = self.players.get_mut(self.active) {
            slot.detach_bot();
            // The thread gets a new player's state; the old one's goes with it.
            slot.thread_state.borrow_mut().swap();
            *slot = player;
        } else {
            self.players.push(player);
            self.active = self.players.len() - 1;
        }
    }

    /// Turu: opens the movie in a new tab's player and makes it the active one.
    pub fn add_tab(
        &mut self,
        opt: &LaunchOptions,
        content_descriptor: &ContentDescriptor,
        movie_view: MovieView,
        tab: crate::turu::TabId,
    ) {
        let player = self.new_player(opt, content_descriptor, movie_view, tab);
        self.players.push(player);
        self.select_tab(tab);
    }

    /// Turu: makes the tab's player the active one (the others are muted); false when it
    /// has none.
    pub fn select_tab(&mut self, tab: crate::turu::TabId) -> bool {
        let Some(i) = self.players.iter().position(|p| p.tab == Some(tab)) else {
            return false;
        };
        if i != self.active {
            // The old active player's state goes into its slot, the new one's into the thread.
            if let Some(old) = self.players.get(self.active) {
                old.thread_state.borrow_mut().swap();
                // Hidden from now on: `tick_hidden` starts timing it afresh.
                old.hidden_tick.set(None);
            }
            self.players[i].thread_state.borrow_mut().swap();
            self.active = i;
        }
        true
    }

    /// Turu: closes the tab's player.
    pub fn remove_tab(&mut self, tab: crate::turu::TabId) {
        let Some(i) = self.players.iter().position(|p| p.tab == Some(tab)) else {
            return;
        };
        let was_active = i == self.active;
        if was_active {
            // The thread's state is the closed player's: it goes with it.
            self.players[i].thread_state.borrow_mut().swap();
        }
        let player = self.players.remove(i);
        player.detach_bot();
        if self.active > i || self.active >= self.players.len() {
            self.active = self.active.saturating_sub(1);
        }
        if was_active && let Some(new) = self.players.get(self.active) {
            new.thread_state.borrow_mut().swap();
        }
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    /// Turu: every player (to tick them all), the active one first.
    /// Use one at a time: each other player's state is swapped in while its guard lives.
    pub fn all(&self) -> impl Iterator<Item = (bool, TabPlayer<'_>)> {
        let active = self.active;
        let order = std::iter::once(active).chain((0..self.players.len()).filter(move |i| *i != active));
        order.filter_map(move |i| Some((i == active, self.players.get(i)?.lock(i == active))))
    }

    /// Turu: ticks the hidden tabs' players whose next frame or timer is due, or that waited
    /// [`HIDDEN_TICK_MAX`], rather than all of them on every pass of the event loop (thousands
    /// a second while the mouse moves); returns when the next one is due.
    pub fn tick_hidden(&self, now: Instant) -> Option<Instant> {
        let mut soonest: Option<Instant> = None;
        for (i, slot) in self.players.iter().enumerate() {
            if i == self.active {
                continue;
            }
            let next = match slot.hidden_tick.get() {
                Some((last, due)) if now >= due => {
                    let mut player = slot.lock(false);
                    let started = Instant::now();
                    player.tick(FloatDuration::from_std(now - last));
                    if let Some(perf) = &slot.perf {
                        perf.record_tick(started.elapsed());
                    }
                    now + player.time_til_next_frame().min(HIDDEN_TICK_MAX)
                }
                Some((_, due)) => {
                    soonest = Some(soonest.map_or(due, |t| t.min(due)));
                    continue;
                }
                // Just hidden: the active tab's loop ticked it until now.
                None => now + HIDDEN_TICK_MAX,
            };
            slot.hidden_tick.set(Some((now, next)));
            soonest = Some(soonest.map_or(next, |t| t.min(next)));
        }
        soonest
    }

    /// Turu: gives the mouse to the active tab's player (when the cursor is in the window) and
    /// takes it from the others, which then stop hit-testing it and drop what they hovered.
    pub fn give_mouse(&self, in_window: bool) {
        for (active, mut player) in self.all() {
            let in_stage = active && in_window;
            if player.mouse_in_stage() != in_stage {
                player.set_mouse_in_stage(in_stage);
                if !in_stage {
                    player.handle_event(PlayerEvent::MouseLeave);
                }
            }
        }
    }

    /// Turu: the tab's player.
    pub fn get_tab(&self, tab: crate::turu::TabId) -> Option<TabPlayer<'_>> {
        let i = self.players.iter().position(|p| p.tab == Some(tab))?;
        Some(self.players[i].lock(i == self.active))
    }

    fn new_player(
        &self,
        opt: &LaunchOptions,
        content_descriptor: &ContentDescriptor,
        movie_view: MovieView,
        tab: crate::turu::TabId,
    ) -> ActivePlayer {
        ActivePlayer::new(
            opt,
            self.event_loop.clone(),
            content_descriptor,
            self.window.clone(),
            self.descriptors.clone(),
            movie_view,
            self.font_database.clone(),
            self.preferences.clone(),
            self.file_picker.clone(),
            tab,
        )
    }

    /// Closes every player.
    pub fn destroy(&mut self) {
        // The thread's state is the active player's: it goes with it.
        if let Some(active) = self.players.get(self.active) {
            active.thread_state.borrow_mut().swap();
        }
        for player in self.players.drain(..) {
            player.detach_bot();
        }
        self.active = 0;
    }

    /// Turu: the active tab's player ticked for `took` (counted in its bot's stats).
    pub fn record_tick(&self, took: Duration) {
        if let Some(perf) = self.players.get(self.active).and_then(|p| p.perf.as_ref()) {
            perf.record_tick(took);
        }
    }

    pub fn get(&self) -> Option<MutexGuard<'_, Player>> {
        match self.players.get(self.active) {
            None => None,
            // We don't want to return None when the lock fails to grab as that's a fatal error, not a lack of player
            Some(player) => Some(
                player
                    .player
                    .try_lock()
                    .expect("Player lock must be available"),
            ),
        }
    }

    pub fn handle_event(&self, event: PlayerEvent) -> bool {
        if let Some(mut player) = self.get()
            && player.is_playing()
        {
            return player.handle_event(event);
        }

        false
    }

    pub fn poll(&self, task: PlayerRunnable) {
        // Only run the task if it matches our current player;
        // otherwise it is stale, and should be cancelled (which
        // happens implicitly on drop).
        let id = *task.0.metadata();
        if let Some(i) = self.players.iter().position(|player| id == player.id) {
            // Turu: another tab's future runs with that player's thread-local state.
            let _swapped =
                (i != self.active).then(|| SwappedIn::new(&self.players[i].thread_state));
            task.0.run();
        }
    }
}

/// A unique identifier for a given `Player` instance.
/// Used to track which player any currently executing future is bound to.
#[derive(Copy, Clone, Eq, PartialEq)]
struct PlayerId(i64);

impl PlayerId {
    fn new() -> Self {
        use std::sync::atomic::{AtomicI64, Ordering};

        static NEXT: AtomicI64 = AtomicI64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        assert!(id >= 0, "PlayerId overflowed!");
        Self(id)
    }
}

/// A `Player`-bound future that is currently running.
pub struct PlayerRunnable(async_task::Runnable<PlayerId>);

/// A bare-bones executor that schedules tasks on the winit event loop.
struct WinitExecutor {
    event_loop: EventLoopProxy<RuffleEvent>,
    player_id: PlayerId,
}

impl<E: std::error::Error + 'static> FutureSpawner<E> for WinitExecutor {
    fn spawn(&self, future: OwnedFuture<(), E>) {
        // Discard any errors.
        let future = async {
            if let Err(e) = future.await {
                tracing::error!("Async error: {}", e);
            }
        };

        let event_loop = self.event_loop.clone();
        let scheduler = move |task| {
            let event = RuffleEvent::TaskPoll(PlayerRunnable(task));
            if event_loop.send_event(event).is_err() {
                tracing::error!("Couldn't schedule task - event loop is closed");
            }
        };

        let (runnable, task) = async_task::Builder::new()
            .metadata(self.player_id)
            .spawn_local(|_| future, scheduler);

        // The future should run in the background.
        task.detach();
        // Immediately schedule the future to be polled for the first time.
        runnable.schedule();
    }
}
