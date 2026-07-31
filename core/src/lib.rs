//! Pure, deterministic elevator simulation: world, physics, passengers,
//! stats, and challenges. No iced, no rhai — headless and testable on its
//! own.
//!
//! A [`World`] is minted from a [`challenge::Challenge`] and a seed, and
//! driven by anything implementing [`controller::Controller`] through the
//! fixed-step runner in [`headless`] — the sole owner of the substep and
//! event-dispatch loop (see [`world`] for the contract). The Rhai runtime
//! arrives in Phase 4 as just another controller.

pub mod challenge;
pub mod controller;
pub mod elevator;
pub mod event;
pub mod floor;
pub mod headless;
pub mod passenger;
pub mod rng;
pub mod stats;
pub mod world;

pub use world::World;
