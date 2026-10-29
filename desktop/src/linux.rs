//! AQW build (Mugetsu): Linux process restart and graphics selection.

use crate::mugetsu::{self, Graphics};

/// Starts Mugetsu again with the same command line.
pub fn start_again() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .spawn();
}

pub fn choose_graphics(_window: &winit::window::Window, choice: Option<Graphics>) {
    if choice == mugetsu::saved_graphics() {
        return;
    }
    let name = choice.map_or("Automatic", Graphics::name);
    let answer = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title("Mugetsu")
        .set_description(format!(
            "Mugetsu will restart to use {name} graphics.\n\n\
             You can switch again any time: right-click the game."
        ))
        .set_buttons(rfd::MessageButtons::OkCancel)
        .show();
    if answer != rfd::MessageDialogResult::Ok {
        return;
    }
    mugetsu::save_graphics(choice);
    tracing::info!("Graphics changed to {name}; starting Mugetsu again");
    start_again();
    std::process::exit(0);
}
