// rawr
//! The browser's universe. The runner rawr-import builds from it covers
//! the native calls its programs make, and the page runs a player's
//! program on it when the program's own calls are among them.
//!
//! Each module of `programs` is a whole program with its own `main`, so
//! the runner counts its types from zero, as it counts a player's.

mod programs;

fn main() {}
