//! What the options pages share (Game Options, Application Options): a section, an option's row
//! (its label, description, value, whether it differs, how it's edited), typed for the shells.
//! Fields inside enum variants are one word: facet-generate doesn't rename inside them as serde
//! does.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct SectionView {
    /// Its short name, for a filter chip ("Combat").
    pub chip: &'static str,
    /// Its heading ("Combat & Targeting").
    pub title: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct OptionView {
    /// Its label, which is also its key for [`set`].
    pub label: &'static str,
    pub description: &'static str,
    /// The section's chip.
    pub section: &'static str,
    /// The small heading inside its section ("" for none).
    pub group: &'static str,
    pub kind: KindView,
    pub value: OptionValue,
    pub saved: OptionValue,
    pub differs: bool,
    /// It matters now: the option it depends on (Private Rooms for Private Number) is on.
    pub applies: bool,
    /// Turu stores it but nothing reads it yet.
    pub unused: bool,
    /// It takes effect at the next start (VibeSkua's `RequiresRestart`).
    pub restart: bool,
}

/// An option's value, typed for the shells (a switch's, a number's, a text's or a choice's key); a
/// shell sends one back to set it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(C)]
pub enum OptionValue {
    Switch(bool),
    Number(i64),
    /// Text, a server's name, or a choice's key ("HighHP").
    Text(String),
    /// An action's (Reload Map): none.
    None,
}

impl OptionValue {
    /// As JSON, as [`set`] takes it.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Switch(on) => json!(on),
            Self::Number(n) => json!(n),
            Self::Text(text) => json!(text),
            Self::None => Value::Null,
        }
    }
}

/// How an option is edited.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(C)]
pub enum KindView {
    Switch,
    #[serde(rename_all = "camelCase")]
    Number {
        min: i32,
        max: Option<i32>,
        unit: Option<&'static str>,
    },
    /// Set FPS: 0 is Off; `presets` for a menu.
    Fps {
        max: i32,
        presets: Vec<i32>,
    },
    Text {
        hint: &'static str,
    },
    /// A server's name (Relogin Server).
    Server,
    /// One of `choices`: (key, words).
    Choice {
        choices: Vec<(String, String)>,
    },
    /// A secret (a key), shown as dots.
    Secret,
    /// Only in the full (egui) window: links, a QR code, keys to copy.
    Elsewhere,
    /// A button (Reload Map), not a value; `login`: it needs one. (One-word fields in variants:
    /// facet-generate doesn't rename inside them as serde does.)
    Action {
        label: &'static str,
        login: bool,
    },
}
