//! The message a program's `update` receives.

use crate::{Elevator, elevator, floor};

/// Messages from car and trip tasks and [`riders()`](crate::riders),
/// handled by the program's `update`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Message {
    /// Something about a car: it is idle, a trip is under way, or a rider
    /// inside pressed a button.
    Elevator(Elevator, elevator::Message),
    /// A rider on this floor called a car.
    Floor(usize, floor::Message),
}
