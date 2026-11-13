//! Pure, deterministic elevator simulation: world, physics, passengers,
//! stats, and challenges. No iced, no rawr - headless and testable on its
//! own.
//!
//! A [`World`] starts from a [`challenge::Challenge`] and a seed. Native
//! strategies implement [`controller::Controller`] and run through
//! [`headless`]. Player programs run through `elevato_api::simulation`.
//! Both drivers follow the substep and event-dispatch contract in [`world`].

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
