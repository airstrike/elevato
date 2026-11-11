//! The building `update` and `view` read.

/// The building as it stands: its floors, the elevators you powered on,
/// and its riders.
pub struct Building;

impl Building {
    /// How many floors the building has; the lobby is floor 0.
    pub fn floors(&self) -> usize;

    /// The floor the car is at, or nearest to while it travels.
    pub fn floor_of(&self, elevator: Elevator) -> usize;
}
