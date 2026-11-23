//! What Android's shell activity (Kotlin) and its Rust side say to each other over JNI, typed:
//! `turu-typegen` writes these records for Kotlin as it writes the models', so a change on one
//! side breaks the other's build rather than a tap (docs/NATIVE_UI_PROPOSAL.md, "Typed actions").
//!
//! * [`ShellAction`]: what the activity does, as `SlotShell.kt` runs it (the answer to its
//!   `native*` calls, and `TuruActivity.shellActions`).
//! * [`ActivityMessage`]: the slots' messages ([`ToShell`]) only the activity carries out.
//! * [`SlotMessage`] and [`TabRequest`]: what the activity sends a slot ([`ToClient`](crate::shell::ToClient)) and asks of
//!   the shell as if the tab on screen had ([`ToShell`]). Kotlin writes them; they are read as the
//!   link's own messages, which tests here hold them to.
//! * [`AccountsView`], [`ServersView`], [`AccountsAsk`] and [`AccountsAnswer`]: the native
//!   Accounts (`android/src/shell_accounts.rs`).
//!
//! Fields inside an enum's variants keep their Rust names on the wire (facet-generate doesn't
//! rename inside them as serde does; Kotlin's properties are camelCase all the same): the link's
//! own messages are snake_case, and so are these.

use crate::shell::{Answered, FieldId, NativeScreen, ToShell};
use crate::slots::NativeTabs;
use serde::{Deserialize, Serialize};

/// What the activity does, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "kind", rename_all = "snake_case")
)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[repr(C)]
pub enum ShellAction {
    /// Start slot `slot`'s service and bind it.
    Start { slot: u32 },
    /// Unbind and stop slot `slot`'s service.
    Stop { slot: u32 },
    /// Hand the surface to slot `slot`.
    Show { slot: u32 },
    /// Pass `json` (a [`ToClient`](crate::shell::ToClient), as the link writes it) on to slot `slot`.
    Send { slot: u32, json: String },
    /// Slot `slot` said hello; `ready` when its engine is up (it gave its control API).
    Hello { slot: u32, ready: bool },
    /// Something only the activity does, for slot `slot`.
    Activity { slot: u32, message: ActivityMessage },
    /// The native Tabs item and panel show this list.
    Tabs { tabs: NativeTabs },
}

/// The actions as the JNI calls answer them.
pub fn actions_json(actions: &[ShellAction]) -> String {
    serde_json::to_string(actions).unwrap_or_else(|e| {
        tracing::warn!("Shell: actions I can't write: {e}");
        "[]".into()
    })
}

/// A slot's message only the activity carries out ([`ToShell`]'s of these names).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "type", rename_all = "snake_case")
)]
#[serde(tag = "type", rename_all = "snake_case")]
#[repr(C)]
pub enum ActivityMessage {
    /// The text field above the keyboard, shown with this text and selection (UTF-16 units). A
    /// game field has `max` characters at most (0: no limit); Turu's say `hint`, `lines` or `number`.
    FieldShow {
        field: FieldId,
        text: String,
        start: u32,
        end: u32,
        password: bool,
        max: u32,
        hint: String,
        lines: bool,
        number: bool,
    },
    FieldUpdate {
        field: FieldId,
        text: String,
        start: u32,
        end: u32,
    },
    FieldHide {
        field: FieldId,
    },
    OpenUrl {
        url: String,
    },
    Share {
        subject: String,
        text: String,
    },
    /// The "open" picker for these MIME types, the document copied into `folder`; answered with
    /// [`SlotMessage::Answer`] and this `id`.
    PickFile {
        id: u32,
        mime_types: String,
        folder: String,
    },
    /// The "save" picker, answered as `PickFile` is.
    SaveFile {
        id: u32,
        mime_type: String,
        file_name: String,
    },
    /// An update's APK to the installer, answered as `PickFile` is.
    Install {
        id: u32,
        path: String,
    },
    /// A display mode refreshing at least `hz` times a second (0: the system's choice).
    PreferRefreshRate {
        hz: u32,
    },
    /// A window the shell has natively, opened in the slot's egui menus.
    OpenScreen {
        screen: NativeScreen,
    },
}

impl ActivityMessage {
    /// `message` as the activity carries it out; `None` for one it doesn't.
    pub fn from_shell(message: ToShell) -> Option<Self> {
        Some(match message {
            ToShell::FieldShow {
                field,
                text,
                start,
                end,
                password,
                max,
                hint,
                lines,
                number,
            } => Self::FieldShow {
                field,
                text,
                start,
                end,
                password,
                max,
                hint,
                lines,
                number,
            },
            ToShell::FieldUpdate {
                field,
                text,
                start,
                end,
            } => Self::FieldUpdate {
                field,
                text,
                start,
                end,
            },
            ToShell::FieldHide { field } => Self::FieldHide { field },
            ToShell::OpenUrl { url } => Self::OpenUrl { url },
            ToShell::Share { subject, text } => Self::Share { subject, text },
            ToShell::PickFile {
                id,
                mime_types,
                folder,
            } => Self::PickFile {
                id,
                mime_types,
                folder,
            },
            ToShell::SaveFile {
                id,
                mime_type,
                file_name,
            } => Self::SaveFile {
                id,
                mime_type,
                file_name,
            },
            ToShell::Install { id, path } => Self::Install { id, path },
            ToShell::PreferRefreshRate { hz } => Self::PreferRefreshRate { hz },
            ToShell::OpenScreen { screen } => Self::OpenScreen { screen },
            _ => return None,
        })
    }
}

/// What the activity sends a slot: [`ToClient`](crate::shell::ToClient)'s messages of these names, read there as such.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "type", rename_all = "snake_case")
)]
#[serde(tag = "type", rename_all = "snake_case")]
#[repr(C)]
pub enum SlotMessage {
    /// The tab list's Start again, for a tab held after kills for memory.
    StartAgain,
    /// One of the tab's egui windows, by key.
    OpenWindow {
        window: String,
    },
    RailHidden {
        hidden: bool,
    },
    /// The text field above the keyboard was edited: all its text, the selection (UTF-16 units).
    FieldEdited {
        field: FieldId,
        text: String,
        start: u32,
        end: u32,
    },
    /// The field's Send.
    FieldEnter {
        field: FieldId,
    },
    /// The field went (Back, a touch outside it).
    FieldClosed {
        field: FieldId,
    },
    /// The soft keyboard's height and the display cutout's insets, in window pixels.
    Insets {
        keyboard: u32,
        cutout_left: u32,
        cutout_right: u32,
    },
    RefreshRate {
        millihertz: u32,
    },
    /// The answer to the slot's request `id` (a picker, the installer).
    Answer {
        id: u32,
        answer: Answered,
    },
}

/// What the activity asks of the shell, as if the tab on screen had: [`ToShell`]'s messages of
/// these names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "type", rename_all = "snake_case")
)]
#[serde(tag = "type", rename_all = "snake_case")]
#[repr(C)]
pub enum TabRequest {
    SelectTab {
        id: u32,
    },
    CloseTab {
        id: u32,
    },
    /// A blank tab.
    OpenTab,
}

// --- Accounts ----------------------------------------------------------------------------------

/// The account list as the native Accounts shows it, with the launch options.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(rename_all = "camelCase")
)]
#[serde(rename_all = "camelCase")]
pub struct AccountsView {
    pub accounts: Vec<AccountRow>,
    pub groups: Vec<String>,
    pub tags: Vec<String>,
    pub server: String,
    pub script: Option<String>,
    pub auto_start: bool,
    /// Favourite and recent scripts, for the launch options.
    pub scripts: Vec<String>,
    pub keeps_passwords: bool,
    /// An open tab's Streamer Mode is on: rows say "Account n", and no username shows.
    pub streamer: bool,
}

/// An account: the tab it plays in (an open tab logging in with it) and whether a password is
/// saved.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(rename_all = "camelCase")
)]
#[serde(rename_all = "camelCase")]
pub struct AccountRow {
    pub username: String,
    pub label: String,
    pub display_name: String,
    pub tags: Vec<String>,
    pub groups: Vec<String>,
    pub tab: Option<u32>,
    pub password: bool,
}

/// The servers for the launch options, or why there are none.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(rename_all = "camelCase")
)]
#[serde(rename_all = "camelCase")]
pub struct ServersView {
    pub servers: Vec<ServerRow>,
    pub error: Option<String>,
}

/// A server: its name and its status line ("Yorumi - 42ms - 120/1000").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(rename_all = "camelCase")
)]
#[serde(rename_all = "camelCase")]
pub struct ServerRow {
    pub name: String,
    pub line: String,
    pub online: bool,
}

/// What the native Accounts asks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "do", rename_all = "camelCase")
)]
#[serde(tag = "do", rename_all = "camelCase")]
#[repr(C)]
pub enum AccountsAsk {
    /// Starts the account in a new tab, with the launch options.
    Start {
        username: String,
    },
    /// Starts each account in a new tab of its own, in the list's order, as far as there are free
    /// tabs.
    StartMany {
        usernames: Vec<String>,
    },
    /// Puts the accounts in the group, making it if there's none of that name.
    AddToGroup {
        group: String,
        usernames: Vec<String>,
    },
    /// Takes the accounts out of the group (the group stays, even empty).
    RemoveFromGroup {
        group: String,
        usernames: Vec<String>,
    },
    /// Removes the group; its accounts stay.
    RemoveGroup {
        group: String,
    },
    /// Adds an account, or changes the one named `original`.
    Save {
        original: Option<String>,
        username: String,
        password: Option<String>,
        #[serde(rename = "displayName")]
        #[cfg_attr(feature = "typegen", facet(rename = "displayName"))]
        display_name: String,
        tags: Vec<String>,
    },
    Remove {
        username: String,
    },
    /// Up (-1) or down (1) the list.
    Move {
        username: String,
        by: i32,
    },
    /// The launch options.
    Launch {
        server: String,
        script: Option<String>,
        #[serde(rename = "autoStart")]
        #[cfg_attr(feature = "typegen", facet(rename = "autoStart"))]
        auto_start: bool,
    },
}

/// The answer to an [`AccountsAsk`]: why not, or the activity's actions (a new tab's).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(rename_all = "camelCase")
)]
#[serde(rename_all = "camelCase")]
pub struct AccountsAnswer {
    pub error: Option<String>,
    pub actions: Vec<ShellAction>,
}

impl AccountsAnswer {
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            actions: Vec::new(),
        }
    }

    pub fn json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| format!(r#"{{"error":"{e}","actions":[]}}"#))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::ToClient;
    use serde_json::{Value, json};

    /// What the activity writes is read as the link's own message, field for field.
    #[test]
    fn what_the_activity_writes_reads_as_the_links_messages() {
        let to_client = |m: &SlotMessage| -> ToClient {
            serde_json::from_value(serde_json::to_value(m).unwrap()).unwrap()
        };
        let field = |field| SlotMessage::FieldEdited {
            field,
            text: "Questwalker".into(),
            start: 1,
            end: 2,
        };
        assert_eq!(
            to_client(&field(FieldId::Game)),
            ToClient::FieldEdited {
                field: FieldId::Game,
                text: "Questwalker".into(),
                start: 1,
                end: 2,
            }
        );
        let pairs = [
            (SlotMessage::StartAgain, ToClient::StartAgain),
            (
                SlotMessage::OpenWindow {
                    window: "script_repo".into(),
                },
                ToClient::OpenWindow {
                    window: "script_repo".into(),
                },
            ),
            (
                SlotMessage::RailHidden { hidden: true },
                ToClient::RailHidden { hidden: true },
            ),
            (
                SlotMessage::FieldEnter { field: FieldId::Ui },
                ToClient::FieldEnter { field: FieldId::Ui },
            ),
            (
                SlotMessage::FieldClosed { field: FieldId::Ui },
                ToClient::FieldClosed { field: FieldId::Ui },
            ),
            (
                SlotMessage::Insets {
                    keyboard: 800,
                    cutout_left: 90,
                    cutout_right: 0,
                },
                ToClient::Insets {
                    keyboard: 800,
                    cutout_left: 90,
                    cutout_right: 0,
                },
            ),
            (
                SlotMessage::RefreshRate {
                    millihertz: 120_000,
                },
                ToClient::RefreshRate {
                    millihertz: 120_000,
                },
            ),
            (
                SlotMessage::Answer {
                    id: 1,
                    answer: Answered::Path { path: None },
                },
                ToClient::Answer {
                    id: 1,
                    answer: Answered::Path { path: None },
                },
            ),
            (
                SlotMessage::Answer {
                    id: 2,
                    answer: Answered::Document {
                        uri: Some("content://a".into()),
                        path: Some("/b".into()),
                    },
                },
                ToClient::Answer {
                    id: 2,
                    answer: Answered::Document {
                        uri: Some("content://a".into()),
                        path: Some("/b".into()),
                    },
                },
            ),
            (
                SlotMessage::Answer {
                    id: 3,
                    answer: Answered::Installed {
                        status: 2,
                        message: Some("no room".into()),
                    },
                },
                ToClient::Answer {
                    id: 3,
                    answer: Answered::Installed {
                        status: 2,
                        message: Some("no room".into()),
                    },
                },
            ),
        ];
        for (sent, read) in pairs {
            assert_eq!(to_client(&sent), read);
        }
        let to_shell = |m: &TabRequest| -> ToShell {
            serde_json::from_value(serde_json::to_value(m).unwrap()).unwrap()
        };
        assert_eq!(
            to_shell(&TabRequest::SelectTab { id: 2 }),
            ToShell::SelectTab { id: 2 }
        );
        assert_eq!(
            to_shell(&TabRequest::CloseTab { id: 3 }),
            ToShell::CloseTab { id: 3 }
        );
        assert!(matches!(
            to_shell(&TabRequest::OpenTab),
            ToShell::OpenTab {
                username: None,
                password: None,
                ..
            }
        ));
    }

    /// The slots' messages the activity carries out keep their fields; the rest stay the shell's.
    #[test]
    fn the_activity_takes_its_own_messages() {
        let show = ToShell::FieldShow {
            field: FieldId::Ui,
            text: "x".into(),
            start: 0,
            end: 1,
            password: false,
            max: 0,
            hint: "Search scripts".into(),
            lines: false,
            number: true,
        };
        // The activity's copy reads as the message it came from.
        let copy =
            serde_json::to_value(ActivityMessage::from_shell(show.clone()).unwrap()).unwrap();
        assert_eq!(serde_json::from_value::<ToShell>(copy).unwrap(), show);
        let picked = ToShell::PickFile {
            id: 7,
            mime_types: "application/json".into(),
            folder: "/f".into(),
        };
        let copy =
            serde_json::to_value(ActivityMessage::from_shell(picked.clone()).unwrap()).unwrap();
        assert_eq!(serde_json::from_value::<ToShell>(copy).unwrap(), picked);
        assert_eq!(ActivityMessage::from_shell(ToShell::CheckUpdates), None);
    }

    /// The Accounts' requests as the generated Kotlin writes them (camelCase names, a `null` for an
    /// option left out).
    #[test]
    fn accounts_asks_read_the_panels_json() {
        let read = |body: Value| serde_json::from_value::<AccountsAsk>(body).unwrap();
        assert_eq!(
            read(
                json!({ "do": "save", "original": null, "username": "Questwalker", "password": null,
                "displayName": "Quest", "tags": ["farm"] })
            ),
            AccountsAsk::Save {
                original: None,
                username: "Questwalker".into(),
                password: None,
                display_name: "Quest".into(),
                tags: vec!["farm".into()],
            }
        );
        assert_eq!(
            read(json!({ "do": "launch", "server": "Yorumi", "script": null, "autoStart": true })),
            AccountsAsk::Launch {
                server: "Yorumi".into(),
                script: None,
                auto_start: true,
            }
        );
        assert_eq!(
            read(json!({ "do": "startMany", "usernames": ["Questwalker"] })),
            AccountsAsk::StartMany {
                usernames: vec!["Questwalker".into()],
            }
        );
        let answer = AccountsAnswer {
            error: None,
            actions: vec![ShellAction::Show { slot: 2 }],
        };
        assert_eq!(
            serde_json::to_value(&answer).unwrap(),
            json!({ "error": null, "actions": [{ "kind": "show", "slot": 2 }] })
        );
    }
}
