# elevato

A faithful rewrite of [Elevator Saga](https://play.elevatorsaga.com) in Rust:
program a bank of elevators in [Rhai](https://rhai.rs), watch the simulation,
and clear the challenges. Built with [iced](https://iced.rs); runs natively
and in the browser via WebAssembly.

## Building

Natively:

```sh
cargo run
```

In the browser (requires [trunk](https://trunkrs.dev) and the
`wasm32-unknown-unknown` target):

```sh
trunk serve
```

## Development note

The workspace currently builds against local checkouts of the
[airstrike/iced](https://github.com/airstrike/iced) and
[airstrike/cosmic-text](https://github.com/airstrike/cosmic-text) forks via
path dependencies (see the root `Cargo.toml`). Clean-clone builds via git
patches land before ship.
