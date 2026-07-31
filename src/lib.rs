//! elevato — Elevator Saga in Rust: program a bank of elevators in Rhai
//! and watch the simulation clear (or fail) the challenges.

pub mod action;
pub mod app;
pub mod editor;
pub mod highlight;
pub mod playback;
pub mod sim;
pub mod storage;
pub mod theme;

pub use elevato_core as core;
