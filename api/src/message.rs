//! The message a program's `update` receives.

use crate::{Elevator, elevator, floor};

/// What happens in the building: news from a car, or a rider calling
/// one from a floor. Every command's task and [`riders()`](crate::riders)
/// produce it, and a program's `update` handles it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Message {
    /// Something about a car: it is idle, a trip is under way, or a rider
    /// inside pressed a button.
    Elevator(Elevator, elevator::Message),
    /// A rider on this floor called a car.
    Floor(usize, floor::Message),
}
