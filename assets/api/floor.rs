/// One floor of the building, as `update` sees it: a read-only
/// snapshot in `floors`, rebuilt before every call.
pub struct Floor {
    /// The floor's number; ground is 0.
    pub floor_num: i64,

    /// Alias of `floor_num`.
    pub level: i64,

    /// Whether the up call button is lit.
    pub up_pressed: bool,

    /// Whether the down call button is lit.
    pub down_pressed: bool,
}

// Call-button presses arrive as `Message` variants - see lib.rs.
