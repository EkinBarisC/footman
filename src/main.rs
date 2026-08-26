//! The Footman binary.
//!
//! A walking skeleton for now: it loads the config, installs the keyboard hook,
//! and reports each Effect the Core produces. Actually carrying Effects out is
//! slices 4 and 5; the tray and settings window are 6 and 7.

fn main() {
    #[cfg(windows)]
    match std::env::args().nth(1).as_deref() {
        // A diagnostic, not a feature: it prints the same enumeration and the
        // same identity cascade the App Action uses, so what it shows is what a
        // Binding will match. The settings window (slice 7) replaces it as the
        // way a user picks an application.
        Some("windows") => survey(),
        // Runs one App Action without involving the keyboard, so the matrix in
        // DESIGN.md §12 can be walked deliberately rather than by pressing a
        // Chord and hoping.
        Some("focus") => match std::env::args().nth(2) {
            Some(identity) => focus(&identity),
            None => eprintln!("footman: focus needs an App Identity, as `footman windows` prints"),
        },
        Some(other) => eprintln!("footman: unknown command {other:?}"),
        None => windows_main(),
    }

    #[cfg(not(windows))]
    eprintln!("footman: only Windows is implemented so far (DESIGN.md §13)");
}

/// Prints every window Footman can see, with the identities it answers to.
#[cfg(windows)]
fn survey() {
    footman::windows::init_thread();
    for (window, identities) in footman::windows::survey() {
        println!("{window}");
        if identities.is_empty() {
            // A window whose owner Footman cannot open — usually one belonging
            // to a more privileged process. It can be seen but never bound.
            println!("    (no identity available)");
        }
        for identity in identities {
            println!("    {identity}");
        }
    }
}

#[cfg(windows)]
fn focus(identity: &str) {
    footman::windows::init_thread();
    match footman::windows::focus(identity) {
        Ok(()) => println!("footman: {identity}"),
        Err(error) => eprintln!("footman: {error}"),
    }
}

#[cfg(windows)]
fn windows_main() {
    use std::sync::mpsc;
    use std::thread;

    use footman::{Config, Core, Effect};

    let Some(path) = Config::default_path() else {
        eprintln!("footman: no config directory on this system");
        return;
    };

    let loaded = match Config::load_or_create(&path) {
        Ok(loaded) => loaded,
        Err(error) => {
            // The keyboard comes first (DESIGN.md §7): if the config cannot be
            // trusted, no hook is installed and the keyboard stays untouched.
            eprintln!("footman: {}", error.message);
            if let Some(line) = error.line {
                eprintln!("        {}:{line}", path.display());
            }
            return;
        }
    };

    for warning in &loaded.warnings {
        eprintln!("footman: {warning:?}");
    }

    let config = loaded.config;
    let bindings = config.bindings.sorted();
    println!(
        "footman: {} + {} binding(s), watching {}",
        config.hyper,
        bindings.len(),
        path.display()
    );
    for (chord, action) in &bindings {
        println!("  {chord:<16} {action:?}");
    }
    if bindings.is_empty() {
        // A config with no bindings is the shape `load_or_create` writes the
        // first time it runs, and it looks identical to a broken hook from the
        // outside: Hyper is swallowed and nothing ever happens.
        println!("  (none — add [[binding]] entries to the file above)");
    }

    let (effects, inbox) = mpsc::channel::<Effect>();

    // The Dispatcher. Every Effect runs here rather than on the hook thread,
    // because a hook callback that overruns its timeout is silently uninstalled
    // (DESIGN.md §9.1).
    thread::spawn(move || footman::windows::dispatch(inbox));

    let core = Core::new(config.hyper, config.tap, config.bindings);
    if let Err(error) = footman::windows::run(core, effects) {
        eprintln!("footman: {error}");
    }
}
