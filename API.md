# elevato scripting API

elevato programs are written in [Rhai](https://rhai.rs). The API mirrors
the original Elevator Saga's JS surface, renamed to snake_case. This
document is the full mapping; deviations from the original are marked
**[deviation]** and collected at the end.

## Program shape

```rhai
fn init(elevators, floors) {
    // called once, when the challenge starts
}

fn update(dt, elevators, floors) {
    // called once per frame with the whole frame's dt, before that
    // frame's physics - optional
}
```

`fn init(elevators, floors)` is **required**; a program without it fails
to compile. `fn update(dt, elevators, floors)` is **optional**
**[deviation]** - the original required both. If an `update` exists with
the wrong parameter count, that is a compile error rather than a
silently ignored function.

`elevators` and `floors` are arrays of handles into the running world.
Any exception thrown from user code - in `init`, `update`, or any event
handler - aborts the frame and pauses the game with the error (the
original's "There is a problem with your code").

## Elevator handle

| Original (JS) | elevato (Rhai) | Notes |
|---|---|---|
| `goToFloor(n)` | `go_to_floor(n)` | Queues a destination. Accepts ints and floats; clamped to `[0, floor_count - 1]`. Duplicate-suppressed against the adjacent queue element only. |
| `goToFloor(n, true)` | `go_to_floor(n, true)` | Forces the destination to the front of the queue ("go there first"). |
| `stop()` | `stop()` | Clears the queue and halts at the projected stop point - usually *between* floors. Advanced in-transit rescheduling only. |
| `currentFloor()` | `current_floor` *(get)* | Cached rounded floor. Does **not** imply the elevator is stopped. |
| `goingUpIndicator()` / `(v)` | `going_up_indicator` *(get/set)* | Both indicators start on. Affects boarding and call-button clearing. Property assignment replaces the original's chainable setter. |
| `goingDownIndicator()` / `(v)` | `going_down_indicator` *(get/set)* | |
| `maxPassengerCount()` | `max_passenger_count` *(get)* | Capacity in passengers (slots). |
| `loadFactor()` | `load_factor` *(get)* | `sum(weights) / (capacity × 100)`; 0 = empty, 1 ≈ full. |
| `destinationDirection()` | `destination_direction` *(get)* | `"up"`, `"down"`, or `"stopped"`. |
| `destinationQueue` | `destination_queue` *(get)* | **Value copy** - mutating the returned array cannot affect the elevator; see below. |
| `destinationQueue = […]` | `set_destination_queue(arr)` | Replaces the queue (ints or floats). Entries are clamped to the building **[deviation]**. Follow with `check_destination_queue()`. |
| `checkDestinationQueue()` | `check_destination_queue()` | Non-empty queue: start moving to its front. Empty queue (and not mid-dwell): fire `idle`. |
| `getPressedFloors()` | `pressed_floors` *(get)* | Lit in-elevator buttons, ascending array of ints. |
| `on(events, handler)` | `on(events, handler)` | See [Events](#events). |
| - | `is_full` *(get)* | **[new]** `true` when every slot is taken. Capacity is slot-count; `load_factor` is weight-based and cannot answer this. |
| - | `move_count` *(get)* | **[new]** Floor boundaries this elevator has crossed (what challenges 6 and 7 score). |
| - | `is_busy` *(get)* | **[new]** `true` during the 1 s door dwell - the elevator cannot be commanded to move. |
| - | `is_moving` *(get)* | **[new]** `true` while under way toward a destination. |
| - | `is_on_a_floor` *(get)* | **[new]** `true` when resting exactly on a floor (false after a mid-flight `stop()`). |

### The destination-queue idiom

Rhai arrays are values: `elevator.destination_queue` returns a copy, so
the original's `destinationQueue.splice(…)` becomes read → modify →
write back → apply:

```rhai
let queue = elevator.destination_queue;
queue.insert(0, 3);
elevator.set_destination_queue(queue);
elevator.check_destination_queue();   // changes take effect now
```

## Floor handle

| Original (JS) | elevato (Rhai) | Notes |
|---|---|---|
| `floorNum()` | `floor_num` *(get)* or `floor_num()` | Both forms work; the method form keeps JS muscle memory intact. |
| `level` | `level` *(get)* | Raw property alias of the same value, as in the original. |
| `buttonStates.up == "activated"` | `up_pressed` *(get)* | `true` while the up call button is lit. |
| `buttonStates.down == "activated"` | `down_pressed` *(get)* | `true` while the down call button is lit. |
| `on(events, handler)` | `on(events, handler)` | See [Events](#events). |

## Events

Subscribe with `handle.on("event_name", handler)`. Several events can be
bound at once with a space-separated string -
`floor.on("up_button_pressed down_button_pressed", handler)` - in which
case the **event name is prepended as the handler's first argument**
(riot.js multi-event semantics, exactly as the original). Single-event
binds pass only the event's own arguments. Binding an unknown event name
is an error **[deviation]** - the original silently registered a
listener that never fired.

### Elevator events

| Event | Handler args | Fires when |
|---|---|---|
| `"idle"` | - | The destination queue is checked while empty and the elevator is not mid-dwell. Fired for every elevator at challenge start, and ~1 s after the last queued destination completes. Handlers close over their elevator - none is passed. |
| `"floor_button_pressed"` | `floor_num` | A passenger pressed an unlit destination button (~1 s after boarding). |
| `"passing_floor"` | `floor_num, direction` | About to pass a floor it is not stopping at; `direction` is `"up"` or `"down"`. Still in time for `go_to_floor(floor_num, true)`. |
| `"stopped_at_floor"` | `floor_num` | Physically arrived and snapped to a floor - before exit/boarding, so indicator changes here affect who boards. |

### Floor events

| Event | Handler args | Fires when |
|---|---|---|
| `"up_button_pressed"` | `floor` (the floor handle) | The up call button went from unlit to lit. Re-fires when overflow passengers re-press after an arrival cleared it. |
| `"down_button_pressed"` | `floor` (the floor handle) | Same, for down. |

### Arity adaptation

JS ignored extra handler arguments; Rhai normally errors on an arity
mismatch. elevato adapts instead: the dispatched argument list is
truncated (or padded with `()`) to what the handler declares, so a
zero-argument closure on an argument-carrying event just works:

```rhai
floor.on("up_button_pressed down_button_pressed", || {
    elevators[0].go_to_floor(floor.floor_num());   // args ignored, JS-style
});
```

## Rhai vs JS gotchas

- **Loop-variable capture**: a Rhai `for` loop shares **one** loop
  variable across iterations. Closures made in the loop body must
  capture a per-iteration shadow, or they will all see the last value:

  ```rhai
  for elevator in elevators {
      let elevator = elevator;          // fresh binding per iteration
      elevator.on("idle", || elevator.go_to_floor(0));
  }
  ```

- **Shared state**: variables captured by several closures are shared
  between them - declare your request lists in `init` and capture them
  in every handler, as the original's solutions did with outer-scope
  variables.
- **Top-level statements** run once, before `init`. Rhai functions
  cannot see top-level variables, so cross-handler state must live in
  captured variables.

## Deviations from the original

1. `fn update` is optional (the original required both members).
2. `set_destination_queue` replaces raw `destinationQueue` assignment,
   and clamps entries to valid floor range (the original clamped only
   `goToFloor`).
3. Unknown event names in `on(...)` error at bind time instead of
   silently never firing.
4. Handler arity is adapted (truncate/pad) instead of JS's implicit
   `undefined` extras.
5. The simulation is deterministic: seeded RNG and fixed-timestep
   playback (see the README) - same seed, same program, same result.
