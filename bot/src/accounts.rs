//! The account list (VibeSkua Manager's Accounts tab): accounts, groups and the launch setup,
//! saved in `accounts.json`. Passwords aren't in it (VibeSkua keeps them there in the clear):
//! they are in a [`SecretStore`], by default the system's keychain under the service
//! [`KEYCHAIN_SERVICE`].

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The keychain service account passwords are saved under.
pub const KEYCHAIN_SERVICE: &str = "Turu";

/// VibeSkua `ManagedAccount` / `AccountData`, without the password.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Account {
    pub username: String,
    /// Shown instead of the username when set.
    pub display_name: String,
    pub tags: Vec<String>,
    /// Checked in the list (VibeSkua `UseCheck`): what "Start Selected" launches.
    pub selected: bool,
}

impl Account {
    pub fn label(&self) -> &str {
        if self.display_name.trim().is_empty() {
            &self.username
        } else {
            &self.display_name
        }
    }
}

/// VibeSkua `GroupData`: a name and its accounts' usernames.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub accounts: Vec<String>,
}

/// Where a launched account plays.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchMode {
    /// A tab of this window (VibeSkua's tabbed host).
    #[default]
    Tab,
    /// A `turu` process with its own window (VibeSkua's "in a separate window").
    Window,
    /// A `turu --headless` process.
    Headless,
}

impl LaunchMode {
    pub const ALL: [Self; 3] = [Self::Tab, Self::Window, Self::Headless];

    pub fn label(self) -> &'static str {
        match self {
            Self::Tab => "In a tab",
            Self::Window => "In its own window",
            Self::Headless => "Without a window",
        }
    }
}

/// `accounts.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Accounts {
    pub accounts: Vec<Account>,
    pub groups: Vec<Group>,
    /// The server launched accounts log in on.
    pub server: String,
    /// The script launched accounts load ("Save Setup").
    pub script: Option<PathBuf>,
    /// Start the script once logged in, not only load it.
    pub auto_start: bool,
    pub mode: LaunchMode,
}

impl Default for Accounts {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
            groups: Vec::new(),
            server: "Twilly".into(),
            script: None,
            auto_start: false,
            mode: LaunchMode::Tab,
        }
    }
}

impl Accounts {
    /// The list in `path`, or an empty one.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, text)
    }

    pub fn find(&self, username: &str) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|a| a.username.eq_ignore_ascii_case(username))
    }

    /// Adds the account, or updates the one with that username (names aren't case-sensitive in
    /// AQW). Returns false when it was already there.
    pub fn upsert(&mut self, account: Account) -> bool {
        match self
            .accounts
            .iter_mut()
            .find(|a| a.username.eq_ignore_ascii_case(&account.username))
        {
            Some(existing) => {
                *existing = account;
                false
            }
            None => {
                self.accounts.push(account);
                true
            }
        }
    }

    /// Removes the account from the list and its groups (not its password: see
    /// [`delete_password`]).
    pub fn remove(&mut self, username: &str) {
        self.accounts
            .retain(|a| !a.username.eq_ignore_ascii_case(username));
        for group in &mut self.groups {
            group.accounts.retain(|a| !a.eq_ignore_ascii_case(username));
        }
    }

    /// Moves the account up (`-1`) or down (`1`) the list.
    pub fn shift(&mut self, username: &str, by: isize) {
        let Some(i) = self
            .accounts
            .iter()
            .position(|a| a.username.eq_ignore_ascii_case(username))
        else {
            return;
        };
        let j = i.saturating_add_signed(by);
        if j < self.accounts.len() && j != i {
            self.accounts.swap(i, j);
        }
    }

    /// Adds the account to the group, making the group if there's none of that name.
    pub fn add_to_group(&mut self, group: &str, username: &str) {
        let group = match self.groups.iter_mut().position(|g| g.name == group) {
            Some(i) => &mut self.groups[i],
            None => {
                self.groups.push(Group {
                    name: group.to_owned(),
                    accounts: Vec::new(),
                });
                self.groups.last_mut().expect("just pushed")
            }
        };
        if !group
            .accounts
            .iter()
            .any(|a| a.eq_ignore_ascii_case(username))
        {
            group.accounts.push(username.to_owned());
        }
    }

    /// Removes the group (its accounts stay).
    pub fn remove_group(&mut self, group: &str) {
        self.groups.retain(|g| g.name != group);
    }

    /// The names of the groups the account is in, in the groups' order.
    pub fn groups_of(&self, username: &str) -> Vec<&str> {
        self.groups
            .iter()
            .filter(|g| g.accounts.iter().any(|a| a.eq_ignore_ascii_case(username)))
            .map(|g| g.name.as_str())
            .collect()
    }

    /// Checks the accounts of the group, and only those.
    pub fn select_group(&mut self, group: &str) {
        let Some(group) = self.groups.iter().find(|g| g.name == group) else {
            return;
        };
        for account in &mut self.accounts {
            account.selected = group
                .accounts
                .iter()
                .any(|a| a.eq_ignore_ascii_case(&account.username));
        }
    }

    /// Every tag in use, sorted.
    pub fn tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .accounts
            .iter()
            .flat_map(|a| a.tags.iter().cloned())
            .collect();
        tags.sort_unstable_by_key(|t| t.to_lowercase());
        tags.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        tags
    }
}

/// Where account passwords are kept. Usernames come lowercased.
pub trait SecretStore: Send + Sync {
    fn set(&self, username: &str, password: &str) -> Result<(), String>;
    /// `None` when none is saved.
    fn get(&self, username: &str) -> Result<Option<String>, String>;
    /// None saved is fine.
    fn delete(&self, username: &str) -> Result<(), String>;
}

static SECRET_STORE: OnceLock<Box<dyn SecretStore>> = OnceLock::new();

/// Replaces the system keychain, before the first password is read or saved (Android has no
/// keychain `keyring` can use). False when a store is already in use.
pub fn set_secret_store(store: Box<dyn SecretStore>) -> bool {
    SECRET_STORE.set(store).is_ok()
}

fn secret_store() -> &'static dyn SecretStore {
    SECRET_STORE.get_or_init(default_store).as_ref()
}

#[cfg(not(target_os = "android"))]
fn default_store() -> Box<dyn SecretStore> {
    Box::new(Keychain)
}

#[cfg(target_os = "android")]
fn default_store() -> Box<dyn SecretStore> {
    Box::new(NoStore)
}

/// The system's keychain (`keyring`).
#[cfg(not(target_os = "android"))]
struct Keychain;

#[cfg(not(target_os = "android"))]
impl Keychain {
    fn entry(username: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYCHAIN_SERVICE, username).map_err(|e| e.to_string())
    }
}

#[cfg(not(target_os = "android"))]
impl SecretStore for Keychain {
    fn set(&self, username: &str, password: &str) -> Result<(), String> {
        Self::entry(username)?
            .set_password(password)
            .map_err(|e| format!("couldn't save the password in the system keychain: {e}"))
    }

    fn get(&self, username: &str) -> Result<Option<String>, String> {
        match Self::entry(username)?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!(
                "couldn't read the password from the system keychain: {e}"
            )),
        }
    }

    fn delete(&self, username: &str) -> Result<(), String> {
        match Self::entry(username)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!(
                "couldn't remove the password from the system keychain: {e}"
            )),
        }
    }
}

/// Until the app installs its store: nothing is saved.
#[cfg(target_os = "android")]
struct NoStore;

#[cfg(target_os = "android")]
impl SecretStore for NoStore {
    fn set(&self, _username: &str, _password: &str) -> Result<(), String> {
        Err("no password store on this system".into())
    }

    fn get(&self, _username: &str) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn delete(&self, _username: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Saves the account's password in the secret store.
pub fn save_password(username: &str, password: &str) -> Result<(), String> {
    secret_store().set(&username.to_lowercase(), password)
}

/// The account's password from the secret store; `None` when none is saved.
pub fn password(username: &str) -> Result<Option<String>, String> {
    secret_store().get(&username.to_lowercase())
}

/// Removes the account's password from the secret store (none saved is fine).
pub fn delete_password(username: &str) -> Result<(), String> {
    secret_store().delete(&username.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(name: &str) -> Account {
        Account {
            username: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn saves_and_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        assert_eq!(Accounts::load(&path), Accounts::default());
        let mut accounts = Accounts::default();
        accounts.upsert(account("Alpha"));
        accounts.add_to_group("Farm", "Alpha");
        accounts.save(&path).unwrap();
        assert_eq!(Accounts::load(&path), accounts);
        assert!(!fs::read_to_string(&path).unwrap().contains("password"));
    }

    #[test]
    fn lists_and_removes_groups() {
        let mut accounts = Accounts::default();
        accounts.upsert(account("Alpha"));
        accounts.add_to_group("Farm", "Alpha");
        accounts.add_to_group("Boss", "alpha");
        accounts.add_to_group("Boss", "Beta");
        assert_eq!(accounts.groups_of("ALPHA"), ["Farm", "Boss"]);
        accounts.remove_group("Farm");
        assert_eq!(accounts.groups_of("Alpha"), ["Boss"]);
        assert_eq!(
            accounts.find("Alpha").map(|a| a.username.as_str()),
            Some("Alpha")
        );
    }

    #[test]
    fn edits_the_list() {
        let mut accounts = Accounts::default();
        assert!(accounts.upsert(account("Alpha")));
        assert!(accounts.upsert(account("Beta")));
        assert!(!accounts.upsert(Account {
            display_name: "B".into(),
            ..account("beta")
        }));
        assert_eq!(accounts.accounts.len(), 2);
        assert_eq!(accounts.find("BETA").unwrap().label(), "B");

        accounts.shift("beta", -1);
        assert_eq!(accounts.accounts[0].username, "beta");
        accounts.shift("beta", -1);
        assert_eq!(accounts.accounts[0].username, "beta");

        accounts.add_to_group("Farm", "Alpha");
        accounts.add_to_group("Farm", "alpha");
        assert_eq!(accounts.groups[0].accounts, ["Alpha"]);
        accounts.select_group("Farm");
        assert!(accounts.find("Alpha").unwrap().selected);
        assert!(!accounts.find("beta").unwrap().selected);

        accounts.remove("ALPHA");
        assert!(accounts.find("Alpha").is_none());
        assert!(accounts.groups[0].accounts.is_empty());
    }

    #[test]
    fn lists_tags() {
        let mut accounts = Accounts::default();
        accounts.upsert(Account {
            tags: vec!["main".into(), "Farm".into()],
            ..account("A")
        });
        accounts.upsert(Account {
            tags: vec!["farm".into()],
            ..account("B")
        });
        assert_eq!(accounts.tags(), ["Farm", "main"]);
    }
}
