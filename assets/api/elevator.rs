/// One car of the bank. Fields are read-only unless marked otherwise.
pub struct Elevator {
    /// The current floor, rounded. Updated continuously while moving —
    /// it does not imply the car is stopped here.
    pub current_floor: i64,

    /// Capacity, in riders. Boarding is slot-count based.
    pub max_passenger_count: i64,

    /// Aboard weight / (capacity × 100). Riders weigh 55–100, so a
    /// slot-full car reads anywhere from ~0.55 to 1.0; `is_full` is
    /// the exact test.
    pub load_factor: f64,

    /// Whether every slot is taken.
    pub is_full: bool,

    /// "up", "down", or "stopped".
    pub destination_direction: String,

    /// The queue, as a value copy: mutations are invisible until
    /// written back through `set_destination_queue`.
    pub destination_queue: Vec<f64>,

    /// Floors with lit in-car destination buttons, ascending. A rider
    /// presses ~1 s after boarding.
    pub pressed_floors: Vec<i64>,

    /// Floor boundaries crossed so far; a 0 → 3 trip counts 3. The
    /// quantity the move-limit challenges score.
    pub move_count: i64,

    /// Whether the car is in the 1 s door dwell that follows every
    /// floor arrival. Movement commands are ignored until it ends.
    pub is_busy: bool,

    /// Whether the car is under way toward a destination.
    pub is_moving: bool,

    /// Whether the car rests exactly on a floor. False after a
    /// mid-flight `stop()`.
    pub is_on_a_floor: bool,

    /// Read-write. Both lamps start on. A waiting rider boards only if
    /// the lamp matching their direction is lit; the engine never
    /// touches the lamps itself.
    pub going_up_indicator: bool,

    /// Read-write. See `going_up_indicator`.
    pub going_down_indicator: bool,
}

impl Elevator {
    /// Queues floor `n`, clamped to the building. Deduplicated against
    /// the adjacent queue entry only: 2, 2 is one stop; 2, 3, 2 visits
    /// 2 twice. Accepts ints and floats.
    pub fn go_to_floor(&self, n: i64);

    /// With `force`, the floor goes to the front of the queue instead
    /// of the back.
    pub fn go_to_floor(&self, n: i64, force: bool);

    /// Clears the queue and halts at the projected stopping point —
    /// generally between floors, doors shut. Intended for in-transit
    /// rescheduling; follow with `go_to_floor`.
    pub fn stop(&self);

    /// Re-examines the queue now: non-empty starts toward the front
    /// entry; empty (outside a dwell) fires `idle`. Queue edits made
    /// through `set_destination_queue` apply on the next check.
    pub fn check_destination_queue(&self);

    /// Replaces the queue, entries clamped to the building.
    ///
    ///     let queue = elevator.destination_queue;
    ///     queue.insert(0, 3);
    ///     elevator.set_destination_queue(queue);
    ///     elevator.check_destination_queue();
    pub fn set_destination_queue(&self, queue: Vec<f64>);

    /// Binds a handler to an `Event` by name, or to several via a
    /// space-separated string — the event's name is then prepended as
    /// the handler's first argument. Handler arity is adapted: extra
    /// arguments are dropped, missing ones padded. Unknown names are a
    /// bind-time error.
    pub fn on(&self, events: &str, handler: impl FnMut(...));
}

/// Dispatched by name: `elevator.on("idle", || …)`.
pub enum Event {
    /// `"idle"` — the queue was checked while empty. Fires for every
    /// elevator at challenge start, and ~1 s after the last
    /// destination completes. Carries nothing; capture the elevator.
    Idle,

    /// `"floor_button_pressed"` — a rider pressed an unlit in-car
    /// destination button.
    FloorButtonPressed { floor: i64 },

    /// `"passing_floor"` — about to pass `floor` without stopping,
    /// early enough that `go_to_floor(floor, true)` still makes the
    /// stop.
    PassingFloor { floor: i64, direction: String },

    /// `"stopped_at_floor"` — arrived and snapped, fired before exit
    /// and boarding: indicator changes here affect who boards.
    StoppedAtFloor { floor: i64 },
}
