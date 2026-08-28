//! The keystrokes Footman produces itself.
//!
//! There are exactly two: the arrows that walk between virtual desktops
//! (ADR-0004) and the Tap Action's Escape. Hyper is never among them — that is
//! ADR-0001, and the reason this module is this short.
//!
//! Everything sent here is stamped so the hook recognises Footman's own work
//! and does not feed it back into the Core. The stamp is deliberately not
//! `LLKHF_INJECTED`: that flag also marks on-screen keyboards, accessibility
//! tools and remote desktop sessions, which Footman must still answer to.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY,
};

use super::hook::FOOTMAN_SIGNATURE;

pub fn press(key: VIRTUAL_KEY) -> INPUT {
    stroke(key, Default::default())
}

pub fn release(key: VIRTUAL_KEY) -> INPUT {
    stroke(key, KEYEVENTF_KEYUP)
}

/// Down and up, as one keystroke.
pub fn tap(key: VIRTUAL_KEY) {
    send(&[press(key), release(key)]);
}

pub fn send(batch: &[INPUT]) {
    unsafe {
        SendInput(batch, size_of::<INPUT>() as i32);
    }
}

fn stroke(key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: FOOTMAN_SIGNATURE,
            },
        },
    }
}
