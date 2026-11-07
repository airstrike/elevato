/// One car of the bank, as `update` sees it: a read-only snapshot in
/// `elevators`, rebuilt before every call. Changes go through the
/// `Command` constructors on `lib.rs`.
#[derive(Debug, Clone, PartialEq)]
pub struct Elevator {
    /// The current floor, rounded. Updated continuously while moving -
    /// it does not imply the car is stopped here.
    pub current_floor: usize,

    /// Capacity, in riders. Boarding is slot-count based.
    pub max_passenger_count: usize,

    /// Aboard weight / (capacity × 100). Riders weigh 55–100, so a
    /// slot-full car reads anywhere from ~0.55 to 1.0; `is_full` is
    /// the exact test.
    pub load_factor: f64,

    /// Whether every slot is taken.
    pub is_full: bool,

    /// Where the car is headed: `Some(Direction::Up)`,
    /// `Some(Direction::Down)`, or `None` when stopped.
    pub destination_direction: Option<Direction>,

    /// The queue, front first. A snapshot: clone it to edit, then
    /// write it back through `set_destination_queue`.
    pub destination_queue: Vec<usize>,

    /// Floors with lit in-car destination buttons, ascending. A rider
    /// presses ~1 s after boarding.
    pub pressed_floors: Vec<usize>,

    /// Floor boundaries crossed so far; a 0 → 3 trip counts 3. The
    /// quantity the move-limit challenges score.
    pub move_count: usize,

    /// Whether the car is in the 1 s door dwell that follows every
    /// floor arrival. Movement commands are ignored until it ends.
    pub is_busy: bool,

    /// Whether the car is under way toward a destination.
    pub is_moving: bool,

    /// Whether the car rests exactly on a floor. False after a
    /// mid-flight `stop`.
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

// What a car does arrives as `Message` variants - see lib.rs.
