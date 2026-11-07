# elevato's API

An elevato program is an iced program. It is Rust with real
dependencies - `iced` and `elevato` - and it lives in
[`program/src/main.rs`](program/src/main.rs), whose first line is
`// rawr`. `cargo run -p program` compiles it natively; `rawr run -p
program` interprets it and reloads it while the game runs.

The original Elevator Saga gave a program live handles and callbacks.
elevato gives it what an iced program has: its own state, messages in,
tasks out. This document maps each of the original's callbacks and
methods to the message or task that replaces it.

## Program shape

```rust
// rawr
use elevato::{Elevator, Floor, Rider, elevator, floor};
use iced::{Function, Subscription, Task};

pub fn main() -> iced::Result {
    elevato::run(Program::new, Program::update, Program::subscription)
}

pub struct Program {
    elevators: Vec<Elevator>,
    floors: Vec<Floor>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Elevator(Elevator, elevator::Message),
    Floor(Floor, floor::Message),
}

impl Program {
    pub fn new(elevators: Vec<Elevator>, floors: Vec<Floor>) -> (Self, Task<Message>) {
        let start = Task::batch(elevators.iter().map(|&elevator| {
            elevator.go_to_floor(0).map(Message::Elevator.with(elevator))
        }));
        (Self { elevators, floors }, start)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Elevator(elevator, elevator::Message::FloorButtonPressed(floor)) => {
                elevator.go_to_floor(floor).map(Message::Elevator.with(elevator))
            }
            Message::Elevator(..) | Message::Floor(..) => Task::none(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders().map(|rider| match rider {
            Rider::Elevator(elevator, message) => Message::Elevator(elevator, message),
            Rider::Floor(floor, message) => Message::Floor(floor, message),
        })
    }
}
```

`elevato::run` opens the game and hosts the program as its child, the
way an iced app hosts a screen: it holds the program's state, calls
its `update`, and runs its tasks and subscription. Every attempt - a
restart, a new challenge - boots the program afresh with `new`
on a fresh world.

`new` receives the challenge's elevators and floors, and the program
keeps what it needs. There is no world to read: nothing the program
calls reads the simulation, and everything it knows arrives as a
message. A panic anywhere in the program ends the attempt, and its
message shows under the editor (the original's "There is a problem
with your code").

## Elevators and floors

`Elevator` and `Floor` are the only types for cars and floors, and
both are `Copy`, comparable and hashable.

- **Elevators** come only from `new` and from messages, so a program
  can never name a car the challenge doesn't have.
- **Floors** come from `new`, from messages, and from integers:
  `Floor` converts from `usize`, `i64` and `i32` (what an integer
  literal is when nothing else types it), and every function that
  takes a floor takes `impl Into<Floor>`. `elevator.go_to_floor(3)`
  works. `floor.level()` reads the level back. A negative level is the
  lobby, and a level past the top floor is the top floor once a car
  is sent there, as the original clamped it.

## Where each message comes from

| Message | Cause | Channel |
|---|---|---|
| `floor::Message::UpButtonPressed` / `DownButtonPressed` | A rider spawns, or one a full car left behind presses again | `elevato::riders()`, as `Rider::Floor(floor, message)` |
| `elevator::Message::FloorButtonPressed(floor)` | A rider who boarded picks a floor, about a second later | `elevato::riders()`, as `Rider::Elevator(elevator, message)` |
| `elevator::Message::PassingFloor(floor, direction)` | A car the program sent is about to pass a floor | Streamed by the task from `go_to_floor` |
| `elevator::Message::StoppedAtFloor { .. }` | That car arriving | The same task's final message, after riders got off and on |

Riders are the only source of messages besides the program's own
tasks. A button already lit stays silent, as in the original.

## The original's callbacks

| Original (JS) | elevato | Notes |
|---|---|---|
| `init(elevators, floors)` | `Program::new(elevators, floors)` | Returns the state and the first tasks. The original's initial `idle` round is gone: the first moves are the tasks `new` returns. |
| `update(dt, elevators, floors)` | - | No tick. Time isn't a message; the program reacts to riders and to its cars. |
| `elevator.on("idle")` | - | The program owns its queues. A car is idle when the program has no trip under way for it; the last `StoppedAtFloor` says so. |
| `elevator.on("floor_button_pressed", f)` | `elevator::Message::FloorButtonPressed(floor)` | From `riders()`. |
| `elevator.on("passing_floor", f, dir)` | `elevator::Message::PassingFloor(floor, direction)` | Streamed by the trip's task, still in time for `go_to_floor_first` to stop there. |
| `elevator.on("stopped_at_floor", f)` | `elevator::Message::StoppedAtFloor { floor, exited, boarded, load_factor, full }` | The trip task's last message. **[deviation]** It arrives after riders got off and on, so it says who did; lamps that steer boarding are set when the car is sent, not on arrival. |
| `floor.on("up_button_pressed")` | `floor::Message::UpButtonPressed` | From `riders()`. |
| `floor.on("down_button_pressed")` | `floor::Message::DownButtonPressed` | From `riders()`. |

## The original's methods

Every command is a `Task<elevator::Message>`; nothing happens until
the program returns the task from `new` or `update`. The tasks a
program returns run in the order it returns them, so a
`Task::batch([elevator.set_going_up_indicator(true),
elevator.go_to_floor(3)])` sets the lamp before the car leaves.

| Original (JS) | elevato | Notes |
|---|---|---|
| `goToFloor(n)` | `elevator.go_to_floor(n)` | A trip queued after the car's others. Its task streams `PassingFloor` and ends with `StoppedAtFloor`. Asking for the floor the car already goes to last adds no stop: both trips end at that one arrival. |
| `goToFloor(n, true)` | `elevator.go_to_floor_first(n)` | Ahead of the car's other trips; the ones it overtakes resume after it. |
| `stop()` | `elevator.stop()` | Halts the car where it can, usually between floors, and aborts its trips: their tasks end with no more messages. |
| `goingUpIndicator(v)` | `elevator.set_going_up_indicator(on)` | Both lamps start on. A car stopping with a lamp lit takes riders going that way and clears that call. |
| `goingDownIndicator(v)` | `elevator.set_going_down_indicator(on)` | |
| `destinationQueue`, `checkDestinationQueue()` | - | The program owns its queues: it sends a car its next trip when the last one ends, or overtakes with `go_to_floor_first`. |
| `currentFloor()`, `destinationDirection()`, `getPressedFloors()` | - | No reads. The program learns where its cars are from `PassingFloor` and `StoppedAtFloor`, and which buttons are lit from `riders()`. |
| `loadFactor()` | `StoppedAtFloor { load_factor, full, .. }` | As the car leaves the stop. `full` is the slot-count test the weight-based load factor can't answer. |
| `maxPassengerCount()` | - | Not exposed; `full` says when a car can take no one. |
| `floor.buttonStates`, `floorNum()` | `riders()`, `floor.level()` | The program keeps its own record of lit calls. |

## Rust as rawr runs it

The program is Rust, checked by rustc. Under `rawr run` it is also
interpreted: rawr checks names, types, traits and `match` arms before
anything runs, and a save that passes reaches the running game at its
next message, keeping the program's state when the new code is
compatible with it. A save that changes the shape of `Program` or
`Message` restarts the game. rawr's
[Supported Rust](https://rawr.airstrike.fyi/docs/language) page lists
what works.

## Deviations from the original

1. The Elm architecture replaces callbacks and handles: events arrive
   as messages, effects are tasks the program returns, and the program
   keeps its own state instead of reading the world's.
2. There is no tick and no idle event: the program reacts to riders and
   to its own trips.
3. `StoppedAtFloor` arrives after riders got off and on (and says how
   many did); the original called `stopped_at_floor` before boarding.
4. Destinations are whole floors. The original's `Number()` coercion
   accepted fractional ones.
5. Mistakes the original found at run time - an unknown event name, a
   missing handler argument - are type errors, refused before the
   program runs.
6. The simulation is deterministic: seeded RNG and fixed-timestep
   playback (see the README) - same seed, same program, same result.
