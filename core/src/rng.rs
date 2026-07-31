//! Seeded, deterministic randomness: a hand-rolled PCG32.
//!
//! The original game uses unseeded `Math.random` via lodash `_.random`
//! (inclusive on both ends), making runs irreproducible. elevato seeds
//! explicitly instead. The generator is PCG-XSH-RR 64/32 transcribed from
//! the reference C implementation — hand-rolled rather than a crate
//! dependency, per the conservative-deps rule.
//!
//! RNG call order per spawn (research §6), fixed here so replays stay
//! stable once the consumers arrive in Phase 3:
//! weight → display type → spawn floor → destination → slot offset
//! (`userEntering`) → rotation offset (`handleButtonRepressing`) →
//! exit walk duration.

/// A PCG-XSH-RR 64/32 generator on a fixed stream, minted from a seed.
#[derive(Debug, Clone)]
pub struct Pcg32 {
    state: u64,
    increment: u64,
}

impl Pcg32 {
    const MULTIPLIER: u64 = 6364136223846793005;
    /// Stream 54 — the stream used by the PCG reference demo, so the
    /// known-answer test below can check against published output.
    const STREAM: u64 = 54;

    /// Seeds the generator (reference `pcg32_srandom_r` with the fixed
    /// stream). The same seed always yields the same sequence.
    pub fn new(seed: u64) -> Self {
        let mut rng = Self {
            state: 0,
            increment: (Self::STREAM << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    /// The next raw 32-bit output.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(self.increment);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform integer with **both ends inclusive**, matching lodash
    /// `_.random(lower, upper)` semantics (including swapping reversed
    /// bounds). Uses a widening multiply to scale; the residual bias is
    /// below `span / 2^32` — irrelevant at simulation ranges.
    pub fn random_inclusive(&mut self, lower: u32, upper: u32) -> u32 {
        let (lower, upper) = if lower <= upper {
            (lower, upper)
        } else {
            (upper, lower)
        };
        let span = u64::from(upper - lower) + 1;
        let scaled = (u64::from(self.next_u32()) * span) >> 32;
        lower + scaled as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_forty_two_matches_the_pcg32_reference_stream() {
        // First outputs of the PCG reference demo, seeded (42, 54).
        let mut rng = Pcg32::new(42);
        let observed: Vec<u32> = (0..5).map(|_| rng.next_u32()).collect();
        assert_eq!(
            observed,
            vec![
                0xa15c_02b7,
                0x7b47_f409,
                0xba1d_3330,
                0x83d2_f293,
                0xbfa4_784b
            ]
        );
    }

    #[test]
    fn identical_seeds_produce_identical_sequences() {
        let mut a = Pcg32::new(1234);
        let mut b = Pcg32::new(1234);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn random_inclusive_stays_in_range_and_covers_both_endpoints() {
        let mut rng = Pcg32::new(7);
        let mut seen = [false; 3];
        for _ in 0..1000 {
            let n = rng.random_inclusive(3, 5);
            assert!((3..=5).contains(&n));
            seen[(n - 3) as usize] = true;
        }
        assert_eq!(seen, [true; 3]);
    }

    #[test]
    fn reversed_bounds_are_swapped_like_lodash() {
        let mut forward = Pcg32::new(99);
        let mut reversed = Pcg32::new(99);
        for _ in 0..100 {
            assert_eq!(
                forward.random_inclusive(2, 8),
                reversed.random_inclusive(8, 2)
            );
        }
    }

    #[test]
    fn a_degenerate_range_always_returns_its_single_value() {
        let mut rng = Pcg32::new(0);
        for _ in 0..10 {
            assert_eq!(rng.random_inclusive(4, 4), 4);
        }
    }
}
