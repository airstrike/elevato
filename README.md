# elevato.rs

A faithful rewrite of [Elevator Saga](https://play.elevatorsaga.com) -
Magnus Wolffelt's elevator programming game - in Rust: program a bank of
elevators in Rust, watch the simulation, and clear the challenges.
Your program is an iced program of its own, which
[rawr](https://github.com/airstrike/rawr) interprets and reloads while
the game runs, and which `cargo` compiles natively. Same physics
constants, same spawn model, same boarding rules, same challenge
roster as the original.

Built with [iced](https://iced.rs) as a gift to the Rust world: one
codebase runs natively and in the browser via WebAssembly, and the repo
doubles as a working example of iced + canvas + a hot-reloaded iced
program hosted inside another.

## Screenshots

<!-- TODO(andy): capture from the deployed build - one light-theme and
     one dark-theme shot of a run in progress, plus one of the editor
     with a compile error. -->

*Coming soon.*

## Playing

Run the game under rawr, so your edits reach it as you save:

```sh
rawr run -p program
```

Pick a challenge, press **Start**, and watch the starter program
struggle. Then make it yours: edit the program - in the editor on the
left, or in [`program/src/main.rs`](program/src/main.rs) in your own
editor - and save. **Save** (Cmd+S; Ctrl elsewhere) writes the file,
and under `rawr run` the running game picks the edit up. Cmd+Enter
saves and starts the run if it isn't running.
**Reset** restores the starter program (with **Undo reset** as the
escape hatch). **Restart** starts a fresh attempt. The −/+ buttons step the
simulation speed along the original's golden-ratio ladder; the
timescale is saved under the platform config dir.

Every run is deterministic: the seed shown in the stats bar plus your
program and timescale reproduce a run exactly. Restarting advances the
seed, so consecutive attempts still vary like the original's. An edit
applied mid-run marks the seed "(edited)" for the rest of the attempt.

## Building

Natively, with your program compiled in:

```sh
cargo run -p program
```

In the browser (requires [trunk](https://trunkrs.dev) and the
`wasm32-unknown-unknown` target):

```sh
trunk serve            # develop at http://localhost:8080
trunk build --release  # deployable bundle in dist/
```

The web build runs the program compiled into the page. Its editor
keeps a draft in localStorage, but edits don't run in the browser
until rawr can interpret marked code against dependencies compiled
into the page's wasm
([rawr#435](https://github.com/airstrike/rawr/issues/435)).

The solutions and the playback's determinism gate are the game's
tests:

```sh
cargo test -p elevato
cargo test -p elevato-core
```

## Programming

Your program is [`program/src/main.rs`](program/src/main.rs), an iced
program that uses the `elevato` crate:

```rust
// rawr
use elevato::{Elevator, elevator, floor};
use iced::{Function, Subscription, Task};

pub fn main() -> iced::Result {
    elevato::run(Program::new, Program::update, Program::subscription)
}

pub struct Program {
    floors: usize,
}

#[derive(Debug, Clone)]
pub enum Message {
    Elevator(Elevator, elevator::Message),
    Floor(usize, floor::Message),
}

impl Program {
    pub fn new(elevators: Vec<Elevator>, floors: usize) -> (Self, Task<Message>) {
        let start = Task::batch(elevators.into_iter().map(|elevator| {
            elevator.go_to_floor(0).map(Message::Elevator.with(elevator))
        }));
        (Self { floors }, start)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Elevator(elevator, elevator::Message::StoppedAtFloor { floor, .. }) => {
                let next = (floor + 1) % self.floors.min(2);
                elevator.go_to_floor(next).map(Message::Elevator.with(elevator))
            }
            Message::Elevator(..) | Message::Floor(..) => Task::none(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders(Message::Elevator, Message::Floor)
    }
}
```

`new` gets the challenge's elevators and its number of floors; a
floor is its level, a number from 0 (the lobby). A car moves when the
program returns the task `elevator.go_to_floor(n)` gives it; that task
streams `PassingFloor` as the car travels and ends with
`StoppedAtFloor`, which says who got off and on. Riders call cars and
press buttons inside them through `elevato::riders()`. There is no
world to read: what the program knows, it learned from a message. See
**[API.md](API.md)** for how each of the original's callbacks maps to
a task or a rider message, and for the documented deviations. The
in-game API tab has the same reference, a cmd+click away from any name
in your program.

## Challenges

The original's 19, verbatim:

| # | Building | Goal |
|---|---|---|
| 1 | 3 floors, 1 elevator | 15 people in 60 s |
| 2 | 5 floors, 1 elevator | 20 people in 60 s |
| 3 | 5 floors, 1 bigger elevator | 23 people in 60 s |
| 4 | 8 floors, 2 elevators | 28 people in 60 s |
| 5 | 6 floors, 4 elevators | 100 people in 68 s |
| 6 | 4 floors, 2 elevators | 40 people using ≤ 60 moves |
| 7 | 3 floors, 3 elevators | 100 people using ≤ 63 moves |
| 8 | 6 floors, 2 elevators | 50 people, nobody waits > 21 s |
| 9 | 7 floors, 3 elevators | 50 people, nobody waits > 20 s |
| 10 | 13 floors, 2 mixed elevators | 50 people in 70 s |
| 11–15 | 8–9 floors, 5–6 elevators | 60–120 people, max wait 19 s → 14 s |
| 16 | 12 floors, 4 mixed elevators | 70 people in 80 s |
| 17 | 21 floors, 5 big elevators | 110 people in 80 s |
| 18 | 21 floors, 8 elevators | 2675 people in 30 min, nobody waits > 45 s |
| 19 | 21 floors, 8 elevators | Perpetual demo |

## Workspace

```
elevato/
├── src/       the game: the iced app hosting your program - canvas, editor, playback, theme
├── program/   your program, the marked `src/main.rs` that runs the game
├── api/       elevato-api - Elevator, riders(), and the Simulation that runs a program
└── core/      elevato-core - the pure simulation (no iced, no rawr)
```

The game re-exports the API at its root, so programs name it
`elevato`. `core` is exhaustively pinned by integration tests, and the
ported community solutions in `tests/solutions/` pass - and fail - real
challenges headlessly, compiled natively against the API, with each
one's final stats pinned.

## Development note

The workspace currently builds against local checkouts of the
[airstrike/iced](https://github.com/airstrike/iced) and
[airstrike/cosmic-text](https://github.com/airstrike/cosmic-text) forks
via path dependencies (see the root `Cargo.toml`). The iced path points
at `~/projects/iced-web-clipboard` - a git worktree of the iced checkout
on the `web-clipboard` branch, which implements browser clipboard
support (`navigator.clipboard`) on top of `span-padding`. Builds require
that worktree to exist and stay on that branch until the dependency flip
to git sources lands - the prepared patch block and the exact steps live
in [PUBLISHING.md](PUBLISHING.md) and the root `Cargo.toml`.

Nothing depends on rawr: `rawr run` is a tool, and the `// rawr` line
is a comment to `cargo`.

## Credits

- [Elevator Saga](https://play.elevatorsaga.com) by
  [Magnus Wolffelt](https://github.com/magwo/elevatorsaga) and
  contributors - the original game this is a loving rewrite of, down to
  its physics constants and challenge configs.
- [iced](https://iced.rs), [rawr](https://github.com/airstrike/rawr), and
  [Fira Code](https://github.com/tonsky/FiraCode) (bundled under the SIL
  Open Font License - `assets/fonts/OFL.txt`).
