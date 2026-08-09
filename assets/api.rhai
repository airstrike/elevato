//! ─────────────────────────────────────────────────────────────────
//! elevato scripting API
//!
//! This reference is code: cmd+click (ctrl+click on Windows/Linux)
//! any name — here or in your own program — to jump to its
//! definition. The API mirrors the original Elevator Saga, renamed
//! to snake_case.
//! ─────────────────────────────────────────────────────────────────

// ───────────────────────── program shape ─────────────────────────

/// Called once, when the challenge starts. `elevators` and `floors`
/// are arrays of live handles into the world. Required — a program
/// without `init` does not compile.
fn init(elevators, floors);

/// Called once per frame with the whole frame's `dt`, before that
/// frame's physics. Optional. The heavy-handed alternative to
/// events: replan everything, every frame.
fn update(dt, elevators, floors);

/// Simulated seconds since the previous `update` call. Scales with
/// the timescale: at 2× each frame hands you twice the seconds.
let dt;

/// Every elevator in the bank, in x order. See `Elevator`.
let elevators;

/// Every floor of the building, ground (0) first. See `Floor`.
let floors;

// Any exception thrown from your code — in `init`, `update`, or a
// handler — pauses the game and shows the error under the editor.

// ─────────────────────────── Elevator ────────────────────────────

/// One car of the bank. Command it with destinations, steer who
/// boards with the indicators, and listen to its events with `on`.
type Elevator;

/// Queues floor `n` as a destination. Accepts ints and floats;
/// clamps to the building. Duplicates are suppressed only against
/// the adjacent queue entry — enqueueing 2 twice in a row is one
/// stop, but 2,3,2 visits 2 twice.
elevator.go_to_floor(n);

/// The same, but `go_to_floor(n, true)` puts the floor at the
/// *front* of the queue: go there first, before anything queued.
/// The classic use is inside `passing_floor`, where there is still
/// braking distance to make the stop.
elevator.go_to_floor(n, force);

/// Clears the queue and halts at the projected stopping point —
/// usually *between* floors, doors shut, riders trapped mid-shaft.
/// For advanced in-transit rescheduling; follow with `go_to_floor`.
elevator.stop();

/// Re-examines the queue right now. Non-empty: start toward the
/// front entry. Empty (and not mid-dwell): fire `idle`. Call after
/// `set_destination_queue` for edits to take effect immediately.
elevator.check_destination_queue();

/// The queue as an array — a VALUE COPY. Mutating it does nothing
/// until written back:
///
///     let queue = elevator.destination_queue;
///     queue.insert(0, 3);
///     elevator.set_destination_queue(queue);
///     elevator.check_destination_queue();
elevator.destination_queue;

/// Replaces the destination queue (ints or floats, clamped to the
/// building). Nothing moves until the next queue check — call
/// `check_destination_queue` to apply immediately.
elevator.set_destination_queue(queue);

/// The current floor, rounded. Updated continuously while moving —
/// it does NOT mean the elevator is stopped there.
elevator.current_floor;

/// Capacity, in passengers. Boarding is slot-count based: when
/// `is_full` no one else fits, whatever `load_factor` says.
elevator.max_passenger_count;

/// Aboard weight / (capacity × 100). 0 is empty; 1 is roughly
/// full — riders weigh 55–100, so a slot-full car can read as low
/// as 0.55. For an exact answer use `is_full`.
elevator.load_factor;

/// True when every slot is taken. The question `load_factor`
/// cannot answer precisely.
elevator.is_full;

/// "up", "down", or "stopped" — where the car is headed right now.
elevator.destination_direction;

/// Floors whose in-car destination buttons are lit, ascending.
/// A rider presses ~1 s after boarding (see `floor_button_pressed`).
elevator.pressed_floors;

/// Floor boundaries this car has crossed — the currency of the
/// move-limit challenges (6 and 7). A trip from 0 to 3 costs 3.
elevator.move_count;

/// True during the 1 s door dwell after arriving at a floor. A
/// dwelling car ignores movement commands until the doors close.
elevator.is_busy;

/// True while under way toward a destination.
elevator.is_moving;

/// True when resting exactly on a floor. False after a mid-flight
/// `stop()` — see `stop` for why that strands riders.
elevator.is_on_a_floor;

/// Whether the up lamp is lit. Both lamps start ON. Steering: a
/// waiting rider boards only if the lamp matching their direction
/// is lit, so clearing one lamp at `stopped_at_floor` time filters
/// who gets in. Assignable: `elevator.going_up_indicator = false;`
elevator.going_up_indicator;

/// Whether the down lamp is lit. See `going_up_indicator`.
elevator.going_down_indicator;

/// Subscribes a handler to an event, by name:
///
///     elevator.on("idle", || elevator.go_to_floor(0));
///     floor.on("up_button_pressed", |floor| { ... });
///
/// Several events can share one handler via a space-separated
/// string; the event name is then prepended as the handler's first
/// argument. Extra handler arguments are dropped, missing ones are
/// padded — a zero-argument closure works on any event. Unknown
/// event names error at bind time.
///
/// Elevator events: idle, floor_button_pressed, passing_floor,
/// stopped_at_floor. Floor events: up_button_pressed,
/// down_button_pressed.
on(events, handler);

// ───────────────────────────── Floor ─────────────────────────────

/// One floor of the building: its number and call-button state,
/// plus the two call-button events via `on`.
type Floor;

/// This floor's number, ground = 0. Also callable as a method —
/// `floor.floor_num()` — for ported JS muscle memory. `level` is
/// the same value under its original raw-property name.
floor.floor_num;

/// Alias of `floor_num`, as in the original.
floor.level;

/// True while the up call button is lit. Polling these two from
/// `update` instead of listening to events is the "Twentyliner"
/// strategy family.
floor.up_pressed;

/// True while the down call button is lit.
floor.down_pressed;

// ──────────────────────── elevator events ────────────────────────

/// The destination queue was checked while empty: this car has
/// nothing to do. Fires for every elevator at challenge start, and
/// ~1 s after the last queued destination completes. Handlers
/// receive nothing — close over your elevator.
event idle;

/// A rider pressed an unlit in-car destination button (about 1 s
/// after boarding). Handler receives the floor number.
event floor_button_pressed;

/// About to pass `floor_num` without stopping; `direction` is "up"
/// or "down". Fired with braking room to spare, so
/// `go_to_floor(floor_num, true)` still makes the stop.
event passing_floor;

/// Physically arrived and snapped to a floor — fired BEFORE riders
/// exit and board, so indicator changes made here decide who gets
/// in. Handler receives the floor number.
event stopped_at_floor;

// ────────────────────────── floor events ─────────────────────────

/// The floor's up call button went from unlit to lit. Handler
/// receives the floor handle. Re-fires when riders who could not
/// fit press again after an arrival cleared the button.
event up_button_pressed;

/// Same, for the down call button.
event down_button_pressed;

// ─────────────────────── Rhai vs JS gotchas ──────────────────────
//
// Loop captures: a Rhai `for` shares ONE loop variable across
// iterations. Closures made in the body must capture a shadow:
//
//     for elevator in elevators {
//         let elevator = elevator;      // fresh binding, each turn
//         elevator.on("idle", || elevator.go_to_floor(0));
//     }
//
// Shared state: variables captured by several closures are shared
// between them — declare request lists in `init` and capture them
// everywhere, like the original's outer-scope variables.
//
// Top-level statements run once, before `init`, and functions
// cannot see top-level variables: cross-handler state lives in
// captures.
//
// Determinism: seeded RNG + fixed-timestep playback. Same seed,
// same program, same result — the Seed readout in the stats bar
// names the run.
