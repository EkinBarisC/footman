//! Which threads Footman attaches its input queue to when raising a window.
//!
//! `AttachThreadInput` is how Footman crosses the foreground lock: joining the
//! conversation between the window being left and the window being arrived at
//! is enough to be granted the foreground. It is also a way to hand this thread
//! over to another one — attached queues process input together, so a thread
//! that stops answering takes its partner with it.
//!
//! Which is why the settings window matters here. When Footman's own window is
//! the one being left, the thread on the other end of the attachment is
//! Footman's own user interface — and the Dispatcher would be waiting on the
//! very thread the user is clicking around in. It does not need to: Windows
//! grants the foreground unconditionally to a process that already holds it,
//! so there is nothing to buy and a Dispatcher to lose.

#![cfg(windows)]

use footman::windows::worth_attaching;

const OURS: u32 = 100;

#[test]
fn another_processs_thread_is_worth_attaching_to() {
    assert!(worth_attaching(200, OURS, false));
}

#[test]
fn our_own_thread_is_not() {
    assert!(!worth_attaching(OURS, OURS, false));
}

/// The settings window being open must not put the Dispatcher behind the user
/// interface thread. We already own the foreground in that case, which is the
/// only thing the attachment was for.
#[test]
fn no_thread_of_ours_is_worth_attaching_to() {
    assert!(!worth_attaching(200, OURS, true));
}

/// `GetWindowThreadProcessId` answers zero for a window that has gone.
#[test]
fn a_thread_that_is_not_there_is_not() {
    assert!(!worth_attaching(0, OURS, false));
}
