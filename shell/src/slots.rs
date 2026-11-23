//! Turu: Android's shell (docs/TAB_PROCESSES_PROPOSAL.md, M3), without Android: the tab list over
//! the slots. Each tab is a slot: a foreground service in a process of its own (`:c1` ... `:c9`),
//! running one account's engine; the activity is the shell, handing its surface to the slot on
//! screen. What a slot needs to start is on disk (`turu_frontend::slots::SlotRecord`), since
//! Android restarts a killed slot with or without the activity.
//!
//! [`SlotShell`] decides, and the activity carries out its [`Action`]s (start or stop a slot's
//! service, hand it the surface, write a record, send it a message): `android/src/slot_shell.rs`
//! and `SlotShell.kt`. Shell and slots talk in `turu_frontend::shell`'s messages, over Binder. It
//! shares the link, and nothing else, with the desktop's shell ([`crate::Shell`]): Android starts
//! services where the desktop starts processes, and hands over a `Surface` where the desktop
//! shares textures.

use std::collections::BTreeMap;
use turu_frontend::shell::{TabGame, TabInfo, ToClient, ToShell};
use turu_frontend::slots::{NativeTab, NativeTabs, POOL, RAIL_TOP, SlotRecord, exit_args, open_args, playlist_args};

/// What the shell does, in the activity.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Write slot `n`'s record (`None`: remove it).
    Record(u32, Option<SlotRecord>),
    /// Keep (or with `None` forget) the password slot `n` logs in with, in the Keystore.
    Password(u32, Option<String>),
    /// Start slot `n`'s service and bind it.
    Start(u32),
    /// Unbind and stop slot `n`'s service.
    Stop(u32),
    /// Hand the surface to slot `n` (the one before lets go of it first).
    Show(u32),
    /// Send slot `n` a message.
    Send(u32, ToClient),
    /// Something only the activity can do, for slot `n`: the text field above the keyboard (for
    /// the slot on screen), a link, the share sheet, a file picker, installing an update.
    Activity(u32, ToShell),
    /// The activity's native Tabs item and panel show this list.
    Tabs(NativeTabs),
}

/// A slot whose process is bound and has said hello.
#[derive(Debug, Clone, Default)]
struct Live {
    label: String,
    busy: bool,
    waits: Option<String>,
    rail_hidden: bool,
    game: TabGame,
    map: Option<String>,
    script: Option<String>,
    held: Option<String>,
    api: Option<String>,
    token: Option<String>,
}

/// The shell's tab list over the slots.
#[derive(Debug, Default)]
pub struct SlotShell {
    records: BTreeMap<u32, SlotRecord>,
    live: BTreeMap<u32, Live>,
    shown: u32,
    /// Headless Mode as a slot last turned it on or off (`None`: none has yet): every slot's.
    headless_mode: Option<bool>,
    /// Options > Application > Tab Limit, as the shell last read it (`None`: [`POOL`]).
    limit: Option<u32>,
}

impl SlotShell {
    /// A shell (the activity starting) over the open tabs' records: each slot started (or bound
    /// again, when it runs), the first shown. With none, a first tab with `first` flags, and the
    /// tabs in `more` in the next slots (those the app had open in its own process, as long as
    /// the slots go: `saved_tabs::tab_args`).
    pub fn new(records: BTreeMap<u32, SlotRecord>, first: Vec<String>, more: Vec<Vec<String>>) -> (Self, Vec<Action>) {
        let mut shell = Self {
            records,
            ..Self::default()
        };
        let mut actions = Vec::new();
        if shell.records.is_empty() {
            for (slot, args) in (1..=POOL).zip(std::iter::once(first).chain(more)) {
                let record = SlotRecord {
                    args,
                    ..Default::default()
                };
                shell.records.insert(slot, record.clone());
                actions.push(Action::Record(slot, Some(record)));
            }
        }
        actions.extend(shell.records.keys().map(|&slot| Action::Start(slot)));
        shell.shown = *shell.records.keys().next().expect("one at least");
        actions.push(Action::Show(shell.shown));
        (shell, actions)
    }

    /// The slot on screen.
    pub fn shown(&self) -> u32 {
        self.shown
    }

    /// The open tabs' slots, in order.
    pub fn slots(&self) -> Vec<u32> {
        self.records.keys().copied().collect()
    }

    /// The open tabs' slots and the login each was opened with (the activity's Accounts shows
    /// which account plays where).
    pub fn usernames(&self) -> Vec<(u32, Option<String>)> {
        self.records
            .iter()
            .map(|(&slot, record)| (slot, record.username().map(str::to_owned)))
            .collect()
    }

    /// Slot `slot`'s process is bound and said hello (its control API with it).
    pub fn connected(&mut self, slot: u32, hello: &ToShell) -> Vec<Action> {
        if !self.records.contains_key(&slot) {
            // A slot whose tab was closed meanwhile.
            return vec![Action::Stop(slot)];
        }
        let (api, token) = match hello {
            ToShell::Hello { api, api_token, .. } => (api.clone(), api_token.clone()),
            _ => (None, None),
        };
        let live = self.live.entry(slot).or_default();
        live.api = api;
        live.token = token;
        let mut actions = Vec::new();
        if slot == self.shown {
            actions.push(Action::Show(slot));
        }
        actions.extend(self.tab_lists());
        // As the other tabs have it, once one changed it.
        if let Some(on) = self.headless_mode {
            actions.push(Action::Send(slot, ToClient::HeadlessMode { on }));
        }
        actions
    }

    /// Slot `slot`'s process died (Android restarts it from its record).
    pub fn disconnected(&mut self, slot: u32) -> Vec<Action> {
        if self.live.remove(&slot).is_none() {
            return Vec::new();
        }
        self.tab_lists()
    }

    /// A message from slot `slot`.
    pub fn message(&mut self, slot: u32, message: ToShell) -> Vec<Action> {
        match message {
            ToShell::SelectTab { id } => self.select(id),
            ToShell::CloseTab { id } => self.close(id),
            ToShell::OpenTab {
                username,
                password,
                server,
                script,
                run_script,
                playlist,
                run_playlist,
                options_storage,
                exit_on_stop,
            } => {
                let mut args = open_args(username, server, script, run_script);
                args.extend(playlist_args(playlist, run_playlist, options_storage));
                args.extend(exit_args(exit_on_stop));
                self.open(args, password)
            }
            ToShell::Status {
                label,
                busy,
                waits,
                rail_hidden,
                game,
                map,
                script,
                streamer,
                held,
            } => {
                let mut actions = self.status(slot, label, busy, waits, rail_hidden, (game, map, script, held));
                if let Some(record) = self.records.get_mut(&slot)
                    && record.streamer != streamer
                {
                    record.streamer = streamer;
                    actions.push(Action::Record(slot, Some(record.clone())));
                    // The activity's Accounts hide the names, or show them again.
                    if !actions.iter().any(|a| matches!(a, Action::Tabs(_))) {
                        actions.push(Action::Tabs(self.native_tabs()));
                    }
                }
                actions
            }
            // A window the activity has natively (Accounts, the Script Loader): its own.
            open @ ToShell::OpenScreen { .. } => vec![Action::Activity(slot, open)],
            hello @ ToShell::Hello { .. } => self.connected(slot, &hello),
            // Every tab's: to the other slots up.
            ToShell::HeadlessMode { on } => {
                if self.headless_mode == Some(on) {
                    return Vec::new();
                }
                self.headless_mode = Some(on);
                self.live
                    .keys()
                    .filter(|&&other| other != slot)
                    .map(|&other| Action::Send(other, ToClient::HeadlessMode { on }))
                    .collect()
            }
            // A hidden slot's text box (it lost the screen while it had the focus) shows nothing,
            // and only the slot on screen chooses the display's mode.
            field @ (ToShell::PreferRefreshRate { .. }
            | ToShell::FieldShow { .. }
            | ToShell::FieldUpdate { .. }
            | ToShell::FieldHide { .. }
            | ToShell::SoftKeyboard { .. }) => {
                if slot == self.shown {
                    vec![Action::Activity(slot, field)]
                } else {
                    Vec::new()
                }
            }
            // From any slot: a hidden one's update may be installed, say.
            call @ (ToShell::OpenUrl { .. }
            | ToShell::Share { .. }
            | ToShell::PickFile { .. }
            | ToShell::SaveFile { .. }
            | ToShell::Install { .. }) => vec![Action::Activity(slot, call)],
            _ => Vec::new(),
        }
    }

    /// How many tabs may be open: Tab Limit, within the slots.
    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(POOL).clamp(1, POOL)
    }

    /// Tab Limit as read from the settings; whether it changed (the tab list then goes again,
    /// with [`Self::native_tabs`]). Tabs open past a lowered limit stay open.
    pub fn set_limit(&mut self, limit: u32) -> bool {
        let before = self.limit();
        self.limit = Some(limit);
        before != self.limit()
    }

    /// Opens a tab in a free slot and shows it; nothing when Tab Limit's tabs are open (or
    /// every slot is taken).
    pub fn open(&mut self, args: Vec<String>, password: Option<String>) -> Vec<Action> {
        if self.records.len() >= self.limit() as usize {
            return Vec::new();
        }
        let Some(slot) = (1..=POOL).find(|slot| !self.records.contains_key(slot)) else {
            return Vec::new();
        };
        let record = SlotRecord {
            args,
            ..Default::default()
        };
        self.records.insert(slot, record.clone());
        let mut actions = vec![
            Action::Record(slot, Some(record)),
            Action::Password(slot, password.filter(|p| !p.is_empty())),
            Action::Start(slot),
        ];
        actions.extend(self.select(slot));
        actions
    }

    /// Shows slot `slot`'s tab.
    pub fn select(&mut self, slot: u32) -> Vec<Action> {
        if !self.records.contains_key(&slot) || slot == self.shown {
            return Vec::new();
        }
        self.shown = slot;
        let mut actions = vec![Action::Show(slot)];
        actions.extend(self.tab_lists());
        actions
    }

    /// The activity couldn't hand the surface over (the slot on screen didn't let go of it):
    /// `slot` is still the one on screen, and the tab lists say so.
    pub fn still_shown(&mut self, slot: u32) -> Vec<Action> {
        if !self.records.contains_key(&slot) || slot == self.shown {
            return Vec::new();
        }
        self.shown = slot;
        self.tab_lists()
    }

    /// Closes slot `slot`'s tab (never the last one): its service stops and its record goes.
    pub fn close(&mut self, slot: u32) -> Vec<Action> {
        if self.records.len() < 2 || !self.records.contains_key(&slot) {
            return Vec::new();
        }
        let index = self.slots().iter().position(|&s| s == slot).unwrap_or(0);
        self.records.remove(&slot);
        self.live.remove(&slot);
        let mut actions = vec![
            Action::Stop(slot),
            Action::Record(slot, None),
            Action::Password(slot, None),
        ];
        if self.shown == slot {
            let slots = self.slots();
            self.shown = slots[index.min(slots.len() - 1)];
            actions.push(Action::Show(self.shown));
        }
        actions.extend(self.tab_lists());
        actions
    }

    /// A slot's status: the tab lists again when what they show changed.
    fn status(
        &mut self,
        slot: u32,
        label: String,
        busy: bool,
        waits: Option<String>,
        rail_hidden: bool,
        (game, map, script, held): (TabGame, Option<String>, Option<String>, Option<String>),
    ) -> Vec<Action> {
        match self.live.get_mut(&slot) {
            Some(live) if live.label != label || live.busy != busy || live.waits != waits => {
                live.label = label;
                live.busy = busy;
                live.waits = waits;
                live.rail_hidden = rail_hidden;
                (live.game, live.map, live.script, live.held) = (game, map, script, held);
                self.tab_lists()
            }
            // What only the activity's list shows.
            Some(live)
                if live.rail_hidden != rail_hidden
                    || (live.game, &live.map, &live.script, &live.held) != (game, &map, &script, &held) =>
            {
                live.rail_hidden = rail_hidden;
                (live.game, live.map, live.script, live.held) = (game, map, script, held);
                vec![Action::Tabs(self.native_tabs())]
            }
            _ => Vec::new(),
        }
    }

    /// The tab list for the activity's native screens.
    pub fn native_tabs(&self) -> NativeTabs {
        let tabs = self
            .records
            .iter()
            .map(|(&slot, record)| {
                let live = self.live.get(&slot);
                NativeTab {
                    id: slot,
                    label: self.label(slot, record),
                    busy: live.is_some_and(|l| l.busy),
                    waits: live.and_then(|l| l.waits.clone()),
                    down: live.is_none().then(|| "starting".into()),
                    game: live.map_or_else(TabGame::default, |l| l.game),
                    map: live.and_then(|l| l.map.clone()),
                    script: live.and_then(|l| l.script.clone()),
                    held: live.and_then(|l| l.held.clone()),
                    account: record.username().is_some(),
                    api: live.and_then(|l| l.api.clone()),
                    token: live.and_then(|l| l.token.clone()),
                }
            })
            .collect();
        NativeTabs {
            tabs,
            shown: self.shown,
            rail_hidden: self.live.get(&self.shown).is_some_and(|l| l.rail_hidden),
            full: self.records.len() >= self.limit() as usize,
            limit: self.limit(),
            most: POOL,
        }
    }

    /// A tab's name: the character once a slot says it, else the login, else "Login". A slot at
    /// the login screen without a password says "Login": the account it was opened for is the
    /// better name then.
    fn label(&self, slot: u32, record: &SlotRecord) -> String {
        let said = self
            .live
            .get(&slot)
            .map(|l| l.label.clone())
            .filter(|l| !l.is_empty() && !(l == "Login" && record.username().is_some()));
        if let Some(said) = said {
            return said;
        }
        // Turu: Streamer Mode hides the login while the tab starts (again), as the tab does: its
        // own, as it last said, or another tab's for a tab that hasn't said yet.
        if record.streamer || self.streamer() {
            let n = self.records.keys().position(|&s| s == slot).unwrap_or_default();
            return format!("Tab {}", n + 1);
        }
        record.username().map_or_else(|| "Login".into(), str::to_owned)
    }

    /// An open tab's Streamer Mode is on (as it last said): the activity's Accounts hide the
    /// account names.
    pub fn streamer(&self) -> bool {
        self.records.values().any(|r| r.streamer)
    }

    /// Every live slot gets the tab list, and the activity's native screens theirs.
    fn tab_lists(&self) -> Vec<Action> {
        let tabs: Vec<TabInfo> = self
            .records
            .iter()
            .map(|(&slot, record)| {
                let live = self.live.get(&slot);
                TabInfo {
                    id: slot,
                    label: self.label(slot, record),
                    busy: live.is_some_and(|l| l.busy),
                    down: live.is_none().then(|| "starting".into()),
                    api: live.and_then(|l| l.api.clone()),
                    token: live.and_then(|l| l.token.clone()),
                    user: record.username().map(str::to_owned),
                }
            })
            .collect();
        let mut actions: Vec<Action> = self
            .live
            .keys()
            .map(|&slot| {
                Action::Send(
                    slot,
                    ToClient::Tabs {
                        tabs: tabs.clone(),
                        shown: self.shown,
                        you: Some(slot),
                        // The activity draws the tabs and the rail: the slots draw neither.
                        rail_top: Some(RAIL_TOP),
                        native_rail: true,
                    },
                )
            })
            .collect();
        actions.push(Action::Tabs(self.native_tabs()));
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello() -> ToShell {
        ToShell::Hello {
            token: String::new(),
            pid: 1,
            version: "test".into(),
            api: Some("127.0.0.1:50001".into()),
            api_token: Some("a-token".into()),
        }
    }

    fn sent_to(actions: &[Action]) -> Vec<u32> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Send(slot, ToClient::Tabs { .. }) => Some(*slot),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_first_shell_opens_a_first_tab() {
        let (shell, actions) = SlotShell::new(BTreeMap::new(), vec!["--user".into(), "Hero".into()], Vec::new());
        assert_eq!(
            actions,
            [
                Action::Record(
                    1,
                    Some(SlotRecord {
                        args: vec!["--user".into(), "Hero".into()],
                        ..Default::default()
                    })
                ),
                Action::Start(1),
                Action::Show(1),
            ]
        );
        assert_eq!(shell.shown(), 1);
    }

    /// On by default; off only when saved off; an `on` file from before stays on.
    /// The first start with process tabs on: the tabs the app had open in its own process come
    /// back in the next slots, as many as there are slots.
    #[test]
    fn the_first_shell_opens_the_app_s_other_tabs_too() {
        let more = (1..=POOL)
            .map(|n| vec!["--user".to_owned(), format!("Alt{n}")])
            .collect();
        let (shell, actions) = SlotShell::new(BTreeMap::new(), Vec::new(), more);
        assert_eq!(shell.slots(), (1..=POOL).collect::<Vec<_>>());
        assert_eq!(shell.shown(), 1);
        let records: Vec<_> = actions
            .iter()
            .filter_map(|a| match a {
                Action::Record(slot, Some(record)) => Some((*slot, record.username().map(str::to_owned))),
                _ => None,
            })
            .collect();
        let expected: Vec<_> = std::iter::once((1, None))
            .chain((2..=POOL).map(|slot| (slot, Some(format!("Alt{}", slot - 1)))))
            .collect();
        assert_eq!(records, expected);
        // With records already (not the first start), `more` isn't read again.
        let mut open = BTreeMap::new();
        open.insert(2, SlotRecord::default());
        let (shell, _) = SlotShell::new(open, Vec::new(), vec![vec!["--user".into(), "Alt".into()]]);
        assert_eq!(shell.slots(), [2]);
    }

    #[test]
    fn a_new_shell_binds_the_open_tabs_again() {
        let mut records = BTreeMap::new();
        records.insert(2, SlotRecord::default());
        records.insert(3, SlotRecord::default());
        let (shell, actions) = SlotShell::new(records, Vec::new(), Vec::new());
        assert_eq!(actions, [Action::Start(2), Action::Start(3), Action::Show(2)]);
        assert_eq!(shell.slots(), [2, 3]);
    }

    #[test]
    fn opening_selecting_and_closing() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new(), Vec::new());
        assert_eq!(sent_to(&shell.connected(1, &hello())), [1]);

        // "In a tab" from slot 1: slot 2, its password kept apart, shown.
        let actions = shell.message(
            1,
            ToShell::OpenTab {
                username: Some("Alt".into()),
                password: Some("secret".into()),
                server: None,
                script: None,
                run_script: false,
                playlist: None,
                run_playlist: false,
                options_storage: None,
                exit_on_stop: false,
            },
        );
        assert!(actions.contains(&Action::Record(
            2,
            Some(SlotRecord {
                args: vec!["--user".into(), "Alt".into()],
                ..Default::default()
            })
        )));
        assert!(actions.contains(&Action::Password(2, Some("secret".into()))));
        assert!(actions.contains(&Action::Start(2)));
        assert!(actions.contains(&Action::Show(2)));
        assert_eq!(shell.shown(), 2);
        // Slot 2 isn't up yet: listed as starting, under its login.
        let tabs = actions.iter().find_map(|a| match a {
            Action::Send(1, ToClient::Tabs { tabs, .. }) => Some(tabs.clone()),
            _ => None,
        });
        let tabs = tabs.unwrap();
        assert_eq!(tabs[1].label, "Alt");
        assert_eq!(tabs[1].down.as_deref(), Some("starting"));
        assert_eq!(tabs[0].api.as_deref(), Some("127.0.0.1:50001"));
        // Army Queue follows its tabs by their login.
        assert_eq!((tabs[0].user.as_deref(), tabs[1].user.as_deref()), (None, Some("Alt")));

        // An Army Queue tab: it closes itself once its script stops.
        let actions = shell.message(
            1,
            ToShell::OpenTab {
                username: Some("Questwalker".into()),
                password: None,
                server: None,
                script: Some("Farm.ts".into()),
                run_script: true,
                playlist: None,
                run_playlist: false,
                options_storage: Some("ArmyQueue_A_Farm".into()),
                exit_on_stop: true,
            },
        );
        let args: Vec<String> = [
            "--user",
            "Questwalker",
            "--run-script",
            "Farm.ts",
            "--script-options-storage",
            "ArmyQueue_A_Farm",
            "--exit-on-script-stop",
        ]
        .map(String::from)
        .into();
        assert!(
            actions.contains(&Action::Record(
                3,
                Some(SlotRecord {
                    args,
                    ..Default::default()
                })
            )),
            "{actions:?}"
        );
        shell.close(3);

        assert_eq!(sent_to(&shell.connected(2, &hello())), [1, 2]);
        let actions = shell.message(2, ToShell::SelectTab { id: 1 });
        assert_eq!(actions[0], Action::Show(1));

        // Closing the shown tab shows the one after it; the last one stays.
        let actions = shell.close(1);
        assert_eq!(
            &actions[..3],
            [Action::Stop(1), Action::Record(1, None), Action::Password(1, None)]
        );
        assert!(actions.contains(&Action::Show(2)));
        assert!(shell.close(2).is_empty());
    }

    #[test]
    fn a_refused_hand_over_keeps_the_slot_on_screen() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new(), Vec::new());
        shell.connected(1, &hello());
        shell.open(Vec::new(), None);
        shell.connected(2, &hello());
        shell.select(1);
        // Slot 2 didn't let go: still on screen, and selecting slot 1 again tries again.
        assert_eq!(sent_to(&shell.still_shown(2)), [1, 2]);
        assert_eq!(shell.shown(), 2);
        assert_eq!(shell.select(1)[0], Action::Show(1));
    }

    #[test]
    fn only_the_slot_on_screen_shows_a_text_field() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new(), Vec::new());
        shell.connected(1, &hello());
        shell.open(Vec::new(), None);
        shell.connected(2, &hello());
        let field = ToShell::FieldHide {
            field: turu_frontend::shell::FieldId::Game,
        };
        assert_eq!(shell.message(2, field.clone()), [Action::Activity(2, field.clone())]);
        assert!(shell.message(1, field).is_empty());
        // So does its soft keyboard.
        let keyboard = ToShell::SoftKeyboard { show: true };
        assert_eq!(
            shell.message(2, keyboard.clone()),
            [Action::Activity(2, keyboard.clone())]
        );
        assert!(shell.message(1, keyboard).is_empty());
        // A hidden slot's update goes to the installer all the same.
        let install = ToShell::Install {
            id: 1,
            path: "/u.apk".into(),
        };
        assert_eq!(shell.message(1, install.clone()), [Action::Activity(1, install)]);
    }

    #[test]
    fn headless_mode_reaches_the_other_slots() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new(), Vec::new());
        shell.connected(1, &hello());
        shell.open(Vec::new(), None);
        // Nothing to pass on before a slot changes it.
        assert!(
            !shell
                .connected(2, &hello())
                .iter()
                .any(|a| matches!(a, Action::Send(_, ToClient::HeadlessMode { .. })))
        );
        let on = ToShell::HeadlessMode { on: true };
        assert_eq!(
            shell.message(1, on.clone()),
            [Action::Send(2, ToClient::HeadlessMode { on: true })]
        );
        // The same again (another slot's echo) goes nowhere.
        assert!(shell.message(2, on).is_empty());
        // A slot coming back gets it as the others have it.
        shell.disconnected(2);
        assert!(
            shell
                .connected(2, &hello())
                .contains(&Action::Send(2, ToClient::HeadlessMode { on: true }))
        );
    }

    #[test]
    fn every_slot_taken_opens_nothing() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new(), Vec::new());
        for _ in 1..POOL {
            assert!(!shell.open(Vec::new(), None).is_empty());
        }
        assert!(shell.open(Vec::new(), None).is_empty());
        assert_eq!(shell.slots().len(), POOL as usize);
    }

    #[test]
    fn tab_limit_keeps_the_tabs_to_fewer_slots() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new(), Vec::new());
        assert!(shell.set_limit(4));
        assert!(!shell.set_limit(4));
        for _ in 1..4 {
            assert!(!shell.open(Vec::new(), None).is_empty());
        }
        let tabs = shell.native_tabs();
        assert!(tabs.full);
        assert_eq!((tabs.limit, tabs.most), (4, POOL));
        assert!(shell.open(Vec::new(), None).is_empty());
        // Raised, a fifth opens; lowered, the open tabs stay.
        assert!(shell.set_limit(6));
        assert!(!shell.open(Vec::new(), None).is_empty());
        assert!(shell.set_limit(2));
        assert_eq!(shell.slots().len(), 5);
        assert!(shell.native_tabs().full);
        // Out of range: within the slots.
        shell.set_limit(0);
        assert_eq!(shell.limit(), 1);
        shell.set_limit(99);
        assert_eq!(shell.limit(), POOL);
    }

    #[test]
    fn a_dead_slot_shows_as_starting_and_a_closed_one_is_stopped() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new(), Vec::new());
        shell.connected(1, &hello());
        shell.open(Vec::new(), None);
        shell.connected(2, &hello());
        let actions = shell.disconnected(2);
        assert_eq!(sent_to(&actions), [1]);
        // A slot that says hello after its tab closed is stopped.
        shell.close(2);
        assert_eq!(shell.connected(2, &hello()), [Action::Stop(2)]);
    }

    /// The activity's native screens get the list with what each tab waits for, and the slots
    /// are told the activity draws the tabs (no strip; the rail starts below its item).
    #[test]
    fn the_native_tabs_follow_the_slots_status() {
        let records = (1..=2)
            .map(|slot| {
                (
                    slot,
                    SlotRecord {
                        args: vec!["--user".into(), format!("Alt{slot}")],
                        ..Default::default()
                    },
                )
            })
            .collect();
        let (mut shell, _) = SlotShell::new(records, Vec::new(), Vec::new());
        shell.message(1, hello());
        let native = |actions: &[Action]| {
            actions.iter().find_map(|a| match a {
                Action::Tabs(tabs) => Some(tabs.clone()),
                _ => None,
            })
        };
        let actions = shell.message(
            1,
            ToShell::Status {
                label: "Hero".into(),
                busy: true,
                waits: Some("a message box".into()),
                rail_hidden: false,
                game: TabGame::InGame,
                map: Some("battleon".into()),
                script: None,
                streamer: false,
                held: None,
            },
        );
        assert!(actions.iter().any(|a| matches!(
            a,
            Action::Send(1, ToClient::Tabs { rail_top: Some(top), .. }) if *top == RAIL_TOP
        )));
        let tabs = native(&actions).expect("the activity's list");
        assert_eq!((tabs.shown, tabs.full, tabs.rail_hidden), (1, false, false));
        assert_eq!(tabs.tabs[0].label, "Hero");
        assert_eq!(tabs.tabs[0].waits.as_deref(), Some("a message box"));
        assert_eq!(tabs.tabs[1].label, "Alt2");
        assert_eq!(tabs.tabs[1].down.as_deref(), Some("starting"));
        // Hiding the rail changes only the activity's list.
        let actions = shell.message(
            1,
            ToShell::Status {
                label: "Hero".into(),
                busy: true,
                waits: Some("a message box".into()),
                rail_hidden: true,
                game: TabGame::InGame,
                map: Some("battleon".into()),
                script: None,
                streamer: false,
                held: None,
            },
        );
        assert_eq!(actions.len(), 1);
        assert!(native(&actions).expect("the activity's list").rail_hidden);
        // So does dropping out of the game.
        let actions = shell.message(
            1,
            ToShell::Status {
                label: "Hero".into(),
                busy: true,
                waits: Some("a message box".into()),
                rail_hidden: true,
                game: TabGame::Dropped,
                map: None,
                script: Some("Farm".into()),
                streamer: false,
                held: None,
            },
        );
        assert_eq!(actions.len(), 1);
        let tabs = native(&actions).expect("the activity's list");
        assert_eq!(tabs.tabs[0].game, TabGame::Dropped);
        assert_eq!(tabs.tabs[0].script.as_deref(), Some("Farm"));
        assert!(tabs.tabs[0].account);
        // So does a hold after kills for memory.
        let actions = shell.message(
            1,
            ToShell::Status {
                label: "Hero".into(),
                busy: true,
                waits: Some("a message box".into()),
                rail_hidden: true,
                game: TabGame::Dropped,
                map: None,
                script: Some("Farm".into()),
                streamer: false,
                held: Some(turu_frontend::memory_kills::held_text(4)),
            },
        );
        assert_eq!(actions.len(), 1);
        let tabs = native(&actions).expect("the activity's list");
        assert_eq!(tabs.tabs[0].held, Some(turu_frontend::memory_kills::held_text(4)));
        // Army > Accounts goes to the activity.
        let open = ToShell::OpenScreen {
            screen: turu_frontend::shell::NativeScreen::Accounts,
        };
        assert_eq!(shell.message(1, open.clone()), [Action::Activity(1, open)]);
    }

    /// Streamer Mode: a tab says "Tab n" itself; while it starts again the shell keeps that name
    /// (saved in its record), and a tab yet to say anything is hidden too while another streams.
    #[test]
    fn streamer_mode_keeps_the_logins_hidden() {
        let records = (1..=2)
            .map(|slot| {
                (
                    slot,
                    SlotRecord {
                        args: vec!["--user".into(), format!("Questwalker{slot}")],
                        ..Default::default()
                    },
                )
            })
            .collect();
        let (mut shell, _) = SlotShell::new(records, Vec::new(), Vec::new());
        shell.message(1, hello());
        let status = |label: &str, streamer| ToShell::Status {
            label: label.into(),
            busy: false,
            waits: None,
            rail_hidden: false,
            game: TabGame::InGame,
            map: None,
            script: None,
            streamer,
            held: None,
        };
        assert!(!shell.streamer());
        let actions = shell.message(1, status("Tab 1", true));
        assert!(shell.streamer());
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, Action::Record(1, Some(r)) if r.streamer))
        );
        let tabs = shell.native_tabs();
        assert_eq!(tabs.tabs[0].label, "Tab 1");
        // Slot 2 hasn't said yet: not by its login while slot 1 streams.
        assert_eq!(tabs.tabs[1].label, "Tab 2");
        // Slot 1 starts again: still "Tab 1", also in a shell started afresh from the records.
        shell.disconnected(1);
        assert_eq!(shell.native_tabs().tabs[0].label, "Tab 1");
        let dir = tempfile::tempdir().unwrap();
        for (&slot, record) in &shell.records {
            record.write(dir.path(), slot).unwrap();
        }
        let (fresh, _) = SlotShell::new(SlotRecord::read_all(dir.path()), Vec::new(), Vec::new());
        assert_eq!(fresh.native_tabs().tabs[0].label, "Tab 1");
        // Off again: the names come back, and the record says so.
        shell.message(1, hello());
        let actions = shell.message(1, status("Questwalker1", false));
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, Action::Record(1, Some(r)) if !r.streamer))
        );
        assert!(!shell.streamer());
        let tabs = shell.native_tabs();
        assert_eq!(
            (tabs.tabs[0].label.as_str(), tabs.tabs[1].label.as_str()),
            ("Questwalker1", "Questwalker2")
        );
    }
}
