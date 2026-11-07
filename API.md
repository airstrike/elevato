# elevato scripting API

elevato programs are written in Rust and run by
[rawr](https://github.com/airstrike/rawr), a Rust interpreter the game
embeds. The API mirrors the original Elevator Saga's JS surface,
renamed to snake_case and reshaped into the Elm architecture: world
events arrive as messages, world state arrives as read-only snapshots,
and the program answers with commands. This document is the full
mapping; deviations from the original are marked **[deviation]** and
collected at the end.

Everything below is in scope in every program, the way Rust's own
prelude is: no `use` needed. The definitions themselves are Rust source
the game appends to your program before rawr checks it
([`script/prelude.rs`](script/prelude.rs)).

## Program shape

```rust
struct Model {
    // whatever state your program keeps between messages
}

fn new() -> Model {
    // called once, when the challenge starts
    Model {}
}

fn update(model: &mut Model, message: Message, elevators: &[Elevator], floors: &[Floor]) -> Vec<Command> {
    // called for every message; the returned commands are applied in
    // order, the moment `update` returns
    match message {
        Message::Idle(elevator) => vec![go_to_floor(elevator, 0), go_to_floor(elevator, 1)],
        _ => vec![],
    }
}
```

Both functions are **required**, and rawr checks the whole program
before it runs, as rustc would. `new` takes no parameters; its return
value is the model, of any type you define. `update` must have exactly
the signature above, over the type `new` returns - a mismatch refuses
to Apply, with the error under the editor. Name parameters you don't
use with a leading underscore (`_floors`), as in any Rust program.

`model` is your state: mutate it through the `&mut` and the changes
persist between messages. `elevators` and `floors` are snapshots of the
world as it stands at that message; index them with the indices
messages carry. A panic anywhere in your code - `panic!`, a failed
`assert!`, an out-of-bounds index, a `None.unwrap()` - aborts the frame
and pauses the game with the message and its line (the original's
"There is a problem with your code").

## Messages

| Original (JS) | elevato (Rust) | Fires when |
|---|---|---|
| `update(dt, …)` | `Message::Tick(dt)` | Once per frame, before that frame's physics; `dt` is the frame's simulated seconds (`f64`). |
| `elevator.on("idle")` | `Message::Idle(elevator)` | The destination queue is checked while empty and the elevator is not mid-dwell. Fired for every elevator at challenge start, and ~1 s after the last queued destination completes. |
| `elevator.on("floor_button_pressed")` | `Message::FloorButtonPressed(elevator, floor)` | A passenger pressed an unlit destination button (~1 s after boarding). |
| `elevator.on("passing_floor")` | `Message::PassingFloor(elevator, floor, direction)` | About to pass a floor it is not stopping at; `direction` is `Direction::Up` or `Direction::Down`. Still in time for `go_to_floor_first(elevator, floor)`. |
| `elevator.on("stopped_at_floor")` | `Message::StoppedAtFloor(elevator, floor)` | Physically arrived and snapped to a floor - before exit/boarding, so indicator changes here affect who boards. |
| `floor.on("up_button_pressed")` | `Message::UpButtonPressed(floor)` | The up call button went from unlit to lit. Re-fires when overflow passengers re-press after an arrival cleared it. |
| `floor.on("down_button_pressed")` | `Message::DownButtonPressed(floor)` | Same, for down. |

Every payload is a `usize` index (or the `f64` dt, or a `Direction`).
A `match` on `message` must cover every variant, as in Rust - end it
with `_ => vec![]` to ignore the rest.

## Commands

| Original (JS) | elevato (Rust) | Notes |
|---|---|---|
| `goToFloor(n)` | `go_to_floor(elevator, floor)` | Queues a destination, clamped to `[0, floor_count - 1]`. Duplicate-suppressed against the adjacent queue element only. |
| `goToFloor(n, true)` | `go_to_floor_first(elevator, floor)` | Forces the destination to the front of the queue ("go there first"). |
| `stop()` | `stop(elevator)` | Clears the queue and halts at the projected stop point - usually *between* floors. Advanced in-transit rescheduling only. |
| `goingUpIndicator(v)` | `set_going_up_indicator(elevator, on)` | Both indicators start on. Affects boarding and call-button clearing. |
| `goingDownIndicator(v)` | `set_going_down_indicator(elevator, on)` | |
| `destinationQueue = […]` | `set_destination_queue(elevator, queue)` | Replaces the queue (`Vec<usize>`). Entries are clamped to the building **[deviation]**. Follow with `check_destination_queue`. |
| `checkDestinationQueue()` | `check_destination_queue(elevator)` | Non-empty queue: start moving to its front. Empty queue (and not mid-dwell): fire `Message::Idle`. |

Each function returns a `Command`; `update` returns them as a
`Vec<Command>`, applied in order before the next message is delivered,
so a command cascade unfolds in exactly the order the original's
synchronous callbacks did. An elevator index the challenge does not
have is a runtime error naming the command; floors are clamped, never
refused.

## Elevator snapshot

`elevators[i]` is an `Elevator`:

| Original (JS) | elevato (Rust) | Notes |
|---|---|---|
| `currentFloor()` | `current_floor: usize` | Cached rounded floor. Does **not** imply the elevator is stopped. |
| `goingUpIndicator()` | `going_up_indicator: bool` | Change it with `set_going_up_indicator`. |
| `goingDownIndicator()` | `going_down_indicator: bool` | |
| `maxPassengerCount()` | `max_passenger_count: usize` | Capacity in passengers (slots). |
| `loadFactor()` | `load_factor: f64` | `sum(weights) / (capacity × 100)`; 0 = empty, 1 ≈ full. |
| `destinationDirection()` | `destination_direction: Option<Direction>` | `Some(Direction::Up)`, `Some(Direction::Down)`, or `None` when stopped. |
| `destinationQueue` | `destination_queue: Vec<usize>` | A snapshot - see below. |
| `getPressedFloors()` | `pressed_floors: Vec<usize>` | Lit in-elevator buttons, ascending. |
| - | `is_full: bool` | **[new]** `true` when every slot is taken. Capacity is slot-count; `load_factor` is weight-based and cannot answer this. |
| - | `move_count: usize` | **[new]** Floor boundaries this elevator has crossed (what challenges 6 and 7 score). |
| - | `is_busy: bool` | **[new]** `true` during the 1 s door dwell - the elevator cannot be commanded to move. |
| - | `is_moving: bool` | **[new]** `true` while under way toward a destination. |
| - | `is_on_a_floor: bool` | **[new]** `true` when resting exactly on a floor (false after a mid-flight `stop`). |

### The destination-queue idiom

Snapshots are read-only, so the original's `destinationQueue.splice(…)`
becomes clone → modify → command → apply:

```rust
let mut queue = elevators[e].destination_queue.clone();
queue.insert(0, 3);
vec![set_destination_queue(e, queue), check_destination_queue(e)]   // changes take effect now
```

## Floor snapshot

`floors[i]` is a `Floor`:

| Original (JS) | elevato (Rust) | Notes |
|---|---|---|
| `floorNum()` | `floor_num: usize` | |
| `level` | `level: usize` | Alias of the same value, as in the original. |
| `buttonStates.up == "activated"` | `up_pressed: bool` | `true` while the up call button is lit. |
| `buttonStates.down == "activated"` | `down_pressed: bool` | `true` while the down call button is lit. |

## Rust as rawr runs it

rawr supports a large part of Rust - structs, enums, traits, generics,
closures, iterators, `Vec`, `HashMap` and most of the std a game program
wants - and rejects the rest with an error that names the construct. Its
[Supported Rust](https://rawr.airstrike.fyi/docs/language) page lists
what works. In elevato specifically:

- **The prelude's names are taken.** Define your own `Model`, helper
  functions and types, but not a second `Message`, `Elevator` or
  `go_to_floor`.
- **One file, no modules.** A program is a single file of items; `mod`
  blocks are not supported there.
- **No I/O.** The engine grants no file, environment or process access.
  `println!` writes to the native app's stdout (nowhere, in the
  browser).

## Deviations from the original

1. The Elm architecture replaces callbacks: events arrive as `Message`
   variants in one `update`, and effects are returned commands instead
   of method calls on live handles. `update` receives every message,
   so the original's separate `update(dt, …)` is `Message::Tick(dt)`.
2. `set_destination_queue` replaces raw `destinationQueue` assignment,
   and clamps entries to valid floor range (the original clamped only
   `goToFloor`).
3. Floors and elevators are `usize` indices. The original's
   `Number()` coercion accepted fractional floors; Rust's types make
   every destination a whole floor.
4. Mistakes the original found at run time - an unknown event name, a
   wrong return value, a missing handler argument - are type errors,
   refused before the program runs.
5. The simulation is deterministic: seeded RNG and fixed-timestep
   playback (see the README) - same seed, same program, same result.
