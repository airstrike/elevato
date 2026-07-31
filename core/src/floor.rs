//! Floors and the y-down pixel space they live in.
//!
//! Screen coordinates grow downward: floor 0 (ground) sits at the *largest*
//! y, the top floor at y = 0. With [`HEIGHT`] fixed at 50.0, every physics
//! constant from the original transcribes verbatim in px units.
//!
//! Call buttons and spawning arrive in Phase 3; for now a floor is just its
//! level and its fixed pixel position.

/// Height of one floor in pixels. Never overridden by any challenge.
pub const HEIGHT: f64 = 50.0;

/// A floor in the building: its level (0 = ground/bottom) and its fixed
/// pixel y position. Minted by the world; levels are always in range.
#[derive(Debug, Clone)]
pub struct Floor {
    level: usize,
    y_position: f64,
}

impl Floor {
    pub(crate) fn new(level: usize, floor_count: usize) -> Self {
        Self {
            level,
            y_position: y_of_level(level as f64, floor_count),
        }
    }

    /// The floor's level, 0-based from the ground.
    pub fn level(&self) -> usize {
        self.level
    }

    /// The floor's pixel y position (y grows downward).
    pub fn y_position(&self) -> f64 {
        self.y_position
    }
}

/// Pixel y of a (possibly fractional) floor level.
pub fn y_of_level(level: f64, floor_count: usize) -> f64 {
    (floor_count as f64 - 1.0) * HEIGHT - level * HEIGHT
}

/// Exact (possibly fractional) floor level at a pixel y.
pub fn level_of_y(y: f64, floor_count: usize) -> f64 {
    (floor_count as f64 - 1.0) - y / HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_floor_sits_at_the_bottom_of_the_pixel_space() {
        assert_eq!(y_of_level(0.0, 4), 150.0);
        assert_eq!(y_of_level(3.0, 4), 0.0);
    }

    #[test]
    fn level_of_y_inverts_y_of_level() {
        for floor_count in [2, 4, 9] {
            for level in 0..floor_count {
                let level = level as f64;
                assert_eq!(
                    level_of_y(y_of_level(level, floor_count), floor_count),
                    level
                );
            }
        }
    }
}
