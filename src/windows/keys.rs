//! Windows virtual-key codes, in both directions.
//!
//! Declared once as a table so the two directions cannot disagree.

use crate::Key;

macro_rules! vk_table {
    ($($variant:ident => $vk:literal),* $(,)?) => {
        /// The virtual-key code Footman sends or compares against for a `Key`.
        pub const fn key_to_vk(key: Key) -> u32 {
            match key { $(Key::$variant => $vk),* }
        }

        /// The `Key` a virtual-key code stands for, if Footman can bind it.
        ///
        /// `None` means "not ours": punctuation (layout-dependent, so
        /// deliberately unbindable), the numeric keypad, Win, and everything
        /// else. The hook passes those straight through.
        pub const fn vk_to_key(vk: u32) -> Option<Key> {
            match vk {
                $($vk => Some(Key::$variant),)*
                // Windows reports each modifier as a specific side; a Chord is
                // written `Shift+C` and never `LeftShift+C`, so both sides
                // arrive as the same Key.
                VK_LSHIFT | VK_RSHIFT => Some(Key::Shift),
                VK_LCONTROL | VK_RCONTROL => Some(Key::Ctrl),
                VK_LMENU | VK_RMENU => Some(Key::Alt),
                _ => None,
            }
        }
    };
}

const VK_LSHIFT: u32 = 0xA0;
const VK_RSHIFT: u32 = 0xA1;
const VK_LCONTROL: u32 = 0xA2;
const VK_RCONTROL: u32 = 0xA3;
const VK_LMENU: u32 = 0xA4;
const VK_RMENU: u32 = 0xA5;

vk_table! {
    A => 0x41, B => 0x42, C => 0x43, D => 0x44,
    E => 0x45, F => 0x46, G => 0x47, H => 0x48,
    I => 0x49, J => 0x4A, K => 0x4B, L => 0x4C,
    M => 0x4D, N => 0x4E, O => 0x4F, P => 0x50,
    Q => 0x51, R => 0x52, S => 0x53, T => 0x54,
    U => 0x55, V => 0x56, W => 0x57, X => 0x58,
    Y => 0x59, Z => 0x5A, Digit0 => 0x30, Digit1 => 0x31,
    Digit2 => 0x32, Digit3 => 0x33, Digit4 => 0x34, Digit5 => 0x35,
    Digit6 => 0x36, Digit7 => 0x37, Digit8 => 0x38, Digit9 => 0x39,
    F1 => 0x70, F2 => 0x71, F3 => 0x72, F4 => 0x73,
    F5 => 0x74, F6 => 0x75, F7 => 0x76, F8 => 0x77,
    F9 => 0x78, F10 => 0x79, F11 => 0x7A, F12 => 0x7B,
    F13 => 0x7C, F14 => 0x7D, F15 => 0x7E, F16 => 0x7F,
    F17 => 0x80, F18 => 0x81, F19 => 0x82, F20 => 0x83,
    F21 => 0x84, F22 => 0x85, F23 => 0x86, F24 => 0x87,
    Left => 0x25, Right => 0x27, Up => 0x26, Down => 0x28,
    Space => 0x20, Enter => 0x0D, Tab => 0x09, Backspace => 0x08,
    Escape => 0x1B, Delete => 0x2E, Insert => 0x2D, Home => 0x24,
    End => 0x23, PageUp => 0x21, PageDown => 0x22, CapsLock => 0x14,
    Shift => 0x10, Ctrl => 0x11, Alt => 0x12,
}
