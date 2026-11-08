/// A floor, by level: the lobby is 0. Your program gets the
/// building's floors from `new`, and makes floors from integers -
/// every function that takes one takes `impl Into<Floor>`, so
/// `elevator.go_to_floor(3)` works. A negative level is the lobby.
/// `Copy`, comparable, ordered by level, hashable.
pub struct Floor;

impl From<usize> for Floor;
impl From<i64> for Floor;
impl From<i32> for Floor;

impl Floor {
    /// The level: 0 for the lobby, counting up.
    pub fn level(self) -> usize;
}

/// What riders on a floor do. Arrives through `riders()`.
pub enum Message {
    /// A rider going up pressed the up button, and it was not lit. A
    /// rider a full car left behind presses it again.
    UpButtonPressed,

    /// A rider going down pressed the down button, and it was not lit.
    DownButtonPressed,
}
