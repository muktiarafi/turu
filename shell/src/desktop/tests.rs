use super::*;

fn shell() -> Shell<NoSurface> {
    let config = Config {
        exe: PathBuf::from("/nonexistent/turu"),
        surfaces_name: None,
        name: Box::new(|| "Turu".to_owned()),
    };
    Shell::new(config, Arc::new(|_| true)).expect("listens on loopback")
}

/// A tab whose client counts as connected (its connection one end of a loopback pair).
fn running_tab(id: u32) -> (Tab, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("binds");
    let theirs = TcpStream::connect(listener.local_addr().expect("address")).expect("connects");
    let (ours, _) = listener.accept().expect("accepts");
    let mut tab = Tab::new(id, Vec::new(), None);
    tab.out = Some(ours);
    (tab, theirs)
}

#[test]
fn a_dead_client_starts_again_later_each_time() {
    let shell = shell();
    let mut tab = Tab::new(1, Vec::new(), None);
    let start = Instant::now();
    let mut delays = Vec::new();
    for death in 0..6 {
        let now = start + Duration::from_secs(death);
        shell.schedule_restart(&mut tab, now);
        delays.push(tab.restart_at.map(|at| at.duration_since(now).as_secs()));
    }
    // Alone, it's never left closed: the window would take no input.
    assert_eq!(delays, [Some(1), Some(4), Some(16), Some(60), Some(60), Some(60)]);
    assert!(!tab.closed);
    assert_eq!(tab.down().as_deref(), Some("restarting"));
}

#[test]
fn a_tab_that_keeps_dying_is_left_closed_while_another_runs() {
    let mut shell = shell();
    let (other, _theirs) = running_tab(2);
    shell.tabs.push(other);
    let mut tab = Tab::new(1, Vec::new(), None);
    let start = Instant::now();
    for death in 0..MAX_DEATHS as u64 {
        shell.schedule_restart(&mut tab, start + Duration::from_secs(death));
    }
    assert!(tab.closed);
    assert_eq!(tab.restart_at, None);
    assert_eq!(tab.down().as_deref(), Some("closed"));
}

#[test]
fn deaths_long_ago_are_forgotten() {
    let mut shell = shell();
    let (other, _theirs) = running_tab(2);
    shell.tabs.push(other);
    let mut tab = Tab::new(1, Vec::new(), None);
    let start = Instant::now();
    for death in 0..MAX_DEATHS as u64 - 1 {
        shell.schedule_restart(&mut tab, start + Duration::from_secs(death));
    }
    // Ten minutes on, the earlier deaths are out of the window: the first delay again.
    let later = start + DEATH_WINDOW + Duration::from_secs(10);
    shell.schedule_restart(&mut tab, later);
    assert!(!tab.closed);
    assert_eq!(
        tab.restart_at.map(|at| at.duration_since(later)),
        Some(Duration::from_secs(1))
    );
}

#[test]
fn grid_view_shares_the_window_less_the_ui_between_the_tabs() {
    let mut shell = shell();
    shell.tabs.push(Tab::new(1, Vec::new(), None));
    shell.tabs.push(Tab::new(2, Vec::new(), None));
    shell.size = Some((2000, 1000, 2.0));
    assert_eq!(shell.grid_tiles(), None, "off");
    shell.set_grid(Some((0.0, 25.0)));
    let tiles = shell.grid_tiles().expect("on with two tabs");
    assert_eq!(tiles.iter().map(|tile| tile.id).collect::<Vec<_>>(), [1, 2]);
    // 25 points under the bar, at scale 2.
    assert!(tiles.iter().all(|tile| tile.y >= 50 && tile.y + tile.height <= 1000));
    assert!(tiles.iter().all(|tile| tile.x + tile.width <= 2000));
    shell.tabs.pop();
    assert_eq!(shell.grid_tiles(), None, "one tab");
}

#[test]
fn the_title_names_the_tab_on_screen_and_counts_the_tabs() {
    let mut shell = shell();
    shell.window_opened();
    assert_eq!(shell.actions(), [Action::Title("Turu".into())]);
    shell.tabs.push(Tab::new(1, Vec::new(), None));
    shell.shown = 1;
    shell.update_title();
    assert_eq!(shell.actions(), [Action::Title("Turu – Login".into())]);
    shell.tabs.push(Tab::new(2, Vec::new(), None));
    shell.update_title();
    assert_eq!(shell.actions(), [Action::Title("Turu – Login (2 tabs)".into())]);
    shell.update_title();
    assert_eq!(shell.actions(), [], "unchanged");
}

#[test]
fn only_one_file_dialog_at_a_time() {
    let mut shell = shell();
    // A tab with a client process (any program that waits).
    let mut tab = Tab::new(1, Vec::new(), None);
    tab.child = Command::new("sleep").arg("30").stdin(Stdio::piped()).spawn().ok();
    let Some(pid) = tab.pid() else {
        return;
    };
    shell.tabs.push(tab);
    let ask = |request| ToShell::FileDialog {
        id: request,
        save: false,
        title: Some("Load".into()),
        directory: None,
        file_name: None,
        filters: vec![("Scripts".into(), vec!["ts".into()])],
    };
    shell.handle(Event::Message(1, ask(7)));
    let asked = shell.actions();
    assert!(
        matches!(&asked[..], [Action::FileDialog(dialog)] if dialog.request == 7 && dialog.pid == pid && dialog.tab == 1),
        "{asked:?}"
    );
    // The second is answered at once with nothing, as the window's own dialogs are.
    shell.handle(Event::Message(1, ask(8)));
    assert_eq!(shell.actions(), []);
    shell.handle(Event::Answered(1, pid, 7, None));
    assert!(!shell.dialog_open());
    shell.stop_all();
}

#[test]
fn the_only_tab_closing_itself_quits() {
    let mut shell = shell();
    shell.tabs.push(Tab::new(1, Vec::new(), None));
    shell.handle(Event::Message(1, ToShell::CloseTab { id: 1 }));
    assert_eq!(shell.actions(), [Action::Quit]);
}

#[test]
fn clients_get_the_shells_flags_but_its_servers() {
    let args = [
        "--shell",
        "--user",
        "Questwalker",
        "--api=127.0.0.1:9091",
        "--monitor",
        "--report-to",
        "https://monitor.example",
        "--run-script",
        "farm/Gold.ts",
        "--report-to=https://other.example",
        "--monitor=0.0.0.0:9021",
        "--api",
        // Gamepads act through these mappings in the clients (the shell reads the pads).
        "-B",
        "south=space",
        "--gamepad-button=north=up",
    ];
    let kept = client_args(args.iter().map(|arg| (*arg).to_owned()));
    assert_eq!(
        kept,
        [
            "--user",
            "Questwalker",
            "--run-script",
            "farm/Gold.ts",
            "-B",
            "south=space",
            "--gamepad-button=north=up"
        ]
    );
}

#[test]
fn a_client_that_stops_reading_is_let_go_not_waited_for() {
    let (tab, mut theirs) = running_tab(1);
    let ours = tab.out.as_ref().expect("connected");
    ours.set_write_timeout(Some(Duration::from_millis(100)))
        .expect("timeout");
    // Large enough that the socket's buffers fill while `theirs` reads nothing.
    let message = ToClient::NoSurfaces {
        reason: "x".repeat(256 * 1024),
    };
    let started = Instant::now();
    for _ in 0..64 {
        send(&tab, &message);
    }
    // Each write waits at most its timeout, and once one has timed out the rest fail at once.
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "took {:?}",
        started.elapsed()
    );
    // The connection was ended: the client reads what came, then the end.
    theirs.set_read_timeout(Some(Duration::from_secs(5))).expect("timeout");
    let mut sink = Vec::new();
    std::io::Read::read_to_end(&mut theirs, &mut sink).expect("reads to the end");
}
