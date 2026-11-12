/// What riders on a floor do. Arrives through `riders()` as an
/// `elevato::Message::Floor`, with the floor, a number: the lobby is 0.
pub enum Message {
    /// A rider going up pressed the up button, and it was not lit. A
    /// rider a full car left behind presses it again.
    UpButtonPressed,

    /// A rider going down pressed the down button, and it was not lit.
    DownButtonPressed,
}
