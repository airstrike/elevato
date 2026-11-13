//! What riders on a floor do. A floor is its level, a plain number:
//! the lobby is 0.

/// What riders on a floor do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// A rider going up presses an unlit call button. Riders left behind
    /// by a full car press it again after the arrival clears it.
    UpButtonPressed,
    /// A rider going down presses an unlit call button.
    DownButtonPressed,
}
