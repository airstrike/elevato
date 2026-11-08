//! What riders on a floor do. A floor is its level, a plain number:
//! the lobby is 0.

/// What riders on a floor do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// A rider who wants to go up pressed the up button, and it was not
    /// lit. A rider left behind by a full car presses it again.
    UpButtonPressed,
    /// A rider who wants to go down pressed the down button, and it was
    /// not lit.
    DownButtonPressed,
}
