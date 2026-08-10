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

/// The floor half of the message catalog: `message.kind` holds the
/// variant name in snake_case, and the fields arrive flattened into
/// the message map alongside `kind`.
pub enum Event {
    /// `"up_button_pressed"` - the up call button went from unlit to
    /// lit. Re-fires when riders who could not board press again
    /// after an arrival cleared it.
    UpButtonPressed { floor: i64 },

    /// `"down_button_pressed"` - likewise, for down.
    DownButtonPressed { floor: i64 },
}
