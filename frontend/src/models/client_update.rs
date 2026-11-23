//! Update Turu (the rail's Update, the nav bar's notice; VibeSkua's Manager asks in a message box
//! at launch, Turu never interrupts: a script may be running): the newer release, how this
//! platform installs it, what the install is doing and whether it may start now. Not per tab: the
//! release and the install are the process's (`turu_bot::updates`).
//!
//! The view is worked out by [`build`] from plain inputs, so it's tested without the process's
//! update state; [`view`] gathers them.

use super::Control;
use serde::{Deserialize, Serialize};
use turu_bot::updates::{self, Install, Release};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ClientUpdateView {
    /// "Turu", or "Turu Mas".
    pub app: String,
    /// This build's version.
    pub current: String,
    /// The newer release; none once there's no newer one (installed meanwhile, or a check found
    /// none): the window closes.
    pub release: Option<NewRelease>,
    pub way: InstallWay,
    /// What the install does, or why it waits; none while there's nothing to say.
    pub status: Option<UpdateStatus>,
    /// The primary button: "Download & install" ([`InstallWay::Package`]) or "Download &
    /// restart" ([`InstallWay::InPlace`]).
    pub install_label: String,
    pub install: Control,
    /// Update when I quit ([`InstallWay::InPlace`] only).
    pub on_quit: Control,
    /// [`InstallWay::Link`]'s button and where it goes: the APK, or the release page.
    pub link_label: String,
    pub link_url: String,
    /// The line at the foot: how the package is checked and what stays, or where to get it.
    pub footnote: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct NewRelease {
    /// "0.5.0".
    pub version: String,
    pub prerelease: bool,
    /// Its release page, with the notes.
    pub notes_url: String,
}

/// How this platform takes the release.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum InstallWay {
    /// Android: Download & install hands the checked APK to the system's installer.
    Package,
    /// Desktop: Download & restart, or Update when I quit.
    InPlace,
    /// A desktop the release has no package for: the release page (said in the footnote).
    NoPackage,
    /// Nothing to install it with: a button opening [`ClientUpdateView::link_url`].
    Link,
}

/// What [`UpdateStatus`] says, for each shell to show its way (a spinner, a colour).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum UpdateStep {
    /// Downloading, unpacking or installing.
    Working,
    /// Downloaded; installed when Turu quits.
    Waiting,
    Failed,
    /// It can't start now (a script runs).
    Held,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub step: UpdateStep,
    pub text: String,
}

/// What [`build`] works from.
pub struct Input<'a> {
    pub app: &'a str,
    pub current: &'a str,
    pub release: Option<&'a Release>,
    pub install: &'a Install,
    /// The platform hands packages to an installer (Android).
    pub installs_packages: bool,
    /// The platform replaces itself and restarts (desktops).
    pub in_place: bool,
    /// This process runs a script.
    pub running: bool,
    /// The shell's other tabs that run a script or Auto, by label (process tabs: installing
    /// closes every one).
    pub others: Vec<String>,
}

pub fn view() -> ClientUpdateView {
    let state = updates::state();
    let platform = crate::host::host();
    let others = crate::turu::shell_tabs()
        .map(|(tabs, _)| {
            tabs.into_iter()
                .filter(|t| !t.me && t.busy && t.down.is_none())
                .map(|t| t.label)
                .collect()
        })
        .unwrap_or_default();
    build(Input {
        app: updates::app_name(),
        current: crate::turu::VERSION,
        release: state.available.as_ref(),
        install: &state.install,
        installs_packages: platform.installs_packages(),
        in_place: updates::Target::current().is_some_and(updates::Target::updates_in_place),
        running: crate::turu::any_script_running(),
        others,
    })
}

/// "Questwalker is running. Stop it", "Questwalker and Hero are running. Stop them".
fn others_running(others: &[String]) -> Option<String> {
    let (last, rest) = others.split_last()?;
    Some(if rest.is_empty() {
        format!("{last} is running. Stop it")
    } else {
        format!("{} and {last} are running. Stop them", rest.join(", "))
    })
}

pub fn build(input: Input) -> ClientUpdateView {
    let Input {
        app,
        current,
        release,
        install,
        installs_packages,
        in_place,
        running,
        others,
    } = input;
    let installable = release.is_some_and(Release::installable);
    let way = if in_place {
        if installable {
            InstallWay::InPlace
        } else {
            InstallWay::NoPackage
        }
    } else if installs_packages && installable {
        InstallWay::Package
    } else {
        InstallWay::Link
    };
    let busy = matches!(install, Install::Busy(_));
    let staged = matches!(install, Install::OnQuit(_));
    // Turu: process tabs (Android's slots, a desktop shell's clients) install for every tab at
    // once, closing them all, so another tab's run holds the install as this tab's script does.
    // The shell's list says only "busy" (a script or Auto), so that is the rule for other tabs.
    let others = others_running(&others);
    let held = match way {
        InstallWay::Package => {
            if running {
                Some(format!("A script is running. Stop it to install: {app} closes."))
            } else {
                others.map(|o| format!("{o} to install: {app} closes every tab."))
            }
        }
        InstallWay::InPlace => {
            if running {
                Some(format!(
                    "A script is running. Stop it to restart now, or update when you quit {app}."
                ))
            } else {
                others.map(|o| format!("{o} to restart now, or update when you quit {app}."))
            }
        }
        InstallWay::NoPackage | InstallWay::Link => None,
    };
    let status = match (way, install) {
        (InstallWay::NoPackage | InstallWay::Link, _) => None,
        (_, Install::Busy(what)) => Some((UpdateStep::Working, what.clone())),
        (InstallWay::InPlace, Install::OnQuit(version)) => Some((
            UpdateStep::Waiting,
            format!("{app} {version} is downloaded and installs when you quit {app}."),
        )),
        (_, Install::Failed(error)) => Some((UpdateStep::Failed, format!("The update failed: {error}. Try again."))),
        _ => held.clone().map(|text| (UpdateStep::Held, text)),
    }
    .map(|(step, text)| UpdateStatus { step, text });
    let install_control = match way {
        InstallWay::Package | InstallWay::InPlace if busy => Control::off("The update is downloading"),
        InstallWay::Package if held.is_some() => Control::off("Stop the running script first"),
        InstallWay::InPlace if held.is_some() => {
            Control::off("Stop the running script first, or choose Update when I quit")
        }
        InstallWay::Package | InstallWay::InPlace => Control::on(),
        InstallWay::NoPackage | InstallWay::Link => Control::off("This release has no package to install here"),
    };
    let on_quit = match way {
        InstallWay::InPlace if busy => Control::off("The update is downloading"),
        InstallWay::InPlace if staged => Control::off("Already downloaded: it installs when you quit"),
        InstallWay::InPlace => Control::on(),
        _ => Control::off("This platform installs updates at once"),
    };
    let apk = release
        .and_then(|r| r.asset.as_ref())
        .filter(|asset| asset.name.ends_with(".apk"));
    let (link_label, link_url) = match (way, apk, release) {
        (InstallWay::Link, Some(asset), _) => ("Download APK", asset.url.clone()),
        (InstallWay::Link | InstallWay::NoPackage, _, Some(r)) => ("Open release page", r.html_url.clone()),
        _ => ("", String::new()),
    };
    let footnote = match way {
        InstallWay::Package => format!(
            "Checked against SHA256SUMS.txt; your settings and scripts stay. \
             The first time, Android asks you to allow installs from {app}."
        ),
        InstallWay::InPlace => format!("Checked against the release's SHA256SUMS.txt before it replaces {app}."),
        InstallWay::NoPackage => {
            "This release has no package for this computer: download it from the release page.".into()
        }
        InstallWay::Link if apk.is_some() => {
            "Android installs it over this app; your settings and scripts stay.".into()
        }
        InstallWay::Link => String::new(),
    };
    ClientUpdateView {
        app: app.into(),
        current: current.into(),
        release: release.map(|r| NewRelease {
            version: r.version.clone(),
            prerelease: r.prerelease,
            notes_url: r.html_url.clone(),
        }),
        way,
        status,
        install_label: match way {
            InstallWay::InPlace => "Download & restart",
            _ => "Download & install",
        }
        .into(),
        install: install_control,
        on_quit,
        link_label: link_label.into(),
        link_url,
        footnote,
    }
}

/// Download & install or Download & restart (`on_quit` false), or Update when I quit (true),
/// when the view allows it now. A failure to start shows as the install's state too.
pub fn install(on_quit: bool) -> Result<(), String> {
    let view = view();
    if view.release.is_none() {
        return Err("There's no update to install".into());
    }
    let control = if on_quit { &view.on_quit } else { &view.install };
    if !control.enabled {
        return Err(control.reason.clone());
    }
    if let Err(e) = crate::turu::install_update(on_quit) {
        updates::update_state(|s| s.install = Install::Failed(e.clone()));
        return Err(e);
    }
    Ok(())
}

/// The client update's actions (`POST /models/client-update/do`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum ClientUpdateAct {
    /// Installs the update now, or (`later`) when Turu quits.
    Install { later: bool },
}

pub fn act(act: ClientUpdateAct) -> Result<(), String> {
    match act {
        ClientUpdateAct::Install { later } => install(later),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use turu_bot::updates::Asset;

    fn release(asset: Option<&str>, sums: bool) -> Release {
        let file = |name: &str| Asset {
            name: name.into(),
            url: format!("https://example.com/{name}"),
            size: 1,
        };
        Release {
            version: "0.5.0".into(),
            tag: "v0.5.0".into(),
            html_url: "https://example.com/releases/v0.5.0".into(),
            prerelease: true,
            asset: asset.map(file),
            sums: sums.then(|| file("SHA256SUMS.txt")),
        }
    }

    fn android<'a>(release: &'a Release, install: &'a Install) -> Input<'a> {
        Input {
            app: "Turu",
            current: "0.4.0",
            release: Some(release),
            install,
            installs_packages: true,
            in_place: false,
            running: false,
            others: Vec::new(),
        }
    }

    #[test]
    fn android_installs_the_checked_apk() {
        let release = release(Some("turu-0.5.0-android-arm64.apk"), true);
        let view = build(android(&release, &Install::Idle));
        assert_eq!(view.way, InstallWay::Package);
        assert_eq!(view.install_label, "Download & install");
        assert!(view.install.enabled);
        assert!(!view.on_quit.enabled);
        assert_eq!(view.status, None);
        assert!(view.footnote.contains("allow installs from Turu"));
        let r = view.release.unwrap();
        assert_eq!((r.version.as_str(), r.prerelease), ("0.5.0", true));
    }

    #[test]
    fn a_running_script_holds_the_install_in_this_tab_or_another() {
        let release = release(Some("turu-0.5.0-android-arm64.apk"), true);
        let view = build(Input {
            running: true,
            ..android(&release, &Install::Idle)
        });
        assert!(!view.install.enabled);
        let status = view.status.unwrap();
        assert_eq!(status.step, UpdateStep::Held);
        assert_eq!(status.text, "A script is running. Stop it to install: Turu closes.");

        let view = build(Input {
            others: vec!["Questwalker".into(), "Hero".into(), "Ranger".into()],
            ..android(&release, &Install::Idle)
        });
        assert!(!view.install.enabled);
        assert_eq!(
            view.status.unwrap().text,
            "Questwalker, Hero and Ranger are running. Stop them to install: Turu closes every tab."
        );
    }

    #[test]
    fn the_install_says_what_it_does_and_how_it_ended() {
        let release = release(Some("turu-0.5.0-android-arm64.apk"), true);
        let busy = Install::Busy("Downloading Turu 0.5.0…".into());
        let view = build(Input {
            running: true,
            ..android(&release, &busy)
        });
        assert_eq!(
            view.status.as_ref().unwrap().step,
            UpdateStep::Working,
            "the download first"
        );
        assert_eq!(view.install.reason, "The update is downloading");

        let failed = Install::Failed("the sums don't match".into());
        let view = build(android(&release, &failed));
        assert_eq!(
            view.status.unwrap().text,
            "The update failed: the sums don't match. Try again."
        );
        assert!(view.install.enabled, "Try again");
    }

    #[test]
    fn desktops_restart_or_wait_for_quit() {
        let release = release(Some("turu-0.5.0-macos.zip"), true);
        let input = |install| Input {
            in_place: true,
            installs_packages: false,
            ..android(&release, install)
        };
        let view = build(input(&Install::Idle));
        assert_eq!(view.way, InstallWay::InPlace);
        assert_eq!(view.install_label, "Download & restart");
        assert!(view.install.enabled && view.on_quit.enabled);

        let staged = Install::OnQuit("0.5.0".into());
        let view = build(input(&staged));
        assert_eq!(view.status.as_ref().unwrap().step, UpdateStep::Waiting);
        assert!(view.install.enabled, "restart now still");
        assert_eq!(view.on_quit.reason, "Already downloaded: it installs when you quit");

        let view = build(Input {
            running: true,
            ..input(&Install::Idle)
        });
        assert!(
            !view.install.enabled && view.on_quit.enabled,
            "update when I quit stays"
        );
    }

    #[test]
    fn without_a_package_it_links() {
        let no_sums = release(Some("turu-0.5.0-android-arm64.apk"), false);
        let view = build(android(&no_sums, &Install::Idle));
        assert_eq!(view.way, InstallWay::Link);
        assert_eq!(view.link_label, "Download APK");
        assert!(view.link_url.ends_with(".apk"));
        assert!(!view.install.enabled);

        let nothing = release(None, false);
        let view = build(android(&nothing, &Install::Idle));
        assert_eq!(view.link_label, "Open release page");
        assert_eq!(view.footnote, "");

        let view = build(Input {
            in_place: true,
            ..android(&nothing, &Install::Idle)
        });
        assert_eq!(view.way, InstallWay::NoPackage);
        assert_eq!(view.link_label, "Open release page");
        assert!(view.footnote.contains("no package for this computer"));
    }

    #[test]
    fn no_release_no_offer() {
        let view = build(Input {
            release: None,
            ..android(&release(None, false), &Install::Idle)
        });
        assert_eq!(view.release, None);
        assert!(!view.install.enabled);
    }
}
