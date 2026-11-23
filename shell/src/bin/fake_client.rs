//! Turu: a stand-in for `turu --client=<address>` in `turu_shell`'s tests: it connects with the
//! shell's token, says hello and its status, and stays until the shell closes the connection or
//! its stdin (`--exit-with-parent`). No player, no window.

use std::io::Read as _;
use turu_frontend::shell::{self, TabGame, ToShell};

fn main() {
    let Some(address) = std::env::args().find_map(|arg| arg.strip_prefix("--client=").map(str::to_owned)) else {
        eprintln!("fake client: no --client=<address>");
        std::process::exit(2);
    };
    let token = std::env::var(shell::TOKEN_ENV).unwrap_or_default();
    let (mut reader, mut writer) = match shell::connect(&address, &token) {
        Ok(streams) => streams,
        Err(e) => {
            eprintln!("fake client: {e}");
            std::process::exit(1);
        }
    };
    let status = ToShell::Status {
        label: format!("Fake {}", std::process::id()),
        busy: false,
        waits: None,
        rail_hidden: false,
        game: TabGame::default(),
        map: None,
        script: None,
        streamer: false,
        held: None,
    };
    if shell::write(&mut writer, &status, None).is_err() {
        std::process::exit(1);
    }
    // Gone with the shell: its stdin closes (`--exit-with-parent`), or its connection.
    std::thread::spawn(|| {
        let _ = std::io::stdin().read_to_end(&mut Vec::new());
        std::process::exit(0);
    });
    while let Ok(Some(_)) = shell::read::<shell::ToClient>(&mut reader) {}
}
