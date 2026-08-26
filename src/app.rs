//! Which window an App Action raises, and when it launches instead.
//!
//! The rule in DESIGN.md §4 is stated entirely over an ordered list of an
//! application's windows, so it belongs to the Core: no OS call is needed to
//! decide, only to gather the list and to carry out the answer.

/// One window of the application a Chord names, as the Shell found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// Stable for as long as the window exists, and comparable — on Windows,
    /// the `HWND`. The cycle depends on both properties.
    pub id: u64,
    pub on_current_desktop: bool,
}

/// What the Dispatcher should do about an App Action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTarget {
    /// Not running: start it.
    Launch,
    /// Bring this window to the front, switching desktop if it is not here.
    Raise(u64),
    /// The user already has what they asked for.
    Nothing,
}

/// Decides an App Action.
///
/// `windows` is every window belonging to the application, in Z-order with the
/// topmost first — the order `EnumWindows` reports. `foreground` is the window
/// the user is currently in, whatever application owns it.
pub fn choose_window(windows: &[Window], foreground: Option<u64>) -> AppTarget {
    if windows.is_empty() {
        return AppTarget::Launch;
    }

    let here: Vec<u64> = windows
        .iter()
        .filter(|window| window.on_current_desktop)
        .map(|window| window.id)
        .collect();

    // Nothing here, but the application is running: go to it. Raising a window
    // on another desktop switches to that desktop, which is what §4 asks for.
    let Some(&topmost) = here.first() else {
        return AppTarget::Raise(windows[0].id);
    };

    // Not already in this application: raise its topmost window and stop. The
    // cycle only begins once the user is inside it.
    if foreground != Some(topmost) {
        return AppTarget::Raise(topmost);
    }

    if here.len() == 1 {
        return AppTarget::Nothing;
    }

    // The cycle runs over a stable order rather than the Z-order, because every
    // raise rewrites the Z-order: cycling "one down from the top" would step
    // back to the window it just left and never reach a third. Sorting by id is
    // arbitrary but consistent, which is all the cycle needs.
    let mut order = here;
    order.sort_unstable();
    let position = order
        .iter()
        .position(|&id| id == topmost)
        .expect("the foreground window is one of the application's own");
    AppTarget::Raise(order[(position + 1) % order.len()])
}
