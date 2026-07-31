//! Pure, deterministic elevator simulation: world, physics, passengers,
//! and challenges. No iced, no rhai — headless and testable on its own.
//!
//! Phase 2 scope: elevator kinematics, movement, door dwell, typed event
//! emission, and the seeded RNG. Passengers, stats, and challenges arrive
//! in Phase 3. See [`world`] for the driver contract (substep loop, event
//! drains, staged arrivals).

pub mod elevator;
pub mod event;
pub mod floor;
pub mod rng;
pub mod world;

pub use world::World;
