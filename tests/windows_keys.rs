//! Translation between Windows virtual-key codes and Footman's `Key`.
//!
//! This is the whole of what the Shell has to know about keys: the Core reasons
//! in `Key`, and nothing above this line has ever heard of a VK code (ADR-0002).

#![cfg(windows)]

use footman::Key;
use footman::windows::{key_to_vk, vk_to_key};

#[test]
fn every_key_survives_a_round_trip_through_windows() {
    for &key in Key::ALL {
        assert_eq!(
            vk_to_key(key_to_vk(key)),
            Some(key),
            "{key} did not survive the round trip"
        );
    }
}

/// A Chord is written `Shift+C`, never `LeftShift+C`, so both sides of the
/// keyboard have to arrive as the same `Key`.
#[test]
fn left_and_right_modifiers_collapse_to_one_key() {
    const VK_LSHIFT: u32 = 0xA0;
    const VK_RSHIFT: u32 = 0xA1;
    const VK_LCONTROL: u32 = 0xA2;
    const VK_RCONTROL: u32 = 0xA3;
    const VK_LMENU: u32 = 0xA4;
    const VK_RMENU: u32 = 0xA5;

    assert_eq!(vk_to_key(VK_LSHIFT), Some(Key::Shift));
    assert_eq!(vk_to_key(VK_RSHIFT), Some(Key::Shift));
    assert_eq!(vk_to_key(VK_LCONTROL), Some(Key::Ctrl));
    assert_eq!(vk_to_key(VK_RCONTROL), Some(Key::Ctrl));
    assert_eq!(vk_to_key(VK_LMENU), Some(Key::Alt));
    assert_eq!(vk_to_key(VK_RMENU), Some(Key::Alt));
}

/// Punctuation, the numeric keypad and everything else Footman cannot bind must
/// come back as `None` so the hook passes it straight through.
#[test]
fn keys_footman_cannot_bind_are_unmapped() {
    const VK_OEM_1: u32 = 0xBA; // `;` on a US layout, something else elsewhere
    const VK_NUMPAD0: u32 = 0x60;
    const VK_LWIN: u32 = 0x5B;

    for vk in [VK_OEM_1, VK_NUMPAD0, VK_LWIN, 0x00, 0xFF] {
        assert_eq!(vk_to_key(vk), None, "VK {vk:#04x} must be unmapped");
    }
}
