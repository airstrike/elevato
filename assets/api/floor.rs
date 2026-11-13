/// Call-button presses, delivered through `riders()` as an
/// `elevato::Message::Floor`. Floor numbers start at 0 for the lobby.
pub enum Message {
    /// A rider going up presses an unlit call button. Riders left behind
    /// by a full car press it again after the arrival clears it.
    UpButtonPressed,

    /// A rider going down presses an unlit call button.
    DownButtonPressed,
}
