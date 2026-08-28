//! The Footman binary.
//!
//! It loads the config, starts the keyboard hook on its own thread and the
//! Dispatcher on another, then sits in the tray. The settings window is slice 7;
//! until then the tray offers Pause and Quit, and the subcommands below are how
//! the manual checks in DESIGN.md §12 get run.

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
        // The other half of the §12 matrix: switch desktops without pressing a
        // Chord, and say where Windows thinks we are first.
        Some("desktop") => desktop(std::env::args().nth(2)),
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

#[cfg(windows)]
fn windows_main() {
    use std::sync::mpsc;
    use std::thread;

    use footman::{Config, Core, Duty, Effect};
    use windows::Win32::System::Threading::GetCurrentThreadId;

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
    let (duty, duties) = mpsc::channel::<Duty>();

    // The Dispatcher. Every Effect runs here rather than on the hook thread,
    // because a hook callback that overruns its timeout is silently uninstalled
    // (DESIGN.md §9.1).
    thread::spawn(move || footman::windows::dispatch(inbox));

    let core = Core::new(config.hyper, config.tap, config.bindings);
    // This thread is the one the tray lives on, and the one the hook nudges
    // when its state changes.
    let here = unsafe { GetCurrentThreadId() };
    let hook = match footman::windows::spawn(core, effects, duty, here) {
        Ok(hook) => hook,
        Err(error) => return eprintln!("footman: {error}"),
    };

    tray(hook, duties);
}

/// Draws the tray and runs the message loop that keeps it, and the whole
/// application, alive (DESIGN.md §10).
#[cfg(windows)]
fn tray(hook: footman::windows::Hook, duties: std::sync::mpsc::Receiver<footman::Duty>) {
    use footman::{Click, Duty, TrayEffect};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{Icon, TrayIconBuilder};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PostQuitMessage, TranslateMessage,
    };

    fn dressed(duty: Duty) -> Option<Icon> {
        Icon::from_rgba(duty.icon(), 32, 32).ok()
    }

    let settings = MenuItem::new("Settings…", true, None);
    // No accelerator, here or ever: a shortcut that disables shortcuts is a
    // trap, and pressing it by accident leaves the user unable to work out why
    // nothing responds (DESIGN.md §10).
    let pause = MenuItem::new(Duty::Active.pause_label(), true, None);
    let quit = MenuItem::new("Quit", true, None);

    let menu = Menu::new();
    for item in [&settings, &pause, &quit] {
        if let Err(error) = menu.append(item) {
            return eprintln!("footman: could not build the tray menu: {error}");
        }
    }

    let mut duty = Duty::Active;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(duty.tooltip())
        .with_icon(dressed(duty).unwrap_or_else(|| unreachable!("a 32×32 icon is a valid icon")))
        .build();
    let tray = match tray {
        Ok(tray) => tray,
        Err(error) => return eprintln!("footman: could not put an icon in the tray: {error}"),
    };

    let clicks = MenuEvent::receiver();
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
        // Dispatching first, then reading: a menu click is delivered to the
        // channel from inside the window procedure that DispatchMessageW runs.
        // Draining before dispatching would leave every click sitting unread
        // until some unrelated message happened to arrive.
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        // The hook thread wakes this loop whenever it changes state, so the
        // channel is never read later than the news arrives.
        while let Ok(reported) = duties.try_recv() {
            duty = reported;
            pause.set_text(duty.pause_label());
            let _ = tray.set_tooltip(Some(duty.tooltip()));
            let _ = tray.set_icon(dressed(duty));
        }

        while let Ok(click) = clicks.try_recv() {
            let click = if click.id == settings.id() {
                Click::Settings
            } else if click.id == pause.id() {
                Click::PauseOrResume
            } else if click.id == quit.id() {
                Click::Quit
            } else {
                continue;
            };

            match duty.resolve(click) {
                TrayEffect::Pause => hook.pause(),
                TrayEffect::Resume => hook.resume(),
                TrayEffect::OpenSettings => {
                    println!("footman: the settings window arrives in slice 7");
                }
                TrayEffect::Quit => {
                    // The hook comes down first, so the keyboard is normal
                    // before this process stops existing.
                    hook.quit();
                    unsafe { PostQuitMessage(0) };
                }
            }
        }
    }
}
