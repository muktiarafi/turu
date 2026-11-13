//! Calls from `skua.swf` (`ExternalInterface.call`) and the bot's event bus
//! (VibeSkua: `ScriptInterface.HandleFlashCall`, `IScriptEvent`).

use crate::Bot;
use crate::models::ItemBase;
use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Something that happened in the game. Scripts subscribe with `bot.events.on(...)`.
#[derive(Debug, Clone)]
pub enum BotEvent {
    /// `skua.swf` finished loading the game client.
    GameLoaded,
    Logout,
    PlayerDeath,
    MonsterKilled {
        map_id: i64,
    },
    QuestAccepted {
        quest_id: i64,
    },
    QuestTurnedIn {
        quest_id: i64,
    },
    MapChanged {
        map: String,
    },
    CellChanged {
        map: String,
        cell: String,
        pad: String,
    },
    /// An item appeared in the drop stack (`dropItem`).
    ItemDropped {
        item: ItemBase,
    },
    /// An item was added to the inventory or temp inventory (`addItems`): VibeSkua's
    /// `ItemDropped` with `addedToInv = true`. `quantity_now` is the stack's new quantity.
    ItemAdded {
        item: ItemBase,
        quantity_now: i64,
    },
    /// A drop went to the bank (`addItems` / `getDrop` with `bBank`).
    ItemAddedToBank {
        item: ItemBase,
        quantity_now: i64,
    },
    /// An outgoing `buyItem` packet (VibeSkua `TryBuyItem`).
    TryBuyItem {
        shop_id: i64,
        item_id: i64,
        shop_item_id: i64,
    },
    ItemBought,
    /// `sellItem` (VibeSkua `ItemSoldMessage`): `quantity_now` is what is left of the stack,
    /// `cost` what the sale paid, in AC when `is_ac`.
    ItemSold {
        char_item_id: i64,
        quantity_sold: i64,
        quantity_now: i64,
        cost: i64,
        is_ac: bool,
    },
    BankLoaded,
    ShopLoaded,
    ConnectionLost,
    /// Auto-relogin started (VibeSkua `ReloginTriggered`); `was_kicked` when the login screen
    /// showed the kick warning.
    ReloginTriggered {
        was_kicked: bool,
    },
    /// `loginResponse`: logged in as `username`.
    Login {
        username: String,
    },
    /// A monster prepares a counter attack (`faded = false`) or it faded (`faded = true`).
    CounterAttack {
        faded: bool,
    },
    /// The server marked us AFK (`uotls ... afk:true`).
    PlayerAfk,
    /// An `event` packet with `zoneSet` (VibeSkua `RunToArea`).
    RunToArea {
        zone: String,
    },
    /// A server-to-client extension packet (`pext`), already parsed.
    ExtensionPacket(Value),
    /// A client-to-server `%xt%zm%...%` packet.
    PacketSent(String),
    /// The script finished, failed or was stopped (VibeSkua `ScriptStoppedMessage`).
    ScriptStopped,
}

impl BotEvent {
    /// The name scripts use: `bot.events.on("monsterKilled", ...)`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::GameLoaded => "gameLoaded",
            Self::Logout => "logout",
            Self::PlayerDeath => "playerDeath",
            Self::MonsterKilled { .. } => "monsterKilled",
            Self::QuestAccepted { .. } => "questAccepted",
            Self::QuestTurnedIn { .. } => "questTurnedIn",
            Self::MapChanged { .. } => "mapChanged",
            Self::CellChanged { .. } => "cellChanged",
            Self::ItemDropped { .. } => "itemDropped",
            Self::ItemAdded { .. } => "itemAdded",
            Self::ItemAddedToBank { .. } => "itemAddedToBank",
            Self::TryBuyItem { .. } => "tryBuyItem",
            Self::ItemBought => "itemBought",
            Self::ItemSold { .. } => "itemSold",
            Self::BankLoaded => "bankLoaded",
            Self::ShopLoaded => "shopLoaded",
            Self::ConnectionLost => "connectionLost",
            Self::ReloginTriggered { .. } => "reloginTriggered",
            Self::Login { .. } => "login",
            Self::CounterAttack { .. } => "counterAttack",
            Self::PlayerAfk => "playerAFK",
            Self::RunToArea { .. } => "runToArea",
            Self::ExtensionPacket(_) => "extensionPacket",
            Self::PacketSent(_) => "packetSent",
            Self::ScriptStopped => "scriptStopped",
        }
    }

    /// The payload passed to script handlers.
    pub fn payload(&self) -> Value {
        use serde_json::json;
        match self {
            Self::MonsterKilled { map_id } => json!({ "mapId": map_id }),
            Self::QuestAccepted { quest_id } | Self::QuestTurnedIn { quest_id } => {
                json!({ "questId": quest_id })
            }
            Self::MapChanged { map } => json!({ "map": map }),
            Self::CellChanged { map, cell, pad } => json!({ "map": map, "cell": cell, "pad": pad }),
            Self::ItemDropped { item } => serde_json::to_value(item).unwrap_or(Value::Null),
            Self::ItemAdded { item, quantity_now }
            | Self::ItemAddedToBank { item, quantity_now } => {
                let mut value = serde_json::to_value(item).unwrap_or(Value::Null);
                if let Value::Object(map) = &mut value {
                    map.insert("quantityNow".into(), json!(quantity_now));
                }
                value
            }
            Self::TryBuyItem {
                shop_id,
                item_id,
                shop_item_id,
            } => {
                json!({ "shopId": shop_id, "itemId": item_id, "shopItemId": shop_item_id })
            }
            Self::ItemSold {
                char_item_id,
                quantity_sold,
                quantity_now,
                cost,
                is_ac,
            } => json!({
                "charItemId": char_item_id,
                "quantitySold": quantity_sold,
                "currentQuantity": quantity_now,
                "cost": cost,
                "isAC": is_ac,
            }),
            Self::ReloginTriggered { was_kicked } => json!({ "wasKicked": was_kicked }),
            Self::Login { username } => json!({ "username": username }),
            Self::CounterAttack { faded } => json!({ "faded": faded }),
            Self::RunToArea { zone } => json!({ "zone": zone }),
            Self::ExtensionPacket(p) => p.clone(),
            Self::PacketSent(p) => Value::String(p.clone()),
            _ => Value::Null,
        }
    }
}

type Handler = Box<dyn Fn(&BotEvent) + Send + Sync>;

/// Work the bot must do on its own thread because it calls into Flash.
#[derive(Debug)]
pub enum Work {
    /// `requestLoadGame`: call `loadClient`.
    LoadClient,
    /// `pre-load`: call `setBackgroundValues`.
    SetBackground,
    /// `loaded`: the command-line login (`startup`).
    AutoLogin,
    /// `addItems`: look the item up in the inventory, temp inventory or bank and emit
    /// [`BotEvent::ItemAdded`] / [`BotEvent::ItemAddedToBank`]. `fallback` is what the
    /// packet itself said about the item.
    ItemAdded {
        item_id: i64,
        quantity_now: i64,
        fallback: Box<ItemBase>,
    },
    /// `getDrop` with `bBank`: look the item up in the bank and emit `ItemAddedToBank`.
    BankItemAdded { item_id: i64, quantity_now: i64 },
    /// A counter attack began (save and cancel the target) or faded (attack it again), with
    /// the counter handler on (VibeSkua `ScriptCombat.CounterAttack`).
    CounterAttack { faded: bool },
}

pub struct EventBus {
    handlers: Mutex<Vec<(u64, Handler)>>,
    next_id: AtomicU64,
    work_tx: Sender<Work>,
    pub(crate) work_rx: Receiver<Work>,
}

impl Default for EventBus {
    fn default() -> Self {
        let (work_tx, work_rx) = unbounded();
        Self {
            handlers: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
            work_tx,
            work_rx,
        }
    }
}

impl EventBus {
    /// Subscribes to every event; returns an id for [`EventBus::unsubscribe`].
    /// Handlers run on whatever thread emitted the event and must not block.
    pub fn subscribe(&self, handler: impl Fn(&BotEvent) + Send + Sync + 'static) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.handlers.lock().push((id, Box::new(handler)));
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        self.handlers.lock().retain(|(i, _)| *i != id);
    }

    pub fn emit(&self, event: BotEvent) {
        for (_, handler) in self.handlers.lock().iter() {
            handler(&event);
        }
    }

    pub(crate) fn post(&self, work: Work) {
        let _ = self.work_tx.send(work);
    }
}

/// Entry point for `ExternalInterface.call(name, rest)` from `skua.swf`.
pub(crate) fn on_flash_call(bot: &Arc<Bot>, name: &str, args: Vec<Value>) {
    // `Externalizer.call(name, ...rest)` passes `rest` as one array argument.
    let args = match <[Value; 1]>::try_from(args) {
        Ok([Value::Array(inner)]) => inner,
        Ok([single]) => vec![single],
        Err(args) => args,
    };
    let arg_str = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or_default();

    // Parity mode: Turu's SWF's events are only compared; VibeSkua's are compared and acted on.
    if let Some(parity) = bot.flash.parity() {
        let (side, event) = match name.strip_prefix(crate::parity::PREFIX) {
            Some(event) => (crate::parity::Side::Turu, event),
            None => (crate::parity::Side::VibeSkua, name),
        };
        for line in parity.event(side, event, &args) {
            bot.logs.debug.push(line);
        }
        // The player's packets stand for Turu's side (R5): its SWF's own aren't compared.
        if side == crate::parity::Side::Turu
            && matches!(event, "packet" | "pext")
            && bot.flash.player_packets()
        {
            return;
        }
        if side == crate::parity::Side::Turu {
            if event == "error"
                && let Some(line) = bot.flash.host_error_line(arg_str(0), arg_str(1))
            {
                bot.logs.debug.push(line);
            }
            return;
        }
        if name == "requestLoadGame" {
            parity.restart();
        }
    }

    match name {
        "requestLoadGame" => {
            bot.flash.forget_host_info();
            bot.state.lock().client_requested = true;
            bot.events.post(Work::LoadClient);
        }
        "pre-load" => bot.events.post(Work::SetBackground),
        "loaded" => {
            bot.state.lock().game_loaded = true;
            bot.logs.debug.push("Game client loaded");
            bot.events.emit(BotEvent::GameLoaded);
            crate::startup::on_game_loaded(bot);
        }
        // The player's packets take their place when it has them (R5): `player_packet_sent`.
        "pext" if bot.flash.player_packets() => {}
        "packet" if bot.flash.player_packets() => {}
        "pext" => match serde_json::from_str::<Value>(arg_str(0)) {
            Ok(packet) => crate::api::packets::on_extension_packet(bot, packet),
            Err(e) => tracing::debug!("Bad pext JSON: {e}"),
        },
        "packet" => packet_sent(bot, arg_str(0)),
        "packetFromServer" => {}
        "connectionLost" => {
            bot.logs
                .debug
                .push("Flash socket connection lost event received.");
            bot.events.emit(BotEvent::ConnectionLost);
            // Never block here: a script may hold the options lock while it waits on Flash.
            let watchdog = bot.options.try_read().is_none_or(|o| o.relogin_watchdog);
            if watchdog && !bot.servers.is_relogging() {
                if bot.state.lock().hand_login {
                    bot.log(
                        "The login from the server list lost its connection before reaching the \
                         world (the game says why, e.g. the server is full). Not relogging: pick \
                         another server.",
                    );
                } else {
                    crate::api::servers::begin_logout(bot, false);
                }
            }
        }
        "userIntentionalAction" => match arg_str(0) {
            "logout" | "changeServer" => {
                bot.logs.debug.push(format!(
                    "Intentional '{}' clicked in game. Auto re-login disarmed.",
                    arg_str(0)
                ));
                bot.state.lock().intentional_logout = true;
                crate::api::servers::cancel_relogin(bot);
                crate::script::stop(bot);
            }
            "selectServer" => {
                // The bot's own `clickServer` clicks the list too: only a click while no relogin
                // runs is the user's.
                let by_hand = !bot.servers.is_relogging();
                bot.logs.debug.push(if by_hand {
                    "Server chosen by user on server list. Watchdog armed once in game."
                } else {
                    "Server chosen on server list by the relogin."
                });
                let mut state = bot.state.lock();
                state.intentional_logout = false;
                state.hand_login = by_hand;
            }
            _ => {}
        },
        "openWebsite" => {
            let url = arg_str(0);
            if url.starts_with("https://") {
                bot.logs.debug.push(format!("Open website: {url}"));
            }
        }
        "debug" => bot.logs.debug.push(format!("[skua] {}", arg_str(0))),
        // Turu's host SWF: its own uncaught errors and the client's, and callbacks that threw.
        "error" => {
            if let Some(line) = bot.flash.host_error_line(arg_str(0), arg_str(1)) {
                bot.logs.debug.push(line);
            }
        }
        // Turu Mas's host SWF (air/app): its start and its errors (the Debug log reaches the app's
        // log too).
        "hostLog" => bot.logs.debug.push(format!("[AIR host] {}", arg_str(0))),
        other => tracing::debug!("Unhandled Flash call {other}: {args:?}"),
    }
}

/// An outgoing game packet: logged (Diagnostics > Logger, `TURU_PACKET_LOG`) and acted on.
fn packet_sent(bot: &Arc<Bot>, packet: &str) {
    if bot.logs.packet_logging.load(Ordering::Relaxed) && bot.logs.keeps_packet(packet) {
        bot.logs.packets.push(packet.to_owned());
    }
    if crate::log::packets_to_log() {
        tracing::info!(target: "turu::packet", "{packet}");
    }
    crate::api::packets::on_packet_sent(bot, packet);
}

/// A packet the game sent, from the player (`player_packets`): as the host SWF's `packet`, and in
/// parity mode compared with VibeSkua's.
pub(crate) fn player_packet_sent(bot: &Arc<Bot>, packet: &str) {
    if let Some(parity) = bot.flash.parity() {
        let args = [Value::String(packet.to_owned())];
        for line in parity.event(crate::parity::Side::Turu, "packet", &args) {
            bot.logs.debug.push(line);
        }
    }
    packet_sent(bot, packet);
}

/// An extension response the client handled, from the player: as the host SWF's `pext`, and in
/// parity mode compared with VibeSkua's.
pub(crate) fn player_extension_packet(bot: &Arc<Bot>, packet: Value) {
    if let Some(parity) = bot.flash.parity() {
        let args = [Value::String(packet.to_string())];
        for line in parity.event(crate::parity::Side::Turu, "pext", &args) {
            bot.logs.debug.push(line);
        }
    }
    crate::api::packets::on_extension_packet(bot, packet);
}
