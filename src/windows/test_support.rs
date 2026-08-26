//! Synthesising real key events, for the end-to-end test only.
//!
//! Compiled only under the `integration-test` feature. It drives the actual
//! Windows input queue, so it is never part of a normal build.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY,
};

use std::thread;
use std::time::Duration;

use crate::Key;
use crate::windows::key_to_vk;

/// Gap left between synthesised events.
///
/// Not politeness: input arriving faster than a human could produce it is the
/// definition of chatter, and a debouncer sitting earlier in the hook chain
/// will eat it. Real keystrokes have gaps, so the test's must too.
const HUMAN_GAP: Duration = Duration::from_millis(40);

/// Presses or releases a key as though a human had.
///
/// Deliberately *not* stamped with `FOOTMAN_SIGNATURE`: the point of the
/// end-to-end test is to look like real input to the hook.
pub fn send(key: Key, down: bool) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(key_to_vk(key) as u16),
                wScan: 0,
                dwFlags: if down {
                    KEYBD_EVENT_FLAGS(0)
                } else {
                    KEYEVENTF_KEYUP
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    unsafe {
        SendInput(&[input], size_of::<INPUT>() as i32);
    }
    thread::sleep(HUMAN_GAP);
}

/// A full press: down then up.
pub fn tap(key: Key) {
    send(key, true);
    send(key, false);
}
