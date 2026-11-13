//! The building as it stands, which a program's `update` and `view` borrow.

use elevato_core::World;

use crate::Elevator;

/// The building as it stands: its floors, the cars the program powered on, and
/// its riders.
///
/// `Clone` because `rawr run` hands a borrowed `Building` to the program's
/// interpreted code as a copy.
#[derive(Debug, Clone)]
pub struct Building {
    world: World,
}

impl Building {
    pub(crate) fn new(world: World) -> Self {
        Self { world }
    }

    /// How many floors the building has; the lobby is floor 0.
    pub fn floors(&self) -> usize {
        self.world.floors().len()
    }

    /// The car's nearest floor, rounded while it travels. Returns 0 for a car
    /// beyond the challenge's bank.
    pub fn floor_of(&self, elevator: Elevator) -> usize {
        self.world
            .elevators()
            .get(elevator.index())
            .map_or(0, |car| car.current_floor())
    }

    /// The whole simulated world, for drawing it.
    #[doc(hidden)]
    pub fn world(&self) -> &World {
        &self.world
    }

    pub(crate) fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}
