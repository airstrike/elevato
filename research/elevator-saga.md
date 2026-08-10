# Elevator Saga - Complete Reference for Reimplementation

Source of truth: https://github.com/magwo/elevatorsaga (commit `e0c55bf`, 2021-01-04, the
version deployed at https://play.elevatorsaga.com/). All file citations below refer to that
repo. This document is intended to be sufficient to reimplement the game faithfully (sim
engine + scripting) without reading the JS again.

---

## 1. Game description

Elevator Saga ("the elevator programming game", by Magnus Wolffelt) is a browser game in
which the player writes a JavaScript program that controls a bank of elevators serving
randomly spawning passengers. The goal of each challenge is to transport people
efficiently enough to meet a success condition (throughput, wait-time, or move-count
based).

### Player loop

1. Edit code in a CodeMirror editor at the bottom of the page (`app.js`).
2. Press **Apply** - this re-`eval`s the code, tears down the current world, creates a
   fresh one for the current challenge, and auto-starts it.
3. Watch the simulation run; a stats bar updates live.
4. The challenge ends with "Success!" (link to next challenge) or "Challenge failed -
   maybe your program needs an improvement?".

### UI elements

- **World view**: floors drawn as horizontal strips (each 50 px tall), each floor showing
  its number and up/down call-button indicators. Elevators are boxes (width = 10 px per
  passenger slot) with an up indicator, a floor-position indicator, a down indicator, and
  a row of lit "pressed floor" buttons. Passengers are FontAwesome person icons
  (male/female/child) that walk in, wait, ride, and walk off (`index.html` templates,
  `presenters.js`).
- **Challenge header**: "Challenge #N: <description>", a Start/Pause/Restart button, and
  time-scale controls: `−` / `+` buttons that divide/multiply the time scale by 1.618
  (rounded to integer, capped below 40; persisted in localStorage key
  `elevatorTimeScale`, default 2.0) (`presenters.js`, `app.js`).
- **Stats bar** (`presenters.js`): **Transported**, **Elapsed time**, **Transported/s**,
  **Avg waiting time**, **Max waiting time**, **Moves**.
- **Editor controls**: **Apply**, **Save**, **Reset** (restore default implementation;
  backs current code up first), **Undo reset**. Code autosaves (debounced 1 s) to
  localStorage key `elevatorCrushCode_v5`; reset backup goes to `develevateBackupCode`
  (`app.js`).
- **URL hash params** (`app.js` `riot.route`): `#challenge=N` (1-based; invalid → 1),
  `autostart`, `timescale=X`, `devtest` (loads the dev demo solution), `fullscreen`.

### Default starter code (`index.html`, `#default-elev-implementation`)

```js
{
    init: function(elevators, floors) {
        var elevator = elevators[0]; // Let's use the first elevator

        // Whenever the elevator is idle (has no more queued destinations) ...
        elevator.on("idle", function() {
            // let's go to all the floors (or did we forget one?)
            elevator.goToFloor(0);
            elevator.goToFloor(1);
        });
    },
    update: function(dt, elevators, floors) {
        // We normally don't need to do anything here
    }
}
```

---

## 2. Full scripting API

### User-code shape (`base.js` `getCodeObjFromCode`)

The editor content is `eval`'d (wrapped in parens if it starts with `{` and ends with
`}`). It must evaluate to an object with **both**:

```js
{
    init:   function(elevators, floors) { ... },  // called once, when the challenge starts (first unpaused frame)
    update: function(dt, elevators, floors) { ... } // called once per animation frame; dt = game-seconds since last call
}
```

Missing `init` or `update` throws "Code must contain an init/update function".
`elevators` is an array of *elevator interface* objects (`interfaces.js` facades over the
physical `Elevator`), `floors` is the array of floor objects. Any exception thrown from
user code (in `init`, `update`, or any event handler) pauses the game and shows
"There is a problem with your code: …".

Event subscription uses riot.js observable syntax on both elevators and floors:
`obj.on("event_name", handler)`, `obj.off(...)`, and multiple events can be bound at once
with a space-separated string: `floor.on("up_button_pressed down_button_pressed", fn)`
(in that case riot passes the event name as the first handler argument).

### Elevator interface (`interfaces.js`)

Methods (all functions unless noted):

| Member | Semantics (exact) |
|---|---|
| `goToFloor(floorNum, [forceNow])` | Queue a destination. `floorNum` is coerced with `Number()` and clamped to `[0, floorCount-1]`. **Duplicate suppression**: if the queue is non-empty and the *adjacent* element (queue **front** if `forceNow`, queue **back** otherwise) equals `floorNum` (epsilon compare), the call is a no-op. Otherwise `forceNow` `unshift`s (go there before anything else), default `push`es. Then `checkDestinationQueue()` runs automatically. |
| `stop()` | Sets `destinationQueue = []`. If the elevator is not busy (i.e., not in its 1 s door dwell), commands the physical elevator to `goToFloor(getExactFutureFloorIfStopped())` - i.e., decelerate and halt at the nearest reachable position, which is **usually not a floor**, so passengers will not get out. Docs: intended only for advanced in-transit rescheduling. |
| `currentFloor()` | Returns the elevator's cached *rounded* floor number (updated whenever the rounded position changes). Does **not** imply the elevator is stopped. |
| `goingUpIndicator([bool])` / `goingDownIndicator([bool])` | Getter/setter (setter returns the interface for chaining; truthiness coerced to bool). Both start `true`. Affects passenger boarding and floor-button clearing (section 3). |
| `maxPassengerCount()` | The elevator's capacity in passengers (slot count). |
| `loadFactor()` | `sum(passenger weights) / (maxPassengerCount * 100)`. 0 = empty, 1 ≈ full. Not exact because weights vary (55–100). |
| `destinationDirection()` | `"stopped"` if physical `destinationY === y`, else `"up"`/`"down"` toward the current physical destination. |
| `destinationQueue` | **Plain array property** of queued floor numbers. May be read, mutated, reordered, or emptied by user code; call `checkDestinationQueue()` afterwards for changes to take effect immediately. |
| `checkDestinationQueue()` | If the physical elevator is not busy: if the queue is non-empty, start moving to `destinationQueue[0]`; **if empty, trigger the `idle` event** (synchronously). |
| `getPressedFloors()` | Array of floor numbers whose in-elevator buttons are currently lit (ascending order). |
| `getFirstPressedFloor()` | Deprecated/undocumented; first lit button or 0. |

Events on the elevator interface:

| Event | Args | Exact trigger |
|---|---|---|
| `idle` | - | Fired by `checkDestinationQueue()` when the destination queue is empty and the elevator is not busy. Notably fired for every elevator **at challenge start** (`world.init()` calls `checkDestinationQueue()` on each interface right after user `init` runs), and again ~1 s after finishing the last queued destination. Can be re-fired by user calls to `checkDestinationQueue()` while empty/idle. |
| `floor_button_pressed` | `floorNum` | A passenger inside pressed a destination button that was **not already lit** (pressing a lit button re-triggers nothing). Passengers press their button ~1 s after boarding (walk-to-slot animation completes). |
| `passing_floor` | `floorNum, direction` | Fired "slightly before" passing a floor - precisely: when `trunc(exactFutureFloorIfStopped)` changes, where *futureFloorIfStopped* = current position projected forward by the braking distance at full deceleration. Never fired for the current destination floor. `direction` is `"up"` or `"down"` (elevator's travel direction). It is a good moment to decide to stop at that floor (via `goToFloor(floorNum, true)`), since braking distance still allows it. Only one event per state change (multi-floor skips within one tick are not enumerated - acknowledged limitation in `elevator.js` comments). |
| `stopped_at_floor` | `floorNum` | The elevator has physically arrived and snapped to a floor. Fired before exit/boarding processing. |

### Floor object (`floor.js`)

| Member | Semantics |
|---|---|
| `floorNum()` | The floor's level (0-based, 0 = ground/bottom). |
| `buttonStates` | Plain object `{up: "", down: ""}`; a pressed button holds `"activated"`, cleared to `""`. Not in the official docs but widely used by community solutions. |
| `level`, `yPosition` | Raw properties (level = floor number; yPosition = pixel y). |

Floor events:

| Event | Args | Trigger |
|---|---|---|
| `up_button_pressed` | `floor` (the floor object) | Someone pressed the up call button while it was **not already activated**. Passengers who fail to board a full elevator press again (after the arrival cleared the state), so this re-fires. |
| `down_button_pressed` | `floor` | Same, for down. |
| `buttonstate_change` | `buttonStates` | Undocumented; fired whenever `buttonStates` changes (press or clear). |

Note: floor events are *not* wrapped in a facade; user handlers are invoked via a
`tryTrigger` that routes exceptions to the error handler (same effect as elevator events).

---

## 3. Simulation mechanics

### Coordinate system and layout (`world.js`, `elevator.js`)

- Screen coordinates: **y increases downward**. Floor `i` (0 = bottom) sits at
  `yPos = (floorCount - 1 - i) * floorHeight`, with `floorHeight = 50` (world default
  options; never overridden by any challenge).
- Elevator y-for-floor: `getYPosOfFloor(n) = (floorCount-1)*floorHeight - n*floorHeight`.
- Elevators are placed left-to-right starting at x = 200, spaced `20 + width` apart;
  `width = maxUsers * 10`. Passengers spawn at x = `105 + random(0..40)`. (Cosmetic.)
- All elevators start at floor 0, both indicators on, no lit buttons.

### Tick model (`app.js`, `world.js` `createWorldController`)

- Driven by `requestAnimationFrame`. `dtMax = 1/60` s (constructor arg in `app.js`).
- Per frame: `scaledDt = frameDelta_ms * 0.001 * timeScale`, clamped to
  `dtMax * 3 * timeScale` ("limit to prevent unhealthy substepping").
- User `update(scaledDt, elevators, floors)` is called **once per frame** with the whole
  scaled dt, *before* the physics substeps.
- Then the world is substepped: `while(scaledDt > 0 && !challengeEnded) { world.update(min(dtMax, scaledDt)); scaledDt -= dtMax; }` -
  i.e., physics always steps at ≤ 1/60 s regardless of time scale.
- `world.update(dt)`: advance `elapsedTime`; spawn passengers (below); for each elevator
  run `movable.update(dt)` (task timers, e.g. door dwell) and `updateElevatorMovement(dt)`
  (physics); for each user run `update(dt)` and refresh `maxWaitTime`; remove users
  flagged `removeMe`; recompute stats and trigger `stats_changed` (which is when the
  challenge condition is evaluated - `app.js`).
- User `init` is deferred to the **first unpaused frame** (so infinite loops in user code
  can't wedge the page while paused); immediately after it, `world.init()` fires the
  initial `idle` events.

### Elevator physics (`elevator.js`)

Constructor: `new Elevator(2.6, floorCount, floorHeight, capacity)` - the speed
`2.6 floors/sec` is hard-coded in `world.js` `createElevators`.

Constants (in pixels, with floorHeight = 50; divide by 50 for floors):

| Constant | Value | In floors |
|---|---|---|
| `ACCELERATION` | `floorHeight * 2.1` = 105 px/s² | 2.1 floors/s² |
| `DECELERATION` | `floorHeight * 2.6` = 130 px/s² | 2.6 floors/s² |
| `MAXSPEED` | `floorHeight * 2.6` = 130 px/s | 2.6 floors/s |

Per-substep movement (`updateElevatorMovement(dt)`), skipped entirely while "busy"
(door dwell task running):

1. Clamp `velocityY` to ±MAXSPEED; integrate position `y += velocityY * dt` (explicit
   Euler, velocity applied *before* acceleration update).
2. Let `destinationDiff = destinationY - y`.
   - Moving toward destination: compute braking distance
     `d = (0 - v²) / (2 * DECELERATION)` (note: comes out negative;
     `base.js` `distanceNeededToAchieveSpeed`). If `d * 1.05 < -|destinationDiff|`
     (i.e., 105% of braking distance ≥ remaining distance), brake: use the exact
     deceleration needed to stop at the destination
     (`accelerationNeededToAchieveChangeDistance`, `v²=u²+2ad` solved for `a`), capped at
     `DECELERATION * 1.1` (10% overshoot-recovery headroom). Otherwise accelerate with
     `min(|destinationDiff * 5|, ACCELERATION)` (the `*5` term gives a soft proportional
     ramp when very close to the target).
   - Standing still: accelerate toward destination, same formula.
   - Moving *away* from destination: decelerate at full `DECELERATION`, clamping to 0 so
     direction never flips within one step.
3. Arrival snap: if `isMoving && |destinationDiff| < 0.5 px && |velocityY| < 3 px/s`,
   snap to `destinationY`, zero velocity, `isMoving = false`, fire arrival handling.

Arrival (`handleDestinationArrival`): trigger internal `stopped(exactFloor)`; if exactly
on a floor (epsilon 1e-8 between exact and rounded floor): clear that floor's in-elevator
button (`buttonStates[currentFloor] = false`, with `floor_buttons_changed`), trigger
`stopped_at_floor`, then `exit_available` (passengers leave) then `entrance_available`
(passengers board) - order matters so leavers free capacity for boarders in the same
arrival.

**Door dwell**: the interface (`interfaces.js`) reacts to `stopped` at the head-of-queue
floor by shifting the queue and, if on a floor, calling `elevator.wait(1, cb)` before
`checkDestinationQueue()` - the parameter is misleadingly named `millis` but is compared
against accumulated `dt` in **seconds**, so this is a **1.0 s stop at every floor
arrival** during which the elevator is "busy" (physics skipped, cannot be commanded to
move; `stop()` during dwell only clears the queue). After the dwell, the next queued
destination starts or `idle` fires. If the elevator stopped *not* on a floor (via
`stop()`), the queue check happens immediately with no dwell.

`currentFloor` bookkeeping and **move counting** (`handleNewState`, fired on every
position change): whenever `round(exactCurrentFloor)` differs from the cached
`currentFloor`, `moveCount++` and `new_current_floor` fires. So **one "move" = crossing
one floor boundary** (a trip from floor 0 to floor 3 costs 3 moves), not one command.
`world.moveCount` = sum over elevators (`world.js` `recalculateStats`).

`passing_floor` generation (`handleNewState`): compute
`futureFloorIfStopped = getExactFloorOfYPos(y - sign(v) * brakingDistance)`; when its
`trunc` changes vs. the previous tick, the floor being passed is
`round(futureFloorIfStopped)`; fire `passing_floor(floor, direction)` unless that floor
is the current destination or the elevator isn't approaching it. Direction:
`velocityY > 0 ? "down" : "up"` (y is screen-down).

### Passenger spawn model (`world.js` `createWorldCreator`)

Timing: `elapsedSinceSpawn` starts at `1.001 / spawnRate`, so **one passenger spawns on
the very first world tick**; thereafter a `while` loop spawns one passenger every
`1.0 / spawnRate` seconds of game time exactly (multiple per tick possible at high time
scale/spawn rate). `spawnRate` is per-challenge (users/second).

Per spawn (`spawnUserRandomly` / `createRandomUser`; all randomness is lodash
`_.random`, which is inclusive on both ends, backed by unseeded `Math.random`):

- **Weight**: uniform integer 55–100.
- **Display type**: `_.random(40) === 0` → child (1/41); else `_.random(1) === 0` →
  female (50%), else male.
- **Spawn floor**: `_.random(1) === 0` → floor 0 (50%); otherwise uniform
  `_.random(floorCount - 1)` over *all* floors (so floor 0's true probability is
  `0.5 + 0.5/floorCount`).
- **Destination**:
  - from floor 0: uniform `_.random(1, floorCount-1)` (always going up);
  - from floor N>0: with probability 1/11 (`_.random(10) === 0`) a uniform *other* floor
    (`(N + _.random(1, floorCount-1)) % floorCount`); otherwise **floor 0** (~91% of
    upper-floor spawns go to the lobby).
- On appearing, the user immediately presses the floor's up or down call button
  (down iff destination < current floor). Pressing an already-activated button does not
  re-fire the event.

### Boarding and exiting rules (`world.js`, `user.js`, `elevator.js`, `floor.js`)

On `entrance_available` (elevator arrived at a floor), the world:

1. **First notifies the floor** (`floor.elevatorAvailable`): if the elevator's
   `goingUpIndicator` is on and the floor's up button is activated, the up button state is
   cleared (likewise down). Clearing first is deliberate "because overflowing users will
   press buttons again", re-firing the event.
2. Then iterates **all users in spawn order**; each user on that floor runs
   `elevatorAvailable`:
   - Skip if already transported, already inside an elevator, or mid-animation (busy).
   - **Indicator/suitability check** (`isSuitableForTravelBetween`): to board, a user
     going up needs `goingUpIndicator` on; going down needs `goingDownIndicator`; same
     floor is always suitable. (Both indicators default on, so naive code takes everyone.)
   - **Capacity**: the user takes a free slot (`userEntering`, random starting slot,
     linear probe). If **no free slot** (elevator full by count - slots, not weight), the
     user stays and **presses the floor call button again** (state was just cleared, so
     the event re-fires for user code).
   - On success: user walks to the slot over 1.0 s, then presses the in-elevator
     destination button.

On `exit_available` (fired at each floor arrival), every rider whose
`destinationFloor === currentFloor` exits: frees the slot, fires the user's
`exited_elevator` (stats), walks off over `1 + rand*0.5` s, then is removed from the
world.

**Elevator "re-arrival"** (`world.js` `handleButtonRepressing`): when any floor call
button is pressed, the world scans elevators in random rotation order; if one with the
matching direction indicator is currently standing still, on that exact floor, and not
full, the world itself calls `elevatorInterface.goToFloor(floor, true)` - causing the
elevator to "re-arrive" (arrival events re-fire, so the presser can board an elevator
that had already stopped there).

### Stats (`world.js`)

| Stat | Definition |
|---|---|
| `transportedCounter` | Number of users who have exited at their destination. |
| `elapsedTime` | Accumulated simulated seconds. |
| `transportedPerSec` | `transportedCounter / elapsedTime`. |
| **wait time** (per user) | `elapsedTime - user.spawnTimestamp` - time since spawn, i.e. **includes riding time**, not just waiting on the floor. |
| `maxWaitTime` | Max over: every user's wait time at exit, **and** every still-present user's current wait time, updated every tick (so it climbs continuously while anyone - waiting, riding, or even walking off post-exit until removal - remains). |
| `avgWaitTime` | Incremental mean of wait time measured at the moment each user exits: `(avg*(n-1) + wait)/n`. |
| `moveCount` | Sum of per-elevator `moveCount` (floor boundaries crossed). |

Challenge conditions are evaluated on every `stats_changed` (each tick); when a condition
returns non-null the world is flagged `challengeEnded`, paused, and feedback shown.

---

## 4. All challenges (`challenges.js`)

Condition templates (each `evaluate(world)` returns `null` = keep running, else pass/fail;
note boundary semantics - evaluation *triggers* at `>=` limit, success requires `<=`
limit, so hitting a limit exactly still passes):

- `requireUserCountWithinTime(userCount, timeLimit)` - "Transport N people in T seconds
  or less"; decides once `elapsedTime >= timeLimit || transported >= userCount`.
- `requireUserCountWithMaxWaitTime(userCount, maxWaitTime)` - "Transport N people and let
  no one wait more than W seconds"; decides once `world.maxWaitTime >= W || transported >= N`
  (no time limit; fails the instant anyone's wait hits W).
- `requireUserCountWithinTimeWithMaxWaitTime(N, T, W)` - both.
- `requireUserCountWithinMoves(userCount, moveLimit)` - "Transport N people using M
  elevator moves or less".
- `requireDemo()` - "Perpetual demo", never ends.

Elevator capacity: `elevatorCapacities` array cycles across elevators
(`capacities[i % len]`); default `[4]`. World defaults: `floorHeight: 50, floorCount: 4,
elevatorCount: 2, spawnRate: 0.5`.

| # | Floors | Elevators | Capacities | Spawn rate (users/s) | Success condition |
|---|---|---|---|---|---|
| 1 | 3 | 1 | [4] | 0.3 | Transport 15 in ≤ 60 s |
| 2 | 5 | 1 | [4] | 0.4 | Transport 20 in ≤ 60 s |
| 3 | 5 | 1 | [6] | 0.5 | Transport 23 in ≤ 60 s |
| 4 | 8 | 2 | [4] | 0.6 | Transport 28 in ≤ 60 s |
| 5 | 6 | 4 | [4] | 1.7 | Transport 100 in ≤ 68 s |
| 6 | 4 | 2 | [4] | 0.8 | Transport 40 in ≤ 60 moves |
| 7 | 3 | 3 | [4] | 3.0 | Transport 100 in ≤ 63 moves |
| 8 | 6 | 2 | [5] | 0.4 | Transport 50, no wait > 21.0 s |
| 9 | 7 | 3 | [4] | 0.6 | Transport 50, no wait > 20.0 s |
| 10 | 13 | 2 | [4, 10] | 1.1 | Transport 50 in ≤ 70 s |
| 11 | 9 | 5 | [4] | 1.1 | Transport 60, no wait > 19.0 s |
| 12 | 9 | 5 | [4] | 1.1 | Transport 80, no wait > 17.0 s |
| 13 | 9 | 5 | [5] | 1.1 | Transport 100, no wait > 15.0 s |
| 14 | 9 | 5 | [6] | 1.0 | Transport 110, no wait > 15.0 s |
| 15 | 8 | 6 | [4] | 0.9 | Transport 120, no wait > 14.0 s |
| 16 | 12 | 4 | [5, 10] | 1.4 | Transport 70 in ≤ 80 s |
| 17 | 21 | 5 | [10] | 1.9 | Transport 110 in ≤ 80 s |
| 18 | 21 | 8 | [6, 8] | 1.5 | Transport 2675 in ≤ 1800 s AND no wait > 45.0 s |
| 19 | 21 | 8 | [6, 8] | 1.5 | Perpetual demo (never ends) |

(Capacities alternate for multi-entry arrays: challenge 10's elevators are 4, 10;
challenge 16's are 5, 10, 5, 10; challenge 18/19's are 6, 8, 6, 8, ….)

`fitness.js` additionally defines three hidden headless "fitness" scenarios (small
4f/2e/0.6, medium 6f/3e/1.5/cap 5, large 18f/6e/1.9/cap 8) run for 12,000 fixed
1/60 s frames in a web worker to score avg wait time; the UI hook for it is commented
out in `app.js`.

---

## 5. Example community solutions (github.com/magwo/elevatorsaga/wiki)

### 5.1 Naive: idle-dispatch (wiki: "Easy solution with explanation")

Strategy: every elevator serves its own pressed buttons directly; idle elevators park at
floor 0; every floor call is blindly given to elevator 0. No direction awareness, no load
awareness. Clears the first few challenges only.

```js
{
    init: function(elevators, floors) {
        elevators.forEach(function(elevator) {
            elevator.on("floor_button_pressed", function(floorNum) {
                elevator.goToFloor(floorNum);
            });
            elevator.on("idle", function() {
                elevator.goToFloor(0);
            });
        });
        floors.forEach(function(floor) {
            floor.on("up_button_pressed down_button_pressed", function() {
                elevators[0].goToFloor(floor.floorNum());
            });
        });
    },
    update: function(dt, elevators, floors) {}
}
```

### 5.2 Intermediate: direction-aware queue insertion (wiki: "Two sorted lists for up and down")

Strategy: maintain global sorted `ups`/`downs` request lists fed by floor button events.
Idle elevators take the lowest pending up-call (or highest down-call), else park at 0.
In-elevator button presses are *inserted into the destination queue in travel order*
(using `destinationQueue.splice` + `checkDestinationQueue()`) rather than appended, so
the car sweeps floors in one direction. Indicators are set from the next destination on
each stop (up if next > current), and both switched on when empty - exploiting the
boarding rule so only same-direction passengers enter. Key techniques: direct
`destinationQueue` manipulation, indicator control, elevator-direction sweeps
(a rudimentary SCAN/"elevator algorithm").

```js
elevator.on("floor_button_pressed", function(floorNum) {
    if (this.destinationQueue.length === 0) {
        this.goToFloor(floorNum);
    } else if (this.destinationQueue.indexOf(floorNum) < 0) {
        for (var i = 0; i < this.destinationQueue.length; i++) {
            if (between(floorNum, this.currentFloor(), this.destinationQueue[i])) {
                this.destinationQueue.splice(i, 0, floorNum);
                this.checkDestinationQueue();
                return;
            }
        }
        this.goToFloor(floorNum);
    }
});
```

### 5.3 Advanced: pure-`update` global scheduler (wiki: "Twentyliner" - clears all 18 challenges)

Strategy: ignores events entirely; every `update` tick it re-plans each elevator from
scratch. It reads `floor.buttonStates.up/.down` directly (encoding down-calls as negative
floor numbers), infers each car's direction from its pressed floors, considers pickups
only when `loadFactor() < 0.5` and only along the current direction, merges with
pressed-floor dropoffs, picks the nearest useful floor, then `stop()`s and issues a fresh
`goToFloor` with indicators set to match direction (`e.goingDownIndicator(dir <= 0)
.goingUpIndicator(dir >= 0).goToFloor(...)`). Demonstrates: polling `buttonStates`
instead of events, continuous replanning via `stop()` + re-dispatch, indicator-based
boarding control, capacity-aware pickup throttling.

```js
{
    init: function(elevators, floors) {},
    update: function(dt, elevators, floors) {
        var arr = floors.filter(f => f.buttonStates.up).map(f => f.floorNum())
            .concat(floors.filter(f => f.buttonStates.down).map(f => -1 * f.floorNum()));
        var ds = (a, b) => Math.abs(Math.abs(a) - Math.abs(b));
        elevators.sort((a, b) => b.maxPassengerCount() - a.maxPassengerCount()).map((e, i) => {
            var [curr_floor, pressed] = [e.currentFloor(), e.getPressedFloors()];
            var dir = Math.sign(pressed.filter(f => f != curr_floor).pop() - curr_floor) || 0;
            var add = dir == 0 ? arr : arr.filter(el => Math.sign(el) == dir
                && [dir, 0].includes(Math.sign(Math.abs(el) - curr_floor)));
            var possible_options = (e.loadFactor() < 0.5 ? add : []).concat(pressed);
            var floor = possible_options.reduce((m, f) =>
                ds(curr_floor, m) < ds(curr_floor, f) ? m : f, Infinity);
            if (isFinite(floor)) {
                arr = e.stop() || arr.filter(e => e != floor);
                e.goingDownIndicator(dir <= 0).goingUpIndicator(dir >= 0).goToFloor(Math.abs(floor));
            }
        });
    }
}
```

### Strategy landscape (from pierrebai's wiki writeup and others)

The three condition families fight each other: minimizing moves (batch, full sweeps,
avoid repositioning) conflicts with minimizing max wait (respond fast, spread out).
Top solutions typically: keep a global request queue with request age; stop for
same-direction pickups en route when under a load threshold (via `passing_floor` +
`goToFloor(f, true)`); avoid two elevators answering one call (or clean up stolen
requests in `update`); never reverse with passengers aboard; and park idle elevators near
expected demand (floor 0 dominates: ~75% of all trips start or end there).

---

## 6. Implementation notes for the Rust rewrite

- **Randomness is unseeded** (`Math.random` via lodash `_.random`, inclusive ranges).
  Runs are not reproducible; some challenges (13, 17) are luck-sensitive and the wiki
  routinely says "may need multiple attempts". A rewrite should use a seedable RNG and
  document the exact call order (weight → display type → spawn floor → destination →
  slot offset in `userEntering` → rotation offset in `handleButtonRepressing` → exit
  walk duration) if replay compatibility matters.
- **Time scaling** multiplies the *frame* dt, but physics is always substepped at
  ≤ 1/60 s, so results are time-scale-invariant except: (a) user `update()` runs once
  per frame with the whole scaled dt (fewer, larger dt calls at high speed), and (b) the
  frame clamp `dtMax*3*timeScale` slows the wall-clock sim under load rather than
  degrading accuracy.
- **"Apply" = full world teardown + rebuild**: `world.unWind()` unbinds every listener
  (`off("*")`) on elevators/interfaces/users/floors/world and empties them; the code
  string is re-eval'd fresh; the same challenge restarts with `autoStart`. Nothing
  persists across applies except the editor text and timescale (localStorage).
- **The 1-second floor dwell** (`elevator.wait(1, …)` in `interfaces.js`) is a
  crucial hidden constant - every floor arrival costs 1 s before the next departure -
  and the parameter name (`millis`) is a lie; units are seconds.
- **moveCount counts floor crossings**, not commands (increments each time the rounded
  floor changes, `elevator.js` `handleNewState`). Move-limit challenges (6, 7) depend on
  this exact semantic.
- **Wait time = time since spawn**, including time riding the elevator (and technically
  ~1–1.5 s of walk-off until removal, still feeding the every-tick `maxWaitTime` update
  in `world.update`). Max-wait challenges fail the instant any user's age reaches the
  limit.
- **Capacity is slot-count, not weight**: `isFull()` checks slots; weight (55–100) only
  affects `loadFactor()` (denominator `maxUsers * 100`). A slot-full 4-cap elevator can
  report loadFactor from 0.55×4/4=0.55 up to 1.0.
- **Button re-press behavior**: floor call button state clears on suitable-indicator
  elevator arrival *before* boarding attempts; passengers who don't fit press again,
  re-firing `up/down_button_pressed`. Also the world auto-issues `goToFloor(f, true)`
  ("re-arrival") to a standing, matching-indicator, non-full elevator on that floor when
  its call button is pressed - reimplement this or late-arriving passengers can be
  stranded next to a parked elevator.
- **Indicators default to on** (both), and are never touched by the engine - only user
  code changes them. All boarding filtering flows from `isSuitableForTravelBetween`.
- **Event dispatch is synchronous** (riot observable): `goToFloor` can trigger `idle` →
  handler → `goToFloor` reentrantly within one call stack. Elevator-facade and floor
  events wrap user handlers in try/catch that pauses the world on throw. Duplicate
  suppression in `goToFloor` only checks the *adjacent* queue element, not the whole
  queue.
- **`stop()` semantics**: clears queue; if mid-flight, targets the projected stop point,
  which is generally between floors - no arrival events, no dwell, passengers stay in.
  During a dwell it only clears the queue (elevator is "busy").
- **`update` before physics**: user `update` sees the world state from the end of the
  previous frame; commands issued there take effect in this frame's substeps.
- **Scripting-boundary surface for RHAI**: the full mutable surface user code touches is:
  elevator interface methods/events + `destinationQueue` as a mutable array, floor
  `floorNum()`/events + readable `buttonStates`, and `dt`. Community solutions also rely
  on `floor.level` and lodash helpers; decide explicitly whether to expose
  `buttonStates` (the Twentyliner-class solutions need it).
- **Challenge-end check runs every tick** via `stats_changed`; conditions decide at
  `>=` boundaries with `<=` success (exact-equality passes).
- **Fitness harness** (`fitness.js`) shows how to run headless: fixed-step frame
  requester (`base.js` `createFrameRequester`), `controller.start(world, codeObj,
  requester.register, true)`, N triggered frames - a good model for the Rust engine's
  test/bench mode.
