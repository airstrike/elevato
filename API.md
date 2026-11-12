# elevato's API

An elevato program is an iced program. It is Rust with real
dependencies - `iced` and `elevato` - and it lives in
[`program/src/main.rs`](program/src/main.rs), whose first line is
`// rawr`. `cargo run -p program` compiles it natively; `rawr run -p
program` interprets it and reloads it while the game runs.

The original Elevator Saga gave a program live handles, callbacks and
a queue of destinations per car. elevato gives it what an iced program
has: its own state, messages in, tasks out, and a `view`. This document
maps each of the original's callbacks and methods to what replaces it.

## Program shape

```rust
// rawr
use elevato::{Building, Elevator, Message, elevator};
use iced::{Element, Subscription, Task};

pub fn main() -> iced::Result {
    elevato::run(
        Program::new,
        Program::update,
        Program::view,
        Program::subscription,
    )
}

pub struct Program;

impl Program {
    pub fn new(_elevators: usize) -> (Self, Task<Message>) {
        let (_, start) = Elevator::new();
        (Self, start)
    }

    pub fn update(&mut self, message: Message, building: &Building) -> Task<Message> {
        match message {
            Message::Elevator(elevator, elevator::Message::Idle) => {
                let next = (building.floor_of(elevator) + 1) % building.floors();
                elevator.go_to_floor(next)
            }
            Message::Elevator(..) | Message::Floor(..) => Task::none(),
        }
    }

    pub fn view(&self, building: &Building) -> Element<'_, Message> {
        elevato::view(building)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders()
    }
}
```

`elevato::run` opens the game and hosts the program as its child, the
way an iced app hosts a screen: it holds the program's state, calls
its `update` and `view`, and runs its tasks and subscription. The
program's view goes where the building shows; the game keeps the
editor, the stats and the challenge picker around it. Every attempt -
a restart, a new challenge - boots the program afresh with `new` in a
fresh building.

A panic anywhere in the program ends the attempt, and its message
shows under the editor (the original's "There is a problem with your
code").

## Elevators, floors and the building

- **Elevators** are `Elevator`s: `Copy`, comparable and hashable.
  `new` learns how many cars the challenge's bank holds and powers on
  the ones it uses with `Elevator::new()`, which returns the car and
  the task of its news. `Elevator::default()` powers one on without its
  news. Only cars the program powers on stand in the building; one
  asked for past the bank never enters it, says nothing, and ignores
  its commands.
- **Floors** are plain numbers, `usize` levels from 0 (the lobby) to
  one below `building.floors()`. `elevator.go_to_floor(3)` works, and
  a level past the top floor is the top floor once a car is sent there,
  as the original clamped it.
- **The building** is borrowed by `update` and `view`, as it stands:
  `building.floors()`, `building.floor_of(elevator)`, and
  `elevato::view(building)` to draw it.

## Where each message comes from

A program's `update` takes `elevato::Message`: `Message::Elevator(car,
elevator::Message)` for news from a car, `Message::Floor(level,
floor::Message)` for a call from a floor. Every command's task and
`elevato::riders()` produce it already wrapped, so a program returns
them as they are.

| Message | Cause | Channel |
|---|---|---|
| `floor::Message::UpButtonPressed` / `DownButtonPressed` | A rider spawns, or one a full car left behind presses again | `elevato::riders()`, as `Message::Floor` |
| `elevator::Message::Idle` | A car has nothing to do: when powered on, and after each trip or stop | The task from `Elevator::new` |
| `elevator::Message::FloorButtonPressed(floor)` | A rider who boarded picks a floor, about a second later | `elevato::riders()`, as `Message::Elevator` |
| `elevator::Message::PassingFloor(floor, direction)` | A car the program sent is about to pass a floor | Streamed by the task from `go_to_floor` |
| `elevator::Message::StoppedAtFloor { .. }` | That car arriving | The same task's final message, after riders got off and on |

A button already lit stays silent, as in the original.

## The original's callbacks

| Original (JS) | elevato | Notes |
|---|---|---|
| `init(elevators, floors)` | `Program::new(elevators)` | Powers on the cars it uses and returns the first tasks. |
| `update(dt, elevators, floors)` | - | No tick. Time isn't a message; the program reacts to riders and to its cars. |
| `elevator.on("idle")` | `elevator::Message::Idle` | From the car's own task, `Elevator::new`'s. |
| `elevator.on("floor_button_pressed", f)` | `elevator::Message::FloorButtonPressed(floor)` | From `riders()`. |
| `elevator.on("passing_floor", f, dir)` | `elevator::Message::PassingFloor(floor, direction)` | Streamed by the trip's task, still in time to send the car there instead. |
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
| `goToFloor(n)` | `elevator.go_to_floor(n)` | **[deviation]** One trip at a time: it replaces the car's current trip, whose task ends with no more messages. Its own task streams `PassingFloor` and ends with `StoppedAtFloor`. |
| `goToFloor(n, true)` | `elevator.go_to_floor(n)` | Every trip goes first; there is nothing queued to overtake. |
| `stop()` | `elevator.stop()` | Halts the car where it can, usually between floors, and aborts its trip, whose task ends with no more messages. The car says `Idle`. |
| `goingUpIndicator(v)` | `elevator.set_going_up_indicator(on)` | Both lamps start on. A car stopping with a lamp lit takes riders going that way and clears that call. |
| `goingDownIndicator(v)` | `elevator.set_going_down_indicator(on)` | |
| `destinationQueue`, `checkDestinationQueue()` | - | No queue. The program keeps its own list of floors to visit and sends the car the next one when it says `Idle`. |
| `currentFloor()` | `building.floor_of(elevator)` | The floor the car is at, or nearest to while it travels. |
| `destinationDirection()`, `getPressedFloors()` | - | The program learns where its cars are going from its own trips, and which buttons are lit from `riders()`. |
| `loadFactor()` | `StoppedAtFloor { load_factor, full, .. }` | As the car leaves the stop. `full` is the slot-count test the weight-based load factor can't answer. |
| `maxPassengerCount()` | - | Not exposed; `full` says when a car can take no one. |
| `floor.buttonStates`, `floorNum()` | `riders()`, the floor itself | The program keeps its own record of lit calls. |

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
   as messages, effects are tasks the program returns, the program
   keeps its own state, and its `view` draws the building.
2. A car makes one trip at a time. There is no destination queue: a
   new trip replaces the current one, and a car says `Idle` when it has
   nothing left to do.
3. Only the cars a program powers on stand in the building.
4. There is no tick: the program reacts to riders, idle cars and its
   own trips.
5. `StoppedAtFloor` arrives after riders got off and on (and says how
   many did); the original called `stopped_at_floor` before boarding.
6. Destinations are whole floors. The original's `Number()` coercion
   accepted fractional ones.
7. Mistakes the original found at run time - an unknown event name, a
   missing handler argument - are type errors, refused before the
   program runs.
8. The simulation is deterministic: seeded RNG and fixed-timestep
   playback (see the README) - same seed, same program, same result.
