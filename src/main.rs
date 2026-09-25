//! The Footman binary.
//!
//! Run with no arguments it loads the config, starts the keyboard hook on its
//! own thread and the Dispatcher on another, then sits in the tray. Nothing
//! opens by itself: the settings window is reached from the tray icon, and the
//! logon task must not put anything on screen at all.
//!
//! The subcommands are two different things. `install`, `uninstall` and `where`
//! are features: installation is a copy of the executable the user is holding,
//! and a terminal is where they are holding it. `windows`, `apps`, `desktop`
//! and `focus` are diagnostics — they run one piece of Footman without the
//! keyboard, which is how the manual checks in DESIGN.md §12 get run.

// Footman starts at logon and lives in the tray, so it must not own a console:
// a console-subsystem executable started by the scheduler puts a terminal on
// screen every time the user logs in, which is the one thing a background
// launcher may not do. The subcommands still print — `attach_console` below
// borrows the terminal they were typed into.
#![windows_subsystem = "windows"]

fn main() {
    #[cfg(windows)]
    attach_console();

    #[cfg(windows)]
    match std::env::args().nth(1).as_deref() {
        // A diagnostic, not a feature: it prints the same enumeration and the
        // same identity cascade the App Action uses, so what it shows is what a
        // Binding will match. The settings window (slice 7) replaces it as the
        // way a user picks an application.
        Some("windows") => survey(),
        // The other list a Binding can be written from: what the settings
        // window offers when an App Action is chosen.
        Some("apps") => {
            footman::windows::init_thread();
            for app in footman::windows::applications() {
                println!("{:<44} {}", app.name, app.identity);
            }
        }
        // Runs one App Action without involving the keyboard, so the matrix in
        // DESIGN.md §12 can be walked deliberately rather than by pressing a
        // Chord and hoping.
        // The other half of the §12 matrix: switch desktops without pressing a
        // Chord, and say where Windows thinks we are first.
        Some("desktop") => desktop(std::env::args().nth(2)),
        Some("focus") => match std::env::args().nth(2) {
            Some(identity) => focus(&identity),
            None => eprintln!("footman: focus needs an App Identity, as `footman windows` prints"),
        },
        // Installation from the command line as well as from the settings
        // window, because the thing being installed is a copy of the executable
        // the user is holding, and a terminal is where they are holding it.
        Some("install") => install(),
        Some("uninstall") => uninstall(),
        // What `install` and `uninstall` did or would do, without doing it.
        Some("where") => where_(),
        Some(other) => eprintln!("footman: unknown command {other:?}"),
        None => windows_main(),
    }

    #[cfg(not(windows))]
    eprintln!("footman: only Windows is implemented so far (DESIGN.md §13)");
}

/// Borrows the terminal Footman was started from, if there is one.
///
/// A Windows-subsystem process starts with no console at all, so without this
/// every subcommand would be silent — and the subcommands exist to be read.
/// Attaching to the parent's console gives them somewhere to print when a
/// person ran Footman, and fails harmlessly when the logon task did, which is
/// exactly the distinction wanted: no console, no output, nothing on screen.
///
/// The shell does not wait for a Windows-subsystem process, so its prompt
/// returns before the output arrives. That is cosmetic and the price of not
/// flashing a terminal at every logon.
#[cfg(windows)]
fn attach_console() {
    use std::os::windows::io::AsRawHandle;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
        SetStdHandle,
    };

    // No parent console: started by the scheduler, or from Explorer. Printing
    // is discarded by the standard library rather than failing, so there is
    // nothing further to arrange.
    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_err() {
        return;
    }

    // Attaching does not fill in standard handles the process already has — a
    // redirect into a file, which must go on working — and a Windows-subsystem
    // process launched from a terminal has none at all. So only the missing
    // ones are opened, and only if any are.
    let missing: Vec<_> = [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
        .into_iter()
        .filter(|&which| match unsafe { GetStdHandle(which) } {
            Ok(handle) => handle.is_invalid(),
            Err(_) => true,
        })
        .collect();
    if missing.is_empty() {
        return;
    }

    let Ok(console) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONOUT$")
    else {
        return;
    };

    let handle = HANDLE(console.as_raw_handle());
    for which in missing {
        let _ = unsafe { SetStdHandle(which, handle) };
    }
    // The handles just installed point at this console, so it has to outlive
    // the file that opened it.
    std::mem::forget(console);
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
        Ok(decision) => println!("footman: {identity} -> {decision:?}"),
        Err(error) => eprintln!("footman: {error}"),
    }
}

#[cfg(windows)]
fn desktop(index: Option<String>) {
    match footman::windows::position() {
        Ok(here) => println!("footman: desktop {} of {}", here.current, here.count),
        Err(error) => return eprintln!("footman: {error}"),
    }

    let Some(index) = index else { return };
    match index
        .parse()
        .map_err(|_| format!("{index} is not a desktop number"))
    {
        Ok(index) => match footman::windows::switch_to(index) {
            Ok(()) => println!("footman: switched to desktop {index}"),
            Err(error) => eprintln!("footman: {error}"),
        },
        Err(error) => eprintln!("footman: {error}"),
    }
}

/// Installs a copy and registers the logon task.
#[cfg(windows)]
fn install() {
    use footman::windows::{install, task};

    match install::install() {
        Ok(copy) => println!("footman: installed at {}", copy.display()),
        Err(error) => return eprintln!("footman: {error}"),
    }
    match install::home().map(|home| task::register(&install::copy_in(&home))) {
        Ok(Ok(())) => println!("footman: registered the {} logon task", task::NAME),
        Ok(Err(error)) | Err(error) => eprintln!("footman: {error}"),
    }
}

/// Removes the task, the config directory and the installed copy.
#[cfg(windows)]
fn uninstall() {
    use footman::{Config, windows::install};

    let Some(path) = Config::default_path() else {
        return eprintln!("footman: no config directory on this system");
    };

    match install::uninstall(&path) {
        // The copy removes itself moments later, once this process is gone: a
        // running executable cannot delete itself.
        Ok(()) => println!("footman: removed. Nothing of Footman is left."),
        Err(error) => eprintln!("footman: {error}"),
    }
}

/// Says where everything is, which is what verifying an install comes down to.
#[cfg(windows)]
fn where_() {
    use footman::{
        Config,
        windows::{install, task},
    };

    println!(
        "running   {}",
        std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| error.to_string())
    );
    match install::home() {
        Ok(home) => println!(
            "installed {} ({})",
            install::copy_in(&home).display(),
            if install::installed() {
                "present"
            } else {
                "absent"
            }
        ),
        Err(error) => println!("installed {error}"),
    }
    println!(
        "config    {}",
        Config::default_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "nowhere".to_string())
    );
    println!(
        "log       {}",
        Config::default_path()
            .map(|path| footman::windows::journal::beside(&path)
                .display()
                .to_string())
            .unwrap_or_else(|| "nowhere".to_string())
    );
    println!(
        "task      {} ({})",
        task::NAME,
        if task::registered() {
            "registered"
        } else {
            "not registered"
        }
    );
}

#[cfg(windows)]
fn windows_main() {
    use std::cell::Cell;
    use std::panic::{self, AssertUnwindSafe};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use footman::windows::{Recovery, Revival, journal};
    use footman::{Config, Core, Duty, Effect, Settings};
    use windows::Win32::System::Threading::GetCurrentThreadId;

    /// How many times the tray may die inside `DEATH_SPAN_MS` before Footman
    /// stops rebuilding it (ADR-0008).
    ///
    /// Eight rather than the five this began as. A laptop waking from
    /// hibernation was measured taking some thirty seconds to have a GPU to
    /// give the window, and every rebuild before that fails — four of the five
    /// were spent waiting for the graphics stack rather than on anything wrong
    /// with Footman.
    const DEATHS: usize = 8;
    const DEATH_SPAN_MS: u64 = 10 * 60 * 1_000;

    let Some(path) = Config::default_path() else {
        eprintln!("footman: no config directory on this system");
        return;
    };

    // First, so that everything after it — a config that will not load, a
    // panic on any thread — has somewhere to be written down. Standard error
    // is not that place: started at logon, Footman has no console.
    journal::start(journal::beside(&path));

    let loaded = match Config::load_or_create(&path) {
        Ok(loaded) => loaded,
        Err(error) => {
            // The keyboard comes first (DESIGN.md §7): if the config cannot be
            // trusted, no hook is installed and the keyboard stays untouched.
            journal::note(format!("the config could not be loaded: {}", error.message));
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
    let (duty, duties) = mpsc::channel::<Duty>();

    // The Dispatcher. Every Effect runs here rather than on the hook thread,
    // because a hook callback that overruns its timeout is silently uninstalled
    // (DESIGN.md §9.1).
    thread::spawn(move || footman::windows::dispatch(inbox));

    let core = Core::new(config.hyper, config.tap, config.bindings.clone());
    // This thread is the one the tray lives on, and the one the hook nudges
    // when its state changes.
    let here = unsafe { GetCurrentThreadId() };
    let hook = match footman::windows::spawn(core, effects, duty, here) {
        Ok(hook) => hook,
        Err(error) => {
            journal::note(format!("the keyboard hook could not start: {error}"));
            return eprintln!("footman: {error}");
        }
    };

    // The first Duty the hook reports arrives on the channel, so the icon is
    // correct even if the hook could not be installed.
    let standing = Cell::new(duties.try_recv().unwrap_or(Duty::Active));
    let mut revival = Revival::new(DEATHS, DEATH_SPAN_MS);
    let born = Instant::now();

    // The tray and the settings window own the main thread from here
    // (DESIGN.md §10) — and can die without taking Footman with them. They
    // belong to eframe, whose GL context does not always survive the laptop
    // waking from hibernation; the hook thread never needed it (ADR-0008).
    loop {
        // Read again for each window rather than once: a window rebuilt after
        // dying should show what was last saved, not what was loaded at logon.
        let settings = Config::load_or_create(&path)
            .map(|loaded| Settings::from(loaded.config))
            .unwrap_or_else(|_| Settings::from(config.clone()));

        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            footman::windows::run_ui(path.clone(), settings, &hook, &duties, &standing)
        }));
        let death = match outcome {
            // Quit, or Uninstall. The hook is already on its way down.
            Ok(Ok(())) => return,
            Ok(Err(error)) => error,
            // What it said, and where, the panic hook has already journalled.
            Err(_) => "it panicked".to_string(),
        };

        let now = u64::try_from(born.elapsed().as_millis()).unwrap_or(u64::MAX);
        match revival.died(now) {
            Recovery::After(wait) => {
                journal::note(format!(
                    "the tray and settings window died ({death}); rebuilding them in {wait} ms"
                ));
                thread::sleep(Duration::from_millis(wait));
            }
            Recovery::GiveUp => {
                // A hook nobody can pause or quit is not one to leave running.
                journal::note(format!(
                    "the tray and settings window died ({death}) too often to go on \
                     rebuilding them; Footman is stopping"
                ));
                eprintln!("footman: the tray could not be kept alive: {death}");
                hook.quit();
                return;
            }
        }
    }
}
