//! Elevator motion, one active trip, and door dwell.
//!
//! Motion uses y-down pixel coordinates: moving up means negative velocity.
//! With [`crate::floor::HEIGHT`] at 50.0, the physics constants match Elevator
//! Saga's pixel units.
//!
//! Each substep applies velocity before acceleration (explicit Euler), then
//! checks for arrival. Motion pauses while the doors are open.

use crate::event::{Direction, Event};
use crate::floor;

/// Acceleration, px/s² (2.1 floors/s²).
pub const ACCELERATION: f64 = floor::HEIGHT * 2.1;

/// Deceleration, px/s² (2.6 floors/s²).
pub const DECELERATION: f64 = floor::HEIGHT * 2.6;

/// Top speed, px/s (2.6 floors/s).
pub const MAXSPEED: f64 = floor::HEIGHT * 2.6;

/// Seconds the doors stay open after a trip or reopening. Motion pauses during
/// the dwell; a pending trip starts after it ends.
pub const DWELL: f64 = 1.0;

/// Float-compare tolerance used for destination matching and the on-a-floor
/// test (original `epsilonEquals`).
const EPSILON: f64 = 1e-8;

/// One elevator: physics state, its one destination, dwell timer, and
/// per-elevator bookkeeping. Minted by the world; commands go through
/// [`crate::World`] so emitted events land in the world's queue.
#[derive(Debug, Clone)]
pub struct Elevator {
    id: usize,
    floor_count: usize,
    y: f64,
    velocity_y: f64,
    destination_y: f64,
    is_moving: bool,
    /// The floor the current trip goes to; `None` once the car arrived or
    /// stopped. One trip at a time: a new one replaces it.
    destination: Option<f64>,
    /// A re-arrival at the floor the car stands on is due: a rider re-pressed a
    /// call the standing car can answer.
    reopen: bool,
    /// Seconds spent so far in the current dwell, if one is running.
    dwell_spent: Option<f64>,
    /// Cached rounded floor; [`Self::move_count`] increments when it changes.
    current_floor: i64,
    /// `trunc(future_floor_if_stopped)` from the previous state change, for
    /// passing-floor detection.
    previous_trunc_future_floor: i64,
    move_count: usize,
    going_up_indicator: bool,
    going_down_indicator: bool,
    pressed_floors: Vec<bool>,
    /// Passenger slots; an occupied slot holds its occupant's weight. Capacity
    /// is the slot count - weight never blocks boarding, it only feeds
    /// [`Self::load_factor`].
    slots: Vec<Option<u32>>,
}

impl Elevator {
    /// A new elevator standing at floor 0, both indicators on, no lit buttons,
    /// every slot free - the original's starting state.
    pub(crate) fn new(id: usize, floor_count: usize, capacity: usize) -> Self {
        let y = floor::y_of_level(0.0, floor_count);
        Self {
            id,
            floor_count,
            y,
            velocity_y: 0.0,
            destination_y: y,
            is_moving: false,
            destination: None,
            reopen: false,
            dwell_spent: None,
            current_floor: 0,
            previous_trunc_future_floor: 0,
            move_count: 0,
            going_up_indicator: true,
            going_down_indicator: true,
            pressed_floors: vec![false; floor_count],
            slots: vec![None; capacity],
        }
    }

    /// Pixel y position (y grows downward).
    pub fn y(&self) -> f64 {
        self.y
    }

    /// Velocity in px/s; positive means moving down the building.
    pub fn velocity_y(&self) -> f64 {
        self.velocity_y
    }

    /// Exact (fractional) floor level at the current position.
    pub fn exact_floor(&self) -> f64 {
        floor::level_of_y(self.y, self.floor_count)
    }

    /// Cached rounded floor number. Updated whenever the rounded position
    /// changes; does **not** imply the elevator is stopped.
    pub fn current_floor(&self) -> usize {
        self.current_floor.max(0) as usize
    }

    /// The floor level the current trip goes to, if the car has one.
    pub fn destination(&self) -> Option<f64> {
        self.destination
    }

    /// Direction of travel toward the current physical destination, or `None`
    /// when the destination equals the current position - the original's
    /// `destinationDirection()` returning `"stopped"`. In the y-down pixel
    /// space a destination at *smaller* y is *higher* up the building.
    pub fn destination_direction(&self) -> Option<Direction> {
        if self.destination_y == self.y {
            None
        } else if self.destination_y < self.y {
            Some(Direction::Up)
        } else {
            Some(Direction::Down)
        }
    }

    /// Floor boundaries crossed so far (a 0→3 trip costs 3 moves).
    pub fn move_count(&self) -> usize {
        self.move_count
    }

    /// Whether the doors are open. Motion pauses, but a command can set the
    /// trip to take after the dwell.
    pub fn is_busy(&self) -> bool {
        self.dwell_spent.is_some()
    }

    /// Whether the elevator is under way toward its physical destination.
    pub fn is_moving(&self) -> bool {
        self.is_moving
    }

    /// The going-up indicator. Starts on; only user code changes it.
    pub fn going_up_indicator(&self) -> bool {
        self.going_up_indicator
    }

    /// Decides whether riders going up board at the next arrival, whether that
    /// floor's up call clears, and whether an up call reopens the car while it
    /// stands on a floor.
    pub fn set_going_up_indicator(&mut self, on: bool) {
        self.going_up_indicator = on;
    }

    /// The going-down indicator. Starts on; only user code changes it.
    pub fn going_down_indicator(&self) -> bool {
        self.going_down_indicator
    }

    /// Decides whether riders going down board at the next arrival, whether
    /// that floor's down call clears, and whether a down call reopens the car
    /// while it stands on a floor.
    pub fn set_going_down_indicator(&mut self, on: bool) {
        self.going_down_indicator = on;
    }

    /// Passenger capacity, in slots (the original's `maxPassengerCount`).
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Whether every slot is occupied. Slot count is the only boarding limit -
    /// weight never blocks.
    pub fn is_full(&self) -> bool {
        self.slots.iter().all(Option::is_some)
    }

    /// Sum of rider weights over `capacity × 100`: 0 = empty, 1 ≈ full (inexact
    /// because weights vary over 55–100).
    pub fn load_factor(&self) -> f64 {
        let load: u32 = self.slots.iter().flatten().sum();
        f64::from(load) / (self.capacity() as f64 * 100.0)
    }

    /// Whether a passenger traveling `from → to` may board: going up needs the
    /// up indicator, going down the down indicator, the same floor is always
    /// suitable (original `isSuitableForTravelBetween`).
    pub fn is_suitable_for_travel_between(&self, from: usize, to: usize) -> bool {
        if from < to {
            self.going_up_indicator
        } else if from > to {
            self.going_down_indicator
        } else {
            true
        }
    }

    /// Whether the elevator sits exactly (within epsilon) on a floor.
    pub fn is_on_a_floor(&self) -> bool {
        let exact = self.exact_floor();
        epsilon_equals(exact, exact.round())
    }

    /// Floors whose in-elevator destination buttons are lit, ascending.
    pub fn pressed_floors(&self) -> Vec<usize> {
        self.pressed_floors
            .iter()
            .enumerate()
            .filter_map(|(level, &lit)| lit.then_some(level))
            .collect()
    }

    /// Starts a trip to `floor`, clamped to `[0, floor_count - 1]`, replacing
    /// the current one. Asking again for the floor the trip already goes to
    /// changes nothing. A dwelling car leaves when its doors close.
    pub(crate) fn go_to_floor(&mut self, floor: f64, events: &mut Vec<Event>) {
        let floor = floor.clamp(0.0, (self.floor_count - 1) as f64);
        if self
            .destination
            .is_some_and(|destination| epsilon_equals(destination, floor))
        {
            return;
        }
        self.destination = Some(floor);
        self.proceed(events);
    }

    /// Ends the current trip. If not busy, commands a halt at the projected
    /// stop point - generally *between* floors, so no arrival events and no
    /// dwell. During a dwell this only drops the trip.
    pub(crate) fn stop(&mut self) {
        self.destination = None;
        self.reopen = false;
        if !self.is_busy() {
            let target = self.exact_future_floor_if_stopped();
            self.move_to_level(target);
        }
    }

    /// Asks the standing car for a re-arrival at its floor, ahead of its trip:
    /// the original's forced `goToFloor` of the current floor.
    pub(crate) fn reopen(&mut self, events: &mut Vec<Event>) {
        self.reopen = true;
        self.proceed(events);
    }

    /// If not busy: a due re-arrival, else the trip, starts movement; with
    /// neither, the car emits [`Event::Idle`].
    fn proceed(&mut self, events: &mut Vec<Event>) {
        if self.is_busy() {
            return;
        }
        if self.reopen {
            self.move_to_level(self.current_floor() as f64);
            return;
        }
        match self.destination {
            Some(level) => self.move_to_level(level),
            None => events.push(Event::Idle { elevator: self.id }),
        }
    }

    /// Lights the destination button for `floor`, emitting
    /// [`Event::FloorButtonPressed`] only on the unlit → lit transition.
    /// Arrival clears the button.
    pub(crate) fn press_floor_button(&mut self, floor: usize, events: &mut Vec<Event>) {
        let lit = self
            .pressed_floors
            .get_mut(floor)
            .expect("invariant: floor level within the building");
        if !*lit {
            *lit = true;
            events.push(Event::FloorButtonPressed {
                elevator: self.id,
                floor,
            });
        }
    }

    /// Occupies a free slot for a boarder of the given weight, probing linearly
    /// (with wraparound) from `offset` - the original's `userEntering`, whose
    /// random starting offset the world draws. Returns the occupied slot index,
    /// or `None` when full (the boarder stays behind and re-presses the floor
    /// call button).
    pub(crate) fn enter_slot(&mut self, weight: u32, offset: usize) -> Option<usize> {
        let capacity = self.slots.len();
        for probe in 0..capacity {
            let slot = (offset + probe) % capacity;
            if self.slots[slot].is_none() {
                self.slots[slot] = Some(weight);
                return Some(slot);
            }
        }
        None
    }

    /// Frees the slot an exiting rider occupied - before boarding runs, so
    /// leavers make room for boarders within the same arrival.
    pub(crate) fn free_slot(&mut self, slot: usize) {
        self.slots
            .get_mut(slot)
            .expect("invariant: slot index within capacity")
            .take()
            .expect("invariant: freed slot was occupied");
    }

    /// Advances the dwell timer (the original's movable task update, run each
    /// substep *before* physics). Completion - strictly past [`DWELL`] -
    /// proceeds, so the next trip or an [`Event::Idle`] follows in this same
    /// substep.
    pub(crate) fn update_tasks(&mut self, dt: f64, events: &mut Vec<Event>) {
        if let Some(spent) = &mut self.dwell_spent {
            *spent += dt;
            if *spent > DWELL {
                self.dwell_spent = None;
                self.proceed(events);
            }
        }
    }

    /// One physics substep, skipped during dwell. An arrival on a floor is
    /// staged in `arrivals` so the driver can dispatch events before processing
    /// riders.
    pub(crate) fn update_movement(
        &mut self,
        dt: f64,
        events: &mut Vec<Event>,
        arrivals: &mut Vec<usize>,
    ) {
        if self.is_busy() {
            return;
        }

        self.velocity_y = self.velocity_y.clamp(-MAXSPEED, MAXSPEED);
        self.move_by(self.velocity_y * dt, events);

        let destination_diff = self.destination_y - self.y;
        let direction_sign = sign(destination_diff);
        let velocity_sign = sign(self.velocity_y);

        if velocity_sign == direction_sign {
            let stopping = distance_to_achieve_speed(self.velocity_y, 0.0, DECELERATION);
            if stopping * 1.05 < -destination_diff.abs() {
                // Braking distance (+5%) covers the rest: brake at exactly the
                // needed deceleration, capped at DECELERATION * 1.1 for
                // overshoot recovery.
                let required =
                    acceleration_to_achieve_change_distance(self.velocity_y, 0.0, destination_diff);
                let deceleration = (DECELERATION * 1.1).min(required.abs());
                self.velocity_y -= direction_sign * deceleration * dt;
            } else {
                // Soft proportional ramp when very close to the target.
                let acceleration = (destination_diff * 5.0).abs().min(ACCELERATION);
                self.velocity_y += direction_sign * acceleration * dt;
            }
        } else if velocity_sign == 0.0 {
            let acceleration = (destination_diff * 5.0).abs().min(ACCELERATION);
            self.velocity_y += direction_sign * acceleration * dt;
        } else {
            // Moving away from the destination: decelerate at full
            // DECELERATION, never flipping direction within one step.
            self.velocity_y -= velocity_sign * DECELERATION * dt;
            if sign(self.velocity_y) != velocity_sign {
                self.velocity_y = 0.0;
            }
        }

        if self.is_moving && destination_diff.abs() < 0.5 && self.velocity_y.abs() < 3.0 {
            self.snap_and_arrive(events, arrivals);
        }
    }

    /// Starts physical movement toward a (possibly fractional) floor level.
    fn move_to_level(&mut self, level: f64) {
        self.is_moving = true;
        self.destination_y = floor::y_of_level(level, self.floor_count);
    }

    fn move_by(&mut self, delta: f64, events: &mut Vec<Event>) {
        self.y += delta;
        self.handle_new_state(events);
    }

    /// Counts rounded floor changes as moves. Passing-floor events use the
    /// braking projection so handlers can still command a stop there.
    fn handle_new_state(&mut self, events: &mut Vec<Event>) {
        let rounded = self.exact_floor().round() as i64;
        if rounded != self.current_floor {
            self.move_count += 1;
            self.current_floor = rounded;
        }

        let future = self.exact_future_floor_if_stopped();
        let trunc_future = future.trunc() as i64;
        if trunc_future != self.previous_trunc_future_floor {
            let passed = future.round();
            let destination = floor::level_of_y(self.destination_y, self.floor_count);
            if !epsilon_equals(passed, destination) && self.is_approaching(passed) {
                // Casting through the bounds check keeps a transient
                // out-of-building projection from ever surfacing.
                let floor = passed as i64;
                if let Ok(floor) = usize::try_from(floor)
                    && floor < self.floor_count
                {
                    let direction = if self.velocity_y > 0.0 {
                        Direction::Down
                    } else {
                        Direction::Up
                    };
                    events.push(Event::PassingFloor {
                        elevator: self.id,
                        floor,
                        direction,
                    });
                }
            }
        }
        self.previous_trunc_future_floor = trunc_future;
    }

    /// Ends a completed trip and starts its dwell synchronously, before
    /// [`Event::StoppedAtFloor`], as the original's `stopped` fires ahead of
    /// `stopped_at_floor`. Riders wait for [`crate::World::process_arrivals`]
    /// so handlers can set indicators.
    fn snap_and_arrive(&mut self, events: &mut Vec<Event>, arrivals: &mut Vec<usize>) {
        let delta = self.destination_y - self.y;
        self.move_by(delta, events);
        self.velocity_y = 0.0;
        self.is_moving = false;

        let stopped_at = self.exact_floor();
        if self.reopen {
            self.reopen = false;
            self.dwell_spent = Some(0.0);
        } else if self
            .destination
            .is_some_and(|destination| epsilon_equals(destination, stopped_at))
        {
            self.destination = None;
            self.dwell_spent = Some(0.0);
        } else {
            // A halt after `stop`: the car has nothing left to do.
            self.proceed(events);
        }

        if self.is_on_a_floor() {
            let floor = self.current_floor();
            self.pressed_floors[floor] = false;
            events.push(Event::StoppedAtFloor {
                elevator: self.id,
                floor,
            });
            arrivals.push(self.id);
        }
    }

    /// Where the elevator would come to rest (as an exact floor level) if it
    /// braked at full [`DECELERATION`] starting now.
    fn exact_future_floor_if_stopped(&self) -> f64 {
        let stopping = distance_to_achieve_speed(self.velocity_y, 0.0, DECELERATION);
        // `stopping` is negative, so this projects *forward* along travel.
        floor::level_of_y(self.y - sign(self.velocity_y) * stopping, self.floor_count)
    }

    fn is_approaching(&self, level: f64) -> bool {
        let to_floor = floor::y_of_level(level, self.floor_count) - self.y;
        self.velocity_y != 0.0 && sign(self.velocity_y) == sign(to_floor)
    }
}

/// Distance needed to change from `current` to `target` speed at the given
/// acceleration (`v² = u² + 2ad` solved for `d`). Braking toward zero yields a
/// *negative* distance - callers rely on that sign.
fn distance_to_achieve_speed(current: f64, target: f64, acceleration: f64) -> f64 {
    (target.powi(2) - current.powi(2)) / (2.0 * acceleration)
}

/// Acceleration needed to change from `current` to `target` speed over a signed
/// `distance` (`v² = u² + 2ad` solved for `a`).
fn acceleration_to_achieve_change_distance(current: f64, target: f64, distance: f64) -> f64 {
    0.5 * ((target.powi(2) - current.powi(2)) / distance)
}

/// `Math.sign` semantics: zero maps to zero. (`f64::signum` maps ±0.0 to ±1.0,
/// which would corrupt the standing-still branch.)
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn epsilon_equals(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braking_distance_at_top_speed_is_sixty_five_pixels_and_negative() {
        assert_eq!(
            distance_to_achieve_speed(MAXSPEED, 0.0, DECELERATION),
            -65.0
        );
    }

    #[test]
    fn exact_stop_deceleration_solves_the_kinematic_equation() {
        // Stopping from top speed over exactly the braking distance needs
        // exactly full deceleration; the sign opposes the (negative, upward)
        // remaining distance.
        let required = acceleration_to_achieve_change_distance(MAXSPEED, 0.0, -65.0);
        assert_eq!(required, DECELERATION);
        // Twice the distance needs half the deceleration.
        let relaxed = acceleration_to_achieve_change_distance(MAXSPEED, 0.0, -130.0);
        assert_eq!(relaxed, DECELERATION / 2.0);
    }

    #[test]
    fn sign_maps_zero_to_zero_unlike_signum() {
        assert_eq!(sign(0.0), 0.0);
        assert_eq!(sign(-0.0), 0.0);
        assert_eq!(sign(3.5), 1.0);
        assert_eq!(sign(-3.5), -1.0);
    }

    #[test]
    fn slot_probing_wraps_around_and_reports_full() {
        let mut elevator = Elevator::new(0, 4, 3);
        // Offset 2, then 2 again: the probe wraps to slot 0.
        assert_eq!(elevator.enter_slot(60, 2), Some(2));
        assert_eq!(elevator.enter_slot(70, 2), Some(0));
        assert_eq!(elevator.enter_slot(80, 1), Some(1));
        assert!(elevator.is_full());
        assert_eq!(elevator.enter_slot(90, 0), None);
        // Weights 60 + 70 + 80 over 3 × 100.
        assert_eq!(elevator.load_factor(), 0.7);
        elevator.free_slot(1);
        assert!(!elevator.is_full());
        assert_eq!(elevator.enter_slot(90, 0), Some(1));
    }

    #[test]
    fn future_floor_projection_extends_forward_along_travel() {
        let mut elevator = Elevator::new(0, 4, 4);
        // Standing still: the projection is the current position.
        assert_eq!(elevator.exact_future_floor_if_stopped(), 0.0);
        // Moving up (negative velocity) at top speed from floor 0: the stop
        // point is 65 px = 1.3 floors above.
        elevator.velocity_y = -MAXSPEED;
        assert!((elevator.exact_future_floor_if_stopped() - 1.3).abs() < 1e-12);
        // Moving down from the top floor mirrors it.
        elevator.y = floor::y_of_level(3.0, 4);
        elevator.velocity_y = MAXSPEED;
        assert!((elevator.exact_future_floor_if_stopped() - 1.7).abs() < 1e-12);
    }
}
