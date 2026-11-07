//! A floor of the building, and what riders on it do.

/// A floor of the building, by its level: the lobby is level 0.
///
/// Programs make floors from integers, so every function that takes a
/// floor takes `impl Into<Floor>` and `elevator.go_to_floor(3)` reads as
/// it says. A negative level is the lobby, and a level past the top
/// floor is the top floor once a car is sent there, as in the original.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Floor(usize);

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

impl From<usize> for Floor {
    fn from(level: usize) -> Self {
        Self(level)
    }
}

impl From<i64> for Floor {
    fn from(level: i64) -> Self {
        Self(usize::try_from(level).unwrap_or(0))
    }
}

/// An integer literal with no other type to take is an `i32`, so this
/// is the impl `go_to_floor(3)` reaches.
impl From<i32> for Floor {
    fn from(level: i32) -> Self {
        Self(usize::try_from(level).unwrap_or(0))
    }
}

impl Floor {
    /// The level: 0 for the lobby, counting up.
    pub fn level(self) -> usize {
        self.0
    }

    /// The level as the building has it: clamped to its top floor.
    pub(crate) fn within(self, floor_count: usize) -> Self {
        Self(self.0.min(floor_count.saturating_sub(1)))
    }
}
