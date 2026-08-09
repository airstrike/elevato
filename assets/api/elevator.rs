/// One car of the bank. Command it with destinations, steer boarding
/// with the indicator lamps, and subscribe to its `Event`s with `on`.
///
/// All fields are read-only unless marked otherwise.
pub struct Elevator {
    /// The current floor, rounded — updated continuously while
    /// moving, so it does NOT mean the car is stopped here.
    pub current_floor: i64,

    /// Capacity, in riders. Boarding is slot-count based: when
    /// `is_full`, nobody else fits, whatever `load_factor` says.
    pub max_passenger_count: i64,

    /// Aboard weight / (capacity × 100). 0.0 is empty; 1.0 is
    /// roughly full — riders weigh 55–100, so a slot-full car can
    /// read as low as 0.55. For an exact answer use `is_full`.
    pub load_factor: f64,

    /// True when every slot is taken — the question `load_factor`
    /// cannot answer precisely.
    pub is_full: bool,

    /// "up", "down", or "stopped" — where the car is headed.
    pub destination_direction: String,

    /// The queue as an array — a VALUE COPY. Mutating it changes
    /// nothing until written back; see `set_destination_queue`.
    pub destination_queue: Vec<f64>,

    /// Floors whose in-car destination buttons are lit, ascending.
    /// A rider presses ~1 s after boarding (`floor_button_pressed`).
    pub pressed_floors: Vec<i64>,

    /// Floor boundaries this car has crossed — the currency of the
    /// move-limit challenges (6 and 7). A 0 → 3 trip costs 3.
    pub move_count: i64,

    /// True during the 1 s door dwell after arriving at a floor. A
    /// dwelling car ignores movement commands until doors close.
    pub is_busy: bool,

    /// True while under way toward a destination.
    pub is_moving: bool,

    /// True when resting exactly on a floor. False after a
    /// mid-flight `stop()` — see `stop` for why that matters.
    pub is_on_a_floor: bool,

    /// Whether the up lamp is lit. READ-WRITE; both lamps start on.
    /// A waiting rider boards only if the lamp matching their
    /// direction is lit, so clearing one at `stopped_at_floor` time
    /// filters who gets in: `elevator.going_up_indicator = false;`
    pub going_up_indicator: bool,

    /// Whether the down lamp is lit. READ-WRITE; see
    /// `going_up_indicator`.
    pub going_down_indicator: bool,
}

impl Elevator {
    /// Queues floor `n` as a destination, clamped to the building.
    ///
    /// Duplicates are suppressed only against the ADJACENT queue
    /// entry: enqueueing 2 twice in a row is one stop, but 2, 3, 2
    /// visits 2 twice. Ints and floats both accepted.
    pub fn go_to_floor(&self, n: i64);

    /// The same, but `force = true` puts the floor at the FRONT of
    /// the queue: go there first. The classic use is inside
    /// `passing_floor`, where braking distance still allows the stop.
    pub fn go_to_floor(&self, n: i64, force: bool);

    /// Clears the queue and halts at the projected stopping point —
    /// usually BETWEEN floors, doors shut, riders trapped. For
    /// advanced in-transit rescheduling; follow with `go_to_floor`.
    pub fn stop(&self);

    /// Re-examines the queue right now. Non-empty: start toward the
    /// front. Empty (and not mid-dwell): fire `idle`. Call after
    /// `set_destination_queue` for edits to take effect immediately.
    pub fn check_destination_queue(&self);

    /// Replaces the destination queue (entries clamped to the
    /// building). The read → modify → write-back idiom:
    ///
    ///     let queue = elevator.destination_queue;
    ///     queue.insert(0, 3);
    ///     elevator.set_destination_queue(queue);
    ///     elevator.check_destination_queue();
    pub fn set_destination_queue(&self, queue: Vec<f64>);

    /// Subscribes a handler to one `Event` by name — or several via
    /// a space-separated string, in which case the event's name is
    /// prepended as the handler's first argument.
    ///
    /// Extra handler arguments are dropped and missing ones padded,
    /// so a zero-argument closure works on any event. Unknown names
    /// error at bind time.
    pub fn on(&self, events: &str, handler: impl FnMut(...));
}

/// Everything an `Elevator` reports, by name:
/// `elevator.on("idle", || …)`.
pub enum Event {
    /// `"idle"` — the destination queue was checked while empty:
    /// this car has nothing to do. Fires for every elevator at
    /// challenge start, and ~1 s after the last destination
    /// completes. Handlers receive nothing — capture your elevator.
    Idle,

    /// `"floor_button_pressed"` — a rider pressed an unlit in-car
    /// destination button, about 1 s after boarding.
    FloorButtonPressed { floor: i64 },

    /// `"passing_floor"` — about to pass `floor` without stopping,
    /// with braking room to spare: `go_to_floor(floor, true)` still
    /// makes the stop. `direction` is "up" or "down".
    PassingFloor { floor: i64, direction: String },

    /// `"stopped_at_floor"` — physically arrived and snapped, fired
    /// BEFORE riders exit and board: indicator changes made here
    /// decide who gets in.
    StoppedAtFloor { floor: i64 },
}
