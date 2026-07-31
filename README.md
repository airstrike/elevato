# elevato

A faithful rewrite of [Elevator Saga](https://play.elevatorsaga.com) —
Magnus Wolffelt's elevator programming game — in Rust: program a bank of
elevators in [Rhai](https://rhai.rs), watch the simulation, and clear
the challenges. Same physics constants, same spawn model, same boarding
rules, same challenge roster as the original.

Built with [iced](https://iced.rs) as a gift to the Rust world: one
codebase runs natively and in the browser via WebAssembly, and the repo
doubles as a working example of iced + canvas + Rhai on wasm.

## Screenshots

<!-- TODO(andy): capture from the deployed build — one light-theme and
     one dark-theme shot of a run in progress, plus one of the editor
     with a compile error. -->

*Coming soon.*

## Playing

Pick a challenge, press **Start**, and watch the default program
struggle. Then make it yours: edit the Rhai program on the right and
press **Apply** — the world rebuilds from scratch and your new program
takes over. **Save** persists the code and timescale (config dir
natively, localStorage in the browser); **Reset** restores the starter
program (with **Undo reset** as the escape hatch). The −/+ buttons step
the simulation speed along the original's golden-ratio ladder.

Every run is deterministic: the seed shown in the stats bar plus your
program and timescale reproduce a run exactly. Restarting advances the
seed, so consecutive attempts still vary like the original's.

## Building

Natively:

```sh
cargo run
```

In the browser (requires [trunk](https://trunkrs.dev) and the
`wasm32-unknown-unknown` target):

```sh
trunk serve            # develop at http://localhost:8080
trunk build --release  # deployable bundle in dist/
```

## Scripting

Programs define `fn init(elevators, floors)` (required) and
`fn update(dt, elevators, floors)` (optional), and drive elevators
through an API that mirrors the original's JS surface in snake_case:

```rhai
fn init(elevators, floors) {
    let elevator = elevators[0];
    elevator.on("idle", || {
        elevator.go_to_floor(0);
        elevator.go_to_floor(1);
    });
}
```

`go_to_floor`, `stop`, `current_floor`, `load_factor`,
`going_up_indicator`, `destination_queue`, floor call buttons, and the
full event set (`idle`, `floor_button_pressed`, `passing_floor`,
`stopped_at_floor`, `up_button_pressed`, `down_button_pressed`) are all
there — see **[API.md](API.md)** for the complete mapping, the
Rhai-vs-JS gotchas, and the documented deviations.

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
├── src/       the iced app: canvas, editor, playback, theme
├── core/      elevato-core — the pure simulation (no iced, no rhai)
└── script/    the Rhai bindings and runtime over core's boundary
```

`core` is exhaustively pinned by integration tests: ported community
solutions pass — and fail — real challenges headlessly exactly as they
do in the original, and a live run replays a headless run byte for byte.

## Development note

The workspace currently builds against local checkouts of the
[airstrike/iced](https://github.com/airstrike/iced) and
[airstrike/cosmic-text](https://github.com/airstrike/cosmic-text) forks
via path dependencies (see the root `Cargo.toml`). The iced path points
at `~/projects/iced-web-clipboard` — a git worktree of the iced checkout
on the `web-clipboard` branch, which implements browser clipboard
support (`navigator.clipboard`) on top of `span-padding`. Builds require
that worktree to exist and stay on that branch until the dependency flip
to git sources lands — the prepared patch block and the exact steps live
in [PUBLISHING.md](PUBLISHING.md) and the root `Cargo.toml`.

## Credits

- [Elevator Saga](https://play.elevatorsaga.com) by
  [Magnus Wolffelt](https://github.com/magwo/elevatorsaga) and
  contributors — the original game this is a loving rewrite of, down to
  its physics constants and challenge configs.
- [iced](https://iced.rs), [Rhai](https://rhai.rs), and
  [Fira Code](https://github.com/tonsky/FiraCode) (bundled under the SIL
  Open Font License — `assets/fonts/OFL.txt`).
