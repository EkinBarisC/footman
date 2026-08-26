//! Finding, raising and launching applications — the Windows half of the App
//! Action (DESIGN.md §4, ADR-0003).
//!
//! Everything here runs on the Dispatcher thread. None of it may be called from
//! the hook callback: enumerating windows and talking to COM costs far more
//! than the callback's budget allows (DESIGN.md §9.1).

use std::cell::OnceCell;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, MAX_PATH};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::Shell::{IVirtualDesktopManager, ShellExecuteW, VirtualDesktopManager};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GWL_EXSTYLE, GetForegroundWindow, GetWindow, GetWindowLongPtrW,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow, ShowWindow, WS_EX_TOOLWINDOW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};

use super::identity::{Launch, launch_of, matches};
use crate::{AppTarget, Window, choose_window};

/// Carries out an App Action: raise, cycle or launch (DESIGN.md §4).
pub fn focus(identity: &str) -> Result<(), String> {
    let windows = windows_of(identity);
    let foreground = unsafe { GetForegroundWindow() };
    let foreground = (!foreground.is_invalid()).then_some(foreground.0 as u64);

    match choose_window(&windows, foreground) {
        AppTarget::Nothing => Ok(()),
        AppTarget::Raise(id) => {
            raise(HWND(id as *mut _));
            Ok(())
        }
        AppTarget::Launch => launch(identity),
    }
}

/// Every task window on the machine with the identities it answers to, topmost
/// first — what `footman windows` prints.
///
/// This is the same enumeration and the same cascade the App Action uses, so
/// what it shows is exactly what a Binding will match. Until the settings
/// window exists (slice 7) it is also the only way to find out what to write in
/// a config file.
pub fn survey() -> Vec<(String, Vec<String>)> {
    let mut found = Vec::new();
    for_each_task_window(&mut |hwnd, here| {
        let where_ = if here {
            "this desktop"
        } else {
            "another desktop"
        };
        found.push((format!("{} ({where_})", title(hwnd)), cascade(hwnd)));
    });
    found
}

fn title(hwnd: HWND) -> String {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    let mut buffer = vec![0u16; length as usize + 1];
    let written = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..written as usize])
}

/// Every window belonging to the application, topmost first.
///
/// `EnumWindows` reports windows in Z-order, which is exactly the order
/// `choose_window` wants, so nothing is sorted here.
fn windows_of(identity: &str) -> Vec<Window> {
    let mut found: Vec<Window> = Vec::new();
    for_each_task_window(&mut |hwnd, on_current_desktop| {
        if matches(identity, &cascade(hwnd)) {
            found.push(Window {
                id: hwnd.0 as u64,
                on_current_desktop,
            });
        }
    });
    found
}

/// Walks every window a user would call a window, topmost first, with whether
/// it is on the desktop they are looking at.
///
/// The two questions are answered together because the cloaking rule needs
/// both, and asking twice would mean two COM round trips per window.
fn for_each_task_window(visit: &mut dyn FnMut(HWND, bool)) {
    let mut filtered = |hwnd: HWND| {
        if !is_task_window(hwnd) {
            return;
        }
        let here = on_current_desktop(hwnd);
        if hidden_by_cloaking(cloak_state(hwnd), here) {
            return;
        }
        visit(hwnd, here);
    };
    // The closure is borrowed for the length of the call and never escapes it.
    let mut filtered: &mut dyn FnMut(HWND) = &mut filtered;
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM((&raw mut filtered) as isize));
    }
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let visit = unsafe { &mut *(lparam.0 as *mut &mut dyn FnMut(HWND)) };
    visit(hwnd);
    true.into()
}

/// Whether DWM cloaking means this window is not really there.
///
/// Cloaking alone does not say. Every window on another virtual desktop is
/// cloaked, and those are real windows the App Action must find (DESIGN.md §4).
/// What separates a ghost from a window merely elsewhere is being cloaked *and*
/// on the desktop the user is looking at — a state no genuinely open window was
/// measured in. See `tests/windows_ghosts.rs` for the measurements.
pub fn hidden_by_cloaking(cloaked: u32, on_current_desktop: bool) -> bool {
    /// The application cloaked the window itself: a suspended UWP, wherever it
    /// claims to be.
    const BY_APPLICATION: u32 = 1;

    cloaked & BY_APPLICATION != 0 || (cloaked != 0 && on_current_desktop)
}

fn cloak_state(hwnd: HWND) -> u32 {
    let mut cloaked = 0u32;
    let read = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        )
    };
    // If DWM will not say, the window is not cloaked as far as we know.
    if read.is_ok() { cloaked } else { 0 }
}

/// Whether a window is one the user thinks of as a window — the same test the
/// taskbar and Alt-Tab apply.
fn is_task_window(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || GetWindowTextLengthW(hwnd) == 0 {
            return false;
        }
        // An owned window is a dialog or a palette belonging to another window,
        // not an entry of its own.
        if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.is_invalid()) {
            return false;
        }
        if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return false;
        }
        true
    }
}

/// Every identity this window answers to, in the order of ADR-0003: window
/// AUMID, then process AUMID, then executable path.
///
/// All three are collected rather than stopping at the first, because the order
/// decides which identity is *stored* while any of them may be the one a
/// Binding was written with.
fn cascade(hwnd: HWND) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(aumid) = window_aumid(hwnd) {
        found.push(format!("aumid:{aumid}"));
    }
    if let Some(process) = process_of(hwnd) {
        if let Some(aumid) = process_aumid(process.0) {
            found.push(format!("aumid:{aumid}"));
        }
        if let Some(path) = process_path(process.0) {
            found.push(format!("path:{path}"));
        }
    }
    found
}

/// Step 1: the AUMID the window itself advertises — what the taskbar groups by,
/// and so the closest thing to what a user means by "the same application".
fn window_aumid(hwnd: HWND) -> Option<String> {
    unsafe {
        let store: IPropertyStore = SHGetPropertyStoreForWindow(hwnd).ok()?;
        let value = store.GetValue(&PKEY_AppUserModel_ID).ok()?;
        let text = PropVariantToStringAlloc(&value).ok()?;
        let owned = text.to_string().ok()?;
        CoTaskMemFree(Some(text.0.cast()));
        (!owned.is_empty()).then_some(owned)
    }
}

/// A handle to the process owning a window, closed when it goes out of scope.
struct Process(HANDLE);

impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn process_of(hwnd: HWND) -> Option<Process> {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        // The limited right is enough for both remaining steps and is granted
        // for processes this one could not otherwise open.
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .ok()
            .map(Process)
    }
}

/// Step 2: the AUMID of the package the process belongs to — the step that
/// catches every MSIX application, whose executable path carries a version
/// number and so cannot be relied on.
fn process_aumid(process: HANDLE) -> Option<String> {
    let mut length = 0u32;
    unsafe {
        // Asking with a null buffer reports the length needed.
        let _ = GetApplicationUserModelId(process, &mut length, Some(PWSTR::null()));
        if length == 0 {
            return None;
        }
        let mut buffer = vec![0u16; length as usize];
        GetApplicationUserModelId(process, &mut length, Some(PWSTR(buffer.as_mut_ptr())))
            .ok()
            .ok()?;
        buffer.truncate(length.saturating_sub(1) as usize);
        Some(String::from_utf16_lossy(&buffer))
    }
}

/// Step 3: the executable path — stable for unpackaged Win32 applications,
/// which are exactly the ones the first two steps miss.
fn process_path(process: HANDLE) -> Option<String> {
    let mut buffer = vec![0u16; MAX_PATH as usize];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
        .ok()?;
    }
    buffer.truncate(length as usize);
    Some(String::from_utf16_lossy(&buffer))
}

/// Whether the window is on the desktop the user is looking at.
///
/// `IVirtualDesktopManager` is the documented half of the virtual desktop API.
/// It can answer this question, though it cannot switch desktops, which is what
/// ADR-0004 exists to work around.
fn on_current_desktop(hwnd: HWND) -> bool {
    // Created once per thread rather than once per window: an enumeration asks
    // this of every window on the machine.
    thread_local! {
        static MANAGER: OnceCell<Option<IVirtualDesktopManager>> = const { OnceCell::new() };
    }

    let query = || -> windows::core::Result<bool> {
        MANAGER.with(|cell| {
            let manager = cell.get_or_init(|| unsafe {
                CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_ALL).ok()
            });
            match manager {
                Some(manager) => {
                    Ok(unsafe { manager.IsWindowOnCurrentVirtualDesktop(hwnd)?.as_bool() })
                }
                None => Ok(true),
            }
        })
    };
    // If Windows will not say, treat the window as present. Raising a window
    // that turns out to be elsewhere is a smaller failure than refusing to
    // raise one that is right here.
    query().unwrap_or(true)
}

/// Brings a window to the front, restoring it if it was minimised.
fn raise(hwnd: HWND) {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        // Windows refuses `SetForegroundWindow` to a process that does not
        // already own the foreground, to stop applications stealing focus.
        // Attaching to the foreground window's input queue makes this process
        // part of that conversation for the length of the call.
        let foreground = GetForegroundWindow();
        let ours = GetCurrentThreadId();
        let theirs = GetWindowThreadProcessId(foreground, None);
        let attached =
            theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, true).as_bool();

        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));

        if attached {
            let _ = AttachThreadInput(ours, theirs, false);
        }
    }
}

fn launch(identity: &str) -> Result<(), String> {
    let target = match launch_of(identity) {
        Some(Launch::AppsFolder(aumid)) => format!(r"shell:AppsFolder\{aumid}"),
        Some(Launch::Executable(path)) => path,
        None => return Err(format!("unknown App Identity scheme: {identity}")),
    };

    let target = HSTRING::from(&target);
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR::null(),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns a fake HINSTANCE: anything at or below 32 is an
    // error code rather than a handle.
    if result.0 as usize > 32 {
        Ok(())
    } else {
        Err(format!("could not launch {identity}"))
    }
}

/// COM has to be initialised on every thread that uses it, and the Dispatcher
/// is the only thread here that does.
pub fn init_thread() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}
