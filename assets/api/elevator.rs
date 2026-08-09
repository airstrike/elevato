/// One car of the bank, as `update` sees it: a read-only snapshot in
/// `elevators`, rebuilt before every call. Changes go through the
/// `Command` constructors on `lib.rs`.
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

    /// The queue, front first. A value copy: edits are invisible until
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

    /// The up lamp's state; change it with `set_going_up_indicator`.
    /// Both lamps start on. A waiting rider boards only if the lamp
    /// matching their direction is lit; the engine never touches the
    /// lamps itself.
    pub going_up_indicator: bool,

    /// The down lamp's state; change it with
    /// `set_going_down_indicator`. See `going_up_indicator`.
    pub going_down_indicator: bool,
}

/// The elevator half of the message catalog: `message.kind` holds the
/// variant name in snake_case, and the fields arrive flattened into
/// the message map alongside `kind` and `elevator` (the car's index
/// into `elevators`).
pub enum Event {
    /// `"idle"` — the queue was checked while empty. Fires for every
    /// elevator at challenge start, and ~1 s after the last
    /// destination completes. The message carries only `elevator`.
    Idle,

    /// `"floor_button_pressed"` — a rider pressed an unlit in-car
    /// destination button.
    FloorButtonPressed { floor: i64 },

    /// `"passing_floor"` — about to pass `floor` without stopping,
    /// early enough that `go_to_floor(elevator, floor, true)` still
    /// makes the stop.
    PassingFloor { floor: i64, direction: String },

    /// `"stopped_at_floor"` — arrived and snapped, fired before exit
    /// and boarding: indicator commands returned here affect who
    /// boards.
    StoppedAtFloor { floor: i64 },
}
