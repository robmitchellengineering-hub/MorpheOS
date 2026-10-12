//! The shell's input model: a key, a pointer, and the event that carries either.
//!
//! **Why these types exist rather than `Shell::type_char(char)`.** Input arrives at a
//! real shell as discrete events, and the shell's behaviour — which session has focus,
//! what text a terminal holds — is a function of those events. A type per event kind keeps
//! that mapping total and testable: every event either lands on a session or it is a
//! no-op, and both paths are asserted. This is the *model*, not the wire format; how a
//! Wayland compositor or a keyboard driver produces an [`Event`] is the compositor's job,
//! later, on Linux.
//!
//! [`Event`] deliberately carries no timestamp or modifier state yet. Those arrive with a
//! real input backend; this slice only needs what changes the frame — a key, or a press at
//! a position.

/// A key press. The set is the small one the frame can reflect: text, the focus keys, and
/// the editing keys a terminal buffer needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A printable character, appended to the focused session's terminal.
    Char(char),
    /// Move focus to the next session, wrapping around.
    Tab,
    /// Delete the character left of the terminal's cursor.
    Backspace,
    /// Start a new line in the focused session's terminal.
    Enter,
}

/// A pointer event, in frame coordinates with the origin at the top left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pointer {
    /// A press at a position. A press inside a session's surface gives it focus.
    Press { x: u32, y: u32 },
}

/// One input event, as the shell consumes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A keyboard event.
    Key(Key),
    /// A pointer event.
    Pointer(Pointer),
}
