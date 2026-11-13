//! The building `update` and `view` read.

/// The building as it stands: its floors, the elevators you powered on,
/// and its riders.
pub struct Building;

impl Building {
    /// How many floors the building has; the lobby is floor 0.
    pub fn floors(&self) -> usize;

    /// The car's nearest floor, rounded while it travels. Returns 0 for
    /// a car beyond the challenge's bank.
    pub fn floor_of(&self, elevator: Elevator) -> usize;
}
